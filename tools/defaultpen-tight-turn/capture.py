#!/usr/bin/env python3
"""Capture a synthetic DefaultPen tool-0 tight-turn path."""
import argparse
import hashlib
from pathlib import Path
import sys

if sys.flags.optimize:
    raise SystemExit("Optimized Python is unsupported")
parser = argparse.ArgumentParser()
parser.add_argument("--tool", type=int, choices=(0,), required=True)
parser.add_argument("output", type=Path)
parser.add_argument("--case", choices=("tool0",), required=True)
parser.parse_args()

root = Path(__file__).resolve().parent.parent / 'defaultpen-scale'
pins = {
    'tool0_diverse_capture.py': 'c27b22bedd06a4d5634d49b120b25111bc55760529e584ae6d2b9992040d5a68',
    'capture.py': '76ffeee13600bc223656cb854879a4c6f0b5bd359ded3eb278e3bd245dd985bd',
    'native1.py': '7c14607342dd5bd361962afa69bd57711ad19617e60003d9eb0c56d9196b0e77',
    'native2.py': '367ccc76d4095095329fb73d4e9a1203b18e7debb77bcfd4d2f4164e68e88cfc',
    'native4.py': 'c5799fcd34c684842c5673d23583413ee0d098120e53f992c8436230e5c6e45e',
    'caller.py': '8531e5bca2406bfbc72a51aaa77e73f814968ffc0f31c241ca324cf9a8a215d1',
}
for name, expected in pins.items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
        raise SystemExit(f'Capture source hash mismatch: {name}')
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
new = '''source = replace_once(source, "out.mkdir(parents=True, exist_ok=True)", "out.mkdir(parents=False, exist_ok=False)")
source = replace_once(source, "[[10.25, 9.5], [16.5, 23.0], [27.75, 8.25], [45.5, 23.5]]", "[[8.25, 8.5], [18.5, 22.25], [20.25, 9.5], [22.0, 22.5]]")
source = replace_once(source, "8.0),\\n    'phase':", "3.0),\\n    'phase':")
''' + old
if source.count(old) != 1:
    raise SystemExit("Unexpected capture wrapper source")
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
