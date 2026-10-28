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
receipt_path = root / 'conformance/montblanc-many-point-controls.json'
inputs = {
    'six_bent': {'points': [[0, 0], [5, 8], [11, -1], [15, 6], [22, -3], [29, 4]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8], 'pen_size': 2},
    'seven_linear': {'points': [[0, 0], [6, 0], [12, 0], [18, 0], [24, 0], [30, 0], [36, 0]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8, 0.4], 'pen_size': 2},
    'six_rejected': {'points': [[0, 0], [6, 0], [7, 1], [12, 0], [18, 0], [24, 0]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5, 0.8], 'pen_size': 2},
}
cases = {}
for name, case in inputs.items():
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case)}
    def run(script):
        result = subprocess.run([sys.executable, str(script)], cwd=root, env=env, text=True, capture_output=True, check=True)
        return json.loads(result.stdout.splitlines()[-1])
    native = run(capture)
    events = run(trace)['events']
    stream = native['stream']
    cases[name] = {'input': case, 'count': native['count'], 'return': native['return'], 'model_count': native['model_count'], 'size': native['size'], 'stream_sha256': hashlib.sha256(json.dumps(stream, separators=(',', ':')).encode()).hexdigest(), 'events': events}
identity = json.loads((root / 'conformance/montblanc-extended-native-streams.json').read_text())
receipt = {'schema': 'montblanc-many-point-controls-v1', 'boundary': 'Synthetic ObjectStroke ARM64 Unicorn execution with pinned host adapters. Stream digests cover all native calls and six float lanes; trace observes selected native entry points. No device or pixel parity.', 'source_harness_sha256': identity['source_harness_sha256'], 'binary_sha256': identity['binary_sha256'], 'capture_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(), 'trace_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(), 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': cases}
if sys.argv[1:] == ['--write']:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(cases)} controls')
elif not sys.argv[1:]:
    if receipt != json.loads(receipt_path.read_text()):
        raise SystemExit('native controls mismatch')
    print(f'verified {len(cases)} controls')
else:
    raise SystemExit('usage: many-point-controls.py [--write]')
