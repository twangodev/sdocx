"""Capture fresh GLV1/GLV2 source controls for public canvas/export validation."""

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
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
SOURCE_FIXTURE = ROOT / "conformance/watercolor-native.json"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
parser = argparse.ArgumentParser()
parser.add_argument("--child", action="store_true")
parser.add_argument("--version", type=int, choices=[1, 2], default=1)
parser.add_argument("--scale", type=int, choices=[1, 2], default=1)
parser.add_argument("--argb", type=lambda s: int(s, 0), default=0x8044AAEE)
parser.add_argument("--output", type=Path)
parser.add_argument("--clear", default="00000000")
args = parser.parse_args()


def sha(data):
    return hashlib.sha256(data).hexdigest()


if args.child:
    source = HELPER.read_text()
    assert sha(source.encode()) == PIN
    anchor = "    exec(\n"
    assert source.count(anchor) == 1
    source = source.replace(
        anchor,
        "    sys.argv[sys.argv.index('--case')+1]='watercolor'+str(PUBLIC_VERSION)\n    sys.argv.extend(['--argb',hex(PUBLIC_ARGB)])\n"
        + anchor,
    )
    source = source.replace("bytes([64, 32, 96, 128])", "PUBLIC_CLEAR")
    sys.argv = [
        str(HELPER),
        "--child",
        "--width",
        str(384 * args.scale),
        "--height",
        str(320 * args.scale),
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
            "__name__": "watercolor_public_capture",
            "__file__": str(HELPER),
            "PUBLIC_VERSION": args.version,
            "PUBLIC_ARGB": args.argb,
            "PUBLIC_CLEAR": bytes.fromhex(args.clear),
        },
    )
    raise SystemExit

receipt = {
    "schema": "sdocx-watercolor-public-source-native-v1",
    "coordinates": "Bottom-origin premultiplied RGBA8; sparse u32 index + RGBA8 relative to the supplied premultiplied prior.",
    "capture": {
        "script": "tools/watercolor_host/translucent_capture.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "frame_helper_sha256": PIN,
        "public_source_fixture": str(SOURCE_FIXTURE.relative_to(ROOT)),
        "public_source_fixture_sha256": sha(SOURCE_FIXTURE.read_bytes()),
    },
    "scope": "Native source-owned GLV1/null or GLV2/empty AddPoint inputs replayed once through fresh RTV1 Init/CreatePenCanvas/SetPenData/Update/SetRedrawState(true)/Draw. Public supplied frame/matrix and verified transparent or translucent prior. Explicit glibc/Mesa host boundary; not a real saved-note or device admission claim.",
    "cases": {},
}
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-public-") as directory:
    directory = Path(directory)
    for version in [1, 2]:
        for scale in [1, 2]:
            for argb, clear in [(0x8044AAEE, "00000000"), (0x8044AAEE, "40206080")]:
                name = f"v{version}_scale{scale}_{argb:08x}_{clear}"
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
                            "--version",
                            str(version),
                            "--scale",
                            str(scale),
                            "--argb",
                            hex(argb),
                            "--clear",
                            clear,
                            "--output",
                            str(output),
                        ],
                        cwd=ROOT,
                        env=os.environ,
                        stdout=stdout,
                        stderr=stderr,
                        timeout=150,
                    )
                if process.returncode:
                    print((directory / "stderr").read_text()[-6000:])
                    raise SystemExit(process.returncode)
                native = json.loads(output.read_text())
                assert native["capture_status"] == "captured_without_host_gl_errors"
                source = native["source_capture"]
                fixture_input = json.loads(SOURCE_FIXTURE.read_text())["cases"][0][
                    "input"
                ]
                native_input = source["input_object_stroke"]

                def bits(value):
                    return struct.pack("<f", value).hex()

                assert native_input["points_f32_bits"] == [
                    [bits(v) for v in point] for point in fixture_input["points_xy"]
                ]
                assert native_input["pressure_f32_bits"] == [
                    bits(v) for v in fixture_input["pressure"]
                ]
                assert native_input["timestamps_i32"] == fixture_input["timestamps"]
                assert native_input["pen_size_f32_bits"] == bits(
                    fixture_input["pen_size"]
                )
                assert native_input["color_argb"] == fixture_input["color_argb"]
                pixels = zlib.decompress(
                    base64.b64decode(native["readback"]["rgba_zlib_base64"])
                )
                sparse = b"".join(
                    struct.pack("<I", i // 4) + pixels[i : i + 4]
                    for i in range(0, len(pixels), 4)
                    if pixels[i : i + 4] != bytes.fromhex(clear)
                )
                receipt["cases"][name] = {
                    "source_capture": {
                        "path": source["path"],
                        "sha256": source["sha256"],
                        "input_object_stroke": native_input,
                    },
                    "version": version,
                    "scale": scale,
                    "frame": [384 * scale, 320 * scale],
                    "argb": argb,
                    "background": list(bytes.fromhex(clear)),
                    "stream_sha256": native["controlled_rt"]["vector_sha256"],
                    "mask_sha256": native["native_mask_pass_readback"]["pixels_sha256"],
                    "final_sha256": sha(pixels),
                    "final_sparse_hex": sparse.hex(),
                    "verified_prior": native["verified_output_prior"],
                }
                if "host" not in receipt:
                    receipt["host"] = {
                        "gles": native["gles"],
                        "libraries": native["source"],
                    }
                print(name, sha(pixels), flush=True)
    path = ROOT / "conformance/watercolor-translucent-source-native.json"
    path.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(path, path.stat().st_size)
