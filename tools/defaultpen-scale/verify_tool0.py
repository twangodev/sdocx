#!/usr/bin/env python3
"""Verify retained DefaultPen tool controls and optionally compare recaptures."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('--tool0', type=Path)
parser.add_argument('--tool1', type=Path)
args = parser.parse_args()
if (args.tool0 is None) != (args.tool1 is None):
    parser.error('provide both --tool0 and --tool1, or neither')
receipt = json.loads((Path(__file__).resolve().parents[2] / 'conformance/defaultpen-tool0-scale-native.json').read_text())
assert receipt['schema'] == 'defaultpen-tool0-tool1-native-control-v1'
assert set(receipt['cases']) == {f'tool{t}-{s}x' for t in (0, 1) for s in (1, 2, 4)}
for tool in (0, 1):
    for scale in (1, 2, 4):
        name = f'tool{tool}-{scale}x'
        case = receipt['cases'][name]
        stream = bytes.fromhex(case['native_stream_hex'])
        rgba = zlib.decompress(base64.b64decode(case['rgba_zlib_base64'], validate=True))
        assert len(stream) == case['instance_count'] * 12 == case['native_route']['stream_length'], name
        assert hashlib.sha256(stream).hexdigest() == case['native_stream_sha256'] == case['native_route']['stream_sha256'], name
        assert len(rgba) == case['framebuffer']['width'] * case['framebuffer']['height'] * 4, name
        assert hashlib.sha256(rgba).hexdigest() == case['rgba_sha256'], name
        assert case['input']['tool_type'] == tool and case['gl_error'] == 0, name
        if args.tool0:
            path = (args.tool0 if tool == 0 else args.tool1) / f'tool0-{scale}.json'
            capture = json.loads(path.read_text())
            native = next(iter(capture['cases'].values()))
            assert capture['sources']['caller_input'] == case['input'], name
            assert capture['sources']['caller_loaded_dsos'] == case['native_libraries'], name
            assert bytes.fromhex(native['native_stream_hex']) == stream, name
            assert base64.b64decode(native['rgba_base64'], validate=True) == rgba, name
            assert native['gl_error'] == 0, name
        print(f'{name}: {len(stream)} stream bytes, {len(rgba)} RGBA bytes verified')
