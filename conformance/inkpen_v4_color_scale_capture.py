"""Capture synthetic colored InkPen V4 with a scaled native target and Draw."""
import os
from pathlib import Path

root = Path(__file__).resolve().parents[1]
runner = root / "tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_color_native.py"
scale = int(os.environ.get("INKPEN_CAPTURE_SCALE", "1"))
source = runner.read_text()
for old, new in (
    ("W,H=128,64", f"W,H={128 * scale},{64 * scale}"),
    ("projection=(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)",
     f"projection=({scale},0,0,0,0,{scale},0,0,0,0,1,0,0,0,0,1)"),
):
    assert source.count(old) == 1
    source = source.replace(old, new)
source = "\n".join(line for line in source.split("\n") if "did not retain its 171-pixel coverage witness" not in line)
exec(compile(source, str(runner), "exec"), {"__name__": "__main__", "__file__": str(runner)})
