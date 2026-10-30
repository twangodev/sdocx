#!/usr/bin/env python3
"""Compare source-derived PNG transport to native opaque-backdrop frames."""

import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image, __version__ as pillow_version

if not __debug__:
    raise SystemExit(
        "Run without Python optimization; assertions verify the diagnostic."
    )
root = Path(__file__).resolve().parents[2]
receipt = json.loads(
    (root / "tools/pen-capture/airbrush_v1_tool1_backdrop_receipt.json").read_text()
)
assert pymupdf.VersionBind == "1.28.2"
assert str(pypdfium2.PDFIUM_INFO) == "150.0.7869.0"
assert pillow_version == "12.1.1"
pins = {
    "reverse-1": "8f5df0580bb2e9dfdce265d6a398622e605558df91d790f84caed27601d5b876",
    "zigzag-1": "97810a0493a127d022a8340453458f39b5fce95ff96b8e121975980e81b6248c",
}
for name, pin in pins.items():
    png = Path(sys.argv[1]) / f"{name}.png"
    assert hashlib.sha256(png.read_bytes()).hexdigest() == pin
    frame = receipt["cases"][name]["frame"]
    raw = zlib.decompress(base64.b64decode(frame["data_base64"]))
    assert hashlib.sha256(raw).hexdigest() == frame["sha256"]
    assert len(raw) == 256 * 256 * 4
    native = b"".join(reversed([raw[i : i + 1024] for i in range(0, len(raw), 1024)]))
    image = Image.open(png).convert("RGBA")
    assert image.size == (256, 256)
    background = Image.new("RGBA", image.size, (40, 90, 160, 255))
    background.alpha_composite(image)

    def error(actual):
        return sum(abs(a - b) for a, b in zip(actual, native, strict=True))

    with pymupdf.open() as pdf:
        page = pdf.new_page(width=192, height=192)
        page.draw_rect(page.rect, fill=(40 / 255, 90 / 255, 160 / 255), color=None)
        page.insert_image(page.rect, filename=str(png))
        encoded = pdf.tobytes()
    with pymupdf.open(stream=encoded, filetype="pdf") as pdf:
        pix = pdf[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True)
        assert (pix.width, pix.height, pix.n) == (256, 256, 4)
        mupdf = error(pix.samples)
    with pypdfium2.PdfDocument(encoded) as pdf:
        pixels = (
            pdf[0]
            .render(scale=4 / 3, rev_byteorder=True, fill_color=(0, 0, 0, 0))
            .to_pil()
            .convert("RGBA")
        )
        assert pixels.size == (256, 256)
        pdfium = error(pixels.tobytes())
    result = {"pillow": error(background.tobytes()), "mupdf": mupdf, "pdfium": pdfium}
    assert result == receipt["rgba_absolute_errors"][name], (name, result)
    print(name, result)
