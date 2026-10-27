#!/usr/bin/env python3
"""Compare PDFs captured by saved_v1_public_short_stroke_matches_native_frame.

Run from the repository root (requires the project's PyMuPDF environment):

    CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
    CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
    SDOCX_AIRBRUSH_PDF_CAPTURE_DIR=/tmp cargo test -p sdocx --features pdf \
    --lib saved_v1_public_short_stroke_matches_native_frame
    .venv/bin/python conformance/compare_airbrush_v1_pdf.py /tmp
"""
import hashlib
import json
import pathlib
import sys

import pymupdf

SIZE = 256
# Native OpenGL bottom-up pixels; public SVG matches the vertically flipped frame.
PIXELS = [(127,126,[0,1,1,1]),(128,126,[6,16,22,23]),(129,126,[0,1,1,1]),
          (127,127,[6,14,20,22]),(128,127,[14,34,48,52]),(129,127,[3,8,11,12]),
          (127,128,[0,1,1,1]),(128,128,[4,9,13,14]),(129,128,[0,1,1,1])]
frame = bytearray(SIZE * SIZE * 4)
for x, y, rgba in PIXELS:
    index = ((SIZE - 1 - y) * SIZE + x) * 4
    frame[index:index + 4] = bytes(rgba)

def digest(data):
    return hashlib.sha256(data).hexdigest()

def compare(path, alpha):
    doc = pymupdf.open(path)
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(96 / 72, 96 / 72), alpha=alpha)
    assert (pix.width, pix.height, pix.n) == (SIZE, SIZE, 4 if alpha else 3)
    expected = bytes(frame) if alpha else bytes(
        min(255, frame[i + c] + 255 - frame[i + 3])
        for i in range(0, len(frame), 4) for c in range(3)
    )
    actual, n = pix.samples, pix.n
    differences = [abs(a - b) for a, b in zip(actual, expected)]
    changed = []
    for i in range(0, len(actual), n):
        if actual[i:i+n] != expected[i:i+n]:
            pixel = i // n
            changed.append([pixel % SIZE, pixel // SIZE, list(actual[i:i+n]), list(expected[i:i+n])])
    return {"raster_sha256": digest(actual), "expected_sha256": digest(expected),
            "different_pixels": len(changed), "max_channel_error": max(differences),
            "sum_absolute_channel_error": sum(differences), "differences_xy_actual_expected": changed}

directory = pathlib.Path(sys.argv[1])
result = {"schema": "sdocx.airbrush-v1-pdf-comparison.v1", "pymupdf_version": pymupdf.VersionBind,
          "scope": "Synthetic host Mesa frame; PDF rasterized at 96 dpi on transparent and white backgrounds. Coordinates top-left. Transparent channels are premultiplied. White reference is integer source-over of native RGBA. Not device or cross-viewer parity.",
          "native_receipt": "airbrush-short-v1-v2-native.json", "width": SIZE, "height": SIZE,
          "pdfs": {}}
for name in ("saved-v1.pdf", "generic.pdf"):
    path = directory / name
    result["pdfs"][name] = {"sha256": digest(path.read_bytes()),
                             "transparent_rgba": compare(path, True),
                             "white_rgb": compare(path, False)}
print(json.dumps(result, indent=2))
