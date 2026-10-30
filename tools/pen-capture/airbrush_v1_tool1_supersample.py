#!/usr/bin/env python3
"""Compare pinned tool-1 PDFs with MuPDF supersampling and box reduction."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

import pymupdf
from PIL import Image, __version__ as pillow_version

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/pen-capture/airbrush_v1_tool1_receipt.json').read_text())
pdf_dir = Path(sys.argv[1])
assert pymupdf.VersionBind == '1.28.2'
assert pillow_version == '12.1.1'
expected = {
    ('zigzag-1', 'candidate'): [(6952, 2969), (5710, 2443), (5132, 2165)],
    ('zigzag-1', 'generic'): [(21348, 13958), (21760, 14325), (21800, 14370)],
    ('reverse-1', 'candidate'): [(2216, 931), (1243, 523), (940, 394)],
    ('reverse-1', 'generic'): [(1558, 324), (1544, 349), (1516, 329)],
}
for name, case in receipt['cases'].items():
    raw = zlib.decompress(base64.b64decode(case['frame']['data_base64']))
    assert hashlib.sha256(raw).hexdigest() == case['frame']['sha256'], name
    native = b''.join(reversed([raw[i:i + 1024] for i in range(0, len(raw), 1024)]))
    for variant in ('candidate', 'generic'):
        filename = f'{name}-{variant}.pdf'
        path = pdf_dir / filename
        assert hashlib.sha256(path.read_bytes()).hexdigest() == receipt['pdf_sha256'][filename]
        with pymupdf.open(path) as doc:
            for index, scale in enumerate((1, 2, 4)):
                pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3 * scale, 4 / 3 * scale), alpha=True)
                image = Image.frombytes('RGBA', (pix.width, pix.height), pix.samples)
                if scale > 1:
                    image = Image.merge('RGBA', tuple(channel.resize((256, 256), Image.Resampling.BOX) for channel in image.split()))
                actual = image.tobytes()
                rgba = sum(abs(a - b) for a, b in zip(actual, native, strict=True))
                alpha = sum(abs(a - b) for a, b in zip(actual[3::4], native[3::4], strict=True))
                assert (rgba, alpha) == expected[(name, variant)][index], (name, variant, scale, rgba, alpha)
                print(f'{name} {variant} {scale}x: RGBA {rgba}, alpha {alpha}')
