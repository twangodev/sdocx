#!/usr/bin/env python3
"""Compare focused public direct-PDF captures with retained host-Mesa frames."""
import argparse
import base64
import hashlib
import json
import pathlib
import zlib

import pymupdf
import pypdfium2 as pdfium

assert pymupdf.VersionBind == '1.28.2'
parser = argparse.ArgumentParser()
parser.add_argument('output', type=pathlib.Path)
parser.add_argument('--case', choices=('zero-half',))
args = parser.parse_args()
expected_pdfium = ('5.9.0', '150.0.7869.0') if args.case else ('5.14.0', '156.0.8076.0')
assert (str(pdfium.PYPDFIUM_INFO), str(pdfium.PDFIUM_INFO)) == expected_pdfium
root = pathlib.Path(__file__).resolve().parents[2]
fixture = json.loads((root / 'conformance/defaultpen-scale-native.json').read_text())
out = args.output
for name, case in fixture['cases'].items():
    if (name == 'zero-half') != (args.case == 'zero-half'):
        continue
    scale = {'half': 0.5, '1': 1, '2': 2, '4': 4, '8': 8}[name.rsplit('-', 1)[1]]
    raw = zlib.decompress(base64.b64decode(case['rgba_zlib_base64']))
    assert hashlib.sha256(raw).hexdigest() == case['rgba_sha256']
    w, h = int(64 * scale), int(32 * scale)
    expected = bytearray()
    for y in range(h):
        for x in range(w):
            i = (y * case['width'] + x) * 4
            alpha = raw[i + 3]
            expected.extend((raw[i + c] * alpha + 252 * (255 - alpha) + 127) // 255 for c in range(3))
    scores = []
    for variant in ('baseline', 'profile'):
        path = out / f'{name}-{variant}.pdf'
        page = pymupdf.open(path)[0]
        pix = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3 * scale, 4 / 3 * scale), alpha=False)
        assert (pix.width, pix.height, pix.n) == (w, h, 3)
        mu = sum(abs(a - b) for a, b in zip(pix.samples, expected))
        image = pdfium.PdfDocument(path)[0].render(scale=4 / 3 * scale).to_pil().convert('RGB')
        assert image.size == (w, h)
        pdf = sum(abs(a - b) for a, b in zip(image.tobytes(), expected))
        scores.append((mu, pdf))
    print(name, scores, flush=True)
    assert all(after < before for before, after in zip(*scores)), (name, scores)
