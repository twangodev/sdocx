#!/usr/bin/env python3
"""Recapture synthetic DefaultPen controls using the project's pinned local inputs."""
import argparse
import os
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
tool = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('output', type=Path)
parser.add_argument('--case', choices=('phase', 'curve', 'wide', 'zero', 'tool2red', 'tool3green', 'tool4purple'))
args = parser.parse_args()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
for path, digest in json.loads((tool / 'hashes.json').read_text()).items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == digest, path
def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'expected one occurrence of {old!r}')
    return source.replace(old, new)

caller = (tool / 'caller.py').read_text()
cases = {
    'phase': ([[20.25, 20.25], [21.85, 20.25], [23.45, 20.25], [25.05, 20.25]], 0.4),
    'curve': ([[19.25, 18.75], [23.0, 21.0], [26.0, 16.0], [30.0, 22.0]], 2.0),
    'wide': ([[18.5, 18.25], [22.0, 21.0], [27.0, 17.0], [32.0, 22.0]], 6.0),
    'zero': ([[20.5, 20.5], [21.5, 19.5], [23.0, 22.0], [25.0, 18.0]], 0.0),
    'tool2red': ([[19.5, 18.25], [22.0, 20.0], [24.5, 16.75], [30.0, 22.0]], 4.0),
    'tool3green': ([[19.25, 18.5], [23.0, 21.0], [26.0, 16.0], [30.0, 22.0]], 8.0),
    'tool4purple': ([[19.75, 19.0], [22.0, 16.0], [26.0, 22.0], [31.0, 18.0]], 3.0),
}
env = dict(os.environ, PYTHONPATH=':'.join(str(root / p) for p in (
    'scratch/apk-analysis-runtime/python', 'tools/montblanc/extended-paint/conformance',
    'tmp/vector-parity-plan/inkpen2-current-a241793/conformance')))
for name, (points, size) in cases.items():
    if args.case and name != args.case:
        continue
    variants = {'tool2red': (2, 'ffe72946'), 'tool3green': (3, '8038bf51'), 'tool4purple': (4, '009550d2')}
    tool_type, color = variants.get(name, (1, 'ff336699'))
    case_env = dict(env, DP_ARGB='0x' + color)
    lines = caller.splitlines()
    entry = (f"input_case={{'points':{points!r},'size':{size!r},'curve_enabled':True,"
             f"'tool_type':{tool_type},'stroke_type':0,'pressure':[.25,.5,.75,1.0],"
             "'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}")
    assert sum(line.startswith('input_case=') for line in lines) == 1
    lines = [entry if line.startswith('input_case=') else line for line in lines]
    caller_path = out / f'caller-{name}.py'
    caller_source = '\n'.join(lines) + '\n'
    if tool_type != 1:
        caller_source = replace_once(caller_source, " if len(motion_sources)!=2 or {row['phase'] for row in motion_sources}!={'draw_object_stroke','saved_redraw'}:\n  raise RuntimeError('expected native MotionEvent source getters in both phases')\n", '')
    caller_path.write_text(caller_source)
    labels = ('1', '2', '4') if name in variants else ('half', '1', '2', '4', '8') if name == 'zero' else ('half', '1', '2', '4', '8')
    for label in labels:
        template = '1' if label == 'half' else '4' if label == '8' else label
        text = (tool / f'native{template}.py').read_text()
        text = replace_once(text, '/home/jding/RustroverProjects/sdocx/tmp/vector-parity-plan/defaultpen-edge-fidelity/caller_tiny.py', str(caller_path))
        if label == 'half':
            text = replace_once(text, 'W,H=128,64', 'W,H=64,32')
            text = replace_once(text, 'len(image)==128*64*4', 'len(image)==64*32*4')
        if label == '8':
            text = replace_once(text, 'W,H=512,256', 'W,H=1024,512')
            text = replace_once(text, 'len(image)==512*256*4', 'len(image)==1024*512*4')
        path = out / f'{name}-{label}.py'
        path.write_text(text)
        with (out / f'{name}-{label}.json').open('w') as result:
            subprocess.run([sys.executable, str(path), '--print-capture'], cwd=root, env=case_env, stdout=result, check=True)
        print(name, label, flush=True)
