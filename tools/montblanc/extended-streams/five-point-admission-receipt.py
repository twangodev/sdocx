#!/usr/bin/env python3
"""Replay native redraw admission around the five-unit axis threshold."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
trace = Path(__file__).with_name('five-point-shape-trace.py')
capture = Path(__file__).with_name('capture.py')
fixture = root / 'conformance/montblanc-five-point-admission-trace.json'

def run(case):
    name, input_case = case
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(input_case)}
    proc = subprocess.run([sys.executable, str(trace)], cwd=root, env=env, text=True, capture_output=True, check=True)
    result = json.loads(proc.stdout.splitlines()[-1])
    native = subprocess.run([sys.executable, str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
    return name, {'input': input_case, 'result': json.loads(native.stdout.splitlines()[-1]), 'move_to_callers': [e['caller_rva'] for e in result['events'] if 'SmPath::moveTo(' in e['symbol']]}

cases = {}
for segment in range(3):
    for distance in (4.9, 5.0, 5.1):
        lengths = [10.0, 10.0, 10.0]
        lengths[segment] = distance
        positions = [0.0]
        for length in lengths:
            positions.append(positions[-1] + length)
        cases[f'{segment}:{distance}'] = [[x, 0] for x in positions] + [[40, 6]]
cases.update({
    'diagonal_rejected': [[0, 0], [4, 4], [10, 0], [20, 0], [30, 6]],
    'negative_y_boundary': [[0, 0], [4, -5], [14, -5], [24, -5], [34, 1]],
    'mixed_x_boundary': [[0, 0], [-5, 4], [-15, 4], [-25, 4], [-35, 10]],
    'zero_accepted': [[0, 0], [1, 1], [2, 2], [3, 3], [20, 6]],
    'one_first': [[0, 0], [10, 0], [11, 1], [12, 2], [20, 6]],
    'one_far': [[0, 0], [1, 1], [2, 2], [20, 0], [30, 6]],
    'one_boundary': [[0, 0], [1, 1], [2, 2], [5, 0], [20, 6]],
    'multiple_rejected': [[0, 0], [4, 4], [-4, -4], [10, 0], [20, 6]],
})
cases = [(name, {'points': points, 'pressure': [0.3, 0.6, 1, 0.7, 0.5], 'pen_size': 2}) for name, points in cases.items()]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    results = dict(pool.map(run, cases))
identity = json.loads((root / 'conformance/montblanc-five-point-shapes-native.json').read_text())
receipt = {'source_harness_sha256': identity['source_harness_sha256'], 'binary_sha256': identity['binary_sha256'], 'schema': 'montblanc-five-point-admission-trace-v1', 'boundary': 'Synthetic ObjectStroke ARM64 Unicorn execution with pinned host adapters; selected controls only, no device or pixel parity.', 'trace_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(), 'capture_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(), 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': results}
if sys.argv[1:] == ['--write']:
    fixture.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(results)} admission traces')
elif not sys.argv[1:]:
    if receipt != json.loads(fixture.read_text()):
        raise SystemExit('admission trace mismatch')
    print(f'verified {len(results)} admission traces')
else:
    raise SystemExit('usage: five-point-admission-receipt.py [--write]')
