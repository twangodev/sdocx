#!/usr/bin/env python3
"""Verify the fixed-display capture against its compact receipt and retained frame."""
import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'fixed-screen-2x-receipt.json').read_text())
assert hashlib.sha256((here / 'fixed-screen-2x.py').read_bytes()).hexdigest() == receipt['wrapper_sha256']
text = Path(sys.argv[1]).read_text()
result, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
assert result['input'] == receipt['input']
assert result['native_hashes'] == receipt['native_hashes']
profile = result['profile']
assert profile['screen_resolution'] == [256, 256]
assert [profile['width'], profile['height']] == [256, 256]
assert profile['bitmap_target_dimensions'] == [512, 512]
assert profile['canvas_model_matrix_f32_bits'] == ['00000040', '00000000', '00000000', '00000000', '00000040', '00000000', '00000000', '00000000', '0000803f']
assert result['native_execution_evidence']['DrawObject_arguments']['screen_profile'] == [256, 256]
frame = result['mesa_frame_observations'][-1]
bitmap = result['native_execution_evidence']['native_canvases']['caller_bitmap']['native_texture_object']
assert bitmap['name_matches_actual_glGenTextures'] and bitmap['name_matches_actual_uploads']
assert frame['color_attachment_object_name'] == bitmap['gl_texture_name']
assert frame['color_attachment_object_type'] == 5890 and frame['framebuffer_status'] == 36053
readback = frame['rgba8_readback_bound']
assert readback['status'] == 'ok'
raw = zlib.decompress(base64.b64decode(readback['data_base64']))
assert hashlib.sha256(raw).hexdigest() == readback['sha256'] == receipt['frame_sha256']
assert raw == zlib.decompress((here / 'fixed-screen-2x-frame.rgba.zlib').read_bytes())
print('verified fixed-display native input, profile, framebuffer and frame hash')
