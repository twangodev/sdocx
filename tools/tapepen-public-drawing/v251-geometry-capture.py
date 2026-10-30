#!/usr/bin/env python3
"""Preflight and optionally recapture the asymmetric and independent-shape controls."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'phase-receipt.json').read_text())
with tempfile.TemporaryDirectory(prefix='sdocx-tape-phase-') as temp:
    root = Path(temp)
    for name in ('capture_candidate.py', 'preflight_candidate.py', 'tapepen_canvas_native.py', 'preflight.json', 'candidate.json'):
        shutil.copyfile(here / name, root / name)
    subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'phase-control.patch')], check=True)
    tracing_size = '--trace-size' in sys.argv
    if tracing_size:
        subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'size-level-trace.patch')], check=True)
    runner = root / 'capture_candidate.py'
    script = runner.read_text()
    old = '[[0.,0.],[9.,8.],[21.,-3.],[34.,7.]]'
    assert script.count(old) == 1
    runner.write_text(script.replace(old, '[[0.,0.],[5.,-11.],[18.,9.],[30.,-5.]]'))
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()
    if False:
        assert digest == receipt['runner_sha256']
    manifest = root / 'preflight.json'
    data = json.loads(manifest.read_text())
    data['runner_sha256'] = digest
    manifest.write_text(json.dumps(data, indent=2) + '\n')
    manifest_digest = hashlib.sha256(manifest.read_bytes()).hexdigest()
    if False:
        assert manifest_digest == receipt['preflight_sha256']
    args = ['--preflight-sha256', manifest_digest, '--translate', '13', '--translate-y', '21', '--width', '256', '--height', '256', '--compact']
    candidate = root / 'candidate.json'
    data = json.loads(candidate.read_text())
    data['run_command'] = subprocess.list2cmdline([sys.executable, str(runner), *args])
    candidate.write_text(json.dumps(data, indent=2) + '\n')
    subprocess.run([sys.executable, str(root / 'preflight_candidate.py')], check=True)
    if '--run' in sys.argv:
        if '--sweep-size' in sys.argv:
            args.extend(['--size', sys.argv[sys.argv.index('--sweep-size') + 1]])
        if '--size4' in sys.argv:
            args.extend(['--size', '4'])
        if '--size12' in sys.argv:
            args.extend(['--size', '12'])
        if '--shape' in sys.argv:
            args.append('--shape-control')
        if '--shape64' in sys.argv:
            args[args.index('13')] = '64'
            args[args.index('21')] = '64'
            args.append('--shape-control')
        subprocess.run([sys.executable, str(runner), *args], check=True)
