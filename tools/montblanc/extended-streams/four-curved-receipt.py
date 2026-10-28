#!/usr/bin/env python3
"""Reproduce the four-point curved native stream and cubic path trace."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
case = {'points': [[1, 3], [10, 10], [20, 2], [30, 0]], 'pressure': [0.3, 0.6, 1, 0.7], 'pen_size': 2}
binary = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so'
binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
assert binary_sha256 == '4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350'
scripts = [root / 'tools/montblanc/extended-streams/capture.py', root / 'tools/montblanc/extended-streams/four-cubic-trace.py']
env = os.environ | {'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python'), 'MB_CASE': json.dumps(case)}
results = []
for script in scripts:
    run = subprocess.run(['python3', str(script)], cwd=root, env=env, text=True, capture_output=True, check=True)
    results.append(json.loads(run.stdout.splitlines()[-1]))
receipt = {
    'schema': 'montblanc-curved-four-native-v1',
    'boundary': 'Synthetic ObjectStroke on pinned ARM64 Unicorn host adapters; cubic trace records s0-s3 only. Ignored scratch binaries, Unicorn and tmp harness required; no device or rendered output parity.',
    'binary_sha256': binary_sha256,
    'capture_script_sha256': {script.name: hashlib.sha256(script.read_bytes()).hexdigest() for script in scripts},
    'input': case,
    'stream': results[0],
    'trace': results[1],
    'reproduce': 'python3 tools/montblanc/extended-streams/four-curved-receipt.py',
}
path = root / 'conformance/montblanc-curved-four-native.json'
if '--write' in sys.argv:
    path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert receipt == json.loads(path.read_text())
    print('verified curved four-point native stream and cubic trace')
