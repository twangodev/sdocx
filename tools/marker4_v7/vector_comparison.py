"""Compare a retained native capture against the rounded vector tip approximation."""
import argparse
import hashlib
import json
import math
import subprocess
import tempfile
from pathlib import Path

from PIL import Image
import pymupdf

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
parser.add_argument('--capture-sha256', required=True, help='expected SHA-256 of the capture receipt')
args = parser.parse_args()
def read_verified(path, expected):
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual != expected:
        raise SystemExit(f'{path}: SHA-256 mismatch: expected {expected}, got {actual}')
    return data

meta = json.loads(read_verified(args.capture, args.capture_sha256))
fixture_hash = '5e6150ffe049ae9775d26724beb25174361b7bf234f448c955e7039dcc6f3c77'
if meta['source_json_sha256'] != fixture_hash:
    raise SystemExit('capture references an unexpected fixture')
fixture = read_verified(Path('conformance/marker4-v7-finger-native.json'), fixture_hash)
case = next(c for c in json.loads(fixture)['cases'] if c['name'] == meta['case'])
dots = case['dots']
if hashlib.sha256(json.dumps(dots, separators=(',', ':')).encode()).hexdigest() != meta['source_dots_sha256']:
    raise SystemExit('source dot hash mismatch')
cx, cy = meta['frame_center_doc']
scale = meta.get('scale', 1)
phase_x, phase_y = meta.get('phase', [0, 0])
native = Image.frombytes('RGBA', tuple(meta['target']), read_verified(args.capture.with_suffix('.rgba'), meta['rgba_sha256'])).transpose(Image.Transpose.FLIP_TOP_BOTTOM).getchannel('A').tobytes()
diameter = 2 * (max(case['size'] / 2, 1) + .5)
w, h = .77 * diameter * .99, diameter * .99
rx, ry = w * 25 / 99, h * 25 / 99
a = dots[0][3]
c, s = math.cos(a), math.sin(a)
chunks = []
for x, y, _, _ in dots:
    xx, yy = x*c+y*s-w/2, -x*s+y*c-h/2
    chunks.append(f'M{xx+rx:.8f},{yy:.8f} h{w-2*rx:.8f} a{rx:.8f},{ry:.8f},0,0,1,{rx:.8f},{ry:.8f} v{h-2*ry:.8f} a{rx:.8f},{ry:.8f},0,0,1,{-rx:.8f},{ry:.8f} h{2*rx-w:.8f} a{rx:.8f},{ry:.8f},0,0,1,{-rx:.8f},{-ry:.8f} v{2*ry-h:.8f} a{rx:.8f},{ry:.8f},0,0,1,{rx:.8f},{-ry:.8f} z')
width, height = meta['target']
svg = f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><path d="{" ".join(chunks)}" fill="black" fill-opacity="{128/255}" transform="translate({width/2-scale*cx+phase_x} {height/2-scale*cy+phase_y}) scale({scale}) rotate({math.degrees(a)})"/></svg>'
with tempfile.TemporaryDirectory() as d:
    path = Path(d) / 'candidate.svg'
    png = Path(d) / 'candidate.png'
    path.write_text(svg)
    subprocess.run(['resvg', str(path), str(png)], check=True)
    candidate = Image.open(png).convert('RGBA').getchannel('A').tobytes()
    source = pymupdf.open(str(path))
    pdf = pymupdf.open('pdf', source.convert_to_pdf())
    pixmap = pdf[0].get_pixmap(alpha=True)
    pdf_alpha = Image.frombytes('RGBA', (pixmap.width, pixmap.height), pixmap.samples).getchannel('A').tobytes()
    def metrics(data):
        return {'support': sum(x > 0 for x in data), 'alpha_l1': sum(abs(x-y) for x,y in zip(native,data)), 'alpha_sum': sum(data)}
    print(json.dumps({'case': meta['case'], 'scale': scale, 'phase': [phase_x, phase_y], 'native': metrics(native), 'resvg': metrics(candidate), 'mupdf': metrics(pdf_alpha)}))
