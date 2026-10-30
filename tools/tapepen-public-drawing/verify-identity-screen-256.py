#!/usr/bin/env python3
"""Verify the identity-matrix display intervention and its retained native frame."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'identity-screen-256-receipt.json').read_text())
assert hashlib.sha256((here / 'identity-screen-256.py').read_bytes()).hexdigest() == receipt['wrapper_sha256']
text = Path(sys.argv[1]).read_text()
result, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
for key in ('input', 'native_hashes', 'profile'):
    assert result[key] == receipt[key]
frame = result['mesa_frame_observations'][-1]
bitmap = result['native_execution_evidence']['native_canvases']['caller_bitmap']['native_texture_object']
assert bitmap['name_matches_actual_glGenTextures'] and bitmap['name_matches_actual_uploads']
assert frame['color_attachment_object_name'] == bitmap['gl_texture_name']
assert frame['color_attachment_object_type'] == 5890 and frame['framebuffer_status'] == 36053
readback = frame['rgba8_readback_bound']
assert readback['status'] == 'ok'
raw = zlib.decompress(base64.b64decode(readback['data_base64']))
assert hashlib.sha256(raw).hexdigest() == readback['sha256'] == receipt['frame_sha256']
assert raw == zlib.decompress((here / 'identity-screen-256-frame.rgba.zlib').read_bytes())
crop = b''.join(raw[y * 2048:y * 2048 + 1024] for y in range(256))
original = zlib.decompress((here / 'v173-1x-frame.rgba.zlib').read_bytes())
assert receipt['crop_vs_v173_1x_exact'] and crop == original
other = zlib.decompress((here / 'identity-screen-512-frame.rgba.zlib').read_bytes())
diff = [abs(a - b) for a, b in zip(raw, other, strict=True)]
assert sum(diff) == receipt['against_identity_512_display']['absolute_rgba_error']
assert sum(v != 0 for v in diff) == receipt['against_identity_512_display']['different_channels']
assert max(diff) == receipt['against_identity_512_display']['maximum_channel_error']
print('verified fixed-bitmap native display intervention and frame difference')
