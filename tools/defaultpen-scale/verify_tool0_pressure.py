#!/usr/bin/env python3
"""Verify pressure-pair native captures against the retained diverse control."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

root = Path(__file__).resolve().parents[2]
tool = root / 'tools/defaultpen-scale'
parser = argparse.ArgumentParser()
parser.add_argument('--zero', type=Path)
parser.add_argument('--full', type=Path)
args = parser.parse_args()
assert (args.zero is None) == (args.full is None)
base = json.loads((root / 'conformance/defaultpen-tool0-diverse-native.json').read_text())
receipt = json.loads((tool / 'tool0-pressure-receipt.json').read_text())
assert hashlib.sha256((tool / 'tool0_pressure_capture.py').read_bytes()).hexdigest() == receipt['capture_script_sha256']
for name, case in base['cases'].items():
    pair = receipt['cases'][name]
    raw = zlib.decompress(base64.b64decode(case['rgba_zlib_base64'], validate=True))
    assert hashlib.sha256(raw).hexdigest() == case['rgba_sha256'] == pair['rgba_sha256']
    stream = bytes.fromhex(case['native_stream_hex'])
    assert hashlib.sha256(stream).hexdigest() == case['native_stream_sha256'] == pair['native_stream_sha256']
    assert len(stream) == 12 * case['instance_count'] == 12 * pair['instance_count']
    for label, directory, bits in (('zero', args.zero, '00000000'), ('full', args.full, '0000803f')):
        control = pair[label]
        assert control['input']['pressure_f32_bits'] == [bits] * 4
        assert {k:v for k,v in control['input'].items() if k != 'pressure_f32_bits'} == {k:v for k,v in case['input'].items() if k != 'pressure_f32_bits'}
        if directory:
            native = next(iter(json.loads((directory / f"tool0-{name.split('-')[1][:-1]}.json").read_text())['cases'].values()))
            assert native['gl_error'] == 0
            assert bytes.fromhex(native['native_stream_hex']) == stream
            assert base64.b64decode(native['rgba_base64'], validate=True) == raw
            assert native['native_drawing_phase_stream_sha256'] == pair['native_stream_sha256']
print('DefaultPen pressure pair verified')
