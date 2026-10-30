import hashlib
import json
import sys
from pathlib import Path

import pypdfium2

base = Path(sys.argv[1])
native_receipt = json.loads((base / 'native.json').read_text())
frame_hash = native_receipt['native_draw_target_readback']['sha256']
native = (base / 'captures' / f'simplebrush-native-draw-rgba-{frame_hash}.bin').read_bytes()
assert hashlib.sha256(native).hexdigest() == frame_hash
assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
assert str(pypdfium2.PYPDFIUM_INFO) == '5.9.0'

def raster(path):
    doc = pypdfium2.PdfDocument(str(path))
    return doc[0].render(scale=4 / 3, rev_byteorder=True).to_pil().convert('RGB')

paper = raster(base / 'paper.pdf')
assert paper.size == (256, 256)
for mode in ('generic', 'one', 'two'):
    image = raster(base / f'{mode}.pdf')
    assert image.size == paper.size
    error = 0
    for y in range(256):
        for x in range(256):
            i = (y * 256 + x) * 4
            a = native[i + 3]
            pixel = image.getpixel((x, 255 - y))
            bg = paper.getpixel((x, 255 - y))
            error += sum(abs(pixel[k] - (native[i + k] + (bg[k] * (255 - a) + 127) // 255)) for k in range(3))
    print(mode, error)
