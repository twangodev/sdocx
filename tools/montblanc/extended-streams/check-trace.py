#!/usr/bin/env python3
"""Replay the extended native transitions against their retained records."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
capture = root / 'tools/montblanc/extended-streams/trace.py'
receipt = json.loads((root / 'conformance/montblanc-extended-transition-trace.json').read_text())
if hashlib.sha256(capture.read_bytes()).hexdigest() != receipt['capture_script_sha256']:
    raise SystemExit('capture script hash mismatch')


def check(item):
    name, case = item
    env = os.environ.copy()
    env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
    env['MB_CASE'] = json.dumps(case['input'])
    run = subprocess.run(['python3', str(capture)], cwd=root, env=env, text=True, capture_output=True, check=True)
    if json.loads(run.stdout.splitlines()[-1]) != case['result']:
        raise RuntimeError(f'{name}: native stream mismatch')
    return name


with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    checked = list(pool.map(check, receipt['cases'].items()))
print(f'verified {len(checked)} extended native transition traces')
