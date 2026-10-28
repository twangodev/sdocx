#!/usr/bin/env python3
"""Capture or replay a compact five-point pressure sweep against native Unicorn."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
capture = root / 'tools/montblanc/extended-streams/capture.py'
receipt_path = root / 'conformance/montblanc-five-point-pressure-sweep.json'
baseline = {'points': [[0, 0], [10, 0], [20, 2], [30, 0], [40, 1]],
            'pressure': [0.3, 0.6, 1, 0.7, 0.5], 'pen_size': 2}
inputs = {'baseline': baseline}
for index in range(5):
    pressure = baseline['pressure'].copy()
    pressure[index] = 0.05
    inputs[f'pressure_{index}_005'] = {**baseline, 'pressure': pressure}


def run(item):
    name, case = item
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'),
           'MB_CASE': json.dumps(case)}
    result = subprocess.run(['python3', str(capture)], cwd=root, env=env,
                            text=True, capture_output=True, check=True)
    native = json.loads(result.stdout.splitlines()[-1])
    stream = native['stream']
    # Include complete stream identity and selected records at both segment boundaries.
    digest = hashlib.sha256(json.dumps(stream, separators=(',', ':')).encode()).hexdigest()
    selected = {str(i): stream[i] for i in (0, 1, 12, 24, 25, 36, 37, 38, 62)}
    return name, {'input': case, 'count': native['count'], 'return': native['return'],
                  'model_count': native['model_count'], 'size': native['size'],
                  'stream_sha256': digest, 'selected_records': selected}


with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    cases = dict(pool.map(run, inputs.items()))
existing = json.loads((root / 'conformance/montblanc-extended-native-streams.json').read_text())
receipt = {
    'schema': 'montblanc-five-point-pressure-sweep-v1',
    'boundary': 'Synthetic ObjectStroke native ARM64 execution under the existing Unicorn host adapters. Selected records are six little-endian f32 bit words; stream digest covers canonical compact JSON of all entry names and lane words. No rendered pixels, saved-note loading, or device parity.',
    'source_harness_sha256': existing['source_harness_sha256'],
    'binary_sha256': existing['binary_sha256'],
    'capture_script_sha256': hashlib.sha256(capture.read_bytes()).hexdigest(),
    'sweep_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'cases': cases,
}
if sys.argv[1:] == ['--write']:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(cases)} native pressure sweep cases')
elif not sys.argv[1:]:
    if receipt != json.loads(receipt_path.read_text()):
        raise SystemExit('native pressure sweep mismatch')
    print(f'verified {len(cases)} native pressure sweep cases')
else:
    raise SystemExit('usage: pressure-sweep.py [--write]')
