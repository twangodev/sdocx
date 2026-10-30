#!/usr/bin/env python3
"""Repeat retained interpolation controls with native single-rounding fmaf."""
import contextlib
import ctypes
import hashlib
import io
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
source = root / 'tools/crayon2-precision-model/triangle_plane.py'
assert hashlib.sha256(source.read_bytes()).hexdigest() == 'e02eb88f7f2e432e716d0ab2ce7bca178dd6b73327c9bdaee63260f8b900afa6'
libm = ctypes.CDLL('libm.so.6')
libm.fmaf.argtypes = [ctypes.c_float] * 3
libm.fmaf.restype = ctypes.c_float
text = source.read_text()
old = 'return f(float(a) * float(b) + float(c))'
assert text.count(old) == 1
text = text.replace(old, 'return f(libm.fmaf(float(a), float(b), float(c)))')
for fixture, expected in [('receipt.json', 36), ('triangle-plane-holdout.json', 24)]:
    sys.argv = [str(source), str(source.parent / fixture)]
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(compile(text, str(source), 'exec'), {'__name__': '__main__', 'libm': libm})
    line = output.getvalue().splitlines()[0]
    assert line == f"((0, 1, 2), 'fma') {expected} []", line
    print(f'{fixture}: {expected}/{expected} exact binary32 FMA axis observations')
