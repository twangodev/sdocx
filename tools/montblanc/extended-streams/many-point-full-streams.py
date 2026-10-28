#!/usr/bin/env python3
"""Capture compact native streams for longer synthetic Montblanc strokes."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
capture = root / 'tools/montblanc/extended-streams/capture.py'
trace = root / 'tools/montblanc/extended-streams/five-point-shape-trace.py'
receipt_path = root / 'conformance/montblanc-many-point-full-streams.json'
reference = json.loads((root / 'conformance/montblanc-many-point-controls.json').read_text())
inputs = {
    'six_bent': {'points': [[0, 0], [5, 8], [11, -1], [15, 6], [22, -3], [29, 4]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8], 'pen_size': 2},
    'seven_linear': {'points': [[0, 0], [6, 0], [12, 0], [18, 0], [24, 0], [30, 0], [36, 0]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8, 0.4], 'pen_size': 2},
    'six_rejected': {'points': [[0, 0], [6, 0], [7, 1], [12, 0], [18, 0], [24, 0]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8], 'pen_size': 2},
}
# The pinned harness verifies all DSO and helper files before executing native code.
expected_capture = 'b277e27bf15167db554f5ecd3f5ed4833642a4afb9b206fa30438486190277ad'
expected_harness = 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'
expected_binaries = {'Drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd', 'Model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a', 'MontblancFountainPen': '4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350'}
reference = json.loads((root / 'conformance/montblanc-many-point-controls.json').read_text())
if reference['capture_sha256'] != expected_capture or reference['source_harness_sha256'] != expected_harness or reference['binary_sha256'] != expected_binaries:
    raise SystemExit('reference identity mismatch')
if hashlib.sha256((root / 'tools/montblanc/extended-streams/capture.py').read_bytes()).hexdigest() != expected_capture:
    raise SystemExit('capture script hash mismatch')
if hashlib.sha256((root / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py').read_bytes()).hexdigest() != expected_harness:
    raise SystemExit('native harness hash mismatch')
cases = {}
for name, case in inputs.items():
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case)}
    def run(script):
        result = subprocess.run([sys.executable, str(script)], cwd=root, env=env, text=True, capture_output=True, check=True)
        return json.loads(result.stdout.splitlines()[-1])
    native = run(capture)
    stream = native['stream']
    digest = hashlib.sha256(json.dumps(stream, separators=(',', ':')).encode()).hexdigest()
    if digest != reference['cases'][name]['stream_sha256']:
        raise SystemExit(f'{name}: stream digest mismatch')
    cases[name] = {'input': case, **native}
identity = json.loads((root / 'conformance/montblanc-extended-native-streams.json').read_text())
receipt = {'schema': 'montblanc-many-point-full-streams-v1', 'boundary': reference['boundary'], 'source_harness_sha256': identity['source_harness_sha256'], 'binary_sha256': identity['binary_sha256'], 'capture_sha256': expected_capture, 'cases': cases}
if sys.argv[1:] == ['--write']:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(cases)} controls')
elif not sys.argv[1:]:
    if receipt != json.loads(receipt_path.read_text()):
        raise SystemExit('native controls mismatch')
    print(f'verified {len(cases)} controls')
else:
    raise SystemExit('usage: many-point-full-streams.py [--write]')
