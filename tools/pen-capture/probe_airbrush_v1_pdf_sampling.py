#!/usr/bin/env python3
"""Probe one captured saved-v1 PDF; generate it as in compare_airbrush_v1_pdf.py."""
import hashlib
import json
import sys

import pymupdf

path = sys.argv[1]
assert hashlib.sha256(open(path, 'rb').read()).hexdigest() == 'c4cae99811523ed338c094262db172e7ceb044455f87b0660fbbfbe452c4873a'
reference = [1, 14, 1, 22, 52, 12, 1, 23, 1]

def sample(page, scale):
    pix = page.get_pixmap(matrix=pymupdf.Matrix(96 / 72 * scale, 96 / 72 * scale), alpha=True)
    samples = pix.samples
    values = [samples[((y * scale + scale // 2) * pix.width + x * scale + scale // 2) * 4 + 3]
              for y in range(127, 130) for x in range(127, 130)]
    return {'alpha': values, 'absolute_error': sum(abs(a - b) for a, b in zip(values, reference))}

doc = pymupdf.open(path)
result = {'center_samples': {str(scale): sample(doc[0], scale) for scale in (1, 2, 4, 8)}}
# Capture-specific object numbers: use both unmasked stamp image forms directly.
doc.xref_set_key(72, 'XObject/t0', '234 0 R')
doc.xref_set_key(72, 'XObject/t1', '256 0 R')
doc.update_stream(230, b'q 0.75 0 0 -0.75 0 192 cm /t0 Do /t1 Do Q')
result['unmasked_1x'] = sample(doc[0], 1)
print(json.dumps(result, indent=2))
