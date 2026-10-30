#!/usr/bin/env python3
"""Compare independent public exports to the retained native host framebuffer."""
import argparse
import hashlib
import json
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image, __version__ as pillow_version

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'corrected-scale-receipt.json').read_text())
root = here.parents[1]
for relative, expected in receipt['input_sha256'].items():
    actual = hashlib.sha256((root / relative).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f'input {relative} hash differs: {actual}')
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('svg', type=Path)
p.add_argument('png', type=Path)
p.add_argument('pdf', type=Path)
args = p.parse_args()
versions = {'pymupdf': pymupdf.VersionBind, 'pypdfium2': str(pypdfium2.PYPDFIUM_INFO),
            'pdfium': str(pypdfium2.PDFIUM_INFO), 'pillow': pillow_version}
if versions != receipt['reader_versions']:
    raise SystemExit(f'reader versions differ: {versions}')
for name in ('svg', 'png', 'pdf'):
    actual = hashlib.sha256(getattr(args, name).read_bytes()).hexdigest()
    if actual != receipt['export_sha256'][name]:
        raise SystemExit(f'{name} hash differs: {actual}')
raw = zlib.decompress((here / 'native-frame.z').read_bytes())
if len(raw) != 192 * 192 * 4 or hashlib.sha256(raw).hexdigest() != receipt['native_frame_sha256']:
    raise SystemExit('native frame hash or dimensions differ')
rows = b''.join(raw[y * 768:(y + 1) * 768] for y in reversed(range(192)))
native = bytes(v for i, v in enumerate(rows) if i % 4 != 3)
with Image.open(args.png) as img:
    if img.size != (192, 192):
        raise SystemExit('PNG dimensions differ')
    svg = img.convert('RGB').tobytes()
with pymupdf.open(args.pdf) as doc:
    pixmap = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
    if (pixmap.width, pixmap.height) != (192, 192):
        raise SystemExit('MuPDF dimensions differ')
    mupdf = pixmap.samples
with pypdfium2.PdfDocument(args.pdf) as doc:
    img = doc[0].render(scale=4/3).to_pil().convert('RGB')
    if img.size != (192, 192):
        raise SystemExit('PDFium dimensions differ')
    pdfium = img.tobytes()
metrics = {}
for name, image in [('resvg', svg), ('mupdf', mupdf), ('pdfium', pdfium)]:
    diff = [abs(a - b) for a, b in zip(native, image, strict=True)]
    metrics[name] = [sum(any(diff[i:i+3]) for i in range(0, len(diff), 3)), max(diff), sum(diff)]
if metrics != receipt['readers_vs_native']:
    raise SystemExit(f'reader metrics differ: {metrics}')
print(json.dumps(metrics, indent=2))
