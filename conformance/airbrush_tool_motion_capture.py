#!/usr/bin/env python3
"""Capture two synthetic AirBrush V2 tool controls with the pinned local harness.

Requires the historical harness and native dependencies at the receipt's paths.
Outputs raw captures to the directory supplied as the sole argument.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
fixture = json.loads((root / 'conformance/airbrush-tool-motion-native.json').read_text())
runner = root / fixture['runner']
source = runner.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == fixture['runner_sha256']
old_points = "'points':[_probe_start]*n"
new_points = "'points':[[_probe_start[0]+i*3.0, _probe_start[1]+i*i*0.5] for i in range(n)]"
assert source.count(old_points) == 1
source = source.replace(old_points, new_points)
env = os.environ.copy()
env['AB_N'] = '5'
env['PYTHONPATH'] = ':'.join(str(root / path) for path in (
    'scratch/apk-analysis-runtime/python',
    'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin',
    'tmp/conformance-cleanup/native-capture-a241793/conformance',
))
env['LD_LIBRARY_PATH'] = str(root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
out = Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
for tool in (1, 2):
    old_tool = "'tool_type':2,'stroke_type':0"
    assert source.count(old_tool) == 1
    candidate = source.replace(old_tool, f"'tool_type':{tool},'stroke_type':0")
    with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=runner.parent) as script:
        script.write(candidate)
        script.flush()
        with (out / f'tool{tool}.json').open('wb') as capture:
            subprocess.run([sys.executable, script.name, '--single-case', '--print-capture',
                            '--advanced-mode', 'explicit-empty', '--width', '256', '--height',
                            '256', '--source-size', '3.144'], cwd=root, env=env, stdout=capture, check=True)
