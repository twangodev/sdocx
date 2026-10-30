#!/usr/bin/env python3
"""Compare public direct PDF outputs to the retained synthetic native frames."""
import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

import pymupdf
import pypdfium2

if not __debug__:
    raise SystemExit('Python optimization disables validation assertions; run without -O.')

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/pen-capture/airbrush_v1_tool1_v204_receipt.json').read_text())
directory = Path(sys.argv[1])
expected = json.loads((root / 'tools/pen-capture/airbrush_v1_tool1_v204_pdf_receipt.json').read_text())
assert pymupdf.VersionBind == expected['pymupdf']
assert str(pypdfium2.PDFIUM_INFO) == expected['pdfium']
results = {}
for name, case in receipt['cases'].items():
    raw = zlib.decompress(base64.b64decode(case['frame']['data_base64']))
    native = b''.join(reversed([raw[i:i + 1024] for i in range(0, len(raw), 1024)]))
    results[name] = {}
    for variant in ('candidate', 'generic'):
        path = directory / f'{name}-{variant}.pdf'
        with pymupdf.open(path) as doc:
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(96 / 72, 96 / 72), alpha=True)
            assert (pix.width, pix.height, pix.n) == (256, 256, 4)
            mupdf = sum(abs(a-b) for a,b in zip(pix.samples, native))
        with pypdfium2.PdfDocument(str(path)) as doc:
            page = doc[0]
            alpha = page.render(scale=4/3, rev_byteorder=True, fill_color=(0,0,0,0)).to_pil().convert('RGBA')
            white = page.render(scale=4/3, rev_byteorder=True, fill_color=(255,255,255,255)).to_pil().convert('RGB')
            native_white = bytes(min(255, native[i+c] + 255-native[i+3]) for i in range(0, len(native), 4) for c in range(3))
            pdfium = [sum(abs(a-b) for a,b in zip(alpha.tobytes()[3::4], native[3::4])), sum(abs(a-b) for a,b in zip(white.tobytes(), native_white))]
        results[name][variant] = {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'mupdf_rgba_error': mupdf, 'pdfium_alpha_white_rgb_error': pdfium}
assert results == expected['results'], 'PDF hashes or measured errors differ from the receipt'
print('PDF hashes and both reader metrics match the receipt.')
