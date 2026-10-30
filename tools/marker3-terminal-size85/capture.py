#!/usr/bin/env python3
"""Capture paired interior fractional and crossing native marker controls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
base = root / 'tools/marker3-zero'
p = argparse.ArgumentParser()
p.add_argument('output', type=Path)
out = p.parse_args().output.resolve()
out.mkdir(parents=True, exist_ok=True)
for path, digest in json.loads((base / 'hashes.json').read_text()).items():
    if hashlib.sha256((root / path).read_bytes()).hexdigest() != digest:
        raise RuntimeError(f'unpinned input {path}')

def replace(source, old, new, count=1):
    if source.count(old) != count:
        raise RuntimeError(f'unexpected template count for {old!r}')
    return source.replace(old, new)

cases = {
 'phase2': ([(22.125,25.375),(31.125,36.375),(43.125,26.375),(53.125,39.375)], [1.0,0.0,0.65,0.25], '5','0x8033cc55',8.5,2),
 'phase4': ([(18.375,19.125),(25.375,28.125),(34.375,20.125),(43.375,29.125)], [0.02,0.95,0.15,0.8], '5','0x8033cc55',8.5,4),
 'boundary': ([(21.75,30.25),(39.75,43.25),(54.75,27.25),(67.75,40.25)], [0.9,0.1,0.7,0.3], '5','0x8033cc55',9.0001,1),
}

env = dict(os.environ, PYTHONPATH=':'.join(str(root / x) for x in ('scratch/apk-analysis-runtime/python', 'tmp/conformance-cleanup/native-capture-a241793/conformance')))
original = "'marker3_linked_saved1': {'points':[[0.0,0.0],[20.0,0.0]],'size':8.0,'curve_enabled':True,'tool_type':3,'stroke_type':0,'pressure':[0.05,1.0],'timestamps':[0,8],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},"
for case, (points, pressure, setting, color, size, scale) in cases.items():
    for tool in (1,):
        pen = 'Marker3'
        directory = out / f'{case}-tool{tool}'
        directory.mkdir(exist_ok=True)
        caller = (base / 'caller-template.py').read_text()
        data = {'points': points, 'size': size, 'curve_enabled': True, 'tool_type': tool, 'stroke_type': 0, 'pressure': pressure, 'timestamps': [8*i for i in range(len(points))], 'tilt': [0.0]*len(points), 'orientation': [0.0]*len(points)}
        caller = replace(caller, original, "'marker3_linked_saved1': " + repr(data) + ',')
        caller = replace(caller, "'tool_type':3", f"'tool_type':{tool}")
        caller = replace(caller, "b'com.samsung.android.sdk.pen.pen.preload.Marker3\\0'", f"b'com.samsung.android.sdk.pen.pen.preload.{pen}\\0'")
        caller = replace(caller, "'pen_name':'com.samsung.android.sdk.pen.pen.preload.Marker3'", f"'pen_name':'com.samsung.android.sdk.pen.pen.preload.{pen}'")
        caller = replace(caller, "'requested_canonical_pen_name':'com.samsung.android.sdk.pen.pen.preload.Marker3'", f"'requested_canonical_pen_name':'com.samsung.android.sdk.pen.pen.preload.{pen}'")
        caller = replace(caller, '[obj,0xff336699]', f'[obj,{color}]')
        caller = replace(caller, "source_color!='0xff336699'", f"source_color!='{color}'")
        caller_path = directory / 'caller.py'
        caller_path.write_text(caller)
        program = (base / 'join-template.py').read_text()
        program = replace(program, "Path('tmp/vector-parity-plan/marker3-tool3/paint/caller.py')", f'Path({str(caller_path)!r})')
        program = replace(program, "'--advanced-setting','5'", f"'--advanced-setting','{setting}'")
        program = replace(program, "source_case=json.loads((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_text())['cases']['marker3_linked_saved1:5:tool2:curve1']", 'source_case=m.result')
        program = replace(program, "(ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_bytes()", 'm.FIXTURE.read_bytes()')
        program = replace(program, 'len(draw_rows)==len(saved_rows)==22', 'len(draw_rows)==len(saved_rows)>0')
        program = replace(program, 'assert len(vector_data)==176', 'assert len(vector_data)==8*len(draw_rows)')
        program = replace(program, 'len(rows)==22', 'len(rows)>0')
        program = replace(program, 'the 22 actual drawObjectStroke', 'the actual drawObjectStroke')
        program = replace(program, "p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)", f"p.put(mat,'16f',{scale},0,0,0,0,{scale},0,0,0,0,1,0,0,0,0,1)")
        script = directory / 'join.py'
        script.write_text(program)
        with (directory / 'run.log').open('w') as log:
            subprocess.run([sys.executable, str(script)], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        print(case, tool, flush=True)
