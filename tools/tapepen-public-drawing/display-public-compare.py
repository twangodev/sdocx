#!/usr/bin/env python3
"""Verify public SVG and direct PDF against the paired native crops on white."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

import pymupdf
from PIL import Image

here = Path(__file__).resolve().parent
out = Path(sys.argv[1])
receipt = json.loads((here / 'display-public-receipt.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
assert pymupdf.VersionBind == receipt['mupdf']
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == receipt['resvg']
for name in ('svg', 'pdf'):
    assert sha((out / f'public.{name}').read_bytes()) == receipt['public_sha256'][name]
with tempfile.TemporaryDirectory(prefix='tape-public-') as tmp:
    png = Path(tmp) / 'public.png'
    subprocess.run(['resvg', str(out / 'public.svg'), str(png)], check=True)
    svg = Image.open(png).convert('RGB').tobytes()
with pymupdf.open(out / 'public.pdf') as doc:
    assert len(doc) == 1
    pdf = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False).samples
for n in (256, 512):
    compressed = (here / f'identity-screen-{n}-frame.rgba.zlib').read_bytes()
    assert sha(compressed) == receipt['compressed_native_sha256'][str(n)]
    raw = zlib.decompress(compressed)
    assert len(raw) == 512 * 512 * 4
    native = b''.join(raw[y * 2048:y * 2048 + 1024] for y in range(256))
    white = bytes(min(255, native[i+c] + 255-native[i+3]) for i in range(0, len(native), 4) for c in range(3))
    for name, data in (('svg', svg), ('mupdf_direct_pdf', pdf)):
        assert len(data) == len(white)
        diffs = [abs(a-b) for a, b in zip(white, data, strict=True)]
        actual = {'l1': sum(diffs), 'max': max(diffs), 'different_channels': sum(v != 0 for v in diffs)}
        assert actual == receipt['metrics'][str(n)][name], (n, name, actual)
        print(n, name, actual)
