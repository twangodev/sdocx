#!/usr/bin/env python3
if not __debug__:
    raise RuntimeError("Evidence verification requires assertions; do not use python -O")

import hashlib
import json
import struct
from pathlib import Path
import numpy as np

root = Path('tools/crayon2-vertex-feedback')
r = json.loads((root / 'receipt.json').read_text())
assert hashlib.sha256((root / 'verify.py').read_bytes()).hexdigest() == r['verify_sha256']
assert hashlib.sha256((root / 'capture.py').read_bytes()).hexdigest() == r['capture_sha256']
vector = Path('tools/crayon2-document-holdout/vector.bin').read_bytes()
assert hashlib.sha256(vector).hexdigest() == r['vector_sha256']
f = np.float32
x, y, rad, _, bx, by = map(f, struct.unpack_from('<6f', vector, 139 * 24))
for name, translation in [('translated', (-.8375, -.8875)), ('identity', (-1., -1.))]:
    rec = r['records'][name]
    data = bytes.fromhex(rec['hex'])
    assert len(data) == 144 and hashlib.sha256(data).hexdigest() == rec['sha256']
    actual = list(struct.iter_unpack('<6f', data))
    expected = []
    for sx, sy in [(-1,-1), (-1,1), (1,-1), (1,1)]:
        pos, uv = [], []
        for c, off, sg, t in [(x,bx,sx,translation[0]), (y,by,sy,translation[1])]:
            v = f(c + f(sg) * rad)
            pos.append(f(f(f(.0125) * v) + f(t)))
            bg = f(v + off)
            uv.append(f(f(f(f(.0125) * bg) + f(t)) + f(1)) * f(.5))
        expected.append((float(pos[0]), float(pos[1]), 0., 1., float(uv[0]), float(uv[1])))
    expected_bytes = b''.join(struct.pack('<6f', *expected[i]) for i in (0,1,2,2,1,3))
    assert data == expected_bytes, (name, data.hex(), expected_bytes.hex())
    print(f'{name}: six position and UV records match binary32 vertex model')

for path, digest in r['script_sha256'].items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, path

import sys
if len(sys.argv) > 1:
    assert len(sys.argv) == 5, 'pass translated JSON, translated log, identity JSON, identity log'
    for name, json_path, log_path in [('translated', sys.argv[1], sys.argv[2]), ('identity', sys.argv[3], sys.argv[4])]:
        observed = json.loads(Path(json_path).read_text())['paint']
        rec = r['records'][name]
        assert 'TF_LINK b\'\'' in Path(log_path).read_text()
        assert 'TF_CAPTURE ' + rec['hex'] + ' error 0' in Path(log_path).read_text()
        shaders = [hashlib.sha256(x.encode()).hexdigest() for item in observed['shader_source_calls'] for x in item['source']]
        assert shaders == rec['shader_sha256']
        assert [x for x in observed['paint_gl_calls'] if x['call'].startswith('glUniform')] == rec['uniform_calls']
        for key in ('draw_calls', 'host_gl_identity', 'execution_closure', 'native_vector_sha256', 'view_translation'):
            assert observed[key] == rec[key], (name, key)
        assert observed['gl_errors_final'] == 0
        print(f'{name}: fresh capture context and feedback match receipt')
