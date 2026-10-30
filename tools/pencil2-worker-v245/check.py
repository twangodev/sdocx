#!/usr/bin/env python3
"""Check selected native callback and renderer handoff instructions and relocations."""
import hashlib
import os
import subprocess
import sys
from pathlib import Path

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
native = root / 'scratch/apk-analysis-native/arm64-v8a'
common = native / 'libSPenPenCommon.so'
pencil = native / 'libSPenPencil2.so'
for path, expected in [(common, 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d'), (pencil, '3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f')]:
    assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
objdump = runtime / 'bin/aarch64-linux-gnu-objdump'
def dis(start, end):
    return subprocess.check_output([str(objdump), '-d', '-C', f'--start-address={start}', f'--stop-address={end}', str(common)], env=env, text=True)
def contains_ordered(output, needles):
    at = 0
    for needle in needles:
        at = output.index(needle, at) + len(needle)
callback = dis(0x504e8, 0x5059c)
contains_ordered(callback, ['ldr\tx8, [x8, #968]', 'add\tx8, x8, #0x10', 'stp\tx8, xzr, [x0]', 'ldr\tx9, [x8, #216]', 'blr\tx9', 'str\td0, [x21, #72]', 'stur\td1, [x21, #84]', 'str\ts0, [x21, #92]', 'str\ts1, [x21, #100]', 'stur\tq0, [x21, #104]', 'str\td1, [x21, #120]', 'str\ts0, [x21, #132]', 'blr\tx8'])
run = dis(0x4676c, 0x469a0)
contains_ordered(run, ['ldr\tx0, [x19, #32]', 'add\tx2, x19, #0x48', 'mov\tx1, x23', 'ldr\tx8, [x8, #104]', 'blr\tx8'])
def reloc(path):
    raw = subprocess.check_output(['readelf', '-rW', str(path)], text=True)
    return subprocess.check_output(['c++filt'], input=raw, text=True)
a = reloc(common)
b = reloc(pencil)
assert any(line.startswith('000000000007a3c8 ') and 'vtable for SPen::PenGLRenderMsg' in line for line in a.splitlines())
assert any(line.startswith('00000000000774d8 ') and line.rstrip().endswith('4676c') for line in a.splitlines())
assert any(line.startswith('00000000000bc420 ') and 'Pencil2StrokeDrawableRTV4::Draw' in line for line in b.splitlines())
print('Verified selected model expansion, render-message dispatch, and RTV4 Draw slot.')
