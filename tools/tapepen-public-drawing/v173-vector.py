#!/usr/bin/env python3
"""Experimental vector transport of a source-generated v114 mask, not a pen profile."""
import math
import struct
import sys
import zlib
from pathlib import Path

mask = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
ox, oy = map(int, sys.argv[3:5])
assert len(mask) == 256 * 256
pattern = zlib.decompress((Path(__file__).parent / 'pattern01-native.rgba.zlib').read_bytes())

def sample(a, w, h, x, y, channels, repeat):
    ix, iy = math.floor(x), math.floor(y)
    fx, fy = x - ix, y - iy
    result = [0.0] * channels
    for dx, wx in ((0, 1-fx), (1, fx)):
        for dy, wy in ((0, 1-fy), (1, fy)):
            xx = (ix+dx) % w if repeat else min(max(ix+dx, 0), w-1)
            yy = (iy+dy) % h if repeat else min(max(iy+dy, 0), h-1)
            for c in range(channels):
                result[c] += a[(yy*w+xx)*channels+c] * wx * wy / 255
    return result

sx, sy = (v * 256 for v in struct.unpack('<ff', bytes.fromhex('003d8a3a009a993a')))
rects = []
for y in range(256):
    for x in range(256):
        a = mask[y*256+x] / 255
        shadow = .2 * sample(mask, 256, 256, x-sx, y-sy, 1, False)[0]
        tex = sample(pattern, 64, 64, x-ox, y-oy, 4, True)
        stroke = tex[3] * (192/255) * a
        alpha = stroke + shadow * (1-stroke)
        q = round(alpha * 255)
        if not q:
            continue
        rgb = [round(tex[c] * stroke / max(alpha, 1e-5) * 255) for c in range(3)]
        rects.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}" fill-opacity="{q/255:.9f}"/>')
out.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">' + ''.join(rects) + '</svg>')
print(f'{len(rects)} vector rectangles')
