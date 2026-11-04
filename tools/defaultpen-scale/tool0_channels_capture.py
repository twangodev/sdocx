#!/usr/bin/env python3
"""Capture a tool-0 control with nonzero tilt and orientation channels."""
from pathlib import Path

root = Path(__file__).resolve().parent
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
new = "source = replace_once(source, \"'tilt':[0.]*4,'orientation':[0.]*4}\", \"'tilt':[0.1,0.4,0.8,1.2],'orientation':[0.2,1.1,2.4,5.3]}\")\n" + old
assert source.count(old) == 1
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
