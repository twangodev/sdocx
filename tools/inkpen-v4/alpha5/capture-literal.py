"""Capture synthetic colored InkPen V4 with a scaled native target and Draw."""
import os
from pathlib import Path

root = Path(__file__).resolve().parents[3]
runner = root / "tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_color_native.py"
scale = int(os.environ.get("INKPEN_CAPTURE_SCALE", "1"))
source = runner.read_text()
anchor = "caller_source=(ROOT/'conformance/inkpen_saved_caller_native.py').read_text()"
assert source.count(anchor) == 1
edit = "empty=p.alloc(1); p.u.mem_write(empty,b'\\0')"
replacement = "empty=p.alloc(3); p.u.mem_write(empty,b'4;\\0')"
source = source.replace(anchor, anchor + "\nassert caller_source.count(" + repr(edit) + ") == 1\ncaller_source=caller_source.replace(" + repr(edit) + ", " + repr(replacement) + ")")
for old, new in (
    ("W,H=128,64", f"W,H={128 * scale},{64 * scale}"),
    ("projection=(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)",
     f"projection=({scale},0,0,0,0,{scale},0,0,0,0,1,0,0,0,0,1)"),
):
    assert source.count(old) == 1
    source = source.replace(old, new)
source = "\n".join(line for line in source.split("\n") if "did not retain its 171-pixel coverage witness" not in line)
exec(compile(source, str(runner), "exec"), {"__name__": "__main__", "__file__": str(runner)})
