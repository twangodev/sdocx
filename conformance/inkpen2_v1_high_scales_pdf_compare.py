"""Compare captured public PDFs to host frames (requires PyMuPDF).

Run inkpen2_v1_distinct_native_shapes_public_svg with SDOCX_V1_SHAPE_PDF_CAPTURE
set to an empty directory, then pass that directory and `--scale 2` here to
reproduce the supported 2x comparison. The 0.5x candidate was captured with
the admission temporarily enabled for measurement.
"""
import base64
import json
from pathlib import Path
import sys
import zlib
import pymupdf

root = Path(__file__).resolve().parents[1]
if len(sys.argv) not in (2, 4) or (len(sys.argv) == 4 and (sys.argv[2] != '--scale' or sys.argv[3] not in ('0.5', '2'))):
    raise SystemExit('usage: inkpen2_v1_high_scales_pdf_compare.py PDF_DIR [--scale 0.5|2]')
cases = json.loads((root / 'conformance/inkpen2-v1-high-scales-native.json').read_text())['cases']
results = {}
for name, scales in cases.items():
    for scale, capture in scales.items():
        if len(sys.argv) == 4 and sys.argv[2] == '--scale' and scale != sys.argv[3]:
            continue
        native = zlib.decompress(base64.b64decode(capture['rgba_zlib_base64']))[3::4]
        errors = {}
        for kind in ('candidate', 'generic'):
            with pymupdf.open(Path(sys.argv[1]) / f'v1-{name}-{scale}-{kind}.pdf') as document:
                factor = 4 / 3 * float(scale)
                pixmap = document[0].get_pixmap(matrix=pymupdf.Matrix(factor, factor), alpha=True)
                assert pixmap.n == 4 and len(pixmap.samples[3::4]) == len(native)
                errors[kind] = sum(abs(a - b) for a, b in zip(pixmap.samples[3::4], native))
        results.setdefault(name, {})[scale] = errors
receipt = {'scope': 'Alpha-byte absolute-error sums against transparent 1440x2560 host Mesa frames. Synthetic controls only; 0.5x candidate was generated with admission temporarily enabled.', 'pymupdf_version': pymupdf.VersionBind, 'cases': results}
if len(sys.argv) == 4 and sys.argv[2] == '--scale':
    print(json.dumps(receipt, indent=2))
else:
    (root / 'conformance/inkpen2-v1-high-scales-pdf-comparison.json').write_text(json.dumps(receipt, indent=2) + '\n')
