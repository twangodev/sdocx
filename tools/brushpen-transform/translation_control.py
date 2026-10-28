"""Replay a translated BrushPen native source control using the pinned local harness.

Requires the original capture tree at tmp/conformance-cleanup/native-capture-a241793,
its scratch symlink, and the pinned Unicorn and AArch64 toolchain under scratch.
Run from the project root with PYTHONPATH=scratch/apk-analysis-runtime/python.
"""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
TREE = ROOT / "tmp/conformance-cleanup/native-capture-a241793"
HARNESS = TREE / "conformance/brushpen_saved_caller_native.py"
EXPECTED = "66f4790e8eeb1ca867f3273a796303b6ffb108286916e1e245fc063cbd744ae0"
source = HARNESS.read_bytes()
if hashlib.sha256(source).hexdigest() != EXPECTED:
    raise SystemExit("native caller harness hash mismatch")
source = source.decode()
assert source.count("ROOT=pathlib.Path(__file__).resolve().parents[1]") == 1
assert source.count("input_case={'points':[[1.,-2.],[3.,1.],[6.,4.]]") == 1
source = source.replace(
    "ROOT=pathlib.Path(__file__).resolve().parents[1]",
    f"ROOT=pathlib.Path({str(TREE)!r})",
).replace(
    "input_case={'points':[[1.,-2.],[3.,1.],[6.,4.]]",
    "input_case={'points':[[101.,98.],[103.,101.],[106.,104.]]",
)
with tempfile.TemporaryDirectory(prefix="brushpen-translation-") as tmp:
    script = Path(tmp) / "capture.py"
    script.write_text(source)
    result = subprocess.run(
        [sys.executable, str(script), "--print-capture"],
        cwd=ROOT,
        env=os.environ.copy(),
        check=True,
        capture_output=True,
        text=True,
    )
capture = json.loads(result.stdout)
fixture = json.loads(
    (ROOT / "conformance/brushpen-translation-native.json").read_text()
)
for key in (
    "schema",
    "profile",
    "input",
    "native_libraries_sha256",
    "native_helper_sha256",
    "host_boundaries",
):
    if capture[key] != fixture[key]:
        raise SystemExit(f"native capture mismatch: {key}")
for key in ("buffer_hex", "buffer_sha256", "buffer_bytes", "callback_count"):
    if capture["producer"][key] != fixture["producer"][key]:
        raise SystemExit(f"native stream mismatch: {key}")
print("BrushPen translated source native capture matches compact receipt")
