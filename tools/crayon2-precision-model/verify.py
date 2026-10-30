#!/usr/bin/env python3
"""Verify compact shader intervention observations and the float32 model."""
import base64
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
DIRECTORY = Path(sys.argv[1])
SVG = Path(sys.argv[2])
RECEIPT = HERE / 'receipt.json'
MODES = ('xuv', 'xsample', 'yuv', 'ysample')
STAMPS = (1, 100, 250, 478)
BACKGROUND = bytes((64, 128, 192, 255))

def sha(data):
    return hashlib.sha256(data).hexdigest()

def decode(rgb, expected):
    low = int.from_bytes(rgb, 'big')
    options = [struct.unpack('<f', struct.pack('<I', high << 24 | low))[0] for high in range(0x3c, 0x41)]
    return min(options, key=lambda candidate: abs(candidate - expected))

def collect():
    vector = zlib.decompress((ROOT / 'tools/crayon2-independent-control/native-vector.z').read_bytes())
    assert len(vector) == 479 * 24
    f = np.float32
    scale, mat = f(1.7066665887832642), f(0.010416666977107525)
    result = {}
    for stamp in STAMPS:
        x, y, radius, _, bx, by = struct.unpack_from('<6f', vector, stamp * 24)
        frames, hashes = {}, {}
        for mode in MODES:
            paint = json.loads((DIRECTORY / f'{mode}{stamp}.json').read_text())['paint']
            assert paint['target_size'] == [192, 192]
            assert bytes.fromhex(paint['native_vector_hex']) == vector[stamp * 24:(stamp + 1) * 24]
            assert paint['draw_calls'][-1]['values'] == [5, 0, 4, 1]
            frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
            assert len(frame) == 192 * 192 * 4
            frames[mode] = frame
            hashes[mode] = sha(frame)
        masks = {mode: {i // 4 for i in range(0, len(frame), 4) if frame[i:i + 4] != BACKGROUND} for mode, frame in frames.items()}
        assert all(mask == masks['xuv'] for mask in masks.values()), (stamp, {k:len(v) for k,v in masks.items()})
        assert len(masks['xuv']) == {1:1, 100:9, 250:4, 478:4}[stamp]
        def vertices(center, offset):
            out = []
            for sign in (-1, 1):
                coord = f(f(center) + f(f(sign) * f(radius)))
                bg = f(coord + f(offset))
                clip = f(f(mat * coord) - f(1))
                bgclip = f(f(mat * bg) - f(1))
                screen = f(f(f(clip + f(1)) * f(.5)) * f(192))
                uv = f(f(bgclip + f(1)) * f(.5))
                out.append((screen, uv))
            return out
        vertices_by_axis = (vertices(x, bx), vertices(y, by))
        observations = []
        for pos in sorted(masks['xuv']):
            native_y, pixel_x = divmod(pos, 192)
            pixel_y = 191 - native_y
            channels = {mode: list(frame[pos * 4:pos * 4 + 4]) for mode, frame in frames.items()}
            assert all(v[3] == 255 for v in channels.values())
            errors, multiplication_errors = [], []
            for axis, (pixel, uv_mode, sample_mode) in enumerate(((pixel_x + .5, 'xuv', 'xsample'), (native_y + .5, 'yuv', 'ysample'))):
                expected = (pixel + (bx if axis == 0 else by)) / 192
                uv = decode(channels[uv_mode][:3], expected)
                sampled = decode(channels[sample_mode][:3], uv * scale)
                (s0, u0), (s1, u1) = vertices_by_axis[axis]
                prediction = f(u0 + f(f(f(pixel) - s0) * f(f(u1 - u0) / f(s1 - s0))))
                errors.append((float(prediction) - uv) * 2**24)
                multiplication_errors.append((float(f(f(uv) * scale)) - sampled) * 2**24)
            observations.append({'xy': [pixel_x, pixel_y], 'encoded_rgba': channels, 'uv_model_error_times_2_24': errors, 'scale_error_times_2_24': multiplication_errors})
        result[str(stamp)] = {'frame_sha256': hashes, 'observations': observations}
    return result

result = collect()
inputs = {str(p.relative_to(ROOT)): sha(p.read_bytes()) for p in (
    HERE / 'capture.py', HERE / 'verify.py', ROOT / 'tools/crayon2-independent-control/capture-stamp.py',
    ROOT / 'tools/crayon2-independent-control/capture.py', ROOT / 'tools/crayon2-independent-control/native-vector.z',
)}
inputs['source_svg'] = sha(SVG.read_bytes())
assert inputs['source_svg'] == 'bf20e396c38802985a235d254a1764ce61ae26e1a99eb6c2c701558433904c86'
errors = [e for stamp in result.values() for row in stamp['observations'] for e in row['uv_model_error_times_2_24']]
scale_errors = [e for stamp in result.values() for row in stamp['observations'] for e in row['scale_error_times_2_24']]
assert len(errors) == 36 and errors.count(0.0) == 31 and sorted(errors).count(-0.5) == 2 and sorted(errors).count(0.5) == 3
assert len(scale_errors) == 36 and all(e == 0 for e in scale_errors)
receipt = {'scope': 'Recompiled fragment shader with opaque RGB encoding; high float byte inferred from expected source coordinate. Host Mesa only.', 'inputs_sha256': inputs, 'stamps': result}
if '--create' in sys.argv[3:]:
    RECEIPT.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert receipt == json.loads(RECEIPT.read_text()), 'capture, input, observation, or model differs from retained receipt'
    print('Verified 16 native intervention frames, 18 covered pixels, 36 UV and scale observations')
