#!/usr/bin/env python3
"""Minimal SVG-to-PDF mask control; uses temporary outputs and the shared Cargo target."""
import hashlib
import re
import subprocess
import tempfile
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image, __version__ as pillow_version

RUNNER = Path('/tmp/sdocx-agent-target/debug/pencil2-mask-control-runner')
SVG = '''<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192"><defs><radialGradient id="r"><stop stop-color="white"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient><mask id="a" mask-type="alpha" maskContentUnits="objectBoundingBox"><rect width="1" height="1" fill="url(#r)"/></mask></defs><rect x="40" y="40" width="2" height="2" fill="black" mask="url(#a)"/></svg>'''


def pixels(alpha):
    assert len(alpha) == 192 * 192
    return [[alpha[y * 192 + x] for x in (40, 41)] for y in (40, 41)]


root = Path(__file__).resolve().parents[2]
for source in ('conformance/pencil2_v14_svg_pdf_runner.rs', 'tools/pencil2-vector-width/Cargo.lock', 'crates/sdocx/src/render.rs'):
    print(source, 'sha256', hashlib.sha256((root / source).read_bytes()).hexdigest())
subprocess.run(['cargo', 'build', '--offline', '--locked', '--manifest-path',
                str(root / 'tools/pencil2-vector-width/Cargo.toml'), '--bin',
                'pencil2-mask-control-runner'], check=True, env={**__import__('os').environ,
                'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0',
                'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0'})
print('Pillow', pillow_version, 'pypdfium2', pypdfium2.PYPDFIUM_INFO)

with tempfile.TemporaryDirectory(prefix='pencil2-radial-mask-') as directory:
    path = Path(directory)
    (path / 'input.svg').write_text(SVG)
    subprocess.run(['resvg', str(path / 'input.svg'), str(path / 'svg.png')], check=True)
    subprocess.run([str(RUNNER), str(path / 'input.svg'), str(path / 'converted.pdf')], check=True)
    svg = Image.open(path / 'svg.png').convert('RGBA')
    assert svg.size == (192, 192)
    with pymupdf.open(path / 'converted.pdf') as pdf:
        assert len(pdf) == 1 and pdf[0].rect == pymupdf.Rect(0, 0, 192, 192)
        objects = [pdf.xref_object(i) for i in range(1, pdf.xref_length())]
        shading = [o for o in objects if '/ShadingType 3' in o and '/DeviceGray' in o]
        assert len(shading) == 1
        assert '/Coords [ .5 .5 0 .5 .5 .5 ]' in shading[0]
        assert '/AntiAlias false' in shading[0]
        function_ref = re.search(r'/Function (\d+) 0 R', shading[0])
        assert function_ref is not None
        function = pdf.xref_object(int(function_ref.group(1)))
        assert all(part in function for part in ('/FunctionType 2', '/C0 [ 1 ]', '/C1 [ 0 ]', '/N 1'))
        assert any('/S /Alpha' in o and '/Type /Mask' in o for o in objects)
        assert any('/BBox [ 40 40 42 42 ]' in o and '/Subtype /Form' in o for o in objects)
        assert not any('/Subtype /Image' in o for o in objects)
        pixmap = pdf[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
        assert (pixmap.width, pixmap.height) == (192, 192)
        mu = pixmap.samples[3::4]
    document = pypdfium2.PdfDocument(str(path / 'converted.pdf'))
    bitmap = document[0].render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
    assert (bitmap.width, bitmap.height) == (192, 192)
    pi = bytes(bitmap.buffer)[3::4]
    print('resvg', subprocess.check_output(['resvg', '--version'], text=True).strip())
    print('MuPDF', pymupdf.VersionBind, 'PDFium', pypdfium2.PDFIUM_INFO)
    print('resvg alpha:', pixels(svg.getchannel('A').tobytes()))
    print('MuPDF alpha:', pixels(mu))
    print('PDFium alpha:', pixels(pi))

    # Repeat the same source mask with a larger aligned rectangle.
    larger = SVG.replace('width="2" height="2" fill="black"', 'width="64" height="64" fill="black"')
    assert larger != SVG
    (path / 'large.svg').write_text(larger)
    subprocess.run(['resvg', str(path / 'large.svg'), str(path / 'large.png')], check=True)
    subprocess.run([str(RUNNER), str(path / 'large.svg'), str(path / 'large.pdf')], check=True)
    large_svg = Image.open(path / 'large.png').convert('RGBA')
    assert large_svg.size == (192, 192)
    reference = large_svg.getchannel('A').tobytes()
    with pymupdf.open(path / 'large.pdf') as large_pdf:
        assert large_pdf[0].rect == pymupdf.Rect(0, 0, 192, 192)
        large_mu = large_pdf[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB).samples[3::4]
    large_pi = bytes(pypdfium2.PdfDocument(str(path / 'large.pdf'))[0].render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0)).buffer)[3::4]
    assert len(reference) == len(large_mu) == len(large_pi) == 192 * 192
    for name, alpha in [('MuPDF', large_mu), ('PDFium', large_pi)]:
        errors = [abs(a - b) for a, b in zip(reference, alpha)]
        print('64x64', name, 'error sum', sum(errors), 'max', max(errors))
