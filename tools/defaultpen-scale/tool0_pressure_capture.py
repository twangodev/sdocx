#!/usr/bin/env python3
"""Capture the same DefaultPen path with uniform zero or full pressure."""
import argparse
from pathlib import Path
import sys

parser = argparse.ArgumentParser(add_help=False)
parser.add_argument('--pressure', choices=('zero', 'full'), required=True)
args, rest = parser.parse_known_args()
sys.argv = [sys.argv[0], *rest]
root = Path(__file__).resolve().parent
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
pressure = '[0.0,0.0,0.0,0.0]' if args.pressure == 'zero' else '[1.0,1.0,1.0,1.0]'
new = 'source = replace_once(source, "\'pressure\':[.25,.5,.75,1.0]", "\'pressure\':' + pressure + '")\n' + old
assert source.count(old) == 1
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
