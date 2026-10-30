"""Measure resolved ColoredPencil canvas transport through two PDF readers."""

import base64
import hashlib
import json
from pathlib import Path
import struct
import sys
import zlib

from PIL import Image
import pymupdf
import pypdfium2

ROOT = Path(__file__).resolve().parents[2]
NATIVE = ROOT / "conformance/colored-pencil-v4-canvas-native.json"
DIRECTORY = ROOT / "scratch/colored-pencil-public/exports"
kind = sys.argv[1] if len(sys.argv) > 1 else "rust"
assert kind in ("rust", "wasm")
mode = sys.argv[2] if len(sys.argv) > 2 else "base"
assert mode in ("base", "context", "depth", "display", "drawing", "mixed")
context_mode = mode == "context"
depth_mode = mode == "depth"
if context_mode:
    NATIVE = ROOT / "conformance/colored-pencil-v4-context-paint-native.json"
if depth_mode:
    NATIVE = ROOT / "conformance/colored-pencil-v4-depth-native.json"
if mode == "display":
    NATIVE = ROOT / "conformance/colored-pencil-v4-display-depth-native.json"
if mode == "drawing":
    NATIVE = ROOT / "conformance/colored-pencil-v4-drawing-lifecycle-native.json"
if mode == "mixed":
    NATIVE = ROOT / "conformance/colored-pencil-v4-pencil2-drawing-native.json"
mode_prefix = "" if mode == "base" else mode + "_"
prefix = ("wasm_" if kind == "wasm" else "") + mode_prefix


def sha(data):
    return hashlib.sha256(data).hexdigest()


def metrics(actual, expected):
    assert len(actual) == len(expected)
    errors = [abs(a - b) for a, b in zip(actual, expected)]
    return {
        "different_channels": sum(e != 0 for e in errors),
        "absolute_sum": sum(errors),
        "maximum": max(errors),
    }


def readers(path, size):
    with pymupdf.open(path) as document:
        image = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
        assert (image.width, image.height, image.n) == (*size, 4)
        mupdf = bytes(image.samples)
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
cases = json.loads(NATIVE.read_text())["cases"]
for name, case in cases.items():
    w, h = case["frame"]
    image = bytearray(bytes(case["background"]) * (w * h))
    if "rgba_zlib_base64" in case:
        image = zlib.decompress(base64.b64decode(case["rgba_zlib_base64"]))
        assert len(image) == w * h * 4
    else:
        sparse = zlib.decompress(base64.b64decode(case["sparse_zlib_base64"]))
        for offset in range(0, len(sparse), 8):
            index = struct.unpack_from("<I", sparse, offset)[0]
            image[index * 4 : index * 4 + 4] = sparse[offset + 4 : offset + 8]
    assert sha(image) == case["frame_sha256"]
    expected = b"".join(image[y * w * 4 : (y + 1) * w * 4] for y in reversed(range(h)))
    actual = (DIRECTORY / (prefix + name + ".rgba")).read_bytes()
    assert actual == expected, f"{name}: native canvas differs"
    if case["background"] != [255] * 4:
        continue
    outcomes = {
        "png": metrics(
            Image.open(DIRECTORY / (prefix + name + ".png")).convert("RGBA").tobytes(),
            expected,
        )
    }
    routes = [("direct_pdf", ".direct.pdf")]
    if kind == "rust":
        routes.append(("svg_pdf", ".svg.pdf"))
    for route, suffix in routes:
        mupdf, pdfium = readers(DIRECTORY / (prefix + name + suffix), [w, h])
        outcomes[route + "_mupdf"] = metrics(mupdf, expected)
        outcomes[route + "_pdfium"] = metrics(pdfium, expected)
    assert all(
        o == {"different_channels": 0, "absolute_sum": 0, "maximum": 0}
        for o in outcomes.values()
    ), (name, outcomes)
    results[name] = outcomes
assert (
    len(results)
    == {
        "base": 36,
        "context": 100,
        "depth": 120,
        "display": 32,
        "drawing": 32,
        "mixed": 12,
    }[mode]
)
receipt = {
    "schema": "sdocx-colored-pencil-public-canvas-export-v1",
    "capture": {
        "script": "tools/colored_pencil/export_compare.py",
        "script_sha256": sha(Path(__file__).read_bytes()),
        "native_receipt_sha256": sha(NATIVE.read_bytes()),
    },
    "binding": kind,
    "canvas_native_frames": len(cases),
    "opaque_export_frames": len(results),
    "dpi": 72,
    "readers": {
        "mupdf": pymupdf.VersionBind,
        "pdfium": str(pypdfium2.PDFIUM_INFO),
        "svg": "Rust/resvg 0.47; WASM SVG equality checked separately",
    },
    "results": results,
    "scope": f"All {len(cases)} native source/context/backdrop controls match canvas RGBA. The {len(results)} resolved opaque-white frames match PNG and retained-scene PDF in MuPDF/PDFium; Rust also compares SVG-page PDF. Intrinsic destination pixels, source origin top/bottom and projection 1x/2x. Translucent carriers, browser UI, default document dispatch, real saved-note context and device fidelity remain separate.",
}
output = (
    ROOT
    / f"conformance/colored-pencil-{mode_prefix.replace('_', '-')}{'wasm' if kind == 'wasm' else 'public'}-canvas-export.json"
)
if mode == "display":
    receipt["capture"]["scale_producer_receipt_sha256"] = sha(
        (
            ROOT / "conformance/colored-pencil-v4-background-scale-native.json"
        ).read_bytes()
    )
    counterexample = json.loads(
        (DIRECTORY / "display-counterexample.json").read_bytes()
    )
    assert counterexample["different_channels"] > 0
    assert counterexample["derived_context_different_channels"] == 0
    counterexample["native_frame_sha256"] = cases[counterexample["case"]][
        "frame_sha256"
    ]
    receipt["fixed_scale_counterexample"] = counterexample
if mode in ("drawing", "mixed"):
    counterexample = json.loads(
        (DIRECTORY / f"{mode}-counterexample.json").read_bytes()
    )
    assert counterexample["fresh_null_different_channels"] > 0
    assert counterexample["retained_depth_different_channels"] == 0
    receipt["fresh_null_counterexample"] = counterexample
output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
print(output, len(results), "opaque native-exact exports")
