"""Capture public RTV1 density, projection and backdrop controls.

PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/watercolor_host/capture.py
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import struct
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
parser.add_argument("--repeat", type=int, default=1)
parser.add_argument(
    "--origin", choices=["center", "bottom", "top", "top2"], default="center"
)
parser.add_argument("--background", default="0,0,0,0")
parser.add_argument("--output", type=Path)
args = parser.parse_args()
SEED_CODE = "\n native_call('SPen::SPSubBitmap::CreateFrameBuffer()',[bitmap],instruction_limit=5000000)\n native_call('SPen::SPSubBitmap::ActivateFrameBuffer()',[bitmap],instruction_limit=5000000)\n attachment=C.c_int()\n ip.cfunc(p.gles.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])(0x8d40,0x8ce0,0x8cd1,C.byref(attachment))\n if not attachment.value:raise RuntimeError('prior upload requires native target attachment')\n saved=p._query_int(0x8069)\n bind=ip.cfunc(p.gles.gl,'glBindTexture',None,[C.c_uint,C.c_uint])\n upload=ip.cfunc(p.gles.gl,'glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])\n prior=BACKGROUND_PIXEL*(ARGS.width*ARGS.height)\n data=C.create_string_buffer(prior)\n bind(0x0de1,attachment.value)\n upload(0x0de1,0,0,0,ARGS.width,ARGS.height,0x1908,0x1401,C.cast(data,C.c_void_p))\n bind(0x0de1,saved)\n verify=C.create_string_buffer(len(prior))\n p.gles.f['glReadPixels'](0,0,ARGS.width,ARGS.height,0x1908,0x1401,C.cast(verify,C.c_void_p))\n if verify.raw!=prior or p._drain_gl_errors():raise RuntimeError('prior pixels do not match actual target before native Draw')\n p.output_prior_receipt={'attachment':attachment.value,'sha256':hashlib.sha256(prior).hexdigest(),'verified_readback':True}\n p.gles.f['glBindFramebuffer'](0x8d40,0)\n"
if args.child:
    original = SOURCE.read_text()
    assert hashlib.sha256(original.encode()).hexdigest() == PIN
    assert args.repeat in (1, 4, 8, 16, 32)
    background = bytes(map(int, args.background.split(",")))
    assert len(background) == 4 and all(c <= background[3] for c in background[:3])
    code = original.replace("choices=(1,32)", "choices=(1,4,8,16,32)")
    old = "initial=p.alloc(ARGS.width*ARGS.height*4); p.u.mem_write(initial,bytes(ARGS.width*ARGS.height*4))"
    assert code.count(old) == 1
    code = code.replace(
        old,
        "initial=p.alloc(ARGS.width*ARGS.height*4); p.u.mem_write(initial,BACKGROUND_PIXEL*(ARGS.width*ARGS.height))",
    )
    old = "matrix=p.alloc(64); p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,128,128,0,1)"
    assert code.count(old) == 1
    matrices = {
        "center": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 128, 128, 0, 1],
        "bottom": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        "top": [1, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 256, 0, 1],
        "top2": [2, 0, 0, 0, 0, -2, 0, 0, 0, 0, 1, 0, 0, 256, 0, 1],
    }
    code = code.replace(old, "matrix=p.alloc(64); p.put(matrix,'16f',*CONTROL_MATRIX)")
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
    code = code.replace(pre_composite, probe + pre_composite)
    draw = " native_call('SPen::WaterColorBrushStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,matrix],instruction_limit=8000000)"
    assert code.count(draw) == 1
    code = code.replace(draw, SEED_CODE + draw)
    schema = "'schema':'sdocx-watercolor-rtv1-public-control-mesa-paint-v15-density-instance-mxcsr-readback'"
    assert code.count(schema) == 1
    code = code.replace(
        schema,
        schema
        + ",'verified_output_prior':p.output_prior_receipt,'pre_composite_prior':p.pre_composite_prior",
    )
    sys.argv = [
        str(SOURCE),
        "--write-fixture",
        "--case",
        "watercolor1",
        "--repeat-count",
        str(args.repeat),
        "--fixture",
        str(args.output),
    ]
    exec(
        compile(code, str(SOURCE), "exec"),
        {
            "__name__": "__main__",
            "__file__": str(SOURCE),
            "BACKGROUND_PIXEL": background,
            "CONTROL_MATRIX": matrices[args.origin],
        },
    )
    raise SystemExit


def sha(data):
    return hashlib.sha256(data).hexdigest()


def sparse(raw, prior):
    return b"".join(
        struct.pack("<H", i // 4) + raw[i : i + 4]
        for i in range(0, len(raw), 4)
        if raw[i : i + 4] != prior[i : i + 4]
    )


controls = [(repeat, "center", [0, 0, 0, 0]) for repeat in [1, 4, 8, 16, 32]]
controls += [
    (repeat, origin, bg)
    for repeat in [1, 32]
    for origin in ["bottom", "top", "top2"]
    for bg in [[0, 0, 0, 0], [255] * 4, [64, 32, 96, 128]]
]
receipt = {
    "schema": "sdocx-watercolor-host-raster-controls-v1",
    "frame": [256, 256],
    "coordinates": "Sparse u16 pixel index + RGBA8, bottom-origin; final deltas relative to transparent black; native Draw clears the verified supplied prior before compositing.",
    "capture": {
        "script": "tools/watercolor_host/capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "base_source_sha256": PIN,
    },
    "scope": "Unmodified WaterColor RTV1 Init/CreatePenCanvas/AddPoint/Update/Draw. Original native AddPoint input tuples repeat through the public API; only the public Draw matrix and verified output target pixels before Draw vary. A passive read immediately before composite verifies the native output target is cleared; these are layer outputs, not page source-over controls. Explicit glibc/Mesa host, synchronous queue and target boundaries; not a saved-note or device profile.",
    "cases": {},
}
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-host-") as directory:
    directory = Path(directory)
    for repeat, origin, bg in controls:
        name = f"{origin}_{repeat}_" + "".join(f"{x:02x}" for x in bg)
        output = directory / (name + ".json")
        with (
            (directory / "stdout").open("w") as stdout,
            (directory / "stderr").open("w") as stderr,
        ):
            process = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    "--repeat",
                    str(repeat),
                    "--origin",
                    origin,
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
        if process.returncode:
            print((directory / "stderr").read_text()[-5000:])
            raise SystemExit(process.returncode)
        native = json.loads(output.read_text())
        assert native["capture_status"] == "captured_without_host_gl_errors"
        mask = bytes.fromhex(native["native_mask_pass_readback"]["pixels_raw"]["hex"])
        final = zlib.decompress(
            base64.b64decode(native["readback"]["rgba_zlib_base64"])
        )
        prior = bytes(bg) * (256 * 256)
        mask_sparse = sparse(mask, bytes(len(mask)))
        final_sparse = sparse(final, bytes(len(final)))
        receipt["cases"][name] = {
            "repeat": repeat,
            "origin": origin,
            "background": bg,
            "mask_sha256": sha(mask),
            "mask_sparse_hex": mask_sparse.hex(),
            "final_sha256": sha(final),
            "final_sparse_hex": final_sparse.hex(),
            "stream_sha256": native["controlled_rt"]["vector_sha256"],
            "verified_output_prior": native["verified_output_prior"],
            "pre_composite_prior": native["pre_composite_prior"],
            "high_unsaturated_red": sum(
                0 < mask[i] < 255 and mask[i + 1] >= 179 for i in range(0, len(mask), 4)
            ),
        }
        if "host" not in receipt:
            receipt["host"] = {
                "gles": native["gles"],
                "libraries": native["source"],
                "source_inputs": native["source_capture"],
            }
        print(
            name,
            "high-unsaturated",
            receipt["cases"][name]["high_unsaturated_red"],
            flush=True,
        )
    path = ROOT / "conformance/watercolor-host-raster-native.json"
    path.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(path, path.stat().st_size)
