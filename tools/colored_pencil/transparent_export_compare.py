"""Measure transparent native canvas frames through PNG, SVG, and PDF readers."""
import base64
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import zlib

from PIL import Image
import pymupdf
import pypdfium2

ROOT = Path(__file__).resolve().parents[2]
NATIVE = ROOT / 'conformance/colored-pencil-v4-canvas-native.json'
parser = argparse.ArgumentParser()
parser.add_argument('--artifacts', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
DIRECTORY = args.artifacts


def sha(data):
    return hashlib.sha256(data).hexdigest()


def metrics(actual, expected):
    assert len(actual) == len(expected)
    errors = [abs(a - b) for a, b in zip(actual, expected)]
    return {'different_channels': sum(e != 0 for e in errors),
            'absolute_sum': sum(errors), 'maximum': max(errors)}


def straight(pixels):
    out = bytearray()
    for r, g, b, a in zip(*[iter(pixels)] * 4):
        out.extend([min(255, (v * 255 + a // 2) // a) if a else 0 for v in (r, g, b)])
        out.append(a)
    return bytes(out)


def readers(path, size):
    with pymupdf.open(path) as document:
        image = document[0].get_pixmap(alpha=True, colorspace=pymupdf.csRGB)
        assert (image.width, image.height, image.n) == (*size, 4)
        mupdf = bytes(image.samples)
    with pypdfium2.PdfDocument(str(path)) as document:
        page = document[0]
        bitmap = page.render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0))
        image = bitmap.to_pil()
        assert image.size == tuple(size)
        pdfium = image.convert('RGBA').tobytes()
        bitmap.close()
        page.close()
    return mupdf, pdfium


cases = json.loads(NATIVE.read_text())['cases']
results = {}
for name, case in cases.items():
    if case['background'][3] == 255:
        continue
    width, height = case['frame']
    native = (DIRECTORY / (name + '.rgba')).read_bytes()
    bottom = bytearray(bytes(case['background']) * (width * height))
    sparse = zlib.decompress(base64.b64decode(case['sparse_zlib_base64']))
    for offset in range(0, len(sparse), 8):
        index = struct.unpack_from('<I', sparse, offset)[0]
        bottom[index * 4:index * 4 + 4] = sparse[offset + 4:offset + 8]
    assert sha(bottom) == case['frame_sha256']
    assert native == b''.join(bottom[y * width * 4:(y + 1) * width * 4]
                              for y in reversed(range(height))), name
    expected_straight = straight(native)
    png_bytes = (DIRECTORY / (name + '.png')).read_bytes()
    with Image.open(DIRECTORY / (name + '.png')) as image:
        assert image.size == (width, height), name
        png = image.convert('RGBA').tobytes()
    assert png == expected_straight, name
    svg = (DIRECTORY / (name + '.svg')).read_text()
    match = re.search(r'href="data:image/png;base64,([^"]+)"', svg)
    assert match and base64.b64decode(match.group(1)) == png_bytes, name
    outcomes = {'sha256': {'rgba': sha(native), 'png': sha(png_bytes),
                           'svg': sha(svg.encode())}}
    for route in ('direct', 'svg'):
        pdf_path = DIRECTORY / (name + '.' + route + '.pdf')
        outcomes['sha256'][route + '_pdf'] = sha(pdf_path.read_bytes())
        mupdf, pdfium = readers(pdf_path, [width, height])
        outcomes[route + '_mupdf_vs_native_premultiplied'] = metrics(mupdf, native)
        outcomes[route + '_pdfium_vs_png_straight'] = metrics(pdfium, png)
        assert pdfium == png, (name, route, outcomes)
    results[name] = outcomes
assert len(results) == 64
receipt = {
    'schema': 'sdocx-colored-pencil-transparent-export-v1',
    'capture': {'script': str(Path(__file__).relative_to(ROOT)), 'script_sha256': sha(Path(__file__).read_bytes()),
                'native_receipt_sha256': sha(NATIVE.read_bytes()),
                'source_sha256': {str(p): sha((ROOT / p).read_bytes()) for p in (
                    'crates/sdocx/tests/colored_pencil_raster.rs',
                    'crates/sdocx/src/ink_raster.rs',
                    'crates/sdocx/src/ink/colored_pencil/raster.rs',
                    'Cargo.lock')}},
    'frames': len(results), 'dpi': 72,
    'readers': {'mupdf': pymupdf.VersionBind, 'pdfium': str(pypdfium2.PDFIUM_INFO)},
    'scope': 'Same synthetic native clear and translucent-prior frames as the canvas receipt. PNG matches the defined premultiplied-to-straight conversion and SVG embeds that PNG. Both PDFium PDF routes match straight PNG exactly. MuPDF returns premultiplied samples; its differences from the native premultiplied frame are reported, not claimed exact. Source hashes identify selected inputs, not a complete build provenance or proof that artifacts came from those sources. Intrinsic size only; not saved-note or device parity.',
    'results': results,
}
with args.output.open('x') as output:
    output.write(json.dumps(receipt, indent=2) + '\n')
