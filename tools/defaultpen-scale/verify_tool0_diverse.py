#!/usr/bin/env python3
"""Verify retained DefaultPen tool-0 diverse control and optional recapture."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('capture', nargs='?', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
fixture = json.loads((root / 'conformance/defaultpen-tool0-diverse-native.json').read_text())
assert fixture['schema'] == 'defaultpen-tool0-diverse-native-control-v1'
assert set(fixture['cases']) == {f'tool0-{s}x' for s in (1, 2, 4)}
for name, case in fixture['cases'].items():
    stream = bytes.fromhex(case['native_stream_hex'])
    rgba = zlib.decompress(base64.b64decode(case['rgba_zlib_base64'], validate=True))
    assert len(stream) == case['instance_count'] * 12 == case['native_route']['stream_length']
    assert hashlib.sha256(stream).hexdigest() == case['native_stream_sha256'] == case['native_route']['stream_sha256']
    assert len(rgba) == case['framebuffer']['width'] * case['framebuffer']['height'] * 4
    assert hashlib.sha256(rgba).hexdigest() == case['rgba_sha256']
    assert case['input']['tool_type'] == 0 and case['gl_error'] == 0
    if args.capture:
        scale = name.removeprefix('tool0-').removesuffix('x')
        capture = json.loads((args.capture / f'tool0-{scale}.json').read_text())
        native = next(iter(capture['cases'].values()))
        assert capture['sources']['caller_input'] == case['input']
        assert capture['sources']['caller_loaded_dsos'] == case['native_libraries']
        assert bytes.fromhex(native['native_stream_hex']) == stream
        assert base64.b64decode(native['rgba_base64'], validate=True) == rgba
        assert native['gl_error'] == 0
    print(f'{name}: verified {len(stream)} stream and {len(rgba)} RGBA bytes')
