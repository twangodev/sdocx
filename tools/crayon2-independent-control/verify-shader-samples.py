#!/usr/bin/env python3
"""Verify diagnostic native shader output and independent SVG stage probes."""
import argparse
import base64
import hashlib
import io
import json
import re
import subprocess
import tempfile
import zlib
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('capture_dir', type=Path)
parser.add_argument('source_svg', type=Path)
parser.add_argument('renderer', type=Path)
args = parser.parse_args()
receipt = json.loads((HERE / 'paint-state-receipt.json').read_text())['independent_prefix_diagnostic']['isolated_stamp_1']
sampling_receipt = json.loads((HERE / 'shader-sampling-receipt.json').read_text())
root = HERE.parents[1]
for relative, digest in sampling_receipt['inputs_sha256'].items():
    assert hashlib.sha256((root / relative).read_bytes()).hexdigest() == digest, relative
source_receipt = json.loads((HERE / 'corrected-scale-receipt.json').read_text())
svg = args.source_svg.read_bytes()
assert hashlib.sha256(svg).hexdigest() == source_receipt['export_sha256']['svg'] == sampling_receipt['source_svg_sha256']
assert hashlib.sha256(args.renderer.read_bytes()).hexdigest() == receipt['renderer_sha256'] == sampling_receipt['renderer_sha256']
vector = zlib.decompress((HERE / 'native-vector.z').read_bytes())[24:48]
expressions = {
    'point': 'brushMask', 'background': 'bgTextureAlpha',
    'alpha': 'inputColor.w * brushMask * bgTextureAlpha * 5.0',
    'xfrac': 'fract(vBgCanvasUV.x * uBgTextureScale.x * 150.0)',
    'xcoord': 'vBgCanvasUV.x * uBgTextureScale.x',
    'ycoord': 'vBgCanvasUV.y * uBgTextureScale.y',
}
native_stage = {}
for mode in ('point', 'background', 'alpha', 'xfrac', 'xcoord', 'ycoord'):
    paint = json.loads((args.capture_dir / f'{mode}.json').read_text())['paint']
    assert bytes.fromhex(paint['native_vector_hex']) == vector
    assert paint['target_size'] == [192, 192]
    assert paint['draw_calls'][-1]['values'] == [5, 0, 4, 1]
    diagnostic = [source for row in paint['shader_source_calls'] for source in row['source'] if '0.000001' in source]
    assert len(diagnostic) == 1
    assert diagnostic[0].count('vec3(' + expressions[mode] + ') + inputColor.xyz * 0.000001') == 2
    frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
    assert len(frame) == 192 * 192 * 4
    offset = ((191 - 143) * 192 + 36) * 4
    native_stage[mode] = list(frame[offset:offset + 4])
expected_native = {'point': [255,255,255,255], 'background': [76,76,76,255], 'alpha': [253,253,253,255], 'xfrac': [0,0,0,255], 'xcoord': [173,173,173,255], 'ycoord': [212,212,212,255]}
assert native_stage == expected_native, native_stage
text = svg.decode()
bg = re.search(r'<image id="native-bg-mask" x="0" y="0" width="([^"]+)" height="([^"]+)"[^>]*base64,([^"]+)', text)
origin = re.search(r'<mask id="background-mask-1".*?<use href="#native-bg-mask" x="([^"]+)" y="([^"]+)"', text)
assert bg and origin
x = (36.5 - float(origin.group(1))) * 150 / float(bg.group(1))
y = 150 - (143.5 - float(origin.group(2))) * 150 / float(bg.group(2))
with Image.open(io.BytesIO(base64.b64decode(bg.group(3), validate=True))) as texture:
    texture = texture.convert('RGBA')
    assert texture.size == (150, 150)
    assert [texture.getpixel((column, int(y)))[3] for column in (101, 102)] == [54, 76]
assert 101.9999 < x < 102 and 124 <= y < 125, (x, y)
head, tail = text.split('</defs>', 1)
pattern = r'<rect[^>]*filter="url\(#native-alpha-product-(\d+)\)"[^>]*/>'
assert [int(m.group(1)) for m in re.finditer(pattern, tail)] == list(range(479))
isolated = re.sub(pattern, lambda m: m.group() if m.group(1) == '1' else '', tail)
isolated = re.sub(r'<rect x="0" y="0" width="192" height="192" fill="#[0-9a-f]+"/>', '', isolated)
raw = zlib.decompress((HERE / 'native-frame.z').read_bytes())
native = b''.join(raw[y*768:(y+1)*768] for y in reversed(range(192)))
svg_stage = {}
shifts = {}
with tempfile.TemporaryDirectory(prefix='crayon-sampling-') as temp:
    path = Path(temp)
    def render(value):
        (path / 'probe.svg').write_text(value)
        subprocess.run([str(args.renderer.resolve()), str(path / 'probe.svg'), str(path / 'probe.png')], check=True)
        with Image.open(path / 'probe.png') as image:
            assert image.size == (192, 192)
            return image.convert('RGBA').tobytes()
    for mode in ('point', 'background', 'product'):
        modified = head
        if mode != 'product':
            composite = r'(<filter id="native-alpha-product-1".*?<feComposite )in="point" in2="background" operator="arithmetic" k1="[^"]+" k2="0" k3="0" k4="0.000000000"'
            replacement = r'\1in="' + mode + r'" in2="background" operator="arithmetic" k1="0" k2="1" k3="0" k4="0"'
            modified, count = re.subn(composite, replacement, modified, flags=re.S)
            assert count == 1
        pixels = render(modified + '</defs>' + isolated)
        offset = (143 * 192 + 36) * 4
        svg_stage[mode] = list(pixels[offset:offset+4])
    assert svg_stage == {'point': [184,76,41,255], 'background': [184,76,43,54], 'product': [184,77,41,180]}, svg_stage
    for shift in (0.0, 0.00001, 0.0001, 0.001):
        modified, count = re.subn(r'(<use href="#native-bg-mask" x=")([^\"]+)', lambda m: m.group(1) + str(float(m.group(2)) - shift), text)
        assert count > 0
        pixels = render(modified)
        diff = [abs(a-b) for a,b in zip(native,pixels,strict=True)]
        shifts[str(shift)] = [sum(any(diff[i:i+3]) for i in range(0,len(diff),4)), max(diff), sum(diff[i] for i in range(len(diff)) if i % 4 != 3)]
    assert shifts == {'0.0':[286,124,5572], '1e-05':[285,124,5452], '0.0001':[288,141,6445], '0.001':[288,141,6445]}, shifts
print(json.dumps({'scope':'Recompiled fragment intervention and independent SVG raster stages; original shader intermediate is inferred', 'native':native_stage, 'svg':svg_stage, 'horizontal_shifts_pixels_max_total':shifts}, indent=2))
