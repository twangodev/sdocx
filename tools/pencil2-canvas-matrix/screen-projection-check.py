#!/usr/bin/env python3
"""Pin the selected screen bitmap projection input and output instructions."""
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
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
def dis(start, end):
    return subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', f'--start-address={start}', f'--stop-address={end}', str(binary)], env=env, text=True)
for start, end, instructions in [
    (0x86070, 0x860fc, ('mov\tw19, w5', 'mov\tw20, w4', 'mov\tw21, w3', 'mov\tw23, w2', 'stp\tw23, w21, [x22, #32]', 'add\tw8, w20, w23', 'add\tw9, w19, w21', 'stp\tw8, w9, [x22, #40]')),
    (0x889d0, 0x88a40, ('mov\tw21, w7', 'strb\tw21, [x20, #204]')),
    (0x805ac, 0x805d0, ('movi\tv0.2d, #0x0', 'mov\tw8, #0x3f800000', 'str\tw8, [x0, #60]', 'str\tw8, [x0, #40]', 'str\tw8, [x0, #20]', 'str\tw8, [x0]')),
    (0x88d84, 0x88e24, ('ldrb\tw21, [x0, #204]', 'add\tx0, x0, #0x20', 'SPen::Rect::Width() const@plt', 'scvtf\ts8, w0', 'add\tx0, x19, #0x20', 'SPen::Rect::Height() const@plt', 'scvtf\ts9, w0', 'SPen::Matrix4<float>::Matrix4()@plt', 'movi\td2, #0x0', 'cmp\tw21, #0x0', 'fneg\ts0, s8', 'fneg\ts4, s9', 'fmov\ts1, #2.000000000000000000e+00', 'mov\tw8, #0x100', 'movk\tw8, #0xb800, lsl #16', 'str\tw8, [x20, #40]', 'mov\tw8, #0x80000000', 'fsub\ts2, s2, s9', 'fdiv\ts0, s0, s8', 'fcsel\ts2, s2, s9, eq', 'fdiv\ts3, s1, s8', 'fdiv\ts1, s1, s2', 'str\ts3, [x20]', 'fdiv\ts2, s4, s2', 'str\ts1, [x20, #20]', 'stp\ts0, s2, [x20, #48]', 'str\tw8, [x20, #56]')),
]:
    out = dis(start, end)
    position = 0
    for instruction in instructions:
        found = out.find(instruction, position)
        assert found >= 0, instruction
        position = found + len(instruction)
print('Verified selected constructor, identity matrix and projection getter instructions.')
