#!/usr/bin/env python3
"""Regenerate Model-setter tolerance boundary controls using the pinned caller."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
for path, digest in {
    'tools/marker3-size9-tolerance-trace/caller.py': '8bdb952ae4f0fa46da3e8c367c3cce11362ccc8f17f7a6b305dd639256bb6385',
    'scratch/apk-analysis-native/arm64-v8a/libSPenMarker3.so': 'd9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5',
    'scratch/apk-analysis-native/arm64-v8a/libSPenModel.so': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
    'tools/marker3-zero/hashes.json': '03dfe68d293627b46c02b47b19dcdb568c4e3da11c99d4654453dfa82999986d',
}.items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == digest, path
source = (Path(__file__).parent / 'caller.py').read_text()
needle = 'input_case=dict(INPUT_CASES[args.case])'
assert source.count(needle) == 1
source = source.replace(needle, '''input_case=dict(INPUT_CASES[args.case])
input_case.update(points=[[0.,0.],[2.,0.],[4.,0.]], pressure=[1.,1.,1.], timestamps=[0,1,2], tilt=[0.,0.,0.], orientation=[0.,0.,0.], tool_type=int(os.environ['MARKER3_TEST_TOOL']))''')
needle = "p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);"
assert source.count(needle) == 1
source = source.replace(needle, "p.f(0,float(os.environ['MARKER3_TEST_TOLERANCE'])); p.call('SPen::ObjectStroke::SetInitialToloerance(float)',[obj]); p.call('SPen::ObjectStroke::GetInitialToloerance() const',[obj]); assert p.u.reg_read(R.UC_ARM64_REG_S0) == struct.unpack('<I',struct.pack('<f',float(os.environ['MARKER3_TEST_TOLERANCE'])))[0]; p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);")
env = dict(os.environ, PYTHONPATH='scratch/apk-analysis-runtime/python:tmp/conformance-cleanup/native-capture-a241793/conformance')
with tempfile.TemporaryDirectory(prefix='marker3-tolerance-') as work:
    work = Path(work)
    caller = work / 'caller.py'
    caller.write_text(source)
    for tool in (1, 2):
        env['MARKER3_TEST_TOOL'] = str(tool)
        out = {}
        for label, value in [('zero','0'), ('one','1'), ('below_two','1.9999998807907104'), ('two','2'), ('above_two','2.000000238418579')]:
            env['MARKER3_TEST_TOLERANCE'] = value
            subprocess.run([sys.executable, str(caller), '--write-fixture', '--case', 'marker3_linked_saved1', '--advanced-setting', '5'], cwd=root, env=env, check=True, stdout=subprocess.DEVNULL)
            case = json.loads((work / 'marker3-saved-caller-native.json').read_text())['cases'][f'marker3_linked_saved1:5:tool{tool}:curve1']
            out[label] = {'tolerance': float(value), 'input': case['input'], 'phase_final_vectors': case['phase_final_vectors']}
        expected = json.loads((root / 'conformance' / f'marker3-tolerance-boundary-tool{tool}.json').read_text())
        assert out == expected, f'tool {tool} receipt differs'
        print(f'tool {tool}: five native vectors match')
