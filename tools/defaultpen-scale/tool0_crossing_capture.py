#!/usr/bin/env python3
"""Capture a synthetic DefaultPen tool-0 crossing path."""
import argparse
from pathlib import Path
import sys

if sys.flags.optimize:
    raise SystemExit("Optimized Python is unsupported")
parser = argparse.ArgumentParser()
parser.add_argument("--tool", type=int, choices=(0,), required=True)
parser.add_argument("output", type=Path)
parser.add_argument("--case", choices=("tool0",), required=True)
parser.parse_args()

root = Path(__file__).resolve().parent
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
new = '''source = replace_once(source, "out.mkdir(parents=True, exist_ok=True)", "out.mkdir(parents=False, exist_ok=False)")
source = replace_once(source, "[[10.25, 9.5], [16.5, 23.0], [27.75, 8.25], [45.5, 23.5]]", "[[10.5, 8.25], [42.25, 23.5], [10.5, 23.5], [42.25, 8.25]]")
source = replace_once(source, "8.0),\\n    'phase':", "3.0),\\n    'phase':")
''' + old
if source.count(old) != 1:
    raise SystemExit("Unexpected capture wrapper source")
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
