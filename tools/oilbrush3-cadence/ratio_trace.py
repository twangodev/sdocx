#!/usr/bin/env python3
"""Capture scalar GL uniform submissions for three native rendering levels."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
directory = Path(__file__).parent
source = root / 'scratch/oilbrush3-public-drawing-v17'
raw = (source / 'oilbrush3_public_drawing_capture.py').read_text()
manifest_bytes = (source / 'preflight.json').read_bytes()
controls = json.loads((directory / 'controls-receipt.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha(raw.encode()) == controls['original_runner_sha256']
assert sha(manifest_bytes) == controls['preflight_sha256']
needle = '_gl_get_integer=ip.cfunc(_gl,'
observer = '''_oil_original_uniform1f=adapter.callbacks['glUniform1f']
def _oil_observe_uniform1f():
 location=ctypes.c_int32(adapter.x(0)&0xffffffff).value
 value=adapter.f(0)
 program=_oil_gl_int(0x8B8D)
 binding=next((row for row in reversed(uniform_location_bindings) if row.get('program')==program and row.get('location')==location),None)
 _console_print('SCALAR',json.dumps({'program':program,'location':location,'binding':binding,'value':value,'bits':struct.pack('<f',value).hex(),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))}),file=sys.stderr)
 _oil_original_uniform1f()
adapter.callbacks['glUniform1f']=_oil_observe_uniform1f;p.callbacks['glUniform1f']=_oil_observe_uniform1f
'''
assert raw.count(needle) == 1
needle2 = "   event={'sequence':queue_cursor,'phase_before_run':phase,"
assert raw.count(needle2) == 1
result = {'scope': 'Synthetic long native Drawing stroke on host Mesa; scalar uniform observer forwards through the original adapter. P3 is identified by its 16x16 viewport. Rendering levels are set on the observed RT before its queued task. No Android or saved-note parity claim.', 'runner_sha256': sha(raw.encode()), 'preflight_sha256': sha(manifest_bytes), 'dso_sha256': sha((root/'scratch/apk-analysis-native/arm64-v8a/libSPenOilBrush3.so').read_bytes()), 'cases': {}}
with tempfile.TemporaryDirectory(prefix='oil-ratio-') as tmp:
    path = Path(tmp)
    (path/'oilbrush3_canvas_native.py').write_bytes((source/'oilbrush3_canvas_native.py').read_bytes())
    for level in range(3):
        injection = f'''   if row.get('native_source_contract') and not globals().get('_level_set'):
    _rt=int(row['native_source_contract']['observed_RT'],16); p.call('SPen::OilBrush3StrokeDrawableRTV1::SetRenderingLevel(int)',[_rt,{level}]); globals()['_level_set']=True
'''
        variant = raw.replace('[[0.0,0.0],[10.0,0.0],[20.0,5.0],[30.0,5.0]]', controls['cases']['long']['points_source']).replace(needle, observer+needle).replace(needle2, injection+needle2)
        runner = path/'oilbrush3_public_drawing_capture.py'
        runner.write_text(variant)
        manifest = json.loads(manifest_bytes)
        manifest['runner_sha256'] = sha(runner.read_bytes())
        mp = path/'preflight.json'
        mp.write_text(json.dumps(manifest))
        proc = subprocess.run([sys.executable, str(runner), '--preflight-manifest', str(mp), '--preflight-sha256', sha(mp.read_bytes()), '--compact', '--failure-json', str(path/'failure.json')], cwd=root, capture_output=True, text=True)
        if proc.returncode:
            raise RuntimeError(proc.stderr[-4000:])
        capture = json.loads(proc.stdout)
        scalars = [json.loads(line[7:]) for line in proc.stderr.splitlines() if line.startswith('SCALAR ')]
        result['cases'][str(level)] = {'variant_sha256': sha(runner.read_bytes()), 'scalars': scalars, 'draws': [{'program': draw['before']['current_program'], 'viewport': draw['before']['viewport']} for draw in capture['mesa_draw_observations']], 'queue_end_error': capture['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error']}
        assert capture['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error'] == 0
        assert scalars and all(row['binding']['name'] == 'uBlendRatio' for row in scalars)
        print(level, len(scalars), scalars[0]['bits'], flush=True)
if '--write' in sys.argv:
    (directory/'ratio-receipt.json').write_text(json.dumps(result, indent=2)+'\n')
else:
    assert result == json.loads((directory/'ratio-receipt.json').read_text())
    print('Ratio receipt matched')
