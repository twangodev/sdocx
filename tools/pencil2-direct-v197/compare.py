import hashlib
import os
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image

work = Path(os.environ['PENCIL2_PROBE_DIR'])
for scale, case, digest in [(1, 'baseline', 'fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b'), (2, 'scale2', 'e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4')]:
    crop = Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes()
    assert len(crop) == 62 * 22 and hashlib.sha256(crop).hexdigest() == digest
    native = bytearray(192 * 192)
    for y in range(22):
        native[(170+y)*192:(170+y)*192+62] = crop[y*62:(y+1)*62]
    image = Image.open(work / f'svg{scale}.png')
    assert image.size == (192, 192)
    svg = image.convert('RGB').tobytes()[::3]
    data = (work / f'pdf{scale}.pdf').read_bytes()
    with pymupdf.open(stream=data, filetype='pdf') as doc:
        pixmap = doc[0].get_pixmap(alpha=False, colorspace=pymupdf.csRGB)
        assert (pixmap.width, pixmap.height) == (192, 192)
        mu = pixmap.samples[::3]
    with pypdfium2.PdfDocument(data) as doc:
        image = doc[0].render(fill_color=(255,255,255,255), rev_byteorder=True).to_pil()
        assert image.size == (192, 192)
        pi = image.convert('RGB').tobytes()[::3]
    expected_results = {1: {'resvg': (27, 2, 22), 'MuPDF': (94, 4, 50), 'PDFium': (60, 3, 37)}, 2: {'resvg': (73, 2, 67), 'MuPDF': (341, 4, 177), 'PDFium': (242, 3, 156)}}
    for name, output in [('resvg', svg), ('MuPDF', mu), ('PDFium', pi)]:
        assert len(output) == len(native) == 192 * 192
        errors = [abs(255 - pixel - expected) for pixel, expected in zip(output, native)]
        result = sum(errors), max(errors), sum(bool(e) for e in errors)
        assert result == expected_results[scale][name], (scale, name, result)
        print(scale, name, 'sum', result[0], 'max', result[1], 'changed', result[2])
    print(scale, 'svg bytes', (work / f'svg{scale}.svg').stat().st_size, 'pdf bytes', len(data))
