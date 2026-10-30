"""Diagnostic host draw splitting at the 1920x1080 control's differing pixel."""

import hashlib
import json
from pathlib import Path
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
SPLIT = """
  saved = self._query_int(0x8894)
  bind = ip.cfunc(self.gles.gl,'glBindBuffer',None,[C.c_uint,C.c_uint])
  pointer = ip.cfunc(self.gles.gl,'glVertexAttribPointer',None,[C.c_uint,C.c_int,C.c_uint,C.c_ubyte,C.c_int,C.c_void_p])
  attributes = [a for a in before['attributes'] if a['divisor']==1]
  if len(attributes)!=3 or vals!=[5,0,4,480]:raise RuntimeError('unexpected diagnostic draw layout')
  self.prefix_probe=[]
  for instance in range(vals[3]):
   for a in attributes:
    bind(0x8892,a['buffer'])
    pointer(a['index'],a['size'],a['type'],a['normalized'],a['stride'],C.c_void_p(int(a['pointer'],16)+instance*a['stride']))
   self.gles.f['glDrawArraysInstanced'](vals[0],vals[1],vals[2],1)
   pixel=C.create_string_buffer(4)
   self.gles.f['glReadPixels'](10,1079,1,1,0x1908,0x1401,C.cast(pixel,C.c_void_p))
   if self._drain_gl_errors():raise RuntimeError('diagnostic prefix draw/read failed')
   self.prefix_probe.append(list(pixel.raw))
  for a in attributes:
   bind(0x8892,a['buffer'])
   pointer(a['index'],a['size'],a['type'],a['normalized'],a['stride'],C.c_void_p(int(a['pointer'],16)))
  bind(0x8892,saved)
"""
source = HELPER.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == PIN
anchor = "    exec(\n"
assert source.count(anchor) == 1
injection = """    target = "  self.gles.f['glDrawArraysInstanced'](*vals)"
    assert code.count(target)==1
    code = code.replace(target,SPLIT)
    code = code.replace(schema,schema+",'prefix_probe':p.prefix_probe")
"""
source = source.replace(anchor, injection + anchor)
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-prefix-") as directory:
    report = Path(directory) / "native.json"
    sys.argv = [
        str(HELPER),
        "--child",
        "--width",
        "1920",
        "--height",
        "1080",
        "--scale",
        "1",
        "--repeat",
        "32",
        "--output",
        str(report),
    ]
    try:
        exec(
            compile(source, str(HELPER), "exec"),
            {
                "__name__": "watercolor_prefix_probe",
                "__file__": str(HELPER),
                "SPLIT": SPLIT,
            },
        )
    except SystemExit as done:
        if done.code not in (None, 0):
            raise
    native = json.loads(report.read_text())
    baseline = json.loads(
        (ROOT / "conformance/watercolor-redraw-frames-native.json").read_text()
    )["cases"]["1920x1080_1_32"]
    assert native["capture_status"] == "captured_without_host_gl_errors"
    assert (
        native["native_mask_pass_readback"]["pixels_sha256"] == baseline["mask_sha256"]
    )
    assert native["readback"]["sha256"] == baseline["final_sha256"]
    receipt = {
        "schema": "sdocx-watercolor-prefix-pixel-probe-v1",
        "frame": [1920, 1080],
        "pixel_bottom_origin": [10, 1079],
        "capture": {
            "script": "tools/watercolor_host/prefix_probe.py",
            "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "frame_helper_sha256": PIN,
        },
        "scope": "Diagnostic host split of the native 480-instance draw into 480 one-instance calls, with divisor-one attribute offsets advanced and restored. Shipped shader, native AddPoint stream, Draw, blend and textures unchanged. Full mask and final hashes required to match the original unsplit native control.",
        "mask_sha256": baseline["mask_sha256"],
        "final_sha256": baseline["final_sha256"],
        "prefix_rgba": native["prefix_probe"],
    }
    target = ROOT / "conformance/watercolor-prefix-pixel-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
