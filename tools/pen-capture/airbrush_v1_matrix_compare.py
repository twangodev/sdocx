#!/usr/bin/env python3
"""Compare PDFs from saved_v1_tool_matrix_public_exports against native RGBA."""
import base64
import json
from pathlib import Path
import sys
import zlib
import pymupdf

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/pen-capture/airbrush_v1_matrix_receipt.json').read_text())
directory = Path(sys.argv[1])
results = {}
for name, case in receipt['cases'].items():
    raw = zlib.decompress(base64.b64decode(case['frame']['data_base64']))
    rows = [raw[i:i + 1024] for i in range(0, len(raw), 1024)]
    native = b''.join(reversed(rows))
    results[name] = {}
    for variant in ('candidate', 'generic'):
        doc = pymupdf.open(directory / f'{name}-{variant}.pdf')
        pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(96 / 72, 96 / 72), alpha=True)
        assert (pix.width, pix.height, pix.n) == (256, 256, 4)
        differences = [abs(a - b) for a, b in zip(pix.samples, native)]
        results[name][variant] = {'sum_absolute_channel_error': sum(differences), 'max_channel_error': max(differences)}
print(json.dumps({'pymupdf_version': pymupdf.VersionBind, 'scope': 'Host Mesa versus transparent direct PDF raster at 96 dpi; not device or cross-reader parity.', 'results': results}, indent=2))
