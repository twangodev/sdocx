#!/usr/bin/env python3
"""Check the screen-bitmap projection route through the canvas virtual calls."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
binary = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so'
assert hashlib.sha256(binary.read_bytes()).hexdigest() == 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621'
relocs = subprocess.check_output(['readelf', '-rW', str(binary)], text=True)
for offset, kind, value, name in [
    ('00000000000d68c8', 'R_AARCH64_GLOB_DAT', '00000000000ce860', '_ZTVN4SPen14SPScreenBitmapE'),
    ('00000000000ce908', 'R_AARCH64_ABS64', '000000000008992c', '_ZN4SPen14SPScreenBitmap17GetBitmapWritableEv'),
    ('00000000000ce9b8', 'R_AARCH64_ABS64', '0000000000088e24', '_ZThn176_N4SPen14SPScreenBitmap19GetProjectionMatrixEv'),
]:
    assert any(line.split()[0] == offset and line.split()[2:] == [kind, value, name, '+', '0'] for line in relocs.splitlines() if line.startswith(offset + ' ')), (offset, name)
symbols = subprocess.check_output(['readelf', '-Ws', str(binary)], text=True)
assert any(line.split()[1:3] == ['00000000000ce860', '464'] and line.split()[-1] == '_ZTVN4SPen14SPScreenBitmapE' for line in symbols.splitlines() if '_ZTVN4SPen14SPScreenBitmapE' in line)
assert 0xce860 + 0x10 + 0x98 == 0xce908
assert 0xce860 + 0x148 + 0x10 == 0xce9b8
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
def dis(start, end):
    return subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', f'--start-address={start}', f'--stop-address={end}', str(binary)], env=env, text=True)
for start, end in [(0xae4e8, 0xae534), (0xb0310, 0xb035c)]:
    out = dis(start, end)
    for instruction in ('ldr\tx9, [x0, #32]', 'ldr\tx0, [x9, #8]', 'ldr\tx9, [x9, #152]', 'ldr\tx9, [x8, #16]'):
        assert instruction in out, instruction
for start, end, instructions in [
    (0xae260, 0xae350, ('mov\tx20, x2', 'mov\tx2, x20', 'SPen::SPPenCanvasImpl::SPPenCanvasImpl', 'str\tx22, [x19, #32]')),
    (0xafa64, 0xafaa4, ('stp\tx9, x2, [x0]',)),
    (0x889d0, 0x88a40, ('adrp\tx8, d6000', 'ldr\tx8, [x8, #2248]', 'add\tx9, x8, #0x10', 'add\tx9, x8, #0x148', 'stp\tx9, x8, [x20, #176]')),
    (0x8992c, 0x89934, ('add\tx0, x0, #0xb0',)),
    (0x88e24, 0x88e2c, ('sub\tx0, x0, #0xb0',)),
]:
    out = dis(start, end)
    for instruction in instructions:
        assert instruction in out, instruction
print('Verified selected screen-bitmap constructor, virtual slots, writable accessor and projection thunk instructions.')
