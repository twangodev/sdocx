"""Expose the known dense 2x mask residual with opaque saved color alpha."""

import base64
import hashlib
import json
from pathlib import Path
import struct
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "tools/watercolor_host/frame_capture.py"
PIN = "d87055488b895e4ee6dce35ca06f615f69c04a2d2f1cf28d3b7ad9786618ffd8"
source = HELPER.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == PIN
anchor = "    exec(\n"
assert source.count(anchor) == 1
source = source.replace(
    anchor, '    sys.argv.extend(["--argb","0xff44aaee"])\n' + anchor
)
with tempfile.TemporaryDirectory(prefix="sdocx-watercolor-opacity-") as directory:
    report = Path(directory) / "native.json"
    sys.argv = [
        str(HELPER),
        "--child",
        "--width",
        "768",
        "--height",
        "640",
        "--scale",
        "2",
        "--repeat",
        "32",
        "--output",
        str(report),
    ]
    try:
        exec(
            compile(source, str(HELPER), "exec"),
            {"__name__": "watercolor_opacity_probe", "__file__": str(HELPER)},
        )
    except SystemExit as done:
        if done.code not in (None, 0):
            raise
    native = json.loads(report.read_text())
    assert native["capture_status"] == "captured_without_host_gl_errors"
    baseline = json.loads(
        (ROOT / "conformance/watercolor-redraw-frames-native.json").read_text()
    )["cases"]["768x640_2_32"]
    assert (
        native["native_mask_pass_readback"]["pixels_sha256"] == baseline["mask_sha256"]
    )
    pixels = zlib.decompress(base64.b64decode(native["readback"]["rgba_zlib_base64"]))
    prior = bytes([64, 32, 96, 128])
    sparse = b"".join(
        struct.pack("<I", i // 4) + pixels[i : i + 4]
        for i in range(0, len(pixels), 4)
        if pixels[i : i + 4] != prior
    )
    receipt = {
        "schema": "sdocx-watercolor-redraw-opacity-control-v1",
        "frame": [768, 640],
        "scale": 2,
        "repeat": 32,
        "argb": 0xFF44AAEE,
        "background": list(prior),
        "capture": {
            "script": "tools/watercolor_host/opacity_probe.py",
            "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "frame_helper_sha256": PIN,
        },
        "scope": "Fresh native RTV1, original 480 AddPoint dabs, public SetPenData with opaque alpha, SetRedrawState(true), verified supplied prior, same top-origin 2x matrix. Host Mesa/glibc only, not a saved-note or device profile.",
        "projection_matrices": baseline["projection_matrices"],
        "mask_sha256": baseline["mask_sha256"],
        "final_sha256": native["readback"]["sha256"],
        "final_sparse_hex": sparse.hex(),
        "verified_prior": native["verified_output_prior"],
        "source_libraries": native["source"],
        "gles": native["gles"],
    }
    target = ROOT / "conformance/watercolor-redraw-opacity-native.json"
    target.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(target, target.stat().st_size)
