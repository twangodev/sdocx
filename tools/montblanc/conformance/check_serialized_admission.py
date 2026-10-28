#!/usr/bin/env python3
"""Replay the serialized Montblanc admission controls."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
receipt = json.loads((root / 'conformance/montblanc-serialized-admission-native.json').read_text())
capture = root / 'tools/montblanc/conformance/serialized_capture.py'
if hashlib.sha256(capture.read_bytes()).hexdigest() != receipt['capture_script_sha256']:
    raise SystemExit('capture script hash mismatch')
for mode, name in [('null', 'serialized_null'), ('explicit', 'serialized_null_then_set_1')]:
    env = os.environ.copy()
    env['MODE'] = mode
    env['PYTHONPATH'] = os.pathsep.join(str(root / path) for path in (
        'scratch/apk-analysis-runtime/python',
        'tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance',
    ))
    run = subprocess.run(['python3', str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
    if json.loads(run.stdout.splitlines()[-1]) != receipt['cases'][name]:
        raise SystemExit(f'{name}: capture mismatch')
print('verified both serialized admission controls')
