#!/usr/bin/env python3
"""Compare source-reconstructed public canvas exports with the native V17 frame."""
import argparse
import hashlib
import json
from pathlib import Path

import pymupdf
import pypdfium2

parser = argparse.ArgumentParser()
parser.add_argument('directory', type=Path)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
path = args.directory
native = (path / 'native.rgb').read_bytes()
svg = (path / 'source-svg.rgba').read_bytes()
assert len(native) == 1920 * 1080 * 3
assert len(svg) == 1920 * 1080 * 4

def metrics(actual):
    assert len(actual) == len(native)
    sums = [0, 0, 0]
    maxima = [0, 0, 0]
    differing = 0
    for i in range(0, len(native), 3):
        delta = [abs(actual[i + c] - native[i + c]) for c in range(3)]
        differing += any(delta)
        for c in range(3):
            sums[c] += delta[c]
            maxima[c] = max(maxima[c], delta[c])
    return {'differing_pixels': differing, 'rgb_absolute_error_sum': sums, 'rgb_maximum_error': maxima}

result = {
    'schema': 'sdocx.oilbrush3.source-public-canvas-export.v1',
    'scope': 'Synthetic V17 source stroke, host-Mesa model, white RGB, 1920x1080 at 1x. PDF rasterized at 96 dpi.',
    'native_capture_provenance': 'conformance/oilbrush3-v17-native-paint/provenance.json',
    'svg_vs_native': metrics(bytes(v for i, v in enumerate(svg) if i % 4 != 3)),
    'mupdf_version': pymupdf.VersionFitz,
    'pdfium_version': str(pypdfium2.PDFIUM_INFO),
    'pdfs': {},
    'limitations': 'Explicit cold host model; one synthetic export control, host arithmetic and explicit initialization and textures. No saved-note, device, lifecycle, scaling or general parity claim.',
}
for name in ('source.pdf', 'source-svg.pdf'):
    pdf = (path / name).read_bytes()
    with pymupdf.open(stream=pdf, filetype='pdf') as doc:
        pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
        assert (pix.width, pix.height, pix.n) == (1920, 1080, 3)
        mupdf = pix.samples
    with pypdfium2.PdfDocument(pdf) as doc:
        image = doc[0].render(scale=4/3, rev_byteorder=True, fill_color=(255, 255, 255, 255)).to_pil().convert('RGB')
        assert image.size == (1920, 1080)
        pdfium = image.tobytes()
    result['pdfs'][name] = {'sha256': hashlib.sha256(pdf).hexdigest(), 'mupdf': metrics(mupdf), 'pdfium': metrics(pdfium)}
result['inputs_sha256'] = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in (
    Path(__file__).resolve(), root / 'conformance/oilbrush3-native.json',
    root / 'conformance/oilbrush3-v17-native-paint/provenance.json',
    root / 'crates/sdocx/src/ink/oilbrush3/paint_conformance.rs',
    root / 'crates/sdocx/src/ink/oilbrush3/paint.rs',
    root / 'crates/sdocx/src/ink/oilbrush3/p3-vbo-6x4-f32le.bin',
    root / 'crates/sdocx/src/ink/oilbrush3.rs',
    root / 'crates/sdocx/src/ink.rs',
    root / 'crates/sdocx/src/ink_raster.rs',
)}
if args.check:
    expected = json.loads((root / 'conformance/oilbrush3-source-public-canvas-export.json').read_text())
    if result != expected:
        raise SystemExit('Comparison differs from retained receipt')
    print('Source public canvas export matches retained receipt')
else:
    print(json.dumps(result, indent=2))
