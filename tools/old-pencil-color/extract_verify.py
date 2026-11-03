#!/usr/bin/env python3
import argparse
import base64
import hashlib
import json
import zlib
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('captures', type=Path)
parser.add_argument('fixtures', type=Path)
args = parser.parse_args()
for name in ('red', 'alpha', 'backdrop', 'varied'):
    case = json.loads((args.captures / f'{name}.json').read_text())['cases'][0]
    observation = case['pixel_evidence']['draw_observations'][0]
    assert observation['evidence_is_complete']
    assert all(observation[key] == 0 for key in ('gl_error_before_queries', 'gl_error_after_queries', 'gl_error_after_native_draw', 'gl_error_after_post_readback'))
    for kind in ('pre', 'post'):
        frame = observation[f'{kind}_draw_rgba8']
        assert frame['status'] == 'ok' and (frame['width'], frame['height']) == (1920, 1080)
        data = zlib.decompress(base64.b64decode(frame['data_base64']))
        assert hashlib.sha256(data).hexdigest() == frame['sha256']
        rows = [data[i:i + 7680] for i in range(0, len(data), 7680)]
        expected = zlib.decompress((args.fixtures / f'{name}-{kind}.rgba.zlib').read_bytes())
        assert b''.join(reversed(rows)) == expected, f'{name} {kind} differs'
    draw = case['native_vertex_input_observer']['draw_snapshots'][0]
    attr = next(item for item in draw['enabled_attribute_arrays'] if item['index'] == 1)
    vbo = bytes.fromhex(attr['buffer_contents_hex'])
    assert vbo == (args.fixtures / f'{name}.vbo').read_bytes(), f'{name} vbo differs'
    print(name, 'full pre/post frame and VBO match')
