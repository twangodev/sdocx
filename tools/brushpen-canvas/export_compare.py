"""Compare explicit BrushPen canvas PDF exports with the native nonwhite control."""

import hashlib
import json
from pathlib import Path
import sys
import zlib

import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
directory = Path(sys.argv[1])
source = root / "conformance/brushpen-second-native/nonwhite-frame.rgba.zlib"
bottom = zlib.decompress(source.read_bytes())
assert (
    hashlib.sha256(bottom).hexdigest()
    == "061b7b9e2124f690c14ee2067f04b607c4f0f09578249c47c8e8cc3b714e8f70"
)
expected = b"".join(
    bottom[y * 321 * 4 : (y + 1) * 321 * 4] for y in reversed(range(228))
)


def metrics(actual):
    assert len(actual) == len(expected)
    errors = [abs(a - b) for a, b in zip(actual, expected)]
    return {
        "different_channels": sum(e != 0 for e in errors),
        "maximum": max(errors),
        "absolute_sum": sum(errors),
    }


results = {}
for route in ("direct", "svg"):
    path = directory / f"{route}.pdf"
    with pymupdf.open(path) as document:
        pixmap = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
        assert (pixmap.width, pixmap.height, pixmap.n) == (321, 228, 4)
        mupdf = metrics(pixmap.samples)
    with pypdfium2.PdfDocument(path) as document:
        page = document[0]
        bitmap = page.render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
        image = bitmap.to_pil()
        assert image.size == (321, 228)
        pdfium = metrics(image.convert("RGBA").tobytes())
        bitmap.close()
        page.close()
    results[route] = {
        "pdf_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "mupdf": mupdf,
        "pdfium": pdfium,
    }
print(
    json.dumps(
        {
            "scope": "One explicit BrushPen host-Mesa canvas control on opaque nonwhite; no device or arbitrary input parity.",
            "native_bottom_rgba_sha256": hashlib.sha256(bottom).hexdigest(),
            "versions": {
                "PyMuPDF": pymupdf.VersionBind,
                "pypdfium2": str(pypdfium2.version.PYPDFIUM_INFO),
            },
            "cases": results,
        },
        indent=2,
    )
)
