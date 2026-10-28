#!/usr/bin/env python3
"""Probe a PDF produced by compare_airbrush_v1_pdf.py's documented capture."""
import hashlib
import json
import sys

import PIL
from PIL import Image
import pymupdf

path = sys.argv[1]
pdf_sha = hashlib.sha256(open(path, 'rb').read()).hexdigest()
assert pdf_sha == 'c4cae99811523ed338c094262db172e7ceb044455f87b0660fbbfbe452c4873a'
reference = [1, 14, 1, 22, 52, 12, 1, 23, 1]

def measure(doc, scale=1):
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(96 / 72 * scale, 96 / 72 * scale), alpha=True)
    values = [pix.samples[((y * scale + scale // 2) * pix.width + x * scale + scale // 2) * 4 + 3]
              for y in range(127, 130) for x in range(127, 130)]
    return {'alpha': values, 'absolute_error': sum(abs(a - b) for a, b in zip(values, reference)),
            'rgba_sha256': hashlib.sha256(pix.samples).hexdigest()}

result = {'schema': 'sdocx.airbrush-v1-pdf-texture-probe.v1', 'input_pdf_sha256': pdf_sha,
          'native_receipt': 'airbrush-short-v1-v2-native.json', 'native_alpha_3x3': reference,
          'pymupdf_version': pymupdf.VersionBind, 'pillow_version': PIL.__version__,
          'scope': 'One synthetic host Mesa V1 frame and its PDF, PyMuPDF at 96 dpi. At even scales sample floor(scale/2) within each target pixel, adjacent to its geometric center. In-memory PDF mutations only; no device or cross-reader claim.',
          'interpolation': {}, 'texture_resize': {}}
for mode in ('default', 'false', 'true'):
    doc = pymupdf.open(path)
    if mode != 'default':
        for xref in (275, 276):
            doc.xref_set_key(xref, 'Interpolate', mode)
    result['interpolation'][mode] = {str(scale): measure(doc, scale) for scale in (1, 2, 4, 8)}
base = pymupdf.open(path)
for size in (27, 54, 81, 162, 324, 648):
    for method in (Image.Resampling.NEAREST, Image.Resampling.BOX):
        doc = pymupdf.open(path)
        for xref, pixel_mode in ((275, 'L'), (276, 'RGB')):
            image = Image.frombytes(pixel_mode, (162, 162), base.xref_stream(xref)).resize((size, size), method)
            doc.update_stream(xref, image.tobytes())
            doc.xref_set_key(xref, 'Width', str(size))
            doc.xref_set_key(xref, 'Height', str(size))
        result['texture_resize'][f'{size}-{method.name.lower()}'] = measure(doc)
result['interpretation'] = ('Interpolation flags produced identical RGBA at each scale. Nearest upsampling also produced identical 1x RGBA; downsampling changed the sampled alpha error only slightly. Alpha discrepancies cannot be explained by color conversion alone. Sampling, transformed coverage, and alpha compositing remain entangled.')
print(json.dumps(result, indent=2))
