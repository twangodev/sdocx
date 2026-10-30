#!/usr/bin/env python3
"""Check selected instructions on the async ObjectDrawing matrix route."""
import hashlib
import os
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[2]
binary = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so'
assert hashlib.sha256(binary.read_bytes()).hexdigest() == '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd'
runtime = root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr'
env = dict(os.environ, LD_LIBRARY_PATH=str(runtime / 'lib/x86_64-linux-gnu'))
def dis(start, end):
 return subprocess.check_output([str(runtime / 'bin/aarch64-linux-gnu-objdump'), '-d', '-C', f'--start-address={start:#x}', f'--stop-address={end:#x}', str(binary)], env=env, text=True)
def contains(out, instructions):
 for instruction in instructions:
  assert instruction in out, instruction
constructor = dis(0x7af38, 0x7afb0)
contains(constructor, ['ldrb\tw8, [x4, #36]', 'mov\tx1, x4', 'Matrix3<float>::Matrix3', 'strb\tw8, [x19, #104]'])
submit = dis(0x79a7c, 0x79af0)
contains(submit, ['ldrb\tw8, [x22, #104]', 'add\tx1, x22, #0x44', 'Matrix3<float>::Matrix3', 'AsyncDrawingData::AsyncDrawingData'])
copy = dis(0x7c590, 0x7c628)
contains(copy, ['strb\twzr, [x0, #60]!', 'ldrb\tw9, [x7, #36]', 'mov\tx1, x7', 'Matrix3<float>::Matrix3', 'strb\tw8, [x19, #96]'])
paint = dis(0x7bbec, 0x7bc68)
contains(paint, ['ldrb\tw8, [x28, #96]', 'add\tx9, x28, #0x3c', 'ldr\tx8, [x10, #16]', 'csel\tx4, xzr, x9, eq', 'blr\tx8'])
relocs = subprocess.check_output(['readelf', '-rW', str(binary)], text=True)
assert any(line.startswith('00000000000c2f80 ') and '_ZN4SPen13ObjectDrawing14DrawObjectList' in line for line in relocs.splitlines())
print('Verified selected async matrix copies and conditional virtual list dispatch.')
