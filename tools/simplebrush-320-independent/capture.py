#!/usr/bin/env python3
"""Capture the 320-point alternate input used by the Rust export fixture."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
source = (root / 'tools/simplebrush-one-long/capture.py').read_text()
old = 'points = [[27.0 + 2*i, 48.0 + [0,-3,-5,-4,0,5,7,3][i%8]] for i in range(count)]'
new = 'points = [[27.0 + (i % 64), 35.0 + (i // 64) * 18.0 + [0,-3,-5,-4,0,5,7,3][i%8]] for i in range(count)]'
assert source.count(old) == 1
with tempfile.TemporaryDirectory(prefix='simplebrush-independent-') as tmp:
    script = Path(tmp) / 'capture.py'
    script.write_text(source.replace(old, new))
    env = dict(os.environ, SDOCX_ROOT=str(root), POINT_COUNT='320', PROBE_SIZE='8', VARIANT='alternate')
    subprocess.run([sys.executable, str(script), 'fraction12', *sys.argv[1:]], env=env, check=True)
