"""Pure interval checks and explicit package/source-drift cases."""

import ast
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import check_stack_contract as stack


class StackPolicyTests(unittest.TestCase):
    def test_retained_witness_has_exact_owner_metadata(self):
        self.assertEqual(stack._tape_stack_source_owner(0x200eedf0, 64, 0x200eed80), {
            "kind": "active_guest_stack", "mapping": ["0x20000000", "0x20100000"],
            "stack_top": "0x200ef000", "callback_sp": "0x200eed80",
            "offset_from_sp": 112, "span_bytes": 64,
        })

    def test_historical_rejections(self):
        for name, inputs in stack.REJECTION_CASES.items():
            with self.subTest(name=name):
                self.assertIsNone(stack._tape_stack_source_owner(*inputs))

    def test_live_interval_endpoints_and_mapping_do_not_admit_inactive_bytes(self):
        self.assertIsNotNone(stack._tape_stack_source_owner(0x20000000, 1, 0x20000000))
        self.assertIsNotNone(stack._tape_stack_source_owner(0x200eefff, 1, 0x200eefff))
        for inputs in ((0x200ef000, 1, 0x200eefff), (0x20000000, 1, 0x20000001),
                       (0x20000000, 1, 0x1fffffff), (0x200eedf0, -1, 0x200eed80),
                       (0x10000000000000000, 1, 0x200eed80),
                       (0x200eedf0, 1.0, 0x200eed80)):
            with self.subTest(inputs=inputs):
                self.assertIsNone(stack._tape_stack_source_owner(*inputs))


class PackageStackContractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sdocx-stack-contract-test-")
        self.addCleanup(self.temporary.cleanup)
        self.package = Path(self.temporary.name)
        tree = ast.parse(Path(stack.__file__).read_text())
        names = {"TAPE_STACK_MAPPING", "TAPE_STACK_TOP", "_tape_stack_source_owner"}
        nodes = [node for node in tree.body if
                 (isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id in names
                                                       for target in node.targets)) or
                 (isinstance(node, ast.FunctionDef) and node.name in names)]
        self.source = "raise RuntimeError('supplied runner must never execute')\n" + "\n".join(ast.unparse(node) for node in nodes)
        self.manifest = {"schema": "tapepen-public-drawing-preflight-v7",
                         "native_stack_upload_contract": {
                             "mapped_stack_and_stub_arena": ["0x20000000", "0x20100000"],
                             "v16_upload_span_inside_active_stack": {
                                 "pointer": "0x200eedf0", "length": 64, "callback_sp": "0x200eed80",
                                 "end": "0x200eee30", "below_stack_top": True,
                                 "overlaps_stub_region": False}}}
        self.write_source()

    def write_manifest(self):
        (self.package / "preflight.json").write_text(json.dumps(self.manifest))

    def write_source(self):
        data = self.source.encode()
        (self.package / "capture_candidate.py").write_bytes(data)
        self.manifest["runner_sha256"] = hashlib.sha256(data).hexdigest()
        self.write_manifest()

    def check(self):
        return stack.check_stack_contract(self.package)

    def phase_status(self, name):
        return next(item["status"] for item in self.check()["phases"] if item["name"] == name)

    def test_matching_source_and_recorded_contract_do_not_execute_runner(self):
        result = self.check()
        self.assertEqual(result["policy_check"], "passed")
        self.assertEqual(result["behavioral_preflight"], "not_run")

    def test_source_pin_mismatch_blocks_ast_and_behavior_checks(self):
        (self.package / "capture_candidate.py").write_text("different source")
        result = self.check()
        self.assertEqual(result["phases"][0]["status"], "mismatched")
        self.assertTrue(all(item["status"] == "not_evaluated" for item in result["phases"][1:]))

    def test_pinned_but_changed_policy_is_rejected(self):
        self.source = self.source.replace("pointer <= 0", "pointer < 0")
        self.write_source()
        self.assertEqual(self.phase_status("selected_policy_ast"), "mismatched")

    def test_duplicate_absent_and_malformed_policy_declarations_are_rejected(self):
        original = self.source
        for changed in (original + "\nTAPE_STACK_TOP = 0x200EF000\n",
                        original.replace("def _tape_stack_source_owner", "def other_owner"),
                        "invalid syntax !"):
            with self.subTest(source=changed):
                self.source = changed
                self.write_source()
                self.assertEqual(self.phase_status("selected_policy_ast"), "invalid")

    def test_changed_or_malformed_recorded_witness_does_not_pass(self):
        witness = self.manifest["native_stack_upload_contract"]["v16_upload_span_inside_active_stack"]
        for field, changed in (("length", 63), ("end", "0x200eee31"),
                               ("below_stack_top", 1), ("overlaps_stub_region", True)):
            with self.subTest(field=field):
                original = witness[field]
                witness[field] = changed
                self.write_manifest()
                self.assertEqual(self.phase_status("recorded_witness"), "mismatched")
                witness[field] = original

    def test_additional_unsupported_direct_policy_bindings_are_rejected_after_repinning(self):
        original = self.source
        for binding in (
            "TAPE_STACK_TOP: int = 0",
            "_tape_stack_source_owner: object = None",
            "TAPE_STACK_TOP += 1",
            "async def _tape_stack_source_owner(pointer, length, current_sp):\n    return None",
            "def TAPE_STACK_TOP():\n    return 0",
            "class TAPE_STACK_MAPPING:\n    pass",
            "TAPE_STACK_TOP, other = (0, 1)",
            "import unknown_module as TAPE_STACK_TOP",
            "from unknown_module import owner as _tape_stack_source_owner",
            "del TAPE_STACK_TOP",
            "for TAPE_STACK_TOP in (0,):\n    pass",
            "with unknown_context() as TAPE_STACK_MAPPING:\n    pass",
            "(_tape_stack_source_owner := None)",
        ):
            with self.subTest(binding=binding):
                self.source = original + "\n" + binding + "\n"
                self.write_source()
                result = self.check()
                self.assertEqual(result["phases"][0]["status"], "passed")
                self.assertEqual(result["phases"][1]["status"], "invalid")
                self.assertTrue(all(item["status"] == "not_evaluated" for item in result["phases"][2:]))

    def test_missing_invalid_and_escaping_package_sources_fail_closed(self):
        (self.package / "capture_candidate.py").unlink()
        self.assertEqual(self.phase_status("source_identity"), "missing")
        with tempfile.TemporaryDirectory(prefix="sdocx-stack-outside-") as outside:
            external = Path(outside) / "runner.py"
            external.write_text(self.source)
            (self.package / "capture_candidate.py").symlink_to(external)
            self.assertEqual(self.phase_status("source_identity"), "invalid")
        (self.package / "preflight.json").write_text("[]")
        self.assertEqual(self.phase_status("source_identity"), "invalid")


if __name__ == "__main__":
    unittest.main()
