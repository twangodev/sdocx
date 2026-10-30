"""Measure public Fountain raster outputs against retained native RGBA."""

import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

from PIL import Image
import pymupdf
import pypdfium2

ROOT = Path(__file__).resolve().parents[2]
NATIVE = ROOT / "conformance/fountain-v14-scaled-source-rgba-native.json"
DIRECTORY = ROOT / "scratch/fountain-v14-public/exports"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def pixels(row):
    data = zlib.decompress(base64.b64decode(row["rgba_zlib_base64"]))
    assert len(data) == 64 * 64 * 4 and sha(data) == row["sha256"]
    return b"".join(data[y * 256 : (y + 1) * 256] for y in reversed(range(64)))


def main():
    kind = sys.argv[1] if len(sys.argv) > 1 else "rust"
    assert kind in ("rust", "wasm")
    mode = sys.argv[2] if len(sys.argv) > 2 else "scaled-source"
    assert mode in ("scaled-source", "clip", "drawing")
    label = {"clip": "clipping", "drawing": "drawing-paint"}.get(mode, mode)
    native = (
        ROOT / f"conformance/fountain-v14-{label}-native.json"
        if mode != "scaled-source"
        else NATIVE
    )
    prefix = "wasm_" if kind == "wasm" else ""
    cases = json.loads(native.read_bytes())["cases"]
    results = {}
    for name, case in cases.items():
        expected = pixels(case["output"])
        assert (DIRECTORY / (prefix + name + ".rgba")).read_bytes() == expected
        if case["prior"] != "white":
            continue
        assert (
            Image.open(DIRECTORY / (prefix + name + ".png")).convert("RGBA").tobytes()
            == expected
        )
        routes = ["direct"] if kind == "wasm" else ["direct", "svg"]
        outcome = {"png_pixel_errors": 0}
        for route in routes:
            pdf = DIRECTORY / f"{prefix}{name}.{route}.pdf"
            with pymupdf.open(pdf) as document:
                image = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
                assert (image.width, image.height, image.n) == (64, 64, 4)
                assert bytes(image.samples) == expected, (name, route, "MuPDF")
            with pypdfium2.PdfDocument(str(pdf)) as document:
                page = document[0]
                bitmap = page.render(
                    scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0)
                )
                image = bitmap.to_pil().convert("RGBA")
                assert image.size == (64, 64)
                assert image.tobytes() == expected, (name, route, "PDFium")
                bitmap.close()
                page.close()
            outcome[route + "_mupdf_pixel_errors"] = 0
            outcome[route + "_pdfium_pixel_errors"] = 0
        results[name] = outcome
    counts = {"scaled-source": (36, 12), "clip": (216, 72), "drawing": (108, 36)}
    assert (len(cases), len(results)) == counts[mode]
    receipt = {
        "schema": "sdocx-fountain-v14-public-canvas-export-v1",
        "capture": {
            "script": "tools/fountain_v14/export_compare.py",
            "script_sha256": sha(Path(__file__).read_bytes()),
            "native_receipt_sha256": sha(native.read_bytes()),
        },
        "binding": kind,
        "native_canvas_frames": len(cases),
        "opaque_export_frames": len(results),
        "dpi": 72,
        "readers": {
            "mupdf": pymupdf.VersionBind,
            "pdfium": str(pypdfium2.PDFIUM_INFO),
            "svg": "Rust/resvg 0.47; WASM SVG equality checked separately",
        },
        "results": results,
        "scope": "All selected native real-source/context/color/prior controls match public canvas RGBA. Opaque frames match PNG and retained-scene direct PDF in both readers; Rust also checks SVG-page PDF. Source geometry is original saved input; AA, crop, RT color and host environment remain explicit context. Translucent PDF carriers, other strokes, full saved-page/native Drawing dispatch and device fidelity remain unverified.",
    }
    if mode == "drawing":
        receipt["scope"] = (
            "Public direct Fountain V14 canvas matches one connected persisted-object native Model/Drawing/GLES route, including individual stamp blending and ignored uniform color alpha. Center, clipped corner and native-culled outside, both origins/scales and three actual priors. All opaque PNG and PDF routes match in both readers; Rust also checks SVG-page PDF. Host context/queue/loader services, other saved objects/pages, translucent carriers and device appearance remain unverified."
        )
    if mode == "scaled-source":
        counterexample = json.loads(
            (DIRECTORY / "source-sampling-counterexample.json").read_bytes()
        )
        assert counterexample["canonical_24_stamp_channel_errors"] > 0
        assert counterexample["native_scaled_49_stamp_channel_errors"] == 0
        receipt["source_sampling_counterexample"] = counterexample
    output = (
        ROOT
        / f"conformance/fountain-v14-{label}-{'public' if kind == 'rust' else 'wasm'}-canvas-export.json"
    )
    output.write_text(json.dumps(receipt, separators=(",", ":")) + "\n")
    print(output, len(results), "opaque native-exact exports")


if __name__ == "__main__":
    main()
