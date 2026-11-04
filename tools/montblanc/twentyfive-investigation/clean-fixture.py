#!/usr/bin/env python3
"""Verify retained 25-point native evidence; do not generate experimental source."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import struct
import zlib

evidence = Path(__file__).resolve().parent


def checked_gzip(name, expected):
    raw = (evidence / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == expected, name
    return gzip.decompress(raw)


capture = json.loads(checked_gzip(
    'native.json.gz', '57c8a777210aa0f50f4c8cdea127289fd3d9f18fa8ad4b5e49631a4b1ba7aac9'))
stderr = checked_gzip(
    'native-stream.stderr.gz', '0aaa47bd6bba987d41d69e0c3f1729f4d7a78bffa9a806fe5183dbe95c94cafd').decode()
lines = [line.partition('=')[2] for line in stderr.splitlines() if line.startswith('CAPTURE_STREAM=')]
assert len(lines) == 1
stream = json.loads(lines[0])
assert len(stream) == 100
for method, words in stream:
    assert method.startswith('SPen::MontblancFountainPenStrokeDrawableRTV1::')
    assert method.removeprefix('SPen::MontblancFountainPenStrokeDrawableRTV1::').split('(')[0] in ('AddStartPoint', 'AddPoint', 'AddEndPoint')
    assert len(words) == 6
    for word in words:
        assert len(word) == 8
        struct.unpack('<f', bytes.fromhex(word))

# Adapter-supplied recipe, not independent native ObjectStroke getter evidence.
points = [[x, y] for x, y in zip(range(0, 100, 4), [
    0, 2, 4, 6, 8, 6, 4, 2, 0, -2, -4, -6, -8, -6, -4, -2,
    0, 2, 4, 6, 8, 6, 4, 2, 0])]
case = {'input': {'points': points, 'pressure': [0.5] * 25, 'pen_size': 1},
        'result': {'stream': stream}}
assert len(case['input']['points']) == len(case['input']['pressure']) == 25
assert capture['style']['size'] == 1.0
assert capture['style']['argb'] == '0xff336699'
frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
assert len(frame) == 256 * 256 * 4
assert hashlib.sha256(frame).hexdigest() == capture['post_readback']['sha256']
assert capture['post_readback']['sha256'] == 'aa73a47bb66e4f2b06114a85e41acc55cc341b4d94b989169691f2672afe4b60'
print('Verified capture/stream hashes, 100 six-lane callbacks and complete native frame.')
