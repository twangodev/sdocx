import hashlib
import json
from pathlib import Path
import sys
import subprocess
import tempfile

import PIL
from PIL import Image
import pymupdf
import pypdfium2 as pdf

if len(sys.argv) != 4:
    raise SystemExit('usage: compare.py CAPTURE.json CAPTURE_SHA256 OUTPUT_DIR')
path = Path(sys.argv[1])
receipt = path.read_bytes()
if hashlib.sha256(receipt).hexdigest() != sys.argv[2]:
    raise SystemExit('capture receipt SHA-256 mismatch')
meta = json.loads(receipt)
expected_receipt = json.loads(Path(__file__).with_name('receipt.json').read_bytes())
versions = {'resvg': subprocess.check_output(['resvg', '--version'], text=True).strip().split()[-1], 'mupdf': pymupdf.VersionBind, 'pypdfium2': str(pdf.PYPDFIUM_INFO), 'pdfium': str(pdf.PDFIUM_INFO), 'pillow': PIL.__version__}
if versions != expected_receipt['versions'] or sys.argv[2] != expected_receipt['capture_sha256']:
    raise SystemExit('reader version or captured input mismatch')
if meta['scale'] not in (1, 2, 4) or meta['target'] != [1024, 512]:
    raise SystemExit('only scales 1, 2 and 4 and 1024x512 target are supported')
fixture = Path('tools/marker4_v7/public-finger/wide-extra/geometry.json').read_bytes()
if hashlib.sha256(fixture).hexdigest() != meta['source_json_sha256']:
    raise SystemExit('source fixture SHA-256 mismatch')
raw_bytes = path.with_suffix('.rgba').read_bytes()
if len(raw_bytes) != 1024 * 512 * 4 or hashlib.sha256(raw_bytes).hexdigest() != meta['rgba_sha256']:
    raise SystemExit('native framebuffer length or SHA-256 mismatch')
raw = Image.frombytes('RGBA', (1024, 512), raw_bytes).transpose(Image.Transpose.FLIP_TOP_BOTTOM)
if any(max(r, g, b) > a for r, g, b, a in raw.getdata()):
    raise SystemExit('native framebuffer is not premultiplied')
native = bytes(v + (255 - a) for r, g, b, a in raw.getdata() for v in (r, g, b))
for name in ('fallback', 'public'):
    prefix = str(Path(sys.argv[3]) / ('marker4-' + name))
    manifest = json.loads(Path(prefix + '.manifest.json').read_bytes())
    if manifest['capture_sha256'] != sys.argv[2] or manifest['role'] != name:
        raise SystemExit(f'{name}: export capture or role mismatch')
    for ext in ('svg', 'pdf'):
        if hashlib.sha256(Path(prefix + '.' + ext).read_bytes()).hexdigest() != manifest[ext + '_sha256'] or manifest[ext + '_sha256'] != expected_receipt['exports'][name][ext]:
            raise SystemExit(f'{name}: {ext} SHA-256 mismatch')
    with tempfile.TemporaryDirectory() as temp:
        png = Path(temp) / 'render.png'
        subprocess.run(['resvg', '--zoom', str(meta['scale']), prefix + '.svg', str(png)], check=True)
        image = Image.open(png).convert('RGB')
    doc = pymupdf.open(prefix + '.pdf')
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3 * meta['scale'], 4/3 * meta['scale']), alpha=False)
    mup = Image.frombytes('RGB', (pix.width, pix.height), pix.samples)
    pdfdoc = pdf.PdfDocument(prefix + '.pdf')
    pdi = pdfdoc[0].render(scale=4/3 * meta['scale']).to_pil().convert('RGB')
    for reader, rendered in [('resvg', image), ('mupdf', mup), ('pdfium', pdi)]:
        w, h = rendered.size
        expected = (1024 * meta['scale'], 512 * meta['scale'])
        if (w, h) != expected:
            raise SystemExit(f'{name} {reader}: expected full image {expected}, got {(w, h)}')
        rendered = rendered.crop(((w-1024)//2, (h-512)//2, (w+1024)//2, (h+512)//2))
        pixels = rendered.tobytes()
        if rendered.size != (1024, 512) or len(pixels) != len(native):
            raise SystemExit(f'{name} {reader}: image dimensions or byte length mismatch')
        error = sum(abs(a-b) for a,b in zip(native,pixels))
        if error != expected_receipt['rgb_l1'][name][reader]:
            raise SystemExit(f'{name} {reader}: error mismatch: {error}')
        print(name, reader, rendered.size, error)
