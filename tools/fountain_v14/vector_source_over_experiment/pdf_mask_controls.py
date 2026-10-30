"""Test PDF mask group color spaces and shading antialias flags against native prefixes."""
import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

import pymupdf
import pypdfium2
from PIL import Image

sha = lambda data: hashlib.sha256(data).hexdigest()
base = Path(__file__).parent
frames_bytes = (base / 'native-prefix-frames.json').read_bytes()
frames = json.loads(frames_bytes)['frames']
exports = Path(sys.argv[1])
results = {}
for name, expected, linear, radial in [
    ('canonical_white_0.direct.pdf', '9e925a3714924d049dfc28e9f117970e51e8f33f9b32aa54767d9f58e68f13d7', [121], []),
    ('scale1_white_0.direct.pdf', 'd5cbc2a848c88df5b5ecff7239a5c8ec38067c288cde3455ca8840c2d325b2d4', [495], list(range(497, 557))),
]:
    source = (exports / name).read_bytes()
    assert sha(source) == expected
    cases = {}
    for variant in ('baseline', 'linear_gray', 'radial_gray', 'both_gray', 'shading_antialias'):
        pdf = pymupdf.open(stream=source, filetype='pdf')
        page = pdf[0]
        assert not page.get_images()
        content_ref = page.get_contents()[0]
        parts = pdf.xref_stream(content_ref).split(b'\nQ\n')
        assert len(parts) == 103
        indices = {'baseline': [], 'linear_gray': linear, 'radial_gray': radial, 'both_gray': linear + radial}.get(variant, [])
        for i in indices:
            assert pdf.xref_get_key(i, 'Group/CS') == ('name', '/DeviceRGB')
            pdf.xref_set_key(i, 'Group/CS', '/DeviceGray')
        if variant == 'shading_antialias':
            indices = []
            for i in range(1, pdf.xref_length()):
                if pdf.xref_get_key(i, 'AntiAlias') == ('bool', 'false'):
                    pdf.xref_set_key(i, 'AntiAlias', 'true')
                    indices.append(i)
            assert indices
        prefixes = {}
        for count in (1, 5, 25, 101):
            pdf.update_stream(content_ref, b'\nQ\n'.join(parts[:count + 1]) + b'\nQ\nQ\n')
            case = frames[str(count)]
            raw = zlib.decompress(base64.b64decode(case['rgba_zlib_base64']))
            assert sha(raw) == case['sha256']
            native = b''.join(raw[y * 512:(y + 1) * 512] for y in reversed(range(256)))
            pix = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=False)
            mupdf = Image.frombytes('RGB', (pix.width, pix.height), pix.samples).convert('RGBA')
            reader = pypdfium2.PdfDocument(pdf.tobytes())
            pdfium = reader[0].render(scale=4 / 3).to_pil().convert('RGBA')
            output = {}
            for label, image in [('mupdf', mupdf), ('pdfium', pdfium)]:
                assert image.size == (208, 328)
                crop = image.crop((80, 72, 208, 328)).tobytes()
                output[label] = {'absolute_rgba_error': sum(abs(a - b) for a, b in zip(native, crop)), 'crop_sha256': sha(crop)}
            prefixes[str(count)] = output
            reader.close()
        cases[variant] = {'modified_objects': indices, 'prefixes': prefixes}
        pdf.close()
    results[name] = {'pdf_sha256': sha(source), 'cases': cases}
receipt = {'scope': 'Candidate PDF group colorspace and shading antialias interventions against native source-3 prefixes; not native PDF or device parity.', 'frames_sha256': sha(frames_bytes), 'script_sha256': sha(Path(__file__).read_bytes()), 'pymupdf': pymupdf.VersionBind, 'pdfium': str(pypdfium2.PDFIUM_INFO), 'pypdfium2': str(pypdfium2.PYPDFIUM_INFO), 'results': results}
Path(sys.argv[2]).write_text(json.dumps(receipt, indent=2) + '\n')
