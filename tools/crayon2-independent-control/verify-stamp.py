#!/usr/bin/env python3
"""Compare isolated native stamp 1 to its independently source-generated SVG."""
import argparse
import base64
import hashlib
import json
import re
import subprocess
import tempfile
import zlib
from pathlib import Path

from PIL import Image

here = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('capture', type=Path)
parser.add_argument('source_svg', type=Path)
parser.add_argument('renderer', type=Path, help='resvg 0.47 PNG renderer executable')
args = parser.parse_args()
receipt = json.loads((here / 'paint-state-receipt.json').read_text())['independent_prefix_diagnostic']['isolated_stamp_1']
if hashlib.sha256(args.renderer.read_bytes()).hexdigest() != receipt['renderer_sha256']:
    raise SystemExit('renderer binary hash mismatch')
source_receipt = json.loads((here / 'corrected-scale-receipt.json').read_text())
svg = args.source_svg.read_bytes()
if hashlib.sha256(svg).hexdigest() != source_receipt['export_sha256']['svg']:
    raise SystemExit('source SVG hash mismatch')
paint = json.loads(args.capture.read_text())['paint']
vector = bytes.fromhex(paint['native_vector_hex'])
full_vector = zlib.decompress((here / 'native-vector.z').read_bytes())
if vector != full_vector[24:48] or paint['target_size'] != [192, 192] or paint['source_argb_input'] != '0xaab84c29':
    raise SystemExit('native vector, target or color mismatch')
if paint['draw_calls'][-1]['values'] != [5, 0, 4, 1]:
    raise SystemExit('expected exactly one native stamp')
raw = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
if len(raw) != 192 * 192 * 4 or hashlib.sha256(raw).hexdigest() != receipt['native_frame_sha256']:
    raise SystemExit('native frame mismatch')
if raw != zlib.decompress((here / 'stamp-1-native-frame.z').read_bytes()):
    raise SystemExit('native frame differs from retained frame')
native = b''.join(raw[y * 768:(y + 1) * 768] for y in reversed(range(192)))
head, tail = svg.decode('utf-8').split('</defs>', 1)
pattern = r'<rect[^>]*filter="url\(#native-alpha-product-(\d+)\)"[^>]*/>'
ids = [int(m.group(1)) for m in re.finditer(pattern, tail)]
if ids != list(range(479)):
    raise SystemExit('source SVG stamp sequence mismatch')
tail = re.sub(pattern, lambda m: m.group(0) if m.group(1) == '1' else '', tail)
with tempfile.TemporaryDirectory(prefix='sdocx-crayon-stamp-') as tmp:
    isolated = Path(tmp) / 'stamp.svg'
    png = Path(tmp) / 'stamp.png'
    isolated.write_text(head + '</defs>' + tail)
    subprocess.run([str(args.renderer.resolve()), str(isolated), str(png)], check=True)
    with Image.open(png) as image:
        if image.size != (192, 192):
            raise SystemExit('rendered image dimensions mismatch')
        rendered = image.convert('RGBA').tobytes()
differences = []
for i in range(192 * 192):
    n = native[i * 4:i * 4 + 3]
    s = rendered[i * 4:i * 4 + 3]
    if n != s:
        differences.append((i % 192, i // 192, list(n), list(s)))
expected = [(36, 143, receipt['native_rgb'], receipt['resvg_rgb'])]
if differences != expected:
    raise SystemExit(f'pixel comparison mismatch: {differences!r}')
error = [abs(a - b) for a, b in zip(differences[0][2], differences[0][3], strict=True)]
if max(error) != 42 or sum(error) != receipt['total_rgb_error']:
    raise SystemExit('comparison metrics mismatch')
print(json.dumps({'differing_pixels': len(differences), 'maximum_channel_error': max(error), 'total_rgb_error': sum(error), 'pixel': differences[0]}))
