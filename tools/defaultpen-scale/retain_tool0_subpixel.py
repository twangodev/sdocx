#!/usr/bin/env python3
"""Retain compact native subpixel controls from a capture directory."""
import argparse
import base64
import hashlib
import json
import struct
from pathlib import Path
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
args = parser.parse_args()
cases = {}
for scale in (1, 2, 4):
    capture = json.loads((args.capture / f'tool0-{scale}.json').read_text())
    native = next(iter(capture['cases'].values()))
    raw = base64.b64decode(native['rgba_base64'], validate=True)
    stream = bytes.fromhex(native['native_stream_hex'])
    assert hashlib.sha256(raw).hexdigest() == native['readback_sha256']
    assert hashlib.sha256(stream).hexdigest() == native['native_stream_sha256'] == native['native_drawing_phase_stream_sha256']
    assert native['gl_error'] == 0 and len(stream) == native['instance_count'] * 12
    assert len(native['uniform_color_values']) == len(native['uniform_matrix_values']) == 1
    color = [struct.pack('<f', float.fromhex(v)).hex() for v in native['uniform_color_values'][0]]
    projection = [float.fromhex(v) for v in native['uniform_matrix_values'][0]]
    assert color == ['cdcc4c3e', 'cdcccc3e', '9a99193f', '0000803f']
    cases[f'tool0-{scale}x'] = dict(native_uniform_color_f32_bits=color, projection=projection, paint_argb='ff336699', input=capture['sources']['caller_input'], native_libraries=capture['sources']['caller_loaded_dsos'], framebuffer=native['framebuffer'], native_stream_sha256=native['native_stream_sha256'], instance_count=native['instance_count'], rgba_sha256=native['readback_sha256'], rgba_zlib_base64=base64.b64encode(zlib.compress(raw, 9)).decode(), gl_error=native['gl_error'], renderer=native['host_renderer'], native_route=native['native_route'], native_stream_hex=native['native_stream_hex'])
print(json.dumps(dict(schema='defaultpen-tool0-subpixel-native-control-v1', scope='Synthetic native Model/Drawing with explicit host vector bridge and Mesa RGBA8; not serialized-note or device parity.', cases=cases), indent=2))
