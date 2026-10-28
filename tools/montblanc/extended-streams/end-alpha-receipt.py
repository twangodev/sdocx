#!/usr/bin/env python3
"""Regenerate or verify compact endPen register evidence."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import struct
import sys

root = Path(__file__).resolve().parents[3]
capture = root / 'tools/montblanc/extended-streams/end-alpha-trace.py'
stream = json.loads((root / 'conformance/montblanc-extended-native-streams.json').read_text())
cases = {}
inputs = {name: stream['cases'][name]['input'] for name in ('four_points', 'four_third_y_20')}
for name, bits in (('threshold_below', 0x41c7ffff), ('threshold_exact', 0x41c80000), ('threshold_above', 0x41c80001)):
    distance = struct.unpack('<f', struct.pack('<I', bits))[0]
    inputs[name] = {'points': [[0, 0], [10, 0], [distance, 0], [30, 0]], 'pressure': [0.3, 0.6, 1, 0.7], 'pen_size': 2}
for name, points, pressure, size in (
    ('pressure_low_end', [[0, 0], [10, 0], [20, 20], [30, 0]], [0.3, 1.0, 1.0, 0.0], 2),
    ('pressure_high_end', [[0, 0], [10, 0], [20, 20], [30, 0]], [0.3, 0.0, 1.0, 1.0], 2),
    ('long_path', [[0, 0], [20, 0], [40, 30], [65, 0]], [0.3, 0.6, 1.0, 0.7], 2),
    ('wide_path', [[0, 0], [10, 0], [20, 20], [30, 0]], [0.3, 0.6, 1.0, 0.7], 6),
):
    inputs[name] = {'points': points, 'pressure': pressure, 'pen_size': size}
for name, case in inputs.items():
    env = os.environ | {'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case)}
    result = subprocess.run(['python3', str(capture)], env=env, text=True, capture_output=True, check=True)
    cases[name] = {'input': case, 'rows': json.loads(result.stdout.splitlines()[-1])}
receipt = {'schema': 'montblanc-end-alpha-trace-v1', 'boundary': 'Synthetic native ARM64 execution under pinned Unicorn host adapters; register observations precede instructions. No device or saved-note parity.', 'capture_script_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(), 'binary_sha256': stream['binary_sha256'], 'cases': cases}
path = root / 'conformance/montblanc-end-alpha-trace.json'
if '--write' in sys.argv:
    path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert receipt == json.loads(path.read_text())
    print('verified endPen alpha traces')
