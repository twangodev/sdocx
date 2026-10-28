#!/usr/bin/env python3
"""Compare flat-alpha direct PDF triangles and inspect MuPDF SVG conversion."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import metrics
from triangle_svg_prototype import alpha_at, build, screen_point, subdivide, triangles_for_buffer, uv_derivatives


def digest(data):
    return hashlib.sha256(data).hexdigest()


def direct_pdf(capture, subdivision):
    doc = pymupdf.open()
    page = doc.new_page(width=256, height=256)
    commands = ['q 1 0 0 -1 0 256 cm .9882353 .9882353 .9882353 rg 0 0 256 256 re f .2 .4 .6 rg']
    alphas = {}
    for proof in capture['draw_provenance']:
        buffer = next(a['buffer'] for a in proof['attributes'] if a['index'] == 0)
        for triangle in triangles_for_buffer(capture, buffer):
            derivatives = uv_derivatives(triangle)
            for patch in subdivide(triangle, subdivision):
                centroid = tuple(sum(v[i] for v in patch) / 3 for i in range(5))
                alpha = round(alpha_at(proof['actual_program'], centroid, derivatives), 4)
                if alpha <= 0:
                    continue
                if alpha not in alphas:
                    xref = doc.get_new_xref()
                    doc.update_object(xref, f'<</Type /ExtGState /ca {alpha}>>')
                    alphas[alpha] = xref
                points = [screen_point(v) for v in patch]
                commands.append(f'/A{alphas[alpha]} gs {points[0][0]:.6f} {points[0][1]:.6f} m '
                                f'{points[1][0]:.6f} {points[1][1]:.6f} l '
                                f'{points[2][0]:.6f} {points[2][1]:.6f} l h f')
    commands.append('Q')
    stream = doc.get_new_xref()
    doc.update_object(stream, '<<>>')
    doc.update_stream(stream, '\n'.join(commands).encode())
    doc.xref_set_key(page.xref, 'Contents', f'{stream} 0 R')
    states = ''.join(f'/A{x} {x} 0 R ' for x in alphas.values())
    doc.xref_set_key(page.xref, 'Resources', '<</ExtGState <<' + states + '>>>>')
    return doc.tobytes(deflate=True)


def main():
    old = json.loads((ROOT / 'conformance/montblanc-public-candidate-host-comparison.json').read_text())
    result = {'schema': 'montblanc-direct-pdf-experiment-v1',
              'scope': 'Captured host-Mesa meshes and shader approximations; direct PDF centroid alpha triangles. No production integration or device parity.',
              'metrics': old['metrics'], 'pymupdf_version': pymupdf.VersionBind,
              'mupdf_version': pymupdf.VersionFitz, 'script_sha256': digest(Path(__file__).read_bytes()),
              'prototype_sha256': digest((ROOT / 'tools/montblanc/triangle_svg_prototype.py').read_bytes()),
              'cases': {}}
    with tempfile.TemporaryDirectory(prefix='montblanc-direct-pdf-') as directory:
        tmp = Path(directory)
        for name, expected in old['cases'].items():
            raw = gzip.decompress((ROOT / f'tools/montblanc/captures/{name}.json.gz').read_bytes())
            if digest(raw) != expected['capture_sha256']:
                raise ValueError(f'{name}: capture hash mismatch')
            capture = json.loads(raw)
            frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
            if digest(frame) != capture['post_readback']['sha256']:
                raise ValueError(f'{name}: framebuffer hash mismatch')
            native = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
            native.alpha_composite(Image.frombytes('RGBA', (256, 256), frame).transpose(Image.Transpose.FLIP_TOP_BOTTOM))
            native = native.convert('RGB').tobytes()
            source = tmp / f'{name}.json'
            source.write_bytes(raw)
            svg = tmp / f'{name}.svg'
            build(source, svg, 4)
            text = svg.read_text().replace('<defs>', '<rect width="256" height="256" fill="#fcfcfc"/><defs>', 1)
            with pymupdf.open(stream=text.encode(), filetype='svg') as document:
                converted = document.convert_to_pdf()
            with pymupdf.open(stream=converted, filetype='pdf') as document:
                content = document[0].read_contents()
                resources = document.xref_object(document[0].xref)
                conversion = {'metrics': metrics(native, document[0].get_pixmap(alpha=False).samples),
                              'svg_linear_gradients': text.count('<linearGradient '),
                              'pdf_shading_resources': '/Shading' in resources,
                              'pdf_shading_operators': content.count(b' sh'),
                              'opaque_black_fill_operators': content.count(b'0 0 0 rg')}
            if conversion['metrics'] != expected['svg_to_pdf']:
                raise ValueError(f'{name}: converted PDF metrics differ from prior receipt')
            direct = {}
            for subdivision in (4, 8):
                pdf = direct_pdf(capture, subdivision)
                with pymupdf.open(stream=pdf, filetype='pdf') as document:
                    direct[str(subdivision)] = {'metrics': metrics(native, document[0].get_pixmap(alpha=False).samples),
                                                'pdf_bytes': len(pdf)}
            result['cases'][name] = {'capture_sha256': digest(raw), 'svg_to_pdf': conversion, 'direct_pdf': direct}
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
