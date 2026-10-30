#!/usr/bin/env python3
"""Check the standalone public host SVG/PDF against the matched native capture."""
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

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('svg', type=Path)
parser.add_argument('png', type=Path)
parser.add_argument('pdf', type=Path)
parser.add_argument('--output', type=Path)
parser.add_argument('--grid', action='store_true', help='compare the matched fixed-grid experiment')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
fixture = root / 'crates/sdocx/tests/assets/crayon2-v2-native/matched-opaque.json.gz'
data = json.loads(gzip.decompress(fixture.read_bytes()))
raw = zlib.decompress(base64.b64decode(data['paint']['readback']['rgba8_zlib_base64']))
if hashlib.sha256(raw).hexdigest() != '313e4d934db65d0d91428b07139ff9c999b69288a2270928b9f678de0b4669b9':
    raise SystemExit('native framebuffer hash mismatch')
rows = b''.join(raw[y*1024:(y+1)*1024] for y in reversed(range(256)))
native = bytes(v for i, v in enumerate(rows) if i % 4 != 3)
with pymupdf.open(args.pdf) as doc:
    mupdf = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False).samples
with pypdfium2.PdfDocument(args.pdf) as doc:
    pdfium = doc[0].render(scale=4/3).to_pil().convert('RGB').tobytes()
with Image.open(args.png) as image:
    svg = image.convert('RGB').tobytes()
metrics = {}
for name, frame in [('resvg', svg), ('mupdf', mupdf), ('pdfium', pdfium)]:
    diff = [abs(a-b) for a, b in zip(frame, native, strict=True)]
    metrics[name] = [sum(any(diff[i:i+3]) for i in range(0, len(diff), 3)), max(diff), sum(diff)]
expected = ({'resvg': [182, 2, 270], 'mupdf': [239, 5, 730], 'pdfium': [240, 5, 753]} if args.grid else
            {'resvg': [174, 34, 732], 'mupdf': [363, 82, 22346], 'pdfium': [314, 75, 15348]})
if metrics != expected:
    raise SystemExit(f'unexpected reader comparison: {metrics}')
paths = {'svg': args.svg, 'png': args.png, 'pdf': args.pdf, 'native_fixture': fixture,
         'sampler': root / 'crates/sdocx/src/ink/crayon2.rs',
         'material': root / 'crates/sdocx/src/ink/crayon2/material.rs',
         'exporter': root / 'crates/sdocx/examples/crayon2_host_export.rs',
         'comparator': Path(__file__)}
result = {'scope': 'one matched synthetic host source stroke, standalone SVG and SVG-page PDF; no device or saved-note parity',
          'metric_order': ['differing_pixels', 'maximum_channel_error', 'absolute_rgb_error'],
          'native_rgb_comparison': metrics,
          'versions': {'resvg': '0.47.0 (Cargo.lock)', 'pymupdf': pymupdf.VersionBind,
                       'pypdfium2': str(pypdfium2.PYPDFIUM_INFO), 'pdfium': str(pypdfium2.PDFIUM_INFO)},
          'sha256': {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in paths.items()}}
text = json.dumps(result, indent=2) + '\n'
if args.output:
    with args.output.open('x') as output:
        output.write(text)
print(text, end='')
