#!/usr/bin/env python3
"""Preflight and optionally recapture the asymmetric and independent-shape controls."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if sys.argv[1:] != ['--run', '--shape64', '--size12']:
    raise SystemExit('expected --run --shape64 --size12')
here = Path(__file__).resolve().parent
pins = {'capture_candidate.py': 'f39dcc671e7ae49ca833f7c1273ed05fba6c417f12ae21e742982346e070476f', 'preflight_candidate.py': '1e0985967e9e26c97d311c96979fe7009ad3d741c955fda1bb77436ef916b8a9', 'tapepen_canvas_native.py': 'cf08a72c39f2fa8f8ad463a8ca87596adbfb9649453ad53cb3e763099c385a97', 'preflight.json': 'dec9fcea51fb776d3b0394ea812a80cd8dd12e2964a85c468dde9bfde82d75e9', 'candidate.json': '55a7535e13c9e41900fb98db556b74a2d6996f7ce6423ee59a6e859fefd83747', 'phase-control.patch': '3b6a85f816a4bd71f8151d731b0469a00b614fbac5f7741267228491f48c089f', 'rotation-v113.patch': '9b27f653d7d334ab5154e3a9ad75056d4a6b3d6c1e0f3a650bbe5ad869e0416e'}
for name, expected in pins.items():
    actual = hashlib.sha256((here / name).read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError(f"capture input {name} hash mismatch: {actual}")
def replace_once(text, old, new):
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected exactly one replacement, got {count}: {old}")
    return text.replace(old, new, 1)
with tempfile.TemporaryDirectory(prefix='sdocx-tape-phase-') as temp:
    root = Path(temp)
    for name in ('capture_candidate.py', 'preflight_candidate.py', 'tapepen_canvas_native.py', 'preflight.json', 'candidate.json'):
        shutil.copyfile(here / name, root / name)
    subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'phase-control.patch')], check=True)
    tracing_size = '--trace-size' in sys.argv
    if tracing_size:
        subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'size-level-trace.patch')], check=True)
    subprocess.run(['patch', '-s', str(root / 'capture_candidate.py'), str(here / 'rotation-v113.patch')], check=True)
    runner = root / 'capture_candidate.py'
    script = runner.read_text()
    script = replace_once(script, '[[0.,0.],[9.,8.],[21.,-3.],[34.,7.]]', '[[0.,0.],[12.,-7.],[24.,6.],[32.,-2.]]')
    script = replace_once(script, '0x8044aaee', '0xc0d74433')
    script = replace_once(script, 'call_address(run_method,[task],limit=5_000_000)', 'call_address(run_method,[task],limit=50_000_000)')
    runner.write_text(script)
    digest = hashlib.sha256(runner.read_bytes()).hexdigest()

    manifest = root / 'preflight.json'
    data = json.loads(manifest.read_text())
    data['runner_sha256'] = digest
    manifest.write_text(json.dumps(data, indent=2) + '\n')
    manifest_digest = hashlib.sha256(manifest.read_bytes()).hexdigest()

    args = ['--preflight-sha256', manifest_digest, '--translate', '13', '--translate-y', '21', '--width', '512', '--height', '512', '--compact']
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
            args[args.index('13')] = '77'
            args[args.index('21')] = '59'
            args.append('--shape-control')
        subprocess.run([sys.executable, str(runner), *args], check=True)
