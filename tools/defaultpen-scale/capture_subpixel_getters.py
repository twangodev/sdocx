#!/usr/bin/env python3
"""Retain native Model and MotionEvent coordinate getters from a prepared capture."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
source = (args.capture / 'tool0-1.py').read_text()
def replace(old, new):
    global source
    if source.count(old) != 1:
        raise ValueError(old)
    source = source.replace(old, new)
replace('caller_case=m.result\n', "caller_case=m.result\ncaller_case['observed_source_arrays']=m.source_object_arrays\ncaller_case['observed_motion_returns']=m.native_motion_getter_returns\n")
replace("'caller_input':caller_case['input'],", "'caller_input':caller_case['input'],'observed_source_arrays':caller_case['observed_source_arrays'],'observed_motion_returns':caller_case['observed_motion_returns'],")
env = dict(os.environ, DP_ARGB='0xff336699', PYTHONPATH=':'.join(str(root / p) for p in ('scratch/apk-analysis-runtime/python', 'tools/montblanc/extended-paint/conformance', 'tmp/vector-parity-plan/inkpen2-current-a241793/conformance')))
with tempfile.TemporaryDirectory() as tmp:
    path = Path(tmp) / 'capture.py'
    path.write_text(source)
    result = json.loads(subprocess.check_output([str(root / '.venv/bin/python'), str(path), '--print-capture'], cwd=root, env=env))
s = result['sources']
rows = [{'phase': r['phase_entry'], 'signature': r['signature'], 'd0': r['return']['d0']} for r in s['observed_motion_returns'] if r['signature'] in ('SPen::MotionEvent::GetX(int) const', 'SPen::MotionEvent::GetY(int) const')]
case = next(iter(result['cases'].values()))
print(json.dumps({'scope': 'Native ObjectStroke GetPoint array and MotionEvent getter returns; synthetic Model/Drawing host capture.', 'object_stroke_points': s['observed_source_arrays']['points'], 'motion_coordinate_returns': rows, 'native_stream_sha256': case['native_stream_sha256'], 'native_rgba_sha256': case['readback_sha256']}, indent=2))
