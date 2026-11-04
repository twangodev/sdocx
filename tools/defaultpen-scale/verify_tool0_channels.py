#!/usr/bin/env python3
"""Verify a fresh nonzero-channel native capture against retained controls."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
tool = root / 'tools/defaultpen-scale'
receipt = json.loads((tool / 'tool0-channels-receipt.json').read_text())
fixture_path = root / 'conformance/defaultpen-tool0-diverse-native.json'
digest = lambda data: hashlib.sha256(data).hexdigest()
assert digest(fixture_path.read_bytes()) == receipt['base_fixture_sha256']
for name in ('tool0_channels_capture.py', 'tool0_diverse_capture.py'):
    assert digest((tool / name).read_bytes()) == receipt['script_sha256'][name], name
fixture = json.loads(fixture_path.read_text())
assert set(fixture['cases']) == set(receipt['cases']) == {f'tool0-{s}x' for s in (1, 2, 4)}
for name, base in fixture['cases'].items():
    scale = name.removeprefix('tool0-').removesuffix('x')
    capture = json.loads((args.capture / f'tool0-{scale}.json').read_text())
    native = next(iter(capture['cases'].values()))
    recorded = receipt['cases'][name]
    assert capture['sources']['caller_input'] == recorded['input']
    assert capture['sources']['caller_loaded_dsos'] == base['native_libraries']
    stream = bytes.fromhex(native['native_stream_hex'])
    rgba = base64.b64decode(native['rgba_base64'], validate=True)
    retained = zlib.decompress(base64.b64decode(base['rgba_zlib_base64'], validate=True))
    assert stream == bytes.fromhex(base['native_stream_hex'])
    assert rgba == retained
    assert digest(stream) == native['native_stream_sha256'] == recorded['native_stream_sha256']
    assert digest(rgba) == native['readback_sha256'] == recorded['rgba_sha256']
    assert native['native_drawing_phase_stream_sha256'] == digest(stream)
    assert native['gl_error'] == recorded['gl_error'] == 0
    print(f'{name}: verified input, DSOs, {len(stream)} stream and {len(rgba)} RGBA bytes')
    base['input'] = recorded['input']
assert digest(json.dumps(fixture).encode()) == receipt['exports']['inputs']['fixture_sha256']
print('verified adapted export fixture and native capture script pins')
