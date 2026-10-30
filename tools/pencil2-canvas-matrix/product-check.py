#!/usr/bin/env python3
"""Verify the bounded Graphics canvas multiplication instruction sequence."""
import hashlib
import os
import subprocess
import sys
from pathlib import Path

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
binary = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenGraphics.so'
assert hashlib.sha256(binary.read_bytes()).hexdigest() == 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621'
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
out = subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', '--start-address=0x93b08', '--stop-address=0x93c04', str(binary)], env=env, text=True)
# The register dataflow is a manual reading of the complete disassembly. Check
# selected instructions in actual address order, without claiming full coverage.
sequence = (
    'mov\tx20, x1', 'mov\tx21, x0', 'ldr\ts0, [x20]',
    'ldur\tq2, [x20, #4]', 'ldp\tq16, q3, [x21]',
    'fmul\tv2.4s, v3.4s, v2.s[0]', 'fmla\tv2.4s, v16.4s, v0.s[0]',
    'ldur\tq6, [x20, #8]', 'ldp\tq0, q1, [x21, #32]',
    'ldur\tq16, [x20, #12]', 'fmla\tv2.4s, v0.4s, v6.s[0]',
    'fmla\tv2.4s, v1.4s, v16.s[0]', 'stp\tq2, q4, [x19]',
    'stp\tq5, q3, [x19, #32]',
)
pos = 0
for instruction in sequence:
    pos = out.find(instruction, pos)
    assert pos >= 0, instruction
    pos += len(instruction)
for start, end in [(0xae4e8, 0xae598), (0xb0310, 0xb03c0)]:
    caller = subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', f'--start-address={start}', f'--stop-address={end}', str(binary)], env=env, text=True)
    for instruction in ('sub\tx8, x29, #0x48', 'sub\tx0, x29, #0x48', 'add\tx1, sp, #0x28', '93b08'):
        assert instruction in caller, instruction
print('Verified selected caller instruction presence and ordered first-column instructions; full dataflow is a manual disassembly reading.')
