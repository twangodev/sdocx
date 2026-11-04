"""Check SVG-derived PDF exports against the retained InkPen2 V1 host frames.

Run inkpen2_v1_pressure_and_diagonal_native_pixels with
SDOCX_V1_PRESSURE_PDF_CAPTURE set to a temporary directory, then pass that
directory to this script. Requires PyMuPDF, pypdfium2 and Pillow.
"""

import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
pdf_dir = Path(sys.argv[1])
assert pymupdf.VersionBind == "1.28.2"
assert str(pypdfium2.PDFIUM_INFO) == "150.0.7869.0"
native_path = root / "conformance/inkpen2-v1-pressure-paint-native.json"
receipt = json.loads(native_path.read_text())
results = {}
pdf_sha256 = {}
for name, capture in receipt["cases"].items():
    native = zlib.decompress(base64.b64decode(capture["rgba_zlib_base64"]))
    assert capture["output_size"] == [128, 96] and len(native) == 128 * 96 * 4
    assert hashlib.sha256(native).hexdigest() == capture["rgba_sha256"]
    errors = {"mupdf": {}, "pdfium": {}}
    for kind in ("candidate", "generic"):
        pdf_path = pdf_dir / f"v1-pressure-{name}-{kind}.pdf"
        pdf_sha256.setdefault(name, {})[kind] = hashlib.sha256(pdf_path.read_bytes()).hexdigest()
        with pymupdf.open(pdf_path) as document:
            pixels = document[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True)
            assert (pixels.width, pixels.height, pixels.n) == (128, 96, 4)
            assert len(pixels.samples) == len(native)
            errors["mupdf"][kind] = sum(abs(a - b) for a, b in zip(pixels.samples[3::4], native[3::4]))
        with pypdfium2.PdfDocument(str(pdf_path)) as document:
            bitmap = document[0].render(scale=4 / 3, rev_byteorder=True, fill_color=(0, 0, 0, 0))
            pixels = bitmap.to_pil().convert("RGBA")
            assert pixels.size == (128, 96)
            assert len(pixels.tobytes()) == len(native)
            errors["pdfium"][kind] = sum(abs(a - b) for a, b in zip(pixels.tobytes()[3::4], native[3::4]))
    for reader, values in errors.items():
        assert values["candidate"] < values["generic"], (name, reader, values)
    results[name] = errors
actual = {
    "scope": "Alpha-byte absolute-error sums against transparent host Mesa frames; synthetic controls only.",
    "readers": {"mupdf": pymupdf.VersionBind, "pdfium": str(pypdfium2.PDFIUM_INFO)},
    "native_receipt_sha256": hashlib.sha256(native_path.read_bytes()).hexdigest(),
    "pdf_sha256": pdf_sha256,
    "cases": results,
}
expected = json.loads((root / "conformance/inkpen2-v1-pressure-pdf-comparison.json").read_text())
assert actual == expected, "PDF comparison differs from the pinned receipt"
print(json.dumps(actual, indent=2))
