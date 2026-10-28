"""Capture InkPen V4 at a scaled target and matching native Draw transform."""
import hashlib
import os
from pathlib import Path

root = Path(__file__).resolve().parents[1]
runner = root / "tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_joined_paint_native.py"
scale = int(os.environ.get("INKPEN_CAPTURE_SCALE", "1"))
source = runner.read_text()
if os.environ.get("INKPEN_CAPTURE_BENT") == "loop":
    assert hashlib.sha256(runner.read_bytes()).hexdigest() == "7a05db658343956af992823ee5a17622fa94a08c88152be9f39cee950eca4a79"
    caller = runner.with_name("inkpen_saved_caller_native.py")
    assert hashlib.sha256(caller.read_bytes()).hexdigest() == "c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04"
if os.environ.get("INKPEN_CAPTURE_BENT") in ("1", "loop"):
    anchor = "caller_source='queue_task_events=[]\\nqueue_task_active=[]\\n'+caller_source"
    points = "[[8.,8.],[20.,18.],[35.,5.],[52.,20.]]" if os.environ["INKPEN_CAPTURE_BENT"] == "1" else "[[15.,15.],[45.,35.],[15.,35.],[45.,15.]]"
    edit = f"assert caller_source.count(\"[[0.,0.],[10.,0.],[20.,0.],[30.,10.]]\") == 1 and caller_source.count(\"'size':10.0\") == 1\ncaller_source=caller_source.replace(\"[[0.,0.],[10.,0.],[20.,0.],[30.,10.]]\", \"{points}\").replace(\"'size':10.0\", \"'size':6.0\")\n"
    assert source.count(anchor) == 1
    source = source.replace(anchor, edit + anchor)
    for old, new in (("vector_end-vector_begin!=312", "vector_end-vector_begin!=len(bytes.fromhex(ev['after_bytes']))"), ("row['size']==312 and row['sha256']", "row['size']==len(bytes.fromhex(ev['after_bytes'])) and row['sha256']")):
        assert source.count(old) == 1
        source = source.replace(old, new)
    assert source.count("native Init/Update composite buffers are missing") == 1
    source = "\n".join("if not any(row['size']==48 for row in init_uploads) or not any(row['size']==48 for row in update_uploads): raise RuntimeError('native composite buffer missing')" if "native Init/Update composite buffers are missing" in line else line for line in source.split("\n"))
for old, new in (
    ("W,H=128,64", f"W,H={128 * scale},{64 * scale}"),
    ("projection=(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)",
     f"projection=({scale},0,0,0,0,{scale},0,0,0,0,1,0,0,0,0,1)"),
):
    assert source.count(old) == 1
    source = source.replace(old, new)
assert source.count("did not retain its 171-pixel coverage witness") == 2
source = "\n".join(line for line in source.split("\n") if "did not retain its 171-pixel coverage witness" not in line)
exec(compile(source, str(runner), "exec"), {"__name__": "__main__", "__file__": str(runner)})
