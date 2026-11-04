#!/usr/bin/env python3
"""Check fresh native outputs against the compact control fixture."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import zlib

root = Path(__file__).resolve().parents[2]
fixture = json.loads((root / 'conformance/defaultpen-scale-native.json').read_text())
parser = argparse.ArgumentParser()
parser.add_argument('source', type=Path)
parser.add_argument('--case', choices=('phase', 'curve', 'wide', 'zero', 'tool2red', 'tool3green', 'tool4purple'))
args = parser.parse_args()
for name, expected in fixture['cases'].items():
    if args.case and name.rsplit('-', 1)[0] != args.case:
        continue
    actual = json.loads((args.source / f'{name}.json').read_text())
    case = next(iter(actual['cases'].values()))
    frame = base64.b64decode(case['rgba_base64'])
    retained = zlib.decompress(base64.b64decode(expected['rgba_zlib_base64']))
    assert actual['sources']['caller_input'] == expected['input']
    assert case['explicit_draw_matrix_f32'] == expected['projection']
    assert case['uniform_color_values'][0] == expected['native_uniform_color_hex']
    assert len(frame) == expected['width'] * expected['height'] * 4
    assert case['framebuffer']['width'] == expected['width']
    assert case['framebuffer']['height'] == expected['height']
    assert frame == retained and hashlib.sha256(frame).hexdigest() == expected['rgba_sha256']
    assert case['native_stream_sha256'] == expected['stream_sha256']
    assert case['native_initialization_uploads'][1]['data'] == expected['stream_hex']
    assert case['gl_error'] == 0
    print(name, 'matches')
