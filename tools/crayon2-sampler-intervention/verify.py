#!/usr/bin/env python3
"""Check record 139 diagnostic texels; optionally verify fresh capture frames."""
import base64
import hashlib
import json
import math
import struct
import sys
import zlib
from pathlib import Path

import numpy as np

if not __debug__:
    raise RuntimeError('This evidence check requires assertions; do not use python -O')
root = Path('tools/crayon2-sampler-intervention')
receipt = json.loads((root / 'receipt.json').read_text())
texture = Path('tmp/vector-parity-plan/crayon2-native-next/native-uploads/g_Crayon2BgPtn.source.bin').read_bytes()
assert hashlib.sha256(texture).hexdigest() == '2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3'
existing = json.loads(Path('tools/crayon2-transform-control/intervention-receipt.json').read_text())
assert hashlib.sha256(Path('tools/crayon2-transform-control/intervention-receipt.json').read_bytes()).hexdigest() == 'b4a56e4a90d2baa7ebbcdaadf1f2a824b14335e09e9639631d01ac9bf354c1ae'
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha(Path('tools/crayon2-transform-control/intervention.py').read_bytes()) == existing['intervention_script_sha256']
for path, digest in existing['source_dependencies_sha256'].items():
    assert sha(Path(path).read_bytes()) == digest, path
vector = Path('tools/crayon2-document-holdout/vector.bin').read_bytes()
assert sha(vector) == '15509eda1113e0bfd007455ba4767b1cba2a22391487f94c75e4121d98ac5f8f'

def verify_capture(path, mode, stamp, expected_frame):
    capture = json.loads(path.read_text())
    paint = capture['paint']
    expected = existing['observations'][mode]
    assert capture['source_capture']['source_input'] == existing['source']
    assert paint['execution_closure'] == existing['execution_closure']
    assert paint['target_size'] == existing['target_size']
    assert paint['view_translation'] == existing['view_translation']
    assert paint['texture_allocations'][:2] == existing['texture_allocations']
    assert [x for x in paint['paint_gl_calls'] if x['call'] == 'glUniformMatrix4fv'] == existing['matrix_calls']
    assert [x for x in paint['paint_gl_calls'] if x['call'] == 'glUniform2fv'] == existing['texture_uniforms']
    assert paint['native_blend_calls'] == existing['blend_calls']
    assert paint['draw_calls'] == expected['draw_calls']
    assert paint['gl_errors_final'] == 0
    record = bytes.fromhex(paint['native_vector_hex'])
    assert record == vector[stamp * 24:(stamp + 1) * 24]
    assert sha(record) == paint['native_vector_sha256']
    assert [sha(''.join(item['source']).encode()) for item in paint['shader_source_calls']] == expected['shader_sources_sha256']
    frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
    assert len(frame) == 160 * 160 * 4
    assert sha(frame) == paint['readback']['sha256'] == expected_frame
    return frame

def verify_range(row, stamp, highs):
    _, _, _, _, bx, by = struct.unpack_from('<6f', vector, stamp * 24)
    for axis, (mode, high, offset) in enumerate(zip(('xuv', 'yuv'), highs, (bx, by))):
        low = int.from_bytes(bytes(row[mode][:3]), 'big')
        expected = (row['xy_gl'][axis] + 0.5 + offset) / 160
        candidates = [struct.unpack('<f', struct.pack('<I', h << 24 | low))[0] for h in range(0x3c, 0x41)]
        actual = struct.unpack('<f', struct.pack('<I', high << 24 | low))[0]
        assert math.isfinite(actual) and abs(actual - expected) < 1e-5
        assert sum(math.isfinite(v) and abs(v - expected) < 1e-5 for v in candidates) == 1

if len(sys.argv) > 1:
    source = Path(sys.argv[1])
    for mode in ('xuv', 'yuv', 'background'):
        frame = verify_capture(source / f'{mode}.json', mode, 139, existing['observations'][mode]['frame_sha256'])
        for row in receipt['pixels']:
            x, y = row['xy_gl']
            assert list(frame[4 * (y * 160 + x):4 * (y * 160 + x) + 4]) == row[mode]
for row in receipt['pixels']:
    verify_range(row, 139, (0x3f, 0x3f))
    uv = []
    for mode in ('xuv', 'yuv'):
        bits = 0x3f000000 | int.from_bytes(bytes(row[mode][:3]), 'big')
        uv.append(np.float32(struct.unpack('<f', struct.pack('<I', bits))[0]))
    indices = [int(np.float32(np.float32(np.float32(v * np.float32(1.4222222566604614)) % np.float32(1)) * np.float32(150))) for v in uv]
    assert indices == row['texel']
    assert texture[indices[1] * 150 + indices[0]] == row['background'][0]
    assert row['background'][:3] == [row['background'][0]] * 3
assert len(receipt['pixels']) == 9
print('9/9 decoded varying coordinates select the diagnostic background value')

additional = json.loads((root / 'additional-receipt.json').read_text())
prior = json.loads(Path('tools/crayon2-f32-interpolation/translated-uv-receipt.json').read_text())
assert hashlib.sha256(Path('tools/crayon2-f32-interpolation/translated-uv-receipt.json').read_bytes()).hexdigest() == '993df346c30a506a6be3b739809deb7ff41b681714482ba9e650898f08170c10'
for stamp, record in additional.items():
    for mode in ('xuv', 'yuv'):
        assert record['frame_sha256'][mode] == prior['stamps'][stamp]['frame_sha256'][mode]
    if len(sys.argv) > 1:
        for mode in ('xuv', 'yuv', 'background'):
            frame = verify_capture(source / f'{mode}{stamp}.json', mode, int(stamp), record['frame_sha256'][mode])
            for row in record['pixels']:
                x, y = row['xy_gl']
                assert list(frame[4 * (y * 160 + x):4 * (y * 160 + x) + 4]) == row[mode]
    for row in record['pixels']:
        verify_range(row, int(stamp), row['inferred_high_bytes'])
        uv = []
        for mode, high in zip(('xuv', 'yuv'), row['inferred_high_bytes']):
            bits = high << 24 | int.from_bytes(bytes(row[mode][:3]), 'big')
            uv.append(np.float32(struct.unpack('<f', struct.pack('<I', bits))[0]))
        indices = [int(np.float32(np.float32(np.float32(v * np.float32(1.4222222566604614)) % np.float32(1)) * np.float32(150))) for v in uv]
        assert indices == row['texel']
        assert texture[indices[1] * 150 + indices[0]] == row['background'][0]
        assert row['background'][:3] == [row['background'][0]] * 3
assert {k: len(v['pixels']) for k, v in additional.items()} == {'20': 1, '100': 4, '200': 6}
print('11/11 additional decoded varying coordinates select the diagnostic background value')
