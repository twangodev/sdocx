"""Isolate the unmodified native WaterColor composite shader over all R/G bytes.

The host replaces the generated mask texture after its draw, before native
compositing. This is an explicit shader-input probe, not natural mask generation.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (
    ROOT
    / "tmp/vector-parity-plan/pen-family-audit/watercolor-mesa-paint-v15-density-instance-readback/capture_candidate.py"
)
PIN = "98c406b9f7ffe7ae36705c33517bda4b88210da3b59d7af5ae7cb3bfb0867058"
parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--argb", default="0x8044aaee")
parser.add_argument("--background", default="0,0,0,0")
parser.add_argument("--output", type=Path)
args = parser.parse_args()
SEED_CODE = "\n native_call('SPen::SPSubBitmap::CreateFrameBuffer()',[bitmap],instruction_limit=5000000)\n native_call('SPen::SPSubBitmap::ActivateFrameBuffer()',[bitmap],instruction_limit=5000000)\n attachment=C.c_int()\n ip.cfunc(p.gles.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])(0x8d40,0x8ce0,0x8cd1,C.byref(attachment))\n if not attachment.value:raise RuntimeError('prior upload requires native target attachment')\n saved=p._query_int(0x8069)\n bind=ip.cfunc(p.gles.gl,'glBindTexture',None,[C.c_uint,C.c_uint])\n upload=ip.cfunc(p.gles.gl,'glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])\n prior=BACKGROUND_PIXEL*(ARGS.width*ARGS.height)\n data=C.create_string_buffer(prior)\n bind(0x0de1,attachment.value)\n upload(0x0de1,0,0,0,ARGS.width,ARGS.height,0x1908,0x1401,C.cast(data,C.c_void_p))\n bind(0x0de1,saved)\n verify=C.create_string_buffer(len(prior))\n p.gles.f['glReadPixels'](0,0,ARGS.width,ARGS.height,0x1908,0x1401,C.cast(verify,C.c_void_p))\n if verify.raw!=prior or p._drain_gl_errors():raise RuntimeError('prior pixels do not match actual target before native Draw')\n p.output_prior_receipt={'attachment':attachment.value,'sha256':hashlib.sha256(prior).hexdigest(),'verified_readback':True}\n p.gles.f['glBindFramebuffer'](0x8d40,0)\n"
if args.child:
    original = SOURCE.read_text()
    assert hashlib.sha256(original.encode()).hexdigest() == PIN
    background = bytes(map(int, args.background.split(",")))
    assert len(background) == 4 and all(c <= background[3] for c in background[:3])
    code = original
    old = "initial=p.alloc(ARGS.width*ARGS.height*4); p.u.mem_write(initial,bytes(ARGS.width*ARGS.height*4))"
    assert code.count(old) == 1
    code = code.replace(
        old,
        "initial=p.alloc(ARGS.width*ARGS.height*4); p.u.mem_write(initial,BACKGROUND_PIXEL*(ARGS.width*ARGS.height))",
    )
    anchor = "   diag(f'passive mask RGBA readback returned bytes="
    start = code.index(anchor)
    end = code.index("\n", start)
    injection = """
   probe=bytes(channel for g in range(256) for r in range(256) for channel in (r,g,0,255))
   if len(attachment_names)!=1:raise RuntimeError('shader probe requires one verified mask attachment')
   texture=next(iter(attachment_names))
   saved=self._query_int(0x8069)
   bind=ip.cfunc(self.gles.gl,'glBindTexture',None,[C.c_uint,C.c_uint])
   upload=ip.cfunc(self.gles.gl,'glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
   data=C.create_string_buffer(probe)
   bind(0x0de1,texture)
   upload(0x0de1,0,0,0,256,256,0x1908,0x1401,C.cast(data,C.c_void_p))
   bind(0x0de1,saved)
   verify=C.create_string_buffer(len(probe))
   read(0,0,256,256,0x1908,0x1401,C.cast(verify,C.c_void_p))
   if verify.raw!=probe or self._drain_gl_errors():raise RuntimeError('shader mask upload/readback mismatch')
   self.shader_probe_receipt={'mask_sha256':hashlib.sha256(probe).hexdigest(),'attachment':texture,'verified_readback':True,'row_origin':'bottom-left','rule':'pixel x=mask red, y=mask green, B=0,A=255'}
"""
    code = code[:end] + injection + code[end:]
    old = "'schema':'sdocx-watercolor-rtv1-public-control-mesa-paint-v15-density-instance-mxcsr-readback'"
    assert code.count(old) == 1
    code = code.replace(old, old + ",'shader_input_probe':p.shader_probe_receipt")
    pre_composite = (
        "  mxcsr_before=self._read_mxcsr()\n  self.gles.f['glDrawArrays'](*vals)"
    )
    assert code.count(pre_composite) == 1
    probe = """
  prior_read = C.create_string_buffer(ARGS.width * ARGS.height * 4)
  self.gles.f['glReadPixels'](0, 0, ARGS.width, ARGS.height, 0x1908, 0x1401, C.cast(prior_read, C.c_void_p))
  if self._drain_gl_errors(): raise RuntimeError('pre-composite readback failed')
  self.pre_composite_prior = {'sha256': hashlib.sha256(prior_read.raw).hexdigest(), 'all_zero': not any(prior_read.raw), 'attachment': before['attachments'][0]['object_name']}
"""
    probe += """
  saved = self._query_int(0x8069)
  bind = ip.cfunc(self.gles.gl, 'glBindTexture', None, [C.c_uint, C.c_uint])
  upload = ip.cfunc(self.gles.gl, 'glTexSubImage2D', None, [C.c_uint, C.c_int, C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p])
  prior = BACKGROUND_PIXEL * (ARGS.width * ARGS.height)
  data = C.create_string_buffer(prior)
  bind(0x0de1, before['attachments'][0]['object_name'])
  upload(0x0de1, 0, 0, 0, ARGS.width, ARGS.height, 0x1908, 0x1401, C.cast(data, C.c_void_p))
  bind(0x0de1, saved)
  self.gles.f['glReadPixels'](0, 0, ARGS.width, ARGS.height, 0x1908, 0x1401, C.cast(prior_read, C.c_void_p))
  if prior_read.raw != prior or self._drain_gl_errors(): raise RuntimeError('composite input prior readback mismatch')
  self.composite_input_prior = {'sha256': hashlib.sha256(prior).hexdigest(), 'verified_readback': True, 'scope': 'diagnostic upload immediately before native composite glDrawArrays'}
"""
    code = code.replace(pre_composite, probe + pre_composite)
    draw = " native_call('SPen::WaterColorBrushStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,matrix],instruction_limit=8000000)"
    assert code.count(draw) == 1
    code = code.replace(draw, SEED_CODE + draw)
    schema = "'schema':'sdocx-watercolor-rtv1-public-control-mesa-paint-v15-density-instance-mxcsr-readback'"
    assert code.count(schema) == 1
    code = code.replace(
        schema,
        schema
        + ",'verified_output_prior':p.output_prior_receipt,'pre_composite_prior':p.pre_composite_prior"
        + ",'composite_input_prior':p.composite_input_prior",
    )
    sys.argv = [
        str(SOURCE),
        "--write-fixture",
        "--case",
        "watercolor1",
        "--argb",
        args.argb,
        "--fixture",
        str(args.output),
    ]
    exec(
        compile(code, str(SOURCE), "exec"),
        {
            "__name__": "__main__",
            "__file__": str(SOURCE),
            "BACKGROUND_PIXEL": background,
        },
    )
    raise SystemExit


def sha(data):
    return hashlib.sha256(data).hexdigest()


controls = [
    (color, [0, 0, 0, 0]) for color in [0x8044AAEE, 0xFF44AAEE, 0x0044AAEE, 0x80C04020]
]
controls += [(0x8044AAEE, bg) for bg in [[255] * 4, [64, 32, 96, 128]]]
receipt = {
    "schema": "sdocx-watercolor-composite-rg-kernel-v1",
    "frame": [256, 256],
    "capture": {
        "script": "tools/watercolor_host/shader_probe.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "base_source_sha256": PIN,
    },
    "mask_rule": "bottom-origin pixel(x,y) has RGBA=(x,y,0,255); verified exact upload into the native mask attachment between native mask and composite passes",
    "scope": "Diagnostic mask replacement and verified output-texture upload immediately before native composite isolate the unchanged native RTV1 non-eraser composite shader and blend over all 65,536 R/G byte pairs. Not evidence of natural mask generation, saved-note admission or device parity.",
    "cases": {},
}
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-kernel-") as directory:
    directory = Path(directory)
    for color, bg in controls:
        name = f"{color:08x}_" + "".join(f"{c:02x}" for c in bg)
        output = directory / (name + ".json")
        with (
            (directory / "stdout").open("w") as stdout,
            (directory / "stderr").open("w") as stderr,
        ):
            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    "--argb",
                    hex(color),
                    "--background",
                    ",".join(map(str, bg)),
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                env=os.environ,
                stdout=stdout,
                stderr=stderr,
                timeout=120,
            )
        if result.returncode:
            print((directory / "stderr").read_text()[-5000:])
            raise SystemExit(result.returncode)
        native = json.loads(output.read_text())
        assert native["capture_status"] == "captured_without_host_gl_errors"
        raw = zlib.decompress(base64.b64decode(native["readback"]["rgba_zlib_base64"]))
        receipt["cases"][name] = {
            "argb": color,
            "background": bg,
            "sha256": sha(raw),
            "rgba_zlib_base64": base64.b64encode(zlib.compress(raw, 9)).decode(),
            "probe": native["shader_input_probe"],
            "verified_output_prior": native["verified_output_prior"],
            "pre_composite_prior": native["pre_composite_prior"],
            "composite_input_prior": native["composite_input_prior"],
        }
        if "host" not in receipt:
            receipt["host"] = {"gles": native["gles"], "libraries": native["source"]}
        print(name, sha(raw), flush=True)
    target = ROOT / "conformance/watercolor-composite-rg-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
