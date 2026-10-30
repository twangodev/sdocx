#!/usr/bin/env python3
"""Compare the direct document PDF and public SVG for a synthetic radial line."""
import os
import subprocess
import tempfile
from pathlib import Path
from xml.etree import ElementTree

import pymupdf
import pypdfium2
from PIL import Image

runner = Path(os.environ.get('SDOCX_RADIAL_CONTROL_RUNNER', '/tmp/sdocx-agent-target/debug/sdocx-direct-radial-shape-control'))
print('resvg', subprocess.check_output(['resvg', '--version'], text=True).strip())
print('MuPDF', pymupdf.VersionBind, 'PDFium', pypdfium2.PDFIUM_INFO)
with tempfile.TemporaryDirectory(prefix='sdocx-direct-radial-') as directory:
    path = Path(directory)
    for size in (2, 64):
        svg, pdf, png = (path / f'out-{size}.{ext}' for ext in ('svg', 'pdf', 'png'))
        subprocess.run([str(runner), str(size), str(svg), str(pdf)], check=True)
        tree = ElementTree.parse(svg)
        ns = {'s': 'http://www.w3.org/2000/svg'}
        gradients = tree.findall('.//s:radialGradient', ns)
        assert len(gradients) == 1
        gradient = gradients[0]
        assert gradient.attrib['gradientUnits'] == 'userSpaceOnUse'
        assert float(gradient.attrib['cx']) == float(gradient.attrib['cy']) == 40 + size / 2
        assert abs(float(gradient.attrib['r']) - size * 2 ** 0.5 / 2) < 1e-5
        assert [(stop.attrib['offset'], stop.attrib['stop-color'], stop.attrib['stop-opacity'])
                for stop in gradient.findall('s:stop', ns)] == [
                    ('0', '#000000', '0'), ('0.25', '#000000', '0'),
                    ('0.25', '#000000', '1'), ('1', '#000000', '1')]
        line = tree.find('.//s:line', ns)
        assert line is not None and float(line.attrib['x1']) == 40 and float(line.attrib['x2']) == 40 + size
        assert float(line.attrib['y1']) == float(line.attrib['y2']) == 40 + size / 2
        groups = tree.findall('.//s:g', ns)
        assert any(group.attrib.get('stroke') == f"url(#{gradient.attrib['id']})"
                   and float(group.attrib['stroke-width']) == size for group in groups)
        subprocess.run(['resvg', str(svg), str(png)], check=True)
        reference = Image.open(png).convert('RGB').tobytes()[::3]
        with pymupdf.open(pdf) as doc:
            objects = [doc.xref_object(i) for i in range(1, doc.xref_length())]
            print(size, 'radial shadings', sum('/ShadingType 3' in x for x in objects),
                  'soft masks', sum('/SMask' in x for x in objects),
                  'images', sum('/Subtype /Image' in x for x in objects))
            mu = doc[0].get_pixmap(colorspace=pymupdf.csRGB).samples[::3]
        with pypdfium2.PdfDocument(str(pdf)) as doc:
            pi = doc[0].render(scale=1, rev_byteorder=True).to_pil().convert('RGB').tobytes()[::3]
        assert len(reference) == len(mu) == len(pi) == 192 * 192
        for name, frame in [('MuPDF', mu), ('PDFium', pi)]:
            errors = [abs(a - b) for a, b in zip(reference, frame)]
            print(name, 'error sum', sum(errors), 'max', max(errors),
                  'at 40,40', [list(frame[y * 192 + 40:y * 192 + 42]) for y in (40, 41)])
        print('resvg at 40,40', [list(reference[y * 192 + 40:y * 192 + 42]) for y in (40, 41)])
