#!/usr/bin/env python3
"""Check the shipped canvas Matrix3 expansion instructions and vtable slots."""
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
relocs = subprocess.check_output(['readelf', '-rW', str(binary)], text=True)
for offset, symbol in [('00000000000d0da0', '_ZN4SPen11SPPenCanvas14GetModelMatrixEv'), ('00000000000d10e0', '_ZN4SPen13SPPenCanvasRT14GetModelMatrixEv')]:
    assert any(line.startswith(offset + ' ') and symbol in line for line in relocs.splitlines())
rodata = subprocess.check_output(['readelf', '-x', '.rodata', str(binary)], text=True)
assert '0x0004d2d0 00000000 00000000 0000803f 00000000' in rodata
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
for start, end in [(0xae4e8, 0xae598), (0xb0310, 0xb03c0)]:
    out = subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', f'--start-address={start}', f'--stop-address={end}', str(binary)], env=env, text=True)
    for instruction in ['ldr\tx9, [x8, #216]', 'ldp\td0, d1, [sp]', 'str\twzr, [sp, #48]', 'str\twzr, [sp, #64]', 'str\td0, [sp, #40]', 'stur\td1, [sp, #52]', 'ldp\ts0, s1, [sp, #16]', 'str\twzr, [sp, #96]', 'str\ts0, [sp, #60]', 'ldr\tq0, [x8, #720]', 'str\ts1, [sp, #68]', 'stur\tq0, [sp, #72]', 'ldr\td0, [sp, #24]', 'str\td0, [sp, #88]', 'ldr\ts0, [sp, #32]', 'str\ts0, [sp, #100]', '93b08']:
        assert instruction in out, instruction
print('Verified default canvas model slots, expansion stores, constant and helper call sites.')
