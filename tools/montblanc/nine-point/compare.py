#!/usr/bin/env python3
"""Compare isolated candidate/fallback exports against the retained host frame."""
import hashlib
import base64
import gzip
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

import pymupdf
import pypdfium2
from PIL import Image

root = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(root / 'tools/montblanc'))
from public_fallback_compare import composite_native_frame, metrics

output = Path(sys.argv[1])
here = root / 'tools/montblanc/nine-point'
manifest = json.loads((here / 'manifest.json').read_text())
for path, digest in manifest['sha256'].items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == digest, path
capture = json.loads(gzip.decompress((here / 'capture.json.gz').read_bytes()))
frame = zlib.decompress((here / 'frame.rgba.zlib').read_bytes())
assert len(frame) == 256 * 256 * 4
assert hashlib.sha256(frame).hexdigest() == capture['post_readback']['sha256']
assert frame == zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
native = composite_native_frame(frame, (256, 256)).tobytes()
result = {}
with tempfile.TemporaryDirectory() as directory:
    for variant in ('candidate', 'fallback'):
        path = output / f'nine-{variant}'
        png = Path(directory) / 'out.png'
        subprocess.run(['resvg', '-w', '256', str(path) + '.svg', str(png)], check=True)
        svg_bytes = Image.open(png).convert('RGB').tobytes()
        assert len(svg_bytes) == len(native)
        svg = metrics(native, svg_bytes)
        with pymupdf.open(str(path) + '.pdf') as doc:
            mu_bytes = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False).samples
            assert len(mu_bytes) == len(native)
            mu = metrics(native, mu_bytes)
        with pypdfium2.PdfDocument(str(path) + '.pdf') as doc:
            pdf_bytes = doc[0].render(scale=4/3).to_pil().convert('RGB').tobytes()
            assert len(pdf_bytes) == len(native)
            pdfium = metrics(native, pdf_bytes)
        result[variant] = {'svg': svg, 'mupdf': mu, 'pdfium': pdfium,
                           'svg_sha256': hashlib.sha256(Path(str(path)+'.svg').read_bytes()).hexdigest(),
                           'pdf_sha256': hashlib.sha256(Path(str(path)+'.pdf').read_bytes()).hexdigest()}
print(json.dumps(result, indent=2))
