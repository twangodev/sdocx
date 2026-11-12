"""Check only the retained TapePen active-stack interval policy, without guest execution."""

import argparse
import ast
import json
from pathlib import Path

from check_package import SCHEMAS, bounded_path, digest_file, hash_check


TAPE_STACK_MAPPING = (0x20000000, 0x20100000)
TAPE_STACK_TOP = 0x200EF000


def _tape_stack_source_owner(pointer, length, current_sp):
    if (not isinstance(pointer, int) or not isinstance(length, int) or
            not isinstance(current_sp, int) or pointer <= 0 or length <= 0 or
            pointer > 0xffffffffffffffff or length > 0xffffffffffffffff - pointer):
        return None
    end = pointer + length
    map_start, map_end = TAPE_STACK_MAPPING
    if not (map_start <= current_sp < TAPE_STACK_TOP <= map_end):
        return None
    if current_sp <= pointer and end <= TAPE_STACK_TOP:
        return {'kind': 'active_guest_stack', 'mapping': [hex(map_start), hex(map_end)],
                'stack_top': hex(TAPE_STACK_TOP), 'callback_sp': hex(current_sp),
                'offset_from_sp': pointer - current_sp, 'span_bytes': length}
    return None


WITNESS_INPUT = (0x200eedf0, 64, 0x200eed80)
WITNESS_OUTPUT = {
    "kind": "active_guest_stack", "mapping": ["0x20000000", "0x20100000"],
    "stack_top": "0x200ef000", "callback_sp": "0x200eed80",
    "offset_from_sp": 0x70, "span_bytes": 64,
}
REJECTION_CASES = {
    "below_callback_sp": (0x200eed7f, 1, 0x200eed80),
    "crosses_stack_top": (0x200eeff0, 32, 0x200eed80),
    "mapped_stub_area": (0x200f2080, 64, 0x200eed80),
    "mapped_but_inactive_lower_region": (0x200e0000, 64, 0x200eed80),
    "invalid_callback_sp": (0x200eedf0, 64, 0x20100000),
    "zero_length": (0x200eedf0, 0, 0x200eed80),
    "overflow": (0xfffffffffffffff0, 64, 0x200eed80),
}


def assignment_names(target):
    if isinstance(target, ast.Name):
        return {target.id}
    if isinstance(target, (ast.Tuple, ast.List)):
        return set().union(*(assignment_names(item) for item in target.elts))
    if isinstance(target, ast.Starred):
        return assignment_names(target.value)
    return set()


def selected_policy_ast(source):
    declarations = {"TAPE_STACK_MAPPING": [], "TAPE_STACK_TOP": [], "_tape_stack_source_owner": []}
    for node in ast.parse(source).body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                for name in assignment_names(target) & declarations.keys():
                    if not isinstance(target, ast.Name):
                        raise ValueError(f"unsupported direct policy binding: {name}")
                    declarations[name].append(node)
        elif isinstance(node, (ast.AnnAssign, ast.AugAssign)):
            if assignment_names(node.target) & declarations.keys():
                raise ValueError("unsupported annotated or augmented policy binding")
        elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            if node.name in declarations:
                if isinstance(node, ast.FunctionDef) and node.name == "_tape_stack_source_owner":
                    declarations[node.name].append(node)
                else:
                    raise ValueError(f"unsupported direct policy binding: {node.name}")
        elif isinstance(node, (ast.Import, ast.ImportFrom)):
            bound = {alias.asname or (alias.name.split(".")[0] if isinstance(node, ast.Import) else alias.name)
                     for alias in node.names}
            if bound & declarations.keys():
                raise ValueError("unsupported imported policy binding")
        elif isinstance(node, ast.Delete):
            if any(assignment_names(target) & declarations.keys() for target in node.targets):
                raise ValueError("unsupported direct policy deletion")
        elif isinstance(node, (ast.For, ast.AsyncFor)):
            if assignment_names(node.target) & declarations.keys():
                raise ValueError("unsupported loop policy binding")
        elif isinstance(node, (ast.With, ast.AsyncWith)):
            if any(assignment_names(item.optional_vars) & declarations.keys() for item in node.items):
                raise ValueError("unsupported context-manager policy binding")
        elif isinstance(node, ast.Expr) and isinstance(node.value, ast.NamedExpr):
            if assignment_names(node.value.target) & declarations.keys():
                raise ValueError("unsupported named-expression policy binding")
    if any(len(nodes) != 1 for nodes in declarations.values()):
        raise ValueError("expected exactly one mapping, stack top and stack-owner function")
    return {name: ast.dump(nodes[0], include_attributes=False) for name, nodes in declarations.items()}


def recorded_witness_matches(manifest):
    contract = manifest.get("native_stack_upload_contract")
    if not isinstance(contract, dict):
        return False
    recorded = contract.get("v16_upload_span_inside_active_stack")
    if not isinstance(recorded, dict):
        return False
    expected = {"pointer": "0x200eedf0", "length": 64, "callback_sp": "0x200eed80",
                "end": "0x200eee30", "below_stack_top": True, "overlaps_stub_region": False}
    return (contract.get("mapped_stack_and_stub_arena") == WITNESS_OUTPUT["mapping"] and
            all(type(recorded.get(key)) is type(value) and recorded[key] == value
                for key, value in expected.items()))


def check_stack_contract(package):
    package = package.resolve()
    identity_phase = {"name": "source_identity", "status": "invalid"}
    phases = [identity_phase]
    result = {"schema": "sdocx-tapepen-stack-policy-check-v1", "package_directory": str(package),
              "scope": "Pure interval ownership only: source identity and selected AST equivalence, "
                       "recorded witness and seven rejection cases. No supplied code execution, "
                       "guest-memory reads, callback wiring, native inspection or runtime admission.",
              "behavioral_preflight": "not_run", "phases": phases}
    try:
        manifest_path = bounded_path(package, "preflight.json")
        manifest = json.loads(manifest_path.read_text())
        if not isinstance(manifest, dict) or manifest.get("schema") != SCHEMAS["preflight.json"]:
            raise ValueError("expected retained V7 preflight manifest")
        source_check = hash_check(package, "capture_candidate.py", manifest.get("runner_sha256"))
        identity_phase.update(status=source_check["status"],
                              manifest_sha256=digest_file(manifest_path), source=source_check)
        if source_check["status"] != "passed":
            raise ValueError("runner identity must match before checking its policy")
        source = bounded_path(package, "capture_candidate.py").read_text()
        maintained = Path(__file__).read_text()
        ast_phase = {"name": "selected_policy_ast", "status": "invalid",
                     "declarations": ["TAPE_STACK_MAPPING", "TAPE_STACK_TOP", "_tape_stack_source_owner"],
                     "maintained_checker_sha256": digest_file(Path(__file__))}
        phases.append(ast_phase)
        matches = selected_policy_ast(source) == selected_policy_ast(maintained)
        ast_phase["status"] = "passed" if matches else "mismatched"
        if not matches:
            raise ValueError("runner policy differs from the maintained interval policy")
        actual = _tape_stack_source_owner(*WITNESS_INPUT)
        recorded_matches = recorded_witness_matches(manifest)
        phases.append({"name": "recorded_witness", "status": "passed" if recorded_matches and actual == WITNESS_OUTPUT else "mismatched",
                       "input": {"pointer": hex(WITNESS_INPUT[0]), "length": WITNESS_INPUT[1], "callback_sp": hex(WITNESS_INPUT[2])},
                       "expected": WITNESS_OUTPUT, "actual": actual, "manifest_witness_matches": recorded_matches})
        rejections = {name: _tape_stack_source_owner(*inputs) is None for name, inputs in REJECTION_CASES.items()}
        phases.append({"name": "rejection_cases", "status": "passed" if all(rejections.values()) else "mismatched",
                       "rejected": rejections})
    except FileNotFoundError as error:
        phases[-1]["status"] = "missing"
        result["error"] = str(error)
    except OSError as error:
        phases[-1]["status"] = "unreadable"
        result["error"] = str(error)
    except (ValueError, SyntaxError, UnicodeError) as error:
        result["error"] = str(error)
    present = {item["name"] for item in phases}
    for name in ("source_identity", "selected_policy_ast", "recorded_witness", "rejection_cases"):
        if name not in present:
            phases.append({"name": name, "status": "not_evaluated"})
    result["policy_check"] = "passed" if all(item["status"] == "passed" for item in phases) else "blocked"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-dir", type=Path, required=True)
    result = check_stack_contract(parser.parse_args().package_dir)
    print(json.dumps(result, indent=2))
    return 0 if result["policy_check"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
