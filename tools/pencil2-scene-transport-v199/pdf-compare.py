import hashlib
import os
import zlib
from pathlib import Path
from PIL import Image, __version__ as pillow_version
import pymupdf
import pypdfium2

if not __debug__:
    raise RuntimeError('Run without Python optimization')
if (pillow_version, pymupdf.VersionBind, pypdfium2.PDFIUM_INFO.version) != ('12.1.1', '1.28.2', '156.0.8076.0'):
    raise RuntimeError('Expected Pillow 12.1.1, MuPDF 1.28.2 and PDFium 156.0.8076.0')

work = Path(os.environ['PENCIL199_PDF_DIR'])
hashes = ('ab11d63776cd76aff35f5365b55cf7e7e11b8eb1efaf5bec5ea51735e15dfddd',
          '84b5770be474561d848b359da6b4084930412220b28c62080e04254f52d616fd')
expected = [((477, 5, 221), (267, 3, 193), (911, 20, 226), (1804, 29, 240)),
            ((8481, 25, 1126), (4304, 12, 1061), (38410, 140, 1148), (39294, 140, 1164))]
for i, name in enumerate(('first', 'second')):
    compressed = Path(f'crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing/{name}.zlib').read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == hashes[i]
    raw = zlib.decompress(compressed)
    assert len(raw) == 192 * 192 * 4
    ref = Image.frombytes('RGBA', (192, 192), raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).convert('RGB').tobytes()
    results = []
    for kind in ('native', 'generic'):
        path = work / kind / f'direct{i}.pdf'
        assert path.stat().st_size < 2 * 1024 * 1024
        with pymupdf.open(path) as doc:
            pixmap = doc[0].get_pixmap(alpha=False, colorspace=pymupdf.csRGB)
            assert (pixmap.width, pixmap.height) == (192, 192)
            mupdf = pixmap.samples
        with pypdfium2.PdfDocument(path) as doc:
            image = doc[0].render(fill_color=(255, 255, 255, 255), rev_byteorder=True).to_pil().convert('RGB')
            assert image.size == (192, 192)
            pdfium = image.tobytes()
        for data in (mupdf, pdfium):
            assert len(data) == len(ref)
            errors = [abs(a - b) for a, b in zip(data, ref)]
            results.append((sum(errors), max(errors), sum(bool(e) for e in errors)))
    assert tuple(results) == expected[i], (i, results)
    print(i, 'native MuPDF, native PDFium, generic MuPDF, generic PDFium:', results)
