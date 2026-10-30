#!/usr/bin/env python3
"""Compare a rendered V16 diagnostic PNG to the retained native frame."""
import argparse
import base64
import gzip
import hashlib
import json
import zlib
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('png', type=Path)
args = parser.parse_args()
x = json.loads(gzip.decompress((ROOT / 'transparent.json.gz').read_bytes()))
paint = x['paint']
assert hashlib.sha256(bytes.fromhex(paint['native_vector_hex'])).hexdigest() == '4e7d8107c77d80a2d7fa9d16816c7fa994109cd692a2c38a51d807f4b1a15ec9'
raw = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64']))
assert hashlib.sha256(raw).hexdigest() == paint['readback']['sha256']
# Native GL readback is bottom-up and premultiplied; PNG is top-down and straight.
native = b''.join(raw[y * 1024:(y + 1) * 1024] for y in reversed(range(256)))
image = Image.open(args.png).convert('RGBA')
assert image.size == (256, 256)
actual = bytearray(image.tobytes())
for i in range(0, len(actual), 4):
    for channel in range(3):
        actual[i + channel] = (actual[i + channel] * actual[i + 3] + 127) // 255
errors = [abs(a - b) for a, b in zip(actual, native, strict=True)]
result = {'differing_pixels': sum(any(errors[i:i + 4]) for i in range(0, len(errors), 4)), 'differing_channels': sum(x != 0 for x in errors), 'max_channel_error': max(errors), 'absolute_rgba_error': sum(errors)}
print(json.dumps(result, indent=2))
assert result == {'differing_pixels': 134, 'differing_channels': 345, 'max_channel_error': 4, 'absolute_rgba_error': 432}
