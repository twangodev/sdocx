"""Reproduce passive native width-state controls with the pinned Unicorn harness."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
fixture = json.loads((root / 'conformance/montblanc-wide-width-state-native.json').read_text())
script = root / fixture['trace_script']
assert hashlib.sha256(script.read_bytes()).hexdigest() == fixture['trace_sha256']
wide = json.loads((root / 'conformance/montblanc-wide-paths-native.json').read_text())

def run(item):
    name, case = item
    env = os.environ.copy()
    env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
    env['MB_CASE'] = json.dumps(case['input'])
    result = subprocess.run([sys.executable, str(script)], cwd=root, env=env,
                            check=True, capture_output=True, text=True)
    rows = {row['rva']: row for row in json.loads(result.stdout.splitlines()[-1])}
    expected = case.get('registers')
    if expected is None:
        expected = {row['rva']: row['s_registers'] for row in fixture['cases'][name]}
    for rva, registers in expected.items():
        actual = rows[rva]['s_registers']
        for key, value in registers.items():
            if actual[key] != value:
                raise AssertionError((name, rva, key, value, actual[key]))
    return name

cases = list(fixture['controls'].items())
cases.extend((name, {'input': case['input']}) for name, case in wide['cases'].items())
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    for name in pool.map(run, cases):
        print(name)
print(f'{len(cases)} native traces matched')
