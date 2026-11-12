"""Check caller-supplied TapePen package identities without executing its code."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex


SOURCE_PINS = {
    "capture_candidate.py": "runner_sha256",
    "tapepen_canvas_native.py": "helper_sha256",
    "preflight_candidate.py": "preflight_script_sha256",
}
SCHEMAS = {
    "preflight.json": "tapepen-public-drawing-preflight-v7",
    "candidate.json": "sdocx-tapepen-public-drawing-candidate-v25",
}


def digest_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def bounded_path(root, declared, *, absolute=False):
    if not isinstance(declared, str) or not declared or "\\" in declared:
        raise ValueError("expected a nonempty POSIX path")
    path = Path(declared)
    if not absolute and path.is_absolute():
        raise ValueError("expected a path relative to its declared root")
    resolved = (root / path).resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise ValueError("declared path leaves its root")
    return resolved


def hash_check(root, declared, expected):
    check = {"path": declared, "expected_sha256": expected}
    if not isinstance(expected, str) or re.fullmatch("[0-9a-f]{64}", expected) is None:
        return {**check, "status": "invalid", "reason": "invalid SHA-256 declaration"}
    try:
        path = bounded_path(root, declared)
        actual = digest_file(path)
    except FileNotFoundError:
        return {**check, "status": "missing"}
    except ValueError as error:
        return {**check, "status": "invalid", "reason": str(error)}
    except OSError as error:
        return {**check, "status": "unreadable", "reason": str(error)}
    return {**check, "actual_sha256": actual,
            "status": "passed" if actual == expected else "mismatched"}


def phase(name, checks):
    statuses = {check["status"] for check in checks}
    status = next((value for value in ("invalid", "unreadable", "mismatched", "missing",
                                       "relocation_required") if value in statuses), "passed")
    return {"name": name, "status": status, "checks": checks}


def load_documents(package):
    documents = {}
    checks = []
    for name, expected_schema in SCHEMAS.items():
        try:
            path = bounded_path(package, name)
            data = json.loads(path.read_text())
            if not isinstance(data, dict) or data.get("schema") != expected_schema:
                raise ValueError(f"expected schema {expected_schema}")
            documents[name] = data
            checks.append({"path": name, "status": "passed", "schema": expected_schema,
                           "sha256": digest_file(path)})
        except FileNotFoundError:
            checks.append({"path": name, "status": "missing"})
        except (ValueError, UnicodeError) as error:
            checks.append({"path": name, "status": "invalid", "reason": str(error)})
        except OSError as error:
            checks.append({"path": name, "status": "unreadable", "reason": str(error)})
    return documents, phase("package_documents", checks)


def candidate_join(package, manifest, candidate):
    checks = [{"field": key, "expected": manifest.get(key), "actual": candidate.get(key),
               "status": "passed" if candidate.get(key) == manifest.get(key) else "mismatched"}
              for key in SOURCE_PINS.values()]
    checks.append(hash_check(package, "preflight.json", candidate.get("manifest_sha256")))
    return phase("candidate_manifest_join", checks)


def historical_invocation(package, workspace, manifest, candidate):
    command = candidate.get("run_command")
    checks = []
    try:
        if not isinstance(command, str):
            raise ValueError("candidate has no recorded run_command string")
        args = shlex.split(command)
        if len(args) < 2:
            raise ValueError("recorded command has no runner path")
        declared = [("runner", args[1], "capture_candidate.py")]
        for option in ("--preflight-manifest", "--preflight-sha256"):
            if args.count(option) != 1 or args.index(option) + 1 >= len(args):
                raise ValueError(f"expected one recorded {option} value")
        declared.append(("manifest", args[args.index("--preflight-manifest") + 1], "preflight.json"))
        for label, document in (("candidate_preflight", candidate), ("manifest_preflight", manifest)):
            declared.append((label, document.get("preflight_script"), "preflight_candidate.py"))
        for label, recorded, local in declared:
            try:
                target = bounded_path(workspace, recorded, absolute=True)
                adjacent = bounded_path(package, local)
                checks.append({"field": label, "recorded_path": recorded,
                               "resolved_recorded_path": str(target), "package_path": str(adjacent),
                               "status": "passed" if target == adjacent else "relocation_required"})
            except ValueError as error:
                checks.append({"field": label, "recorded_path": recorded,
                               "status": "invalid", "reason": str(error)})
        expected = candidate.get("manifest_sha256")
        actual = args[args.index("--preflight-sha256") + 1]
        checks.append({"field": "command_manifest_sha256", "expected": expected, "actual": actual,
                       "status": "passed" if actual == expected else "mismatched"})
    except ValueError as error:
        checks.append({"field": "run_command", "status": "invalid", "reason": str(error)})
    return {**phase("historical_invocation_location", checks), "recorded_command": command}


def dependency_inventory(workspace, manifest):
    pins = manifest.get("pinned_files")
    if not isinstance(pins, dict) or not pins:
        return phase("declared_workspace_inputs", [
            {"status": "invalid", "reason": "expected a nonempty pinned_files object"}])
    return phase("declared_workspace_inputs", [hash_check(workspace, name, expected)
                                               for name, expected in pins.items()])


def check_package(package, workspace):
    package = package.resolve()
    workspace = workspace.resolve()
    documents, document_phase = load_documents(package)
    phases = [document_phase]
    if len(documents) == len(SCHEMAS):
        manifest = documents["preflight.json"]
        candidate = documents["candidate.json"]
        phases.extend([
            phase("local_source_identities", [hash_check(package, name, manifest.get(key))
                                              for name, key in SOURCE_PINS.items()]),
            candidate_join(package, manifest, candidate),
            historical_invocation(package, workspace, manifest, candidate),
            dependency_inventory(workspace, manifest),
        ])
    else:
        phases.extend({"name": name, "status": "not_evaluated", "reason": "package documents unavailable or invalid"}
                      for name in ("local_source_identities", "candidate_manifest_join",
                                   "historical_invocation_location", "declared_workspace_inputs"))
    return {
        "schema": "sdocx-tapepen-package-readiness-v1",
        "package_directory": str(package), "workspace_directory": str(workspace),
        "package_admission": "passed" if all(item["status"] == "passed" for item in phases) else "blocked",
        "behavioral_preflight": "not_run",
        "scope": "Only declared package identities, joins, paths and workspace hashes. "
                 "No code imports, execution, native inspection or full transitive-runtime validation. "
                 "Recorded paths are preserved; relocation is reported without rewriting commands.",
        "phases": phases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", type=Path, required=True)
    parser.add_argument("--workspace-root", type=Path, required=True)
    args = parser.parse_args()
    result = check_package(args.package_dir, args.workspace_root)
    print(json.dumps(result, indent=2))
    return 0 if result["package_admission"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
