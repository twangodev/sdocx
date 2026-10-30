#!/usr/bin/env python3
"""Capture independent native trajectories for replay-window counterexamples."""
import argparse, hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
root = Path(__file__).resolve().parents[3]
cases = {}
for name, n, size in [('zigzag10', 10, 2.4), ('vertical12', 12, 0.6), ('turn20', 20, 3.0), ('loop32', 32, 1.2)]:
    if name == 'zigzag10': points = [[i * 5, (i % 3) * 4] for i in range(n)]
    elif name == 'vertical12': points = [[(i % 4) * 3, i * 5] for i in range(n)]
    elif name == 'turn20': points = [[i * 3 if i < 10 else 60 - i * 3, i * 3] for i in range(n)]
    else: points = [[(i % 8) * 5, (i // 8) * 6 + (i % 2) * 2] for i in range(n)]
    cases[name] = {'points': points, 'pressure': [round(0.2 + (i % 9) * 0.07, 2) for i in range(n)], 'pen_size': size}
env = os.environ | {'SDOCX_CAPTURE_ROOT': str(root), 'PYTHONPATH': str(root/'scratch/apk-analysis-runtime/python') + os.pathsep + str(root/'tools/montblanc/extended-paint/conformance')}
parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True, help='new receipt path; never overwritten')
args = parser.parse_args()
if args.output.exists():
    parser.error('output already exists')
temporary = tempfile.TemporaryDirectory(prefix='mb-independent-')
work = Path(temporary.name)
receipt = {'capture_script_sha256': hashlib.sha256((root/'tools/montblanc/narrow-probe/capture.py').read_bytes()).hexdigest(), 'cases': {}}
for name, case in cases.items():
    p = subprocess.run([sys.executable, str(root/'tools/montblanc/narrow-probe/capture.py'), '--case', 'baseline', '--output', str(work/f'{name}.json')], cwd=root, env=env | {'MB_CASE': json.dumps(case)}, text=True, capture_output=True, check=True)
    stream = json.loads(next(line.partition('=')[2] for line in p.stderr.splitlines() if line.startswith('CAPTURE_STREAM=')))
    cap = json.loads((work/f'{name}.json').read_text())
    receipt['cases'][name] = {'input': case, 'stream': stream, 'frame_sha256': cap['post_readback']['sha256'], 'input_pins': cap['input_pins'], 'host_gl_sha256': cap['host_gl_sha256']}
    print(name, len(stream), flush=True)
with args.output.open('x') as output:
    output.write(json.dumps(receipt, indent=2)+'\n')
