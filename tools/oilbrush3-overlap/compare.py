#!/usr/bin/env python3
"""Compare public PDF exports to the retained native opaque-page frame."""
import hashlib
import json
import sys
from pathlib import Path
import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
path = Path(sys.argv[1])
native = (path / 'native.rgb').read_bytes()
assert len(native) == 1920 * 1080 * 3
result = {'scope': 'Synthetic host-Mesa full opaque caller page, clear=0, native RGB vs public exports at 1x; not device, saved-note, or general lifecycle parity.', 'capture': 'conformance/oilbrush3-overlap/receipt.json', 'mupdf_version': pymupdf.VersionFitz, 'pdfium_version': str(pypdfium2.PDFIUM_INFO), 'pdfs': {}}
for name in ('source.pdf', 'source-svg.pdf'):
    pdf = (path / name).read_bytes()
    with pymupdf.open(stream=pdf, filetype='pdf') as doc:
        pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
        assert (pix.width, pix.height, pix.n) == (1920, 1080, 3)
        mupdf = pix.samples
    with pypdfium2.PdfDocument(pdf) as doc:
        image = doc[0].render(scale=4/3, rev_byteorder=True, fill_color=(255,255,255,255)).to_pil().convert('RGB')
        assert image.size == (1920, 1080)
        pdfium = image.tobytes()
    row = {'sha256': hashlib.sha256(pdf).hexdigest()}
    for reader, actual in [('mupdf', mupdf), ('pdfium', pdfium)]:
        assert len(actual) == len(native)
        differences = [abs(a-b) for a,b in zip(actual,native)]
        row[reader] = {'differing_channels': sum(v != 0 for v in differences), 'maximum_error': max(differences)}
        assert not any(differences), (name, reader, row[reader])
    result['pdfs'][name] = row
result['native_rgb_sha256'] = hashlib.sha256(native).hexdigest()
(root / 'conformance/oilbrush3-overlap/export-receipt.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
