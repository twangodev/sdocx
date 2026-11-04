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
trajectory = "[[0.,0.],[10.,0.],[20.,0.],[30.,10.]]"
crossing = "[[15.,15.],[45.,35.],[15.,35.],[45.,15.]]"
edit_path = f"assert caller_source.count({trajectory!r}) == 1 and caller_source.count(\"'size':10.0\") == 1\ncaller_source=caller_source.replace({trajectory!r}, {crossing!r}).replace(\"'size':10.0\", \"'size':6.0\")\n"
queue_anchor = "caller_source='queue_task_events=[]\\nqueue_task_active=[]\\n'+caller_source"
assert source.count(queue_anchor) == 1
source = source.replace(queue_anchor, edit_path + queue_anchor)
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
source = "\n".join(line for line in source.split("\n") if "did not retain its 171-pixel coverage witness" not in line)
exec(compile(source, str(runner), "exec"), {"__name__": "__main__", "__file__": str(runner)})
