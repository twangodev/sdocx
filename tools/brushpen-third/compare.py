#!/usr/bin/env python3
"""Compare public serialized BrushPen canvas exports to the retained host frame.

Generate exports with SDOCX_BRUSHPEN_THIRD_EXPORT pointing to an existing
folder and cargo test -p sdocx --lib --features pdf,serde
third_serialized_brushpen_native. Pass that folder to this script.
"""
import hashlib
import importlib.metadata
import json
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

import numpy as np
import pymupdf as fitz
import pypdfium2 as pdfium
from PIL import Image

root = Path(__file__).resolve().parents[2]
folder = Path(sys.argv[1])
frame = zlib.decompress((root / 'crates/sdocx/tests/assets/brushpen-third/frame.rgba.zlib').read_bytes())
assert hashlib.sha256(frame).hexdigest() == '11e461b0c48ed4d1f62331f758061b243839eba307881d8e5077cb17dae8761c'
native = np.frombuffer(frame, dtype=np.uint8).reshape(228, 321, 4)[::-1].astype(int)

def metrics(image):
    assert image.shape == native.shape
    delta = np.abs(image.astype(int) - native)
    return {'alpha_l1': int(delta[:, :, 3].sum()), 'alpha_max': int(delta[:, :, 3].max()),
            'premultiplied_rgb_l1': int(delta[:, :, :3].sum())}

def premultiply(image):
    image = np.array(image, dtype=int)
    image[:, :, :3] = (image[:, :, :3] * image[:, :, 3:4] + 127) // 255
    return image

results = {'canvas_png': metrics(premultiply(Image.open(folder / 'canvas.png').convert('RGBA')))}
with tempfile.TemporaryDirectory() as tmp:
    raster = Path(tmp) / 'svg.png'
    subprocess.run(['resvg', str(folder / 'canvas.svg'), str(raster)], check=True)
    results['svg_resvg'] = metrics(premultiply(Image.open(raster).convert('RGBA')))
for route in ('direct', 'svg'):
    path = folder / f'{route}.pdf'
    doc = fitz.open(path)
    assert len(doc) == 1
    pix = doc[0].get_pixmap(alpha=True)
    assert (pix.width, pix.height, pix.n) == (321, 228, 4)
    results[f'{route}_mupdf'] = metrics(np.frombuffer(pix.samples, dtype=np.uint8).reshape(228, 321, 4))
    document = pdfium.PdfDocument(str(path))
    assert len(document) == 1
    image = document[0].render(scale=1, fill_color=(0, 0, 0, 0)).to_pil().convert('RGBA')
    results[f'{route}_pdfium'] = metrics(premultiply(image))
print(json.dumps({'scope': 'A synthetic serialized GLV10 pressure and path variant; no device or saved-note parity.',
                  'native_sha256': hashlib.sha256(frame).hexdigest(),
                  'versions': {name: importlib.metadata.version(name) for name in ('numpy', 'Pillow', 'PyMuPDF')},
                  'pypdfium2_version': str(pdfium.PYPDFIUM_INFO), 'pdfium_version': str(pdfium.PDFIUM_INFO),
                  'resvg_version': subprocess.check_output(['resvg', '--version'], text=True).strip(),
                  'exports_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(folder.iterdir()) if p.is_file()},
                  'results': results}, indent=2))
