#!/usr/bin/env python3
"""Replay the captured 512-display pattern upload with the v173 source mask."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zlib

from PIL import Image
import pymupdf

here = Path(__file__).resolve().parent
root = Path(sys.argv[1])
assert pymupdf.VersionBind == '1.28.2'
digest = lambda data: hashlib.sha256(data).hexdigest()
raw_log = gzip.decompress((here / 'identity-screen-state-512.log.gz').read_bytes())
assert digest(raw_log) == '026bb206b55207e9b3e358088068762b91214f1f3e7aa826744d24a12f0cd31b'
text = raw_log.decode()
capture, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
event = capture['mesa_texture_uploads']['events'][6]
assert (event['api'], event['width'], event['height'], event['format'], event['type']) == ('glTexSubImage2D', 128, 128, 6408, 5121)
assert event['payload']['source_kind'] == 'owned_guest_client_copy'
texture = zlib.decompress(base64.b64decode(event['payload']['data_base64'], validate=True))
assert digest(texture) == event['payload']['sha256'] == 'f290e56addf62df35c7fbd4bfb6bddaeccc047ab382c52aa9766cafb38b7d292'
(root / 'pattern.z').write_bytes(zlib.compress(texture))
mask = root / 'old.mask'
assert digest(mask.read_bytes()) == '7bf0e2bd0dcf16e2d5805101727e13cacde38048f39165b56a83c7d9cb40ed20'
source = (here / 'v173-vector.py').read_text()
old = "Path(__file__).parent / 'pattern01-native.rgba.zlib'"
assert source.count(old) == source.count('sample(pattern, 64, 64,') == 1
source = source.replace(old, f'Path({str(root / "pattern.z")!r})').replace('sample(pattern, 64, 64,', 'sample(pattern, 128, 128,')
(root / 'vector.py').write_text(source)
svg, pdf, png = (root / name for name in ('texture.svg', 'texture.pdf', 'texture.png'))
subprocess.run([sys.executable, str(root / 'vector.py'), str(mask), str(svg), '77', '59'], check=True)
assert digest(svg.read_bytes()) == '32273ff1c137136ca03644275c14fd3feef77615d4d8da315a8103384f3c3a12'
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == '0.45.1'
subprocess.run(['resvg', str(svg), str(png)], check=True)
subprocess.run(['/tmp/sdocx-agent-target/debug/svg_pdf', str(svg), str(pdf), str(root / 'pdf.json'), '256', '256'], check=True)
assert json.loads((root / 'pdf.json').read_text())['image_xobjects'] == 0
assert digest(pdf.read_bytes()) == 'c3dabdf2a726709c3955893a30014e2f2d70a7ee59ca99a76a9f998a7d308556'
native = zlib.decompress((here / 'identity-screen-512-frame.rgba.zlib').read_bytes())
assert digest(native) == json.loads((here / 'identity-screen-512-receipt.json').read_text())['frame_sha256'] == '5cb0a0ca907f14dc9143cc48e1bd5dfdfc06c026be42ea127e4c94ec318ea2bc'
native = b''.join(native[y * 2048:y * 2048 + 1024] for y in range(256))
data = Image.open(png).convert('RGBA').tobytes()
svg_rgba = bytes(round(v * data[i - i % 4 + 3] / 255) if i % 4 < 3 else v for i, v in enumerate(data))
with pymupdf.open(pdf) as doc:
    pdf_rgba = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True).samples
for name, data, expected in [('svg', svg_rgba, (28, 1, 28)), ('svg-derived PDF', pdf_rgba, (453, 2, 446))]:
    diff = [abs(a - b) for a, b in zip(data, native, strict=True)]
    result = (sum(diff), max(diff), sum(v != 0 for v in diff))
    assert result == expected, (name, result)
    print(name, 'L1, max, different channels:', result)

# The optional APK resource is a source input, not a captured upload.
asset = Path(os.environ.get('SDOCX_TAPE_PATTERN_SVG', here.parents[1] / 'scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg'))
if asset.exists():
    assert digest(asset.read_bytes()) == '9f8b208a01db838235f795226acd6e68aa42e609b06c6e171a29447f8b68322b'
    asset_png = root / 'asset128.png'
    subprocess.run(['resvg', '--width', '128', '--height', '128', str(asset), str(asset_png)], check=True)
    asset_texture = Image.open(asset_png).convert('RGBA').tobytes()
    diff = [abs(a - b) for a, b in zip(asset_texture, texture, strict=True)]
    assert (sum(diff), max(diff), sum(v != 0 for v in diff)) == (58560, 33, 3072)
    (root / 'pattern.z').write_bytes(zlib.compress(asset_texture))
    subprocess.run([sys.executable, str(root / 'vector.py'), str(mask), str(svg), '77', '59'], check=True)
    assert digest(svg.read_bytes()) == '8aaac7897090341195852590d40e10d88478c6effef36c46c4f31df9a6251372'
    subprocess.run(['resvg', str(svg), str(png)], check=True)
    subprocess.run(['/tmp/sdocx-agent-target/debug/svg_pdf', str(svg), str(pdf), str(root / 'pdf.json'), '256', '256'], check=True)
    assert json.loads((root / 'pdf.json').read_text())['image_xobjects'] == 0
    assert digest(pdf.read_bytes()) == 'd690716ccf16266747039f11d69166376e122d69fafdbf74d466a09e26340c57'
    data = Image.open(png).convert('RGBA').tobytes()
    svg_rgba = bytes(round(v * data[i - i % 4 + 3] / 255) if i % 4 < 3 else v for i, v in enumerate(data))
    with pymupdf.open(pdf) as doc:
        pdf_rgba = doc[0].get_pixmap(matrix=pymupdf.Matrix(4 / 3, 4 / 3), alpha=True).samples
    for name, data, expected in [('asset SVG', svg_rgba, (612, 25, 68)), ('asset SVG-derived PDF', pdf_rgba, (1036, 25, 473))]:
        diff = [abs(a - b) for a, b in zip(data, native, strict=True)]
        result = (sum(diff), max(diff), sum(v != 0 for v in diff))
        assert result == expected, (name, result)
        print(name, 'L1, max, different channels:', result)
else:
    print('Optional packaged SVG unavailable; source-asset control skipped')
