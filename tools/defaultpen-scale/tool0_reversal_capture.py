#!/usr/bin/env python3
"""Capture a DefaultPen tool-0 reversal with alternating pressures."""
from pathlib import Path

root = Path(__file__).resolve().parent
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
new = '''source = replace_once(source, "[[10.25, 9.5], [16.5, 23.0], [27.75, 8.25], [45.5, 23.5]]", "[[8.0, 16.0], [42.0, 16.0], [12.0, 18.0], [48.0, 20.0]]")
source = replace_once(source, "'pressure':[.25,.5,.75,1.0]", "'pressure':[1.0,.15,1.0,.15]")
''' + old
assert source.count(old) == 1
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
