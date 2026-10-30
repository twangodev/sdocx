#!/usr/bin/env python3
"""Compare source-rendered tool-1 PNG transport with retained Mesa controls."""
import base64
import json
import sys
import zlib
from pathlib import Path

import pymupdf
import pypdfium2

if not __debug__:
    raise SystemExit('Run without Python optimization; assertions verify the diagnostic.')

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/pen-capture/airbrush_v1_tool1_receipt.json').read_text())
assert pymupdf.VersionBind == '1.28.2'
assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
expected = {'reverse-1': (98, 0, 46), 'zigzag-1': (727, 0, 415)}
for name, target in expected.items():
    raw = zlib.decompress(base64.b64decode(receipt['cases'][name]['frame']['data_base64']))
    native = b''.join(reversed([raw[i:i + 1024] for i in range(0, len(raw), 1024)]))
    with pymupdf.open() as pdf:
        page = pdf.new_page(width=192, height=192)
        page.insert_image(page.rect, filename=str(Path(sys.argv[1]) / f'{name}.png'))
        encoded = pdf.tobytes()
    with pymupdf.open(stream=encoded, filetype='pdf') as pdf:
        pix = pdf[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True)
        assert (pix.width, pix.height, pix.n) == (256, 256, 4)
        mupdf = sum(abs(a - b) for a, b in zip(pix.samples, native))
    with pypdfium2.PdfDocument(encoded) as pdf:
        alpha = pdf[0].render(scale=4 / 3, rev_byteorder=True, fill_color=(0, 0, 0, 0)).to_pil().convert('RGBA')
        white = pdf[0].render(scale=4 / 3, rev_byteorder=True, fill_color=(255, 255, 255, 255)).to_pil().convert('RGB')
        assert alpha.size == white.size == (256, 256)
        native_white = bytes(min(255, native[i + c] + 255 - native[i + 3]) for i in range(0, len(native), 4) for c in range(3))
        actual = (mupdf, sum(abs(a - b) for a, b in zip(alpha.tobytes()[3::4], native[3::4])), sum(abs(a - b) for a, b in zip(white.tobytes(), native_white)))
    assert actual == target, (name, actual, target)
    print(f'{name}: MuPDF RGBA, PDFium alpha and white RGB errors: {actual}')
