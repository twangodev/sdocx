import hashlib
import json
from pathlib import Path
import sys

import pypdfium2

assert str(pypdfium2.PDFIUM_INFO) == "156.0.8076.0"
b = Path(sys.argv[1])
j = json.loads((b / "native.json").read_text())
h = j["native_draw_target_readback"]["sha256"]
native = (b / "captures" / ("simplebrush-native-draw-rgba-" + h + ".bin")).read_bytes()
assert hashlib.sha256(native).hexdigest() == h


def raster(path):
    document = pypdfium2.PdfDocument(str(path))
    return document[0].render(scale=8 / 3, rev_byteorder=True).to_pil().convert("RGB")


paper = raster(b / "paper.pdf")
for mode in ("generic", "one", "two"):
    image = raster(b / f"{mode}.pdf")
    error = 0
    for y in range(256):
        for x in range(256):
            i = (y * 256 + x) * 4
            alpha = native[i + 3]
            pixel = image.getpixel((x, 511 - y))
            background = paper.getpixel((x, 511 - y))
            error += sum(
                abs(
                    pixel[k]
                    - (native[i + k] + (background[k] * (255 - alpha) + 127) // 255)
                )
                for k in range(3)
            )
    print(mode, error)
