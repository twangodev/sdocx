#!/usr/bin/env python3
"""Verify the selected model and decoded diagnostic texture indices."""
import hashlib
import json
import runpy
import struct
from pathlib import Path
import numpy as np


def checked(path, digest):
    data = Path(path).read_bytes()
    assert hashlib.sha256(data).hexdigest() == digest, path
    return data

model = 'tools/crayon2-transform-control/model139.py'
checked(model, '6ba12620e9d0d38bb199b208fcec54e69d2df6ab2b3ce093a356a05dc6c56dfe')
receipt = json.loads(checked('tools/crayon2-transform-control/intervention-receipt.json', 'b4a56e4a90d2baa7ebbcdaadf1f2a824b14335e09e9639631d01ac9bf354c1ae'))
assert receipt['input_record_index'] == 139 and receipt['pixel_gl_xy'] == [52, 57]
result = runpy.run_path(model)
assert result['inds'] == [113, 4]
background = checked('tmp/vector-parity-plan/crayon2-native-next/native-uploads/g_Crayon2BgPtn.source.bin', '2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3')
assert background[4 * 150 + 113] == 41
uv = []
for mode in ('xuv', 'yuv'):
    encoded = receipt['observations'][mode]['pixel_rgba']
    assert encoded[3] == 255
    bits = (0x3f << 24) | int.from_bytes(bytes(encoded[:3]), 'big')
    uv.append(np.float32(struct.unpack('<f', struct.pack('<I', bits))[0]))
assert [struct.unpack('<I', struct.pack('<f', v))[0] for v in uv] == [0x3f08cccd, 0x3f38cccc]
scale = np.float32(1.4222222566604614)
indices = [int(np.float32(np.float32(np.float32(v * scale) % np.float32(1)) * np.float32(150))) for v in uv]
assert indices == [114, 3] and background[3 * 150 + 114] == 111
assert receipt['observations']['background']['pixel_rgba'][:3] == [111] * 3
print('diagnostic varying selects (114, 3), value 111; model selects (113, 4), value 41')
