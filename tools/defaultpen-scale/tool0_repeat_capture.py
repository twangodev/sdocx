#!/usr/bin/env python3
"""Capture a DefaultPen tool-0 repeated-point path."""
from pathlib import Path

root = Path(__file__).resolve().parent
source = (root / 'tool0_diverse_capture.py').read_text()
old = "source = replace_once(source, \"variants = {'tool2red':\""
new = '''source = replace_once(source, "[[10.25, 9.5], [16.5, 23.0], [27.75, 8.25], [45.5, 23.5]]", "[[10.25, 12.5], [10.25, 12.5], [24.5, 18.25], [24.5, 18.25]]")
source = replace_once(source, "8.0),\\n    'phase':", "1.0),\\n    'phase':")
''' + old
assert source.count(old) == 1
source = source.replace(old, new)
exec(compile(source, str(root / 'tool0_diverse_capture.py'), 'exec'), {'__file__': str(root / 'tool0_diverse_capture.py'), '__name__': '__main__'})
