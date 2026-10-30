"""Verify the captured host frame and compare actual document exports."""
import hashlib
import json
import sys
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'receipt.json').read_text())
svg, png, pdf = map(Path, sys.argv[1:])
raw = zlib.decompress((here / 'frame.rgba.zlib').read_bytes())
assert hashlib.sha256(raw).hexdigest() == receipt['native_frame_sha256']
assert hashlib.sha256((here / 'vector.bin').read_bytes()).hexdigest() == receipt['native_evidence']['native_vector_sha256']
native = b''.join(raw[y * 768:(y + 1) * 768] for y in reversed(range(192)))
native = bytes(v for i, v in enumerate(native) if i % 4 != 3)
with Image.open(png) as image:
    assert image.size == (192, 192)
    frames = {'resvg': image.convert('RGB').tobytes()}
with pymupdf.open(pdf) as doc:
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False)
    assert (pix.width, pix.height) == (192, 192)
    frames['mupdf'] = pix.samples
with pypdfium2.PdfDocument(pdf) as doc:
    image = doc[0].render(scale=4 / 3).to_pil().convert('RGB')
    assert image.size == (192, 192)
    frames['pdfium'] = image.tobytes()
metrics = {}
for name, frame in frames.items():
    diff = [abs(a - b) for a, b in zip(native, frame, strict=True)]
    metrics[name] = [sum(any(diff[i:i + 3]) for i in range(0, len(diff), 3)), max(diff), sum(diff)]
result = {'readers_vs_native': metrics, 'export_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (svg, png, pdf)}}
print(json.dumps(result, indent=2))
if 'readers_vs_native' in receipt:
    assert result['readers_vs_native'] == receipt['readers_vs_native']
    assert result['export_sha256'] == receipt['export_sha256']
