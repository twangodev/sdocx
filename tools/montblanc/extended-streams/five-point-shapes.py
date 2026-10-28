#!/usr/bin/env python3
"""Capture independent five-point native streams with nonuniform geometry."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
capture = Path(__file__).with_name('capture.py')
fixture = root / 'conformance/montblanc-five-point-shapes-native.json'
inputs = {
    'bent': {'points': [[0, 0], [5, 8], [11, -1], [15, 6], [22, -3]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5], 'pen_size': 2},
    'reverse': {'points': [[35, 3], [28, -2], [22, 7], [16, 2], [8, 6]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5], 'pen_size': 2},
    'uneven': {'points': [[0, 0], [2, 1], [9, 4], [18, 0], [40, 6]], 'pressure': [0.3, 0.6, 1, 0.7, 0.5], 'pen_size': 2},
}
def run(item):
    name, case = item
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case)}
    proc = subprocess.run([sys.executable, str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
    return name, {'input': case, 'result': json.loads(proc.stdout.splitlines()[-1])}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    cases = dict(pool.map(run, inputs.items()))
identity = json.loads((root / 'conformance/montblanc-extended-native-streams.json').read_text())
receipt = {'schema': 'montblanc-five-point-shapes-native-v1', 'boundary': 'Synthetic ObjectStroke native ARM64 execution under pinned Unicorn host adapters; complete six-lane streams. No saved-note, device, or rendered-pixel parity.', 'source_harness_sha256': identity['source_harness_sha256'], 'binary_sha256': identity['binary_sha256'], 'capture_script_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(), 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': cases}
if sys.argv[1:] == ['--write']:
    fixture.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(cases)} cases')
elif not sys.argv[1:]:
    if receipt != json.loads(fixture.read_text()):
        raise SystemExit('five-point shapes mismatch')
    print(f'verified {len(cases)} cases')
else:
    raise SystemExit('usage: five-point-shapes.py [--write]')
