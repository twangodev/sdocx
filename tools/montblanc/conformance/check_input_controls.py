#!/usr/bin/env python3
"""Replay the pinned input controls and compare their compact native receipts."""
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
RECEIPT = ROOT / 'conformance/montblanc-input-controls.json'
CAPTURE = ROOT / 'tools/montblanc/conformance/input_capture.py'
receipt = json.loads(RECEIPT.read_text())


def check(item):
    name, expected = item
    env = os.environ.copy()
    env['PYTHONPATH'] = str(ROOT / 'scratch/apk-analysis-runtime/python')
    env['MONTBLANC_INPUT_CASE'] = json.dumps(expected['input'])
    run = subprocess.run(['python3', str(CAPTURE)], cwd=ROOT, env=env, text=True, capture_output=True)
    if name == 'setting_bad':
        if run.returncode == 0 or 'ValueError("invalid literal for int()' not in run.stderr:
            raise RuntimeError(f'{name}: expected host adapter failure changed')
    elif run.returncode or json.loads(run.stdout.splitlines()[-1]) != expected['result']:
        raise RuntimeError(f'{name}: capture mismatch: {run.stderr[-1000:]}')
    return name


with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    checked = list(pool.map(check, receipt['cases'].items()))
print(f'verified {len(checked)} native input controls, including host adapter failure')
