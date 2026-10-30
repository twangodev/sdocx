#!/usr/bin/env python3
"""Verify a capture log against the retained native input and framebuffer."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('scale', choices=('1', '2'))
parser.add_argument('log', type=Path)
args = parser.parse_args()
here = Path(__file__).resolve().parent
receipt = json.loads((here / 'v173-receipt.json').read_text())['native'][args.scale]
text = args.log.read_text()
result, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
assert result['input'] == receipt['input']
assert result['native_hashes'] == receipt['native_hashes']
assert result['profile'] == receipt['profile']
frame = result['mesa_frame_observations'][-1]
bitmap = result['native_execution_evidence']['native_canvases']['caller_bitmap']['native_texture_object']
assert bitmap['name_matches_actual_glGenTextures'] and bitmap['name_matches_actual_uploads']
assert frame['color_attachment_object_name'] == bitmap['gl_texture_name']
assert frame['color_attachment_object_type'] == 5890 and frame['framebuffer_status'] == 36053
readback = frame['rgba8_readback_bound']
assert readback['status'] == 'ok'
raw = zlib.decompress(base64.b64decode(readback['data_base64']))
assert hashlib.sha256(raw).hexdigest() == readback['sha256'] == receipt['sha256']
assert raw == zlib.decompress((here / f'v173-{args.scale}x-frame.rgba.zlib').read_bytes())
print(f"verified {args.scale}x native input, attached framebuffer and {readback['sha256']}")
