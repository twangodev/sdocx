#!/usr/bin/env python3
"""Compare source-generated public PDF against the opaque native target."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

if not __debug__:
    raise RuntimeError("run without Python optimization")

import pymupdf
import pypdfium2

here = Path(__file__).resolve().parent
capture = json.loads((here / 'native-opaque.json').read_text())
frame = zlib.decompress(base64.b64decode(capture['second']['data_base64']))
assert len(frame) == 256 * 256 * 4
assert hashlib.sha256(frame).hexdigest() == capture['second']['sha256']
target = b''.join(reversed([frame[i:i+1024] for i in range(0, len(frame), 1024)]))
pdf_path = Path(sys.argv[1])
with pymupdf.open(pdf_path) as doc:
    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=True)
    assert (pix.width, pix.height, pix.n) == (256, 256, 4)
    mupdf = pix.samples
with pypdfium2.PdfDocument(pdf_path) as doc:
    pix = doc[0].render(scale=4/3, rev_byteorder=True, fill_color=(0,0,0,0)).to_pil().convert('RGBA')
    assert pix.size == (256, 256)
    pdfium = pix.tobytes()
def metrics(actual):
    errors = [abs(a-b) for a,b in zip(actual,target,strict=True)]
    return {'sum':sum(errors), 'max':max(errors), 'differing_channels':sum(x != 0 for x in errors)}
def crop(pixels):
    return b''.join(pixels[(y*256+105)*4:(y*256+145)*4] for y in range(105,145))
def region_metrics(actual):
    errors = [abs(a-b) for a,b in zip(crop(actual),crop(target),strict=True)]
    return {'sum':sum(errors), 'max':max(errors), 'differing_channels':sum(x != 0 for x in errors)}
result = {'pdf_sha256':hashlib.sha256(pdf_path.read_bytes()).hexdigest(), 'native_sha256': capture['second']['sha256'], 'pymupdf':pymupdf.VersionBind, 'pdfium':str(pypdfium2.PDFIUM_INFO), 'mupdf':metrics(mupdf),'pdfium_result':metrics(pdfium),'ink_region_xy_105_145':{'mupdf':region_metrics(mupdf),'pdfium':region_metrics(pdfium)},'corner_pixels':{'native':list(target[:4]),'mupdf':list(mupdf[:4]),'pdfium':list(pdfium[:4])}}
assert result == json.loads((here / 'pdf-receipt.json').read_text()), 'receipt mismatch'
print(json.dumps(result, indent=2))
