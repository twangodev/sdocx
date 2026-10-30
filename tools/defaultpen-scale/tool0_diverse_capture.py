#!/usr/bin/env python3
"""Capture a synthetic DefaultPen tool-0 or tool-1 scale control."""
import argparse
import hashlib
from pathlib import Path
import sys

parser = argparse.ArgumentParser(add_help=False)
parser.add_argument('--tool', type=int, choices=(0, 1), required=True)
args, rest = parser.parse_known_args()
sys.argv = [sys.argv[0], *rest]
root = Path(__file__).resolve().parent
pins = {
    'capture.py': '76ffeee13600bc223656cb854879a4c6f0b5bd359ded3eb278e3bd245dd985bd',
    'native1.py': '7c14607342dd5bd361962afa69bd57711ad19617e60003d9eb0c56d9196b0e77',
    'native2.py': '367ccc76d4095095329fb73d4e9a1203b18e7debb77bcfd4d2f4164e68e88cfc',
    'native4.py': 'c5799fcd34c684842c5673d23583413ee0d098120e53f992c8436230e5c6e45e',
    'caller.py': '8531e5bca2406bfbc72a51aaa77e73f814968ffc0f31c241ca324cf9a8a215d1',
}
for name, expected in pins.items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'capture source changed: {name}')

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError(f'expected exactly one occurrence: {old!r}')
    return source.replace(old, new)

source_path = root / 'capture.py'
source = source_path.read_text()
source = replace_once(source, "'tool4purple'))", "'tool4purple', 'tool0'))")
source = replace_once(source, "    'phase': (", "    'tool0': ([[10.25, 9.5], [16.5, 23.0], [27.75, 8.25], [45.5, 23.5]], 8.0),\n    'phase': (")
source = replace_once(source, "variants = {'tool2red':", f"variants = {{'tool0': ({args.tool}, 'ff336699'), 'tool2red':")
source = replace_once(source, "        text = (tool / f'native{template}.py').read_text()", "        text = (tool / f'native{template}.py').read_text()\n        text = replace_once(text, \"report.update({'case_id':\", \"report['native_stream_hex'] = native_stream.hex()\\nreport.update({'case_id':\")")
exec(compile(source, str(source_path), 'exec'), {'__file__': str(source_path), '__name__': '__main__'})
