"""Pure package-admission cases; no native runtime or research bundle needed."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import check_package


def digest(data):
    return hashlib.sha256(data).hexdigest()


class PackageReadinessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sdocx-tapepen-package-test-")
        self.addCleanup(self.temporary.cleanup)
        self.workspace = Path(self.temporary.name)
        self.package = self.workspace / "package"
        self.package.mkdir()
        dependency = self.workspace / "inputs/control.bin"
        dependency.parent.mkdir()
        dependency.write_bytes(b"synthetic dependency")
        self.manifest = {"schema": check_package.SCHEMAS["preflight.json"],
                         "pinned_files": {"inputs/control.bin": digest(dependency.read_bytes())},
                         "preflight_script": "package/preflight_candidate.py"}
        for name, key in check_package.SOURCE_PINS.items():
            data = b"raise RuntimeError('package source must never execute')\n"
            (self.package / name).write_bytes(data)
            self.manifest[key] = digest(data)
        self.candidate = {"schema": check_package.SCHEMAS["candidate.json"],
                          **{key: self.manifest[key] for key in check_package.SOURCE_PINS.values()},
                          "preflight_script": "package/preflight_candidate.py"}
        self.write_documents()

    def write_documents(self):
        raw = json.dumps(self.manifest).encode()
        (self.package / "preflight.json").write_bytes(raw)
        self.candidate["manifest_sha256"] = digest(raw)
        self.candidate["run_command"] = (
            "python3 package/capture_candidate.py --preflight-manifest package/preflight.json "
            "--preflight-sha256 " + digest(raw))
        self.write_candidate()

    def write_candidate(self):
        (self.package / "candidate.json").write_text(json.dumps(self.candidate))

    def result(self):
        return check_package.check_package(self.package, self.workspace)

    def phase(self, result, name):
        return next(item for item in result["phases"] if item["name"] == name)

    def test_consistent_package_admits_only_recorded_inputs_without_running_sources(self):
        result = self.result()
        self.assertEqual(result["package_admission"], "passed")
        self.assertEqual(result["behavioral_preflight"], "not_run")
        self.assertTrue(all(phase["status"] == "passed" for phase in result["phases"]))

    def test_missing_and_changed_inputs_are_distinct(self):
        (self.package / "tapepen_canvas_native.py").unlink()
        (self.workspace / "inputs/control.bin").write_bytes(b"changed")
        result = self.result()
        self.assertEqual(self.phase(result, "local_source_identities")["status"], "missing")
        self.assertEqual(self.phase(result, "declared_workspace_inputs")["status"], "mismatched")

    def test_candidate_source_and_manifest_joins_are_checked(self):
        self.candidate["runner_sha256"] = "0" * 64
        self.candidate["manifest_sha256"] = "1" * 64
        self.write_candidate()
        checks = self.phase(self.result(), "candidate_manifest_join")["checks"]
        self.assertEqual(sum(check["status"] == "mismatched" for check in checks), 2)

    def test_historical_command_location_is_retained_instead_of_rewritten(self):
        self.candidate["run_command"] = self.candidate["run_command"].replace(
            "package/capture_candidate.py", "historical/capture_candidate.py")
        self.write_candidate()
        result = self.result()
        phase = self.phase(result, "historical_invocation_location")
        self.assertEqual(phase["status"], "relocation_required")
        self.assertEqual(phase["recorded_command"], self.candidate["run_command"])
        self.assertEqual(phase["checks"][0]["recorded_path"], "historical/capture_candidate.py")
        self.assertEqual(result["package_admission"], "blocked")

    def test_external_and_symlink_escaped_dependencies_are_not_admitted(self):
        with tempfile.TemporaryDirectory(prefix="sdocx-tapepen-outside-") as outside:
            external = Path(outside) / "secret.bin"
            external.write_bytes(b"outside boundary")
            (self.workspace / "link").symlink_to(external)
            self.manifest["pinned_files"] = {"../outside.bin": "0" * 64,
                                             str(external): digest(external.read_bytes()),
                                             "link": digest(external.read_bytes())}
            self.write_documents()
            checks = self.phase(self.result(), "declared_workspace_inputs")["checks"]
            self.assertTrue(all(check["status"] == "invalid" for check in checks))
            self.assertTrue(all("actual_sha256" not in check for check in checks))

    def test_missing_or_malformed_documents_leave_dependent_phases_unevaluated(self):
        (self.package / "candidate.json").unlink()
        (self.package / "preflight.json").write_text("{broken")
        result = self.result()
        self.assertEqual(result["phases"][0]["status"], "invalid")
        self.assertEqual({check["status"] for check in result["phases"][0]["checks"]},
                         {"invalid", "missing"})
        self.assertTrue(all(item["status"] == "not_evaluated" for item in result["phases"][1:]))

    def test_wrong_schema_empty_inventory_and_malformed_digest_fail_closed(self):
        self.manifest["schema"] = "unexpected"
        self.write_documents()
        self.assertEqual(self.result()["phases"][0]["status"], "invalid")
        self.manifest["schema"] = check_package.SCHEMAS["preflight.json"]
        self.manifest["pinned_files"] = {}
        self.manifest["runner_sha256"] = "not a digest"
        self.write_documents()
        result = self.result()
        self.assertEqual(self.phase(result, "declared_workspace_inputs")["status"], "invalid")
        self.assertEqual(self.phase(result, "local_source_identities")["status"], "invalid")

    def test_invocation_escapes_duplicate_options_and_changed_manifest_hash_are_rejected(self):
        original = self.candidate["run_command"]
        for command, expected in (
            (original.replace("package/capture_candidate.py", "../outside.py"), "invalid"),
            (original + " --preflight-manifest second.json", "invalid"),
            (original.replace(self.candidate["manifest_sha256"], "0" * 64), "mismatched"),
        ):
            with self.subTest(command=command):
                self.candidate["run_command"] = command
                self.write_candidate()
                self.assertEqual(self.phase(self.result(), "historical_invocation_location")["status"], expected)


if __name__ == "__main__":
    unittest.main()
