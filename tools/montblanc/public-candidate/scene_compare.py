#!/usr/bin/env python3
"""Compare Rust Scene exports to retained native frames (requires resvg and PyMuPDF)."""
import argparse
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser()
parser.add_argument('output_dir', type=Path)
OUTPUT = parser.parse_args().output_dir
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import metrics

receipt = json.loads((ROOT / 'conformance/montblanc-scene-candidate-host-comparison.json').read_text())
captures = json.loads((ROOT / 'conformance/montblanc-public-candidate-host-comparison.json').read_text())
with tempfile.TemporaryDirectory(prefix='montblanc-scene-') as tmp:
    for name, expected in receipt['cases'].items():
        raw = gzip.decompress((ROOT / f'tools/montblanc/captures/{name}.json.gz').read_bytes())
        assert hashlib.sha256(raw).hexdigest() == captures['cases'][name]['capture_sha256']
        capture = json.loads(raw)
        frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
        assert hashlib.sha256(frame).hexdigest() == capture['post_readback']['sha256']
        native = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
        native.alpha_composite(Image.frombytes('RGBA', (256, 256), frame).transpose(Image.Transpose.FLIP_TOP_BOTTOM))
        reference = native.convert('RGB').tobytes()
        raster = Path(tmp) / 'raster.png'
        subprocess.run(['resvg', str(OUTPUT / f'{name}.svg'), str(raster)], check=True)
        image = Image.open(raster).convert('RGB')
        assert image.size == (256, 256)
        svg = metrics(reference, image.tobytes())
        with pymupdf.open(OUTPUT / f'{name}.pdf') as doc:
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
            assert (pix.width, pix.height) == (256, 256)
            pdf = metrics(reference, pix.samples)
        assert svg == expected['svg'], (name, svg)
        assert pdf == expected['pdf'], (name, pdf)
        print(name, 'svg', svg, 'pdf', pdf)
