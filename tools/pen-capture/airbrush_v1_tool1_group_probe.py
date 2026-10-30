#!/usr/bin/env python3
"""Syntactic isolation interventions on two hash-pinned candidate PDFs.

Usage: PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/pen-capture/airbrush_v1_tool1_group_probe.py PDF_DIRECTORY
Generate the pinned candidate PDFs using airbrush_v1_tool1_README.md in this
directory. Requires PyMuPDF 1.28.2, PDFium 150.0.7869.0 via pypdfium2,
and Pillow 12.1.1. The 2x reader image is box-reduced per channel against
the 1x native frame;
there is no independent 2x native capture. Group labels describe these pinned
streams only and do not establish general PDF semantics or a safe lowering.
"""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

import PIL
from PIL import Image
import pymupdf
import pypdfium2

if not __debug__:
    raise RuntimeError('This probe requires assertions; do not use Python -O')
assert pymupdf.VersionBind == '1.28.2'
assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
assert PIL.__version__ == '12.1.1'
assert len(sys.argv) == 2
receipt_path = Path(__file__).resolve().parent / 'airbrush_v1_tool1_receipt.json'
assert hashlib.sha256(receipt_path.read_bytes()).hexdigest() == '4802a26da96ddf3e2f9a6656aff5becae7dea7ab8ac19bfdc03585dbd308699b'
receipt = json.loads(receipt_path.read_bytes())
pins = {
    'zigzag-1': {
        'image': (51, 'c198839201b7babf65586d8761f02c2889308914f6c28280e8551c75aab8137c'),
        'isolated_form': (108, 'f58c3ef33c199990843c059d75b2897cd5560ca20ba469966e5bc1f3448b1dbf'),
        'shade': (162, '8282fb85c5892c5dd692c6230b82f7b94cf2f6794e5e680ff98f76ea51f07b5c'),
        'pattern': (162, '6e8721a78409e41f3440f6034f7b386a55df344cdfe81fe88ddf9656ad11a8df'),
        'default_form': (159, 'fe58b44c81e84442a8d515bdd42acdf2b03bf37cd5bff1c5be9a62d6f885583d'),
    },
    'reverse-1': {
        'image': (96, '1644a51fe48fae1872659d096c8c0a54d1d099d301d5256aac8e25e346457c8d'),
        'isolated_form': (212, 'd4a5923421eeb8dd3f8879a6d7354f3874c69dcda9d1424eb952412eea775d03'),
        'shade': (318, 'bf7359c66419bcf87f88811c0249147d1c3e2ca05f0480525146674cb2983519'),
        'pattern': (318, '18e7ba9b4674d8d1dc833a870cf2326583d05f21bb14f509c0ba1154922f6515'),
        'default_form': (308, 'b6b6525ede0f1b94e3dfda70e059488d3a86829bcb5276977c6335a7d0d7872a'),
    },
}
results = {}
for name, expected in pins.items():
    case = receipt['cases'][name]
    path = Path(sys.argv[1]) / f'{name}-candidate.pdf'
    assert hashlib.sha256(path.read_bytes()).hexdigest() == receipt['pdf_sha256'][path.name]
    raw = zlib.decompress(base64.b64decode(case['frame']['data_base64']))
    assert hashlib.sha256(raw).hexdigest() == case['frame']['sha256']
    assert len(raw) == 256 * 256 * 4
    native = b''.join(reversed([raw[i:i + 1024] for i in range(0, len(raw), 1024)]))
    results[name] = {}
    for mode in ['baseline', *expected]:
        doc = pymupdf.open(path)
        groups = {key: [] for key in expected}
        for xref in range(1, doc.xref_length()):
            if not doc.xref_is_stream(xref) or doc.xref_get_key(xref, 'Group/S') != ('name', '/Transparency'):
                continue
            stream = doc.xref_stream(xref)
            isolated = doc.xref_get_key(xref, 'Group/I') == ('bool', 'true')
            if isolated:
                role = 'image' if b' cm/x0 Do' in stream else 'isolated_form'
            else:
                role = 'shade' if b' sh' in stream else 'pattern' if b'/Pattern' in stream else 'default_form'
            groups[role].append((xref, hashlib.sha256(stream).hexdigest()))
        for role, items in groups.items():
            count, digest = expected[role]
            assert len(items) == count and hashlib.sha256(repr(items).encode()).hexdigest() == digest
        if mode != 'baseline':
            for xref, _ in groups[mode]:
                doc.xref_set_key(xref, 'Group/I', 'false' if mode in ('image', 'isolated_form') else 'true')
        encoded = doc.tobytes()
        metrics = {}
        for scale in (1, 2):
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 * scale / 3, 4 * scale / 3), alpha=True)
            img = Image.frombytes('RGBA', (256 * scale, 256 * scale), pix.samples)
            if scale == 2:
                img = Image.merge('RGBA', tuple(ch.resize((256, 256), Image.Resampling.BOX) for ch in img.split()))
            data = img.tobytes()
            with pypdfium2.PdfDocument(encoded) as pdf:
                alpha = pdf[0].render(scale=4 * scale / 3, rev_byteorder=True, fill_color=(0, 0, 0, 0)).to_pil().convert('RGBA').getchannel('A')
                if scale == 2:
                    alpha = alpha.resize((256, 256), Image.Resampling.BOX)
                pdfium_error = sum(abs(x - y) for x, y in zip(alpha.tobytes(), native[3::4]))
            metrics[f'{scale}x'] = {
                'mupdf_premultiplied_rgba_absolute_error': sum(abs(x - y) for x, y in zip(data, native)),
                'mupdf_alpha_absolute_error': sum(abs(x - y) for x, y in zip(data[3::4], native[3::4])),
                'pdfium_alpha_absolute_error': pdfium_error,
            }
        results[name][mode] = metrics
print(json.dumps({'scope': __doc__, 'pymupdf': pymupdf.VersionBind, 'pdfium': str(pypdfium2.PDFIUM_INFO), 'pillow': PIL.__version__, 'receipt_sha256': hashlib.sha256(receipt_path.read_bytes()).hexdigest(), 'candidate_pdf_sha256': {name: receipt['pdf_sha256'][f'{name}-candidate.pdf'] for name in pins}, 'results': results}, indent=2))
