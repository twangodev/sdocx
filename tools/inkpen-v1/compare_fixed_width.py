"""Compare fixed-width native frames with paired public exports from an isolated probe.

The export directory contains {index}-{candidate,generic}.{svg,pdf}. Requires
Pillow, PyMuPDF, pypdfium2, and a resvg helper accepting input/output paths.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import zlib

import pymupdf
import pypdfium2
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('fixture', type=Path)
parser.add_argument('exports', type=Path)
parser.add_argument('svg_renderer', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--offset', type=int, default=0)
args = parser.parse_args()
if args.output.exists():
    raise FileExistsError(args.output)
fixture = json.loads(args.fixture.read_text())
results = []
with tempfile.TemporaryDirectory(prefix='sdocx-v1-fixed-compare-') as tmp:
    temp = Path(tmp)
    for index, case in enumerate(fixture['cases']):
        scale = case.get('scale', 1)
        width, height = 128 * scale, 64 * scale
        native = zlib.decompress(base64.b64decode(case['rgba_zlib_base64']))
        assert len(native) == width * height * 4
        assert not any(native[channel] for channel in range(len(native)) if channel % 4 != 3), 'expected black native ink'
        assert hashlib.sha256(native).hexdigest() == case['rgba_sha256']
        background = bytes(round(252 * (255 - alpha) / 255) for alpha in native[3::4])
        record = {key: case[key] for key in ('width', 'tool', 'curve')}
        record.update(scale=scale, shape=case.get('shape', 'straight'), native_sha256=case['rgba_sha256'])
        for label in ('candidate', 'generic'):
            svg = (args.exports / f'{index + args.offset}-{label}.svg').read_text()
            svg = svg.replace('height="64"', f'height="{height}"').replace('width="128"', f'width="{width}"')
            svg, count = re.subn(r'<rect fill="#fcfcfc"[^>]*/>', '', svg)
            assert count == 1
            svg_path, png_path = temp / 'probe.svg', temp / 'probe.png'
            svg_path.write_text(svg)
            subprocess.run([str(args.svg_renderer), str(svg_path), str(png_path)], check=True)
            rgba = Image.open(png_path).convert('RGBA').tobytes()
            assert len(rgba) == len(native)
            assert all(rgba[i] == rgba[i + 1] == rgba[i + 2] for i in range(0, len(rgba), 4) if rgba[i + 3]), "expected grayscale SVG ink"
            if label == "candidate":
                assert not any(rgba[i] for i in range(0, len(rgba), 4) if rgba[i + 3]), "expected black candidate SVG ink"
            svg_error = sum(abs(a - b) for a, b in zip(native[3::4], rgba[3::4]))
            pdf = args.exports / f'{index + args.offset}-{label}.pdf'
            doc = pymupdf.open(pdf)
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 * scale / 3, 4 * scale / 3), alpha=False, colorspace=pymupdf.csRGB)
            assert (pix.width, pix.height, pix.n, pix.stride, len(pix.samples)) == (width, height, 3, width * 3, width * height * 3)
            bitmap = pypdfium2.PdfDocument(pdf)[0].render(scale=4 * scale / 3, rev_byteorder=True)
            rgb = bytes(bitmap.buffer)
            assert (bitmap.width, bitmap.height, bitmap.n_channels, bitmap.stride, len(rgb)) == (width, height, 3, width * 3, width * height * 3)
            def error(pixels):
                return sum(abs(background[i // 3] - value) for i, value in enumerate(pixels))
            record[label] = {'svg_alpha_error': svg_error, 'mupdf_rgb_error': error(pix.samples), 'pdfium_rgb_error': error(rgb)}
        for metric in ('svg_alpha_error', 'mupdf_rgb_error', 'pdfium_rgb_error'):
            assert 2 * record['candidate'][metric] < record['generic'][metric], (index, metric, record)
        results.append(record)
with args.output.open('x') as f:
    f.write(json.dumps({'schema': 'sdocx-inkpen-v1-fixed-width-export-comparison-v1', 'scope': 'Synthetic host Mesa and public SVG/direct PDF; not device or saved-note parity', 'cases': results}, indent=2) + '\n')
