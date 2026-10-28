#!/usr/bin/env python3
"""Replay independent wide three-point native controls with the project Unicorn harness."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
fixture = root / 'conformance/montblanc-wide-validation-native.json'
capture = root / 'tools/montblanc/extended-streams/capture.py'
receipt = json.loads(fixture.read_text())
assert hashlib.sha256(capture.read_bytes()).hexdigest() == receipt['capture_sha256']
assert hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest() == receipt['dso_sha256']
for name, case in receipt['cases'].items():
    env = os.environ.copy()
    env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
    env['MB_CASE'] = json.dumps(case['input'])
    result = subprocess.run([sys.executable, str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
    assert json.loads(result.stdout.splitlines()[-1])['stream'] == case['stream'], name
    print(name, 'matches')
