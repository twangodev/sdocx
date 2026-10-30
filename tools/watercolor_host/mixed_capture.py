"""Capture WaterColor continuation over an actual native Pencil2 pixel frame."""

import argparse
import base64
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
PENCIL = ROOT / "conformance/pencil2-v14-page-context-native.json"
parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--scale", type=int, choices=[1, 2], default=1)
parser.add_argument("--output", type=Path)
args = parser.parse_args()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def prior(scale):
    case = json.loads(PENCIL.read_text())["results"][f"colored_top_scale{scale}_white"]
    data = bytearray(bytes(case["background"]) * (192 * 192))
    for x, y, *rgba in case["pixels"]:
        data[(y * 192 + x) * 4 : (y * 192 + x + 1) * 4] = bytes(rgba)
    assert sha(data) == case["sha256"]
    return bytes(data)


if args.child:
    source = HELPER.read_text()
    assert sha(source.encode()) == PIN
    anchor = "    exec(\n"
    assert source.count(anchor) == 1
    source = source.replace(
        anchor,
        "    code=code.replace('prior=BACKGROUND_PIXEL*(ARGS.width*ARGS.height)','prior=PUBLIC_PRIOR')\n"
        + anchor,
    )
    source = source.replace(
        '"BACKGROUND_PIXEL": bytes([64, 32, 96, 128]),',
        '"BACKGROUND_PIXEL": bytes([64, 32, 96, 128]),"PUBLIC_PRIOR":PUBLIC_PRIOR,',
    )
    sys.argv = [
        str(HELPER),
        "--child",
        "--width",
        "192",
        "--height",
        "192",
        "--scale",
        str(args.scale),
        "--repeat",
        "1",
        "--output",
        str(args.output),
    ]
    exec(
        compile(source, str(HELPER), "exec"),
        {
            "__name__": "watercolor_mixed_capture",
            "__file__": str(HELPER),
            "PUBLIC_PRIOR": prior(args.scale),
        },
    )
    raise SystemExit
receipt = {
    "schema": "sdocx-watercolor-pencil2-continuation-native-v1",
    "frame": [192, 192],
    "capture": {
        "script": "tools/watercolor_host/mixed_capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "frame_helper_sha256": PIN,
        "pencil_receipt_sha256": sha(PENCIL.read_bytes()),
    },
    "scope": "Fresh native WaterColor RTV1 Draw with redraw=true over supplied verified native Pencil2 top-origin white-frame pixels at the same dimensions and scale. Original 15 source-owned WaterColor V1 dabs. This is a native pixel continuation, not both Drawing factories or a complete saved-note page in one native session.",
    "cases": {},
}
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-mixed-") as directory:
    directory = Path(directory)
    for scale in [1, 2]:
        output = directory / "native.json"
        with (
            (directory / "stdout").open("w") as stdout,
            (directory / "stderr").open("w") as stderr,
        ):
            process = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--child",
                    "--scale",
                    str(scale),
                    "--output",
                    str(output),
                ],
                cwd=ROOT,
                stdout=stdout,
                stderr=stderr,
                timeout=150,
            )
        if process.returncode:
            print((directory / "stderr").read_text()[-6000:])
            raise SystemExit(process.returncode)
        native = json.loads(output.read_text())
        assert native["capture_status"] == "captured_without_host_gl_errors"
        data = zlib.decompress(base64.b64decode(native["readback"]["rgba_zlib_base64"]))
        sparse = b"".join(
            struct.pack("<I", i // 4) + data[i : i + 4]
            for i in range(0, len(data), 4)
            if data[i : i + 4] != bytes([255] * 4)
        )
        receipt["cases"][str(scale)] = {
            "scale": scale,
            "prior_sha256": sha(prior(scale)),
            "final_sha256": sha(data),
            "final_sparse_hex": sparse.hex(),
            "verified_prior": native["verified_output_prior"],
        }
        if "host" not in receipt:
            receipt["host"] = {"gles": native["gles"], "libraries": native["source"]}
        print(scale, sha(data), flush=True)
        output.unlink()
    path = ROOT / "conformance/watercolor-pencil2-continuation-native.json"
    path.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(path, path.stat().st_size)
