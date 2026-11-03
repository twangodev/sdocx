#!/usr/bin/env python3
import argparse
import gzip
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
here = Path(__file__).resolve().parent
source = gzip.decompress((here / 'capture-v17.py.gz').read_bytes()).decode()

def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f'expected one occurrence: {old!r}')
    return text.replace(old, new)

source = replace_once(source, 'ROOT = Path(__file__).resolve().parents[4]', f'ROOT = Path({json.dumps(str(args.root.resolve()))})')
red = source.replace('0xFF000000', '0xFFDD4411').replace('0xff000000', '0xffdd4411')
alpha = red.replace('0xFFDD4411', '0x80DD4411').replace('0xffdd4411', '0x80dd4411')
needle = '        pre=None\n        if target.get("object_name")'
clear = '''        ip.cfunc(adapter.gles.gl, "glClearColor", None, [C.c_float, C.c_float, C.c_float, C.c_float])(0.2, 0.4, 0.6, 1.0)
        ip.cfunc(adapter.gles.gl, "glClear", None, [C.c_uint])(0x4000)
'''
backdrop = replace_once(alpha, needle, clear + needle)
varied = replace_once(backdrop, '[0.5, 0.5, 0.5, 0.5], None)', '[0.2, 0.4, 0.7, 1.0], None)')
needle = '        ip.cfunc(adapter.gles.gl, "glClear", None, [C.c_uint])(0x4000)\n        pre=None'
extra = '''        ip.cfunc(adapter.gles.gl, "glClear", None, [C.c_uint])(0x4000)
        ip.cfunc(adapter.gles.gl, "glScissor", None, [C.c_int,C.c_int,C.c_int,C.c_int])(0, 0, 16, 1080)
        ip.cfunc(adapter.gles.gl, "glClearColor", None, [C.c_float,C.c_float,C.c_float,C.c_float])(0.8, 0.15, 0.35, 1.0)
        ip.cfunc(adapter.gles.gl, "glClear", None, [C.c_uint])(0x4000)
        ip.cfunc(adapter.gles.gl, "glScissor", None, [C.c_int,C.c_int,C.c_int,C.c_int])(0, 0, 1920, 1080)
        pre=None'''
varied = replace_once(varied, needle, extra)
args.output.mkdir(parents=True, exist_ok=True)
(args.output / 'queue_contract.py').write_bytes((here / 'queue_contract.py').read_bytes())
for name, text in [('red', red), ('alpha', alpha), ('backdrop', backdrop), ('varied', varied)]:
    path = args.output / f'{name}.py'
    path.write_text(text)
    print(name, hashlib.sha256(path.read_bytes()).hexdigest())
