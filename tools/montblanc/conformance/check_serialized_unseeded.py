#!/usr/bin/env python3
"""Replay the native unseeded synthetic serialized Montblanc control."""
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
receipt = json.loads((root / 'conformance/montblanc-serialized-unseeded-native.json').read_text())
capture = root / 'tools/montblanc/conformance/serialized_unseeded_capture.py'
if hashlib.sha256(capture.read_bytes()).hexdigest() != receipt['capture_script_sha256']:
    raise SystemExit('capture script hash mismatch')
env = os.environ.copy()
env['MODE'] = 'null'
env['PYTHONPATH'] = os.pathsep.join(str(root / path) for path in (
    'scratch/apk-analysis-runtime/python',
    'tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance',
))
run = subprocess.run(['python3', str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
if json.loads(run.stdout.splitlines()[-1]) != receipt['case']:
    raise SystemExit('serialized unseeded capture mismatch')
print('verified serialized unseeded admission control')
