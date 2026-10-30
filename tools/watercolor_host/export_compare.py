"""Measure public canvas PNG, SVG-PDF and retained-scene PDF against native RGBA."""

import hashlib
import json
from pathlib import Path
import struct
import sys

import pymupdf
import pypdfium2
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = ROOT / "scratch/watercolor-public/exports"
NATIVE = ROOT / "conformance/watercolor-public-source-native.json"
kind = sys.argv[1] if len(sys.argv) > 1 else "rust"
assert kind == "rust"
prefix = ""
fixture = json.loads(NATIVE.read_text())


def metrics(actual, expected):
    assert len(actual) == len(expected)
    errors = [abs(a - b) for a, b in zip(actual, expected)]
    return {
        "different_channels": sum(e != 0 for e in errors),
        "maximum": max(errors),
        "absolute_sum": sum(errors),
    }


def pdf_pixels(path, size):
    with pymupdf.open(path) as document:
        pixmap = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
        assert (pixmap.width, pixmap.height, pixmap.n) == (*size, 4)
        mupdf = bytes(pixmap.samples)
    with pypdfium2.PdfDocument(str(path)) as document:
        page = document[0]
        bitmap = page.render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
        image = bitmap.to_pil()
        assert image.size == tuple(size)
        pdfium = image.convert("RGBA").tobytes()
        bitmap.close()
        page.close()
    return mupdf, pdfium


results = {}
for name, case in fixture["cases"].items():
    w, h = case["frame"]
    bottom = bytearray(bytes(case["background"]) * (w * h))
    packed = bytes.fromhex(case["final_sparse_hex"])
    for offset in range(0, len(packed), 8):
        index = struct.unpack_from("<I", packed, offset)[0]
        bottom[index * 4 : index * 4 + 4] = packed[offset + 4 : offset + 8]
    assert hashlib.sha256(bottom).hexdigest() == case["final_sha256"]
    expected = b"".join(bottom[y * w * 4 : (y + 1) * w * 4] for y in reversed(range(h)))
    readers = {
        "canvas": metrics(
            (DIRECTORY / (prefix + name + ".rgba")).read_bytes(), expected
        ),
        "png": metrics(
            Image.open(DIRECTORY / (prefix + name + ".png")).convert("RGBA").tobytes(),
            expected,
        ),
    }
    for route, suffix in (
        [("svg_pdf", ".pdf"), ("direct_pdf", ".direct.pdf")]
    ):
        mupdf, pdfium = pdf_pixels(DIRECTORY / (prefix + name + suffix), [w, h])
        readers[route + "_mupdf"] = metrics(mupdf, expected)
        readers[route + "_pdfium"] = metrics(pdfium, expected)
    results[name] = readers
    print(name, readers, flush=True)
receipt = {
    "schema": "sdocx-watercolor-public-canvas-export-v1",
    "capture": {
        "script": "tools/watercolor_host/export_compare.py",
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "native_receipt_sha256": hashlib.sha256(NATIVE.read_bytes()).hexdigest(),
    },
    "readers": {
        "mupdf": pymupdf.VersionBind,
        "pdfium": str(pypdfium2.PDFIUM_INFO),
        "svg": "Rust integration test with resvg 0.47",
    },
    "dpi": 72,
    "binding": kind,
    "api": "Public Stroke + explicit WaterColorHostRtv1 -> InkRasterCanvas -> PNG/SVG plus "
    + "SVG-page and retained-scene PDF APIs",
    "results": results,
    "scope": "Eight controlled source profiles, GLV1/GLV2, 1x/2x projection and two saved alpha values. All export readers must preserve canvas pixels exactly. SVG/PDF retain intrinsic destination pixel dimensions. Not normal document dispatch, saved-note page layout, Android GPU or device fidelity.",
}
path = ROOT / "conformance/watercolor-public-canvas-export.json"
path.write_text(json.dumps(receipt, indent=2) + "\n")
for name, readers in results.items():
    baseline = readers["canvas"]
    expected = {"different_channels": 0, "maximum": 0, "absolute_sum": 0}
    assert baseline == expected, f"Unexpected native raster difference: {name}"
    assert all(m == baseline for m in readers.values()), (
        f"Export transport differs from the canvas: {name}"
    )
