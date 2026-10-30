"""Compare generated public and generic exports with retained native paint."""
import hashlib
import json
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image

root = Path('tools/marker4_v7/saved-v197')
receipt = json.loads((root / 'receipt.json').read_bytes())
native_receipt = (root / 'native.json').read_bytes()
assert hashlib.sha256(native_receipt).hexdigest() == receipt['native_receipt_sha256']
meta = json.loads(native_receipt)
raw = zlib.decompress((root / 'frame.rgba.zlib').read_bytes())
assert hashlib.sha256(raw).hexdigest() == meta['rgba_sha256']
assert meta['tool'] == 2 and meta['set_pen_argb'] == '0x733396ff'
image = Image.frombytes('RGBA', (1024, 512), raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM)
expected = bytes(v + 255 - a for r, g, b, a in image.getdata() for v in (r, g, b))
assert len(sys.argv) == 2, 'usage: compare.py EXPORT_DIR'
output = Path(sys.argv[1])
results = {}
for name in ('public', 'generic'):
    base = output / name
    for ext in ('svg', 'pdf'):
        assert hashlib.sha256(base.with_suffix('.' + ext).read_bytes()).hexdigest() == receipt['export_sha256'][name][ext], (name, ext, 'export hash mismatch')
    with tempfile.TemporaryDirectory(prefix='marker4-v197-render-') as temp:
        png = Path(temp) / 'render.png'
        subprocess.run(['resvg', str(base.with_suffix('.svg')), str(png)], check=True)
        svg_image = Image.open(png).convert('RGB')
        assert svg_image.size == (1024, 512)
        svg = svg_image.tobytes()
    pdf = pymupdf.open(base.with_suffix('.pdf'))
    pix = pdf[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
    assert (pix.width, pix.height) == (1024, 512)
    pdfium = pypdfium2.PdfDocument(base.with_suffix('.pdf'))
    image = pdfium[0].render(scale=4/3).to_pil().convert('RGB')
    assert image.size == (1024, 512)
    assert all(len(pixels) == len(expected) for pixels in (svg, pix.samples, image.tobytes()))
    results[name] = {key: sum(abs(a-b) for a,b in zip(expected, pixels)) for key,pixels in [('resvg',svg),('mupdf',pix.samples),('pdfium',image.tobytes())]}
print(json.dumps(results))
