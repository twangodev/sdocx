#!/usr/bin/env python3
"""Compare the public generic Crayon2 exports with the opaque host control."""
import argparse
import base64
import gzip
import hashlib
import json
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('directory', type=Path, help='directory containing public.png and public.pdf')
args = parser.parse_args()
fixture = Path(__file__).resolve().parents[2] / 'crates/sdocx/tests/assets/crayon2-v2-native/matched-opaque.json.gz'
control = json.loads(gzip.decompress(fixture.read_bytes()))
case = control['source_capture']['cases'][-1]
assert case['advanced_setting'] == '2;'
assert case['source_model_get_pen_size_f32_bits'] == '00002041'
assert case['source_arrays']['points_f32_bits'] == ['42800000', '42800000', '42900000', '428c0000', '42a40000', '42700000']
assert case['source_arrays']['pressure_f32_bits'] == ['3e99999a', '3f19999a', '3f800000']
assert case['native_model_metadata'] == {'color_argb': '0x80336699', 'curve_enabled': True, 'stroke_type': 0, 'tool_type': 2}
assert control['paint']['view_translation'] == [0.0, 0.0]
vector = bytes.fromhex(control['paint']['native_vector_hex'])
assert len(vector) == 303 * 24
assert hashlib.sha256(vector).hexdigest() == control['paint']['native_vector_sha256'] == 'a70a03345714235011fd28a90162b325f39e9ce28739181bae7941c39b0ed483'
raw = zlib.decompress(base64.b64decode(control['paint']['readback']['rgba8_zlib_base64']))
expected_sha = '313e4d934db65d0d91428b07139ff9c999b69288a2270928b9f678de0b4669b9'
if len(raw) != 256 * 256 * 4 or hashlib.sha256(raw).hexdigest() != expected_sha:
    raise ValueError('unexpected native framebuffer')
native = b''.join(raw[y * 1024:(y + 1) * 1024] for y in reversed(range(256)))
expected = bytes(native[i] for i in range(len(native)) if i % 4 != 3)
pdf = args.directory / 'public.pdf'
page = pymupdf.open(pdf)[0]
pixmap = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False)
pdfium = pypdfium2.PdfDocument(pdf)[0].render(scale=4 / 3).to_pil().convert('RGB')
svg = Image.open(args.directory / 'public.png').convert('RGB')
if svg.size != (256, 256) or pdfium.size != (256, 256) or (pixmap.width, pixmap.height) != (256, 256):
    raise ValueError('unexpected output dimensions')
for name, data in [('resvg', svg.tobytes()), ('mupdf', pixmap.samples), ('pdfium', pdfium.tobytes())]:
    errors = [abs(a - b) for a, b in zip(data, expected, strict=True)]
    result = {'differing_pixels': sum(any(errors[i:i + 3]) for i in range(0, len(errors), 3)), 'maximum_channel_error': max(errors), 'absolute_rgb_error': sum(errors)}
    expected_results = {'resvg': (349, 39, 15718), 'mupdf': (360, 39, 15718), 'pdfium': (376, 39, 15756)}
    assert tuple(result.values()) == expected_results[name], (name, result)
    print(name, json.dumps(result))
