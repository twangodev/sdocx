#!/usr/bin/env python3
"""Recapture bounded native Marker3 V5 zero-width Drawing and paint controls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
tool = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('output', type=Path)
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
for path, digest in json.loads((tool / 'hashes.json').read_text()).items():
    if hashlib.sha256((root / path).read_bytes()).hexdigest() != digest:
        raise RuntimeError(f'unpinned input {path}')

def replace(source, old, new, count=1):
    if source.count(old) != count:
        raise RuntimeError(f'unexpected template count for {old!r}')
    return source.replace(old, new)

caller = (tool / 'caller-template.py').read_text()
join = (tool / 'join-template.py').read_text()
env = dict(os.environ, PYTHONPATH=':'.join(str(root / p) for p in (
    'scratch/apk-analysis-runtime/python', 'tmp/conformance-cleanup/native-capture-a241793/conformance')))
for tool_type in (2, 3):
    for name, size in [('zero', '0.0'), ('minimum', '0.4'), ('below', '0.9999999403953552'), ('exact', '1.0')]:
        directory = out / f'join-{tool_type}-{name}'
        directory.mkdir(exist_ok=True)
        source = replace(caller, "'size':8.0", f"'size':{size}", 2)
        source = replace(source, "'tool_type':3", f"'tool_type':{tool_type}", 2)
        source = replace(source, 'if marker3_method or motion_getter:', "if marker3_method or motion_getter or _name.startswith('SPen::PenDrawableRT::SetPenData('):")
        caller_path = directory / 'caller.py'
        caller_path.write_text(source)
        program = replace(join, "Path('tmp/vector-parity-plan/marker3-tool3/paint/caller.py')", f"Path({str(caller_path)!r})")
        old = 'bbox=[min(pt[0] for pt in nonzero),min(pt[1] for pt in nonzero),max(pt[0] for pt in nonzero),max(pt[1] for pt in nonzero)]'
        program = replace(program, old, 'bbox=([min(pt[0] for pt in nonzero),min(pt[1] for pt in nonzero),max(pt[0] for pt in nonzero),max(pt[1] for pt in nonzero)] if nonzero else None)')
        old = "source_case=json.loads((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_text())['cases']['marker3_linked_saved1:5:tool2:curve1']"
        program = replace(program, old, 'source_case=m.result')
        program = replace(program, "(ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_bytes()", 'm.FIXTURE.read_bytes()')
        script = directory / 'join.py'
        script.write_text(program)
        with (directory / 'run.log').open('w') as log:
            subprocess.run([sys.executable, str(script)], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        print(tool_type, name, flush=True)
for label, size in [('below', '0.9999999403953552'), ('exact', '1')]:
    path = out / f'rt-{label}.json'
    if path.exists():
        raise FileExistsError(path)
    rt_env = dict(os.environ, M3_ARGB='ff336699', PYTHONPATH=':'.join(str(root / p) for p in (
        'scratch/apk-analysis-runtime/python', 'tmp/vector-parity-plan/inkpen2-current-a241793/conformance')))
    with (out / f'rt-{label}.log').open('w') as log:
        subprocess.run([sys.executable, str(tool / 'rt-capture.py'), '--write-fixture', '--fixture', str(path), '--case', 'saved1', '--scale', '1', '--size', size], cwd=root, env=rt_env, stdout=log, stderr=subprocess.STDOUT, check=True)
    print('SetPenData', label, flush=True)
