#!/usr/bin/env python3
"""Reproduce compact native trace for independent five-point shapes."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
root = Path(__file__).resolve().parents[3]
directory = Path(__file__).parent
trace = directory / 'five-point-shape-trace.py'
fixture = root / 'conformance/montblanc-five-point-shapes-trace.json'
source = json.loads((root / 'conformance/montblanc-five-point-shapes-native.json').read_text())
def run(item):
    name, case = item
    env = {**os.environ, 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case['input'])}
    proc = subprocess.run([sys.executable, str(trace)], cwd=root, env=env, text=True, capture_output=True, check=True)
    return name, {'input': case['input'], 'result': json.loads(proc.stdout.splitlines()[-1])}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    cases = dict(pool.map(run, source['cases'].items()))
receipt = {'schema': 'montblanc-five-point-shapes-trace-v1', 'boundary': 'Selected native symbol entries in synthetic ObjectStroke execution under pinned ARM64 Unicorn host adapters. Symbol coverage is limited by the harness; missing path construction symbols do not identify the curve. No saved-note, device or pixel parity.', 'source_harness_sha256': source['source_harness_sha256'], 'binary_sha256': source['binary_sha256'], 'trace_script_sha256': hashlib.sha256(trace.read_bytes()).hexdigest(), 'receipt_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'cases': cases}
if sys.argv[1:] == ['--write']:
    fixture.write_text(json.dumps(receipt, indent=2) + '\n')
    print(f'wrote {len(cases)} shape traces')
elif not sys.argv[1:]:
    if receipt != json.loads(fixture.read_text()): raise SystemExit('shape trace mismatch')
    print(f'verified {len(cases)} shape traces')
else:
    raise SystemExit('usage: five-point-shape-trace-receipt.py [--write]')
