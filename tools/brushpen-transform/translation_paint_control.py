"""Replay translated Drawing-to-Mesa paint with pinned project-local inputs.

Requires the native capture tree and scratch environment described by
translation_control.py. Run with PYTHONPATH=scratch/apk-analysis-runtime/python
and EGL_PLATFORM=surfaceless; the trace and intermediate script stay temporary.
"""

import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "tools/brushpen-transform/second_mapdraw.py"
PROBE = ROOT / "tools/brushpen-transform/long_probe.py"
FIXTURE = ROOT / "conformance/brushpen-translation-native.json"
RECEIPT = ROOT / "conformance/brushpen-translation-paint/receipt.json"
CROP = ROOT / "conformance/brushpen-translation-paint/native-crop-5x7.rgba"
PINS = {
    RUNNER: "9e9632df53c725b6018ba6892edc3605649762f7eba6e7b7098f25af6a8d185f",
    PROBE: "77e48b4625361cd14eb5aa4637e31af5e7e3da52794547e93681f5b0e6bbeb40",
    FIXTURE: "1daa319db795ed173356762e7ddae1fa5759d3c2b1a5237be81c578dabea45d5",
}
for path, expected in PINS.items():
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit(f"input hash mismatch: {path}")


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError(f"expected one injection anchor: {old}")
    return source.replace(old, new)


source = RUNNER.read_text()
source = replace_once(
    source, "Path(__file__).with_name('long_probe.py')", f"Path({str(PROBE)!r})"
)
source = replace_once(
    source,
    '.read_text().split("for n in [\'SPen::ObjectDrawing")[0])',
    ".read_text().split(\"for n in ['SPen::ObjectDrawing\")[0].replace('ROOT=Path(__file__).resolve().parents[2]', "
    + repr(f"ROOT=Path({str(ROOT)!r})")
    + "))",
)
source = replace_once(source, "[obj],[7.0]", "[obj],[3.144]")
source = replace_once(source, "[obj,0x806e42d1]", "[obj,0xff6cdd8f]")
source = replace_once(
    source, "Path('/tmp/brushpen-second-geometry.json')", f"Path({str(FIXTURE)!r})"
)
source = replace_once(source, "'len(raw) != 221 * 44'", "'len(raw) != 26 * 44'")
a = source.index("ns['OBJECT_CASE']=")
b = source.index(";exec(compile", a)
source = (
    source[:a]
    + "ns['OBJECT_CASE']={'name':'translated-synthetic','points':[[101.,98.],[103.,101.],[106.,104.]],'tool_type':2,'pressures':[.3,.6,.85],'times':[300,315,340],'tilts':[.12,.32,.55],'orientations':[.2,.35,.55]}"
    + source[b:]
)
with tempfile.TemporaryDirectory(prefix="brushpen-translated-paint-") as directory:
    source = replace_once(
        source,
        "Path('/tmp/brushpen-rebound-source.py')",
        f"Path({str(Path(directory) / 'rebound.py')!r})",
    )
    script = Path(directory) / "capture.py"
    script.write_text(source)
    env = dict(os.environ, BRUSHPEN_NATIVE_PEN_DATA_MANAGER="1")
    result = subprocess.run(
        [sys.executable, str(script)],
        cwd=ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
lines = result.stdout.splitlines()
if not any(line.startswith("SAME_DRAWING_RESULT ") for line in lines) or any(
    line.startswith("SAME_DRAWING_STOP ") for line in lines
):
    raise SystemExit("native same-Drawing paint did not complete")
target = json.loads(
    next(line[12:] for line in lines if line.startswith("TASK2_STATE "))
)["native_canvas_rgba_target"]
frame = base64.b64decode(target["after_task2_base64"], validate=True)
receipt = json.loads(RECEIPT.read_text())
if hashlib.sha256(frame).hexdigest() != receipt["native_frame_sha256"]:
    raise SystemExit("native frame digest mismatch")
crop = b"".join(
    frame[(y * 321 + 100) * 4 : (y * 321 + 105) * 4] for y in range(97, 104)
)
if crop != CROP.read_bytes():
    raise SystemExit("native crop mismatch")
uniforms = json.loads(
    next(
        line.split(" ", 1)[1]
        for line in lines
        if line.startswith("UNIFORM_NATIVE_IMPORT_TRACE ")
    )
)
if uniforms != receipt["draw_time_uniforms"]:
    raise SystemExit("draw-time uniforms mismatch")
print(
    "Translated BrushPen native Drawing-to-Mesa frame, crop and draw uniforms match receipt"
)
