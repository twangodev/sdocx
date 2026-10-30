"""Capture redraw=true WaterColor at non-square destination resolutions."""

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


def sha(data):
    return hashlib.sha256(data).hexdigest()


def sparse(raw, base):
    return b"".join(
        struct.pack("<I", i // 4) + raw[i : i + 4]
        for i in range(0, len(raw), 4)
        if raw[i : i + 4] != base
    )


parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--width", type=int, default=384)
parser.add_argument("--height", type=int, default=320)
parser.add_argument("--scale", type=int, choices=[1, 2], default=1)
parser.add_argument("--repeat", type=int, choices=[1, 32], default=1)
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
    source = SOURCE.read_text()
    assert sha(source.encode()) == PIN
    code = source.replace(
        "if ARGS.width != 256 or ARGS.height != 256:",
        "if not (1 <= ARGS.width <= 1920 and 1 <= ARGS.height <= 1080):",
    )
    matrix = (
        "matrix=p.alloc(64); p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,128,128,0,1)"
    )
    assert code.count(matrix) == 1
    code = code.replace(
        matrix, "matrix=p.alloc(64); p.put(matrix,'16f',*CONTROL_MATRIX)"
    )
    draw = " native_call('SPen::WaterColorBrushStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,matrix],instruction_limit=8000000)"
    assert code.count(draw) == 1
    code = code.replace(
        draw,
        " native_call('SPen::PenDrawableRT::SetRedrawState(bool)',[rt,1])\n"
        + seed
        + draw,
    )
    schema = "'schema':'sdocx-watercolor-rtv1-public-control-mesa-paint-v15-density-instance-mxcsr-readback'"
    code = code.replace(
        schema, schema + ",'verified_output_prior':p.output_prior_receipt"
    )
    sys.argv = [
        str(SOURCE),
        "--write-fixture",
        "--case",
        "watercolor1",
        "--repeat-count",
        str(args.repeat),
        "--width",
        str(args.width),
        "--height",
        str(args.height),
        "--fixture",
        str(args.output),
    ]
    matrix = [
        args.scale,
        0,
        0,
        0,
        0,
        -args.scale,
        0,
        0,
        0,
        0,
        1,
        0,
        0,
        args.height,
        0,
        1,
    ]
    exec(
        compile(code, str(SOURCE), "exec"),
        {
            "__name__": "__main__",
            "__file__": str(SOURCE),
            "CONTROL_MATRIX": matrix,
            "BACKGROUND_PIXEL": bytes([64, 32, 96, 128]),
        },
    )
    raise SystemExit

receipt = {
    "schema": "sdocx-watercolor-redraw-frames-v1",
    "coordinates": "Bottom-origin premultiplied RGBA8. Sparse u32 index + RGBA8; masks relative to zero, final relative to background.",
    "capture": {
        "script": "tools/watercolor_host/frame_capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "base_source_sha256": PIN,
        "seed_helper_sha256": HELPER_PIN,
    },
    "scope": "Fresh native RTV1 public AddPoint/Update/SetRedrawState(true)/Draw with supplied non-square dimensions and 1x/2x top-origin matrix. Verified output prior; host Mesa/glibc, not a saved-note or device profile.",
    "cases": {},
}
controls = [
    (w, h, scale, repeat)
    for w, h, scale in [(384, 320, 1), (768, 640, 2), (1920, 1080, 1)]
    for repeat in [1, 32]
]
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-frames-") as directory:
    directory = Path(directory)
    for w, h, scale, repeat in controls:
        name = f"{w}x{h}_{scale}_{repeat}"
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
                    "--width",
                    str(w),
                    "--height",
                    str(h),
                    "--scale",
                    str(scale),
                    "--repeat",
                    str(repeat),
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                env=os.environ,
                stdout=stdout,
                stderr=stderr,
                timeout=180,
            )
        if result.returncode:
            print((directory / "stderr").read_text()[-6000:])
            raise SystemExit(result.returncode)
        native = json.loads(output.read_text())
        assert native["capture_status"] == "captured_without_host_gl_errors"
        mask = bytes.fromhex(native["native_mask_pass_readback"]["pixels_raw"]["hex"])
        final = zlib.decompress(
            base64.b64decode(native["readback"]["rgba_zlib_base64"])
        )
        matrices = [
            row["values"]
            for row in native["gl_calls"]
            if row["call"] == "glUniformMatrix4fv"
        ]
        receipt["cases"][name] = {
            "frame": [w, h],
            "scale": scale,
            "repeat": repeat,
            "background": [64, 32, 96, 128],
            "projection_matrices": matrices,
            "mask_sha256": sha(mask),
            "mask_sparse_hex": sparse(mask, bytes(4)).hex(),
            "final_sha256": sha(final),
            "final_sparse_hex": sparse(final, bytes([64, 32, 96, 128])).hex(),
            "verified_prior": native["verified_output_prior"],
        }
        if "host" not in receipt:
            receipt["host"] = {
                "gles": native["gles"],
                "libraries": native["source"],
                "source_inputs": native["source_capture"],
            }
        print(name, sha(final), flush=True)
    target = ROOT / "conformance/watercolor-redraw-frames-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
