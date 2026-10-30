import os
from pathlib import Path
import sys

import PIL
from PIL import Image
import pymupdf
import pypdfium2

if not __debug__:
    raise RuntimeError('Run without Python optimization')
if (PIL.__version__, pymupdf.VersionBind, pypdfium2.PDFIUM_INFO.version) != ('12.1.1', '1.28.2', '156.0.8076.0'):
    raise RuntimeError('Expected Pillow 12.1.1, MuPDF 1.28.2 and PDFium 156.0.8076.0')
work = Path(os.environ['PENCIL2_PROBE_DIR'])

def pixels(path, reader):
    if reader == 'MuPDF':
        with pymupdf.open(path) as doc:
            pix = doc[0].get_pixmap(alpha=False, colorspace=pymupdf.csRGB)
            assert (pix.width, pix.height) == (192, 192)
            return pix.samples
    with pypdfium2.PdfDocument(path) as doc:
        img = doc[0].render(fill_color=(255, 255, 255, 255), rev_byteorder=True).to_pil().convert('RGB')
        assert img.size == (192, 192)
        return img.tobytes()

def pixel(data, x, y):
    offset = (y * 192 + x) * 3
    return tuple(data[offset:offset + 3])

expected = {1: {'resvg': (27, 2, 22), 'MuPDF': (133, 5, 50), 'PDFium': (60, 3, 37)},
            2: {'resvg': (73, 2, 67), 'MuPDF': (466, 5, 177), 'PDFium': (242, 3, 156)}}
for scale, case in ((1, 'baseline'), (2, 'scale2')):
    crop = Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes()
    assert len(crop) == 62 * 22
    native = bytearray(192 * 192)
    for y in range(22):
        native[(170 + y) * 192:(170 + y) * 192 + 62] = crop[y * 62:(y + 1) * 62]
    svg = Image.open(work / f'svg{scale}.png').convert('RGB')
    assert svg.size == (192, 192)
    outputs = {'resvg': svg.tobytes(), **{name: pixels(work / f'pdf{scale}.pdf', name) for name in ('MuPDF', 'PDFium')}}
    for name, rgb in outputs.items():
        errors = [abs(255 - p - n) for p, n in zip(rgb[::3], native, strict=True)]
        result = sum(errors), max(errors), sum(bool(e) for e in errors)
        assert result == expected[scale][name], (scale, name, result)
        print(scale, name, result)
    for name in ('MuPDF', 'PDFium'):
        forward = pixels(work / f'scene{scale}.pdf', name)
        reverse = pixels(work / f'reverse{scale}.pdf', name)
        for image in (forward, reverse):
            assert pixel(image, 0, 0) == (255, 255, 255)
            assert pixel(image, 10, 185) == (255, 0, 0)
        assert pixel(forward, 27, 185) == (0, 0, 255)
        assert pixel(reverse, 27, 185) == (255, 0, 0)
        if scale == 2:
            assert pixel(forward, 46, 184) == (107, 0, 0)
            assert pixel(reverse, 46, 184) == (255, 0, 0)
        print(scale, name, 'scene order passed')
