"""Capture public SetRedrawState and repeated WaterColor draws on one RTV1."""

import argparse
import ast
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
HELPER = ROOT / "tools/watercolor_host/capture.py"
HELPER_PIN = "e858481552bf018a6f72be0d0142bc505815a8f10fadfe0f14aedd6beba81c63"
PENCIL = ROOT / "conformance/pencil2-v14-mixed-rgb-native.json"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def backdrop(kind):
    prior = bytearray(bytes([64, 32, 96, 128]) * (256 * 256))
    if kind == "pencil":
        source = json.loads(PENCIL.read_text())
        case = source["results"]["mixed_background"]
        assert case["background"] == [64, 32, 96, 128]
        original = bytearray(bytes(case["background"]) * (192 * 192))
        for x, y, *rgba in case["pixels"]:
            original[(y * 192 + x) * 4 : (y * 192 + x + 1) * 4] = bytes(rgba)
        assert sha(original) == case["sha256"]
        for y in range(144):
            a = ((y + 112) * 256 + 112) * 4
            prior[a : a + 144 * 4] = original[y * 192 * 4 : (y * 192 + 144) * 4]
    return bytes(prior)


def sparse(raw, prior):
    return b"".join(
        struct.pack("<H", i // 4) + raw[i : i + 4]
        for i in range(0, len(raw), 4)
        if raw[i : i + 4] != prior[i : i + 4]
    )


parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--repeat", type=int, choices=[1, 32], default=1)
parser.add_argument("--states", default="0,1,0")
parser.add_argument("--backdrop", choices=["uniform", "pencil"], default="uniform")
parser.add_argument("--output", type=Path)
args = parser.parse_args()

if args.child:
    helper = HELPER.read_text()
    assert sha(helper.encode()) == HELPER_PIN
    seed = next(
        ast.literal_eval(n.value)
        for n in ast.parse(helper).body
        if isinstance(n, ast.Assign)
        and any(isinstance(t, ast.Name) and t.id == "SEED_CODE" for t in n.targets)
    )
    seed = seed.replace(
        "prior=BACKGROUND_PIXEL*(ARGS.width*ARGS.height)", "prior=CONTROL_PRIOR"
    )
    source = SOURCE.read_text()
    assert sha(source.encode()) == PIN
    states = list(map(int, args.states.split(",")))
    assert 1 <= len(states) <= 3 and all(x in (0, 1) for x in states)
    code = source
    anchor = "  mxcsr_before=self._read_mxcsr()\n  self.gles.f['glDrawArrays'](*vals)"
    assert code.count(anchor) == 1
    code = code.replace(
        anchor,
        """
  incoming = C.create_string_buffer(ARGS.width * ARGS.height * 4)
  self.gles.f['glReadPixels'](0, 0, ARGS.width, ARGS.height, 0x1908, 0x1401, C.cast(incoming, C.c_void_p))
  if self._drain_gl_errors(): raise RuntimeError('pre-composite read failed')
  self.incoming_composite = {'sha256': hashlib.sha256(incoming.raw).hexdigest(), 'all_zero': not any(incoming.raw), 'matches_supplied_prior': incoming.raw == CONTROL_PRIOR, 'attachment': before['attachments'][0]['object_name']}
"""
        + anchor,
    )
    draw = " native_call('SPen::WaterColorBrushStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,matrix],instruction_limit=8000000)"
    assert code.count(draw) == 1
    sequence = " p.redraw_sequence=[]\n for requested_state in CONTROL_STATES:\n"
    sequence += "  native_call('SPen::PenDrawableRT::SetRedrawState(bool)',[rt,requested_state])\n"
    sequence += "  if p.get(rt+25,'B')[0]!=requested_state: raise RuntimeError('native SetRedrawState field join failed')\n"
    sequence += "\n".join(" " + line if line else line for line in seed.split("\n"))
    sequence += "\n " + draw + "\n"
    sequence += """
  pixels_now=C.create_string_buffer(ARGS.width*ARGS.height*4)
  p.gles.f['glReadPixels'](0,0,ARGS.width,ARGS.height,0x1908,0x1401,C.cast(pixels_now,C.c_void_p))
  if p._drain_gl_errors(): raise RuntimeError('post Draw readback failed')
  p.redraw_sequence.append({'redraw':bool(requested_state), 'native_state_byte':p.get(rt+25,'B')[0], 'verified_prior':p.output_prior_receipt, 'incoming_composite':p.incoming_composite, 'mask_hex':p.mask_pass_readback_receipt['pixels_raw'].hex(), 'final_hex':pixels_now.raw.hex()})
"""
    code = code.replace(draw, sequence)
    schema = "'schema':'sdocx-watercolor-rtv1-public-control-mesa-paint-v15-density-instance-mxcsr-readback'"
    assert code.count(schema) == 1
    code = code.replace(schema, schema + ",'redraw_sequence':p.redraw_sequence")
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
            "CONTROL_STATES": states,
            "CONTROL_PRIOR": backdrop(args.backdrop),
        },
    )
    raise SystemExit

receipt = {
    "schema": "sdocx-watercolor-redraw-state-v1",
    "frame": [256, 256],
    "coordinates": "Bottom-origin premultiplied RGBA8. Sparse records are little-endian u16 index followed by RGBA8. Final sparse base is supplied prior for redraw=true and transparent for redraw=false.",
    "capture": {
        "script": "tools/watercolor_host/redraw_capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "base_source_sha256": PIN,
        "seed_helper_sha256": HELPER_PIN,
    },
    "scope": "Public SetRedrawState controls and repeated native Draw on one RTV1; same unchanged AddPoint buffer submitted on every Draw. Prior uploaded and verified on actual native attachment before each Draw, with passive read immediately before composite. Host Mesa/glibc only, not SDK-deserialized saved-note or device evidence.",
    "pencil_backdrop": {
        "receipt_sha256": sha(PENCIL.read_bytes()),
        "case": "mixed_background",
        "rule": "Paste bottom-origin source [0,0,144,144] at destination [112,112] over [64,32,96,128]. Cropping and placement are supplied host context.",
    },
    "priors": {},
    "cases": {},
}
controls = [
    (repeat, states, "uniform")
    for repeat in [1, 32]
    for states in ["0,1,0", "1,0,1", "1,1"]
]
controls += [(repeat, "1", "pencil") for repeat in [1, 32]]
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-redraw-") as directory:
    directory = Path(directory)
    for repeat, states, kind in controls:
        name = f"{kind}_{repeat}_{states.replace(',', '')}"
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
                    "--repeat",
                    str(repeat),
                    "--states",
                    states,
                    "--backdrop",
                    kind,
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                env=os.environ,
                stdout=stdout,
                stderr=stderr,
                timeout=150,
            )
        if result.returncode:
            print((directory / "stderr").read_text()[-6000:])
            raise SystemExit(result.returncode)
        native = json.loads(output.read_text())
        assert native["capture_status"] == "captured_without_host_gl_errors"
        prior = backdrop(kind)
        receipt["priors"][kind] = {
            "sha256": sha(prior),
            "rgba_zlib_base64": base64.b64encode(zlib.compress(prior, 9)).decode(),
        }
        frames = []
        for frame in native["redraw_sequence"]:
            mask = bytes.fromhex(frame.pop("mask_hex"))
            final = bytes.fromhex(frame.pop("final_hex"))
            base = prior if frame["redraw"] else bytes(len(prior))
            frames.append(
                {
                    **frame,
                    "mask_sha256": sha(mask),
                    "mask_sparse_hex": sparse(mask, bytes(len(mask))).hex(),
                    "final_sha256": sha(final),
                    "final_sparse_hex": sparse(final, base).hex(),
                }
            )
        receipt["cases"][name] = {
            "repeat": repeat,
            "prior": kind,
            "stream_sha256": native["controlled_rt"]["vector_sha256"],
            "frames": frames,
        }
        if "host" not in receipt:
            receipt["host"] = {
                "gles": native["gles"],
                "libraries": native["source"],
                "source_inputs": native["source_capture"],
            }
        print(
            name,
            [
                (
                    f["redraw"],
                    f["incoming_composite"]["all_zero"],
                    f["incoming_composite"]["matches_supplied_prior"],
                )
                for f in frames
            ],
            flush=True,
        )
    target = ROOT / "conformance/watercolor-redraw-state-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
