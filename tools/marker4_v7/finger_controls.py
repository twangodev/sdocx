"""Capture V7 finger geometry and compare it with V8's saved sampler.

Run with PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/conformance-cleanup/native-capture-a241793/conformance:scratch/defaultpen-native
The native library and channel adapters are those used by the retained V7 oracle.
"""
import hashlib
import json
import math
from pathlib import Path

from native_geometry import NativeMarker4, case_stroke
from native_marker4_v8 import NativeV8

root = Path.cwd()
reference = json.loads((root / 'conformance/marker4-v7.json').read_text())
reference['stroke']['rendering']['tool_type_raw'] = 0
library = root / 'scratch/apk-analysis-native/arm64-v8a'
objdump = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
marker4_hash = '23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff'
assert hashlib.sha256((library / 'libSPenMarker4.so').read_bytes()).hexdigest() == marker4_hash
v7 = NativeMarker4(library, objdump)
v8 = NativeV8(library, objdump)
v7.callbacks['atan2f'] = lambda: v7.f(0, math.atan2(v7.f(0), v7.f(1)))
v7.callbacks['sinf'] = lambda: v7.f(0, math.sin(v7.f(0)))
v7.callbacks['cosf'] = lambda: v7.f(0, math.cos(v7.f(0)))
rows = []
for case in reference['cases']:
    stroke = case_stroke(reference['stroke'], case)
    a = v7.render(stroke)
    b = v8.render_v8(stroke)
    error = max((max(abs(v - w) for v, w in zip(p[:2] + p[3:], q))
                 for p, q in zip(a['dots'], b['dots'])), default=0)
    assert len(a['dots']) == len(b['dots'])
    assert a['sample_ends'] == b['sample_ends']
    assert error <= 0.0001
    rows.append({'name': case['name'], 'size': case['size'], 'samples': case['samples'], **a,
                 'v8_max_xy_angle_error': error})
result = {'scope': 'Host-controlled native V7/V8 saved redraw tool 0; unit inverse scale. Geometry only, not paint or device parity.',
          'marker4_sha256': marker4_hash,
          'cases': rows}
(root / 'conformance/marker4-v7-finger-native.json').write_text(json.dumps(result, separators=(',', ':')) + '\n')
print(f"{len(rows)} cases, {sum(len(row['dots']) for row in rows)} stamps")
