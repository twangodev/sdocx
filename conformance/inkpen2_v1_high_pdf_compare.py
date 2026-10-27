"""Compare PDFs emitted by inkpen2_v1_distinct_native_shapes_public_svg to host frames.

Run the test with SDOCX_V1_SHAPE_PDF_CAPTURE set to an empty directory, then
invoke this script with that directory. Requires PyMuPDF.
"""
import base64
import json
from pathlib import Path
import sys
import zlib

import pymupdf

root = Path(__file__).resolve().parents[1]
pdf_dir = Path(sys.argv[1])
cases = json.loads((root / 'conformance/inkpen2-v1-screen-paint-native.json').read_text())['cases']
results = {}
for name, profiles in cases.items():
    native = zlib.decompress(base64.b64decode(profiles['1440x2560']['rgba_zlib_base64']))
    errors = {}
    for kind in ('high', 'generic'):
        with pymupdf.open(pdf_dir / f'v1-{name}-{kind}.pdf') as document:
            pixmap = document[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True)
            assert (pixmap.width, pixmap.height, pixmap.n) == (128, 96, 4)
            errors[kind] = sum(abs(a - b) for a, b in zip(pixmap.samples[3::4], native[3::4]))
    assert errors['high'] <= errors['generic'], (name, errors)
    results[name] = errors
receipt = {'scope': 'Alpha-byte absolute-error sums against 1440x2560 host Mesa frames; transparent PDF rasterized at 4/3 pixels per point. Synthetic inputs only.',
           'pymupdf_version': pymupdf.VersionBind, 'cases': results}
(root / 'conformance/inkpen2-v1-high-pdf-comparison.json').write_text(json.dumps(receipt, indent=2) + '\n')
