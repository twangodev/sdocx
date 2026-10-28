#!/usr/bin/env python3
"""Replay the discriminating five-point midpoint stream and transition trace."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
receipt = json.loads((root / 'conformance/montblanc-midpoint-native.json').read_text())
harness = root / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
assert hashlib.sha256(harness.read_bytes()).hexdigest() == receipt['source_harness_sha256']
env = os.environ.copy()
env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
env['MB_CASE'] = json.dumps(receipt['input'])
for script, digest, expected in (
    ('capture.py', 'capture_script_sha256', 'stream'),
    ('trace.py', 'trace_script_sha256', 'trace'),
):
    path = Path(__file__).with_name(script)
    assert hashlib.sha256(path.read_bytes()).hexdigest() == receipt[digest]
    run = subprocess.run(['python3', str(path)], cwd=root, env=env, text=True, capture_output=True, check=True)
    assert json.loads(run.stdout.splitlines()[-1]) == receipt[expected], f'{script}: native result mismatch'
print('verified midpoint native stream and transition trace')
