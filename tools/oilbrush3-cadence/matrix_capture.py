#!/usr/bin/env python3
"""Run the V17 native capture with doubled source coordinates and check its draw trace."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
source = root / 'scratch/oilbrush3-public-drawing-v17'
receipt = json.loads((Path(__file__).parent / 'receipt.json').read_text())
original = source / 'oilbrush3_public_drawing_capture.py'
assert hashlib.sha256(original.read_bytes()).hexdigest() == receipt['original_runner_sha256']
old = '[[0.0,0.0],[10.0,0.0],[20.0,5.0],[30.0,5.0]]'
new = '[[0.0,0.0],[20.0,0.0],[40.0,10.0],[60.0,10.0]]'
text = original.read_text()
assert text.count(old) == 1
with tempfile.TemporaryDirectory(prefix='sdocx-oil-cadence-') as tmp:
    path = Path(tmp)
    runner = path / original.name
    runner.write_text(text.replace(old, new))
    assert hashlib.sha256(runner.read_bytes()).hexdigest() == receipt['derived_runner_sha256']
    (path / 'oilbrush3_canvas_native.py').write_bytes((source / 'oilbrush3_canvas_native.py').read_bytes())
    manifest_raw = (source / 'preflight.json').read_bytes()
    assert hashlib.sha256(manifest_raw).hexdigest() == receipt['preflight_sha256']
    manifest = json.loads(manifest_raw)
    manifest['runner_sha256'] = receipt['derived_runner_sha256']
    manifest_path = path / 'preflight.json'
    manifest_path.write_text(json.dumps(manifest))
    digest = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    result = subprocess.run([sys.executable, str(runner), '--preflight-manifest', str(manifest_path), '--preflight-sha256', digest, '--compact', '--failure-json', str(path / 'failure.json')], cwd=root, capture_output=True, check=False)
    if result.returncode: raise RuntimeError(result.stderr.decode() + result.stdout.decode()[-2000:])
    capture = json.loads(result.stdout)
    draws = []
    count = 0
    for draw in capture['mesa_draw_observations']:
        before = draw['before']
        kind = 'P3' if before['viewport'] == [0, 0, 16, 16] else 'P4'
        count += kind == 'P4'
        uniforms = [{'name': e.get('uniform_location_binding', {}).get('name'), 'payload_base64': e.get('payload_base64')} for e in draw['uniform_events_since_prior_draw'] if 'payload_base64' in e]
        draws.append({'kind': kind, 'p4_count': count, 'viewport': before['viewport'], 'uniforms': uniforms})
    assert draws == receipt['draws']
    assert capture['input'] == receipt['input']
    assert capture['native_execution_evidence']['drawing_return'] == 1
    assert capture['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error'] == 0
    assert len(capture['native_execution_evidence']['native_queue_run_events']) == receipt['queue_run_count']
    assert [e['actual_mesa_error'] for e in capture['native_execution_evidence']['native_gl_error_events']] == receipt['guest_gl_errors']
    events = capture['mesa_program_lifecycle']['events']
    assert [e['compile_status'] for e in events if 'compile_status' in e] == receipt['shader_compile_status']
    assert [e['link_status'] for e in events if 'link_status' in e] == receipt['program_link_status']
    compact = {'scope': 'Doubled-coordinate synthetic host-Mesa Drawing capture; source AddPoint bytes and per-draw uniforms. No saved-note or device parity.', 'input': capture['input'], 'native_abi': capture['native_abi'], 'draws': draws, 'derived_runner_sha256': receipt['derived_runner_sha256'], 'preflight_sha256': receipt['preflight_sha256'], 'queue_end_error': capture['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error']}
    retained = json.loads((Path(__file__).parent / 'matrix-alternate-receipt.json').read_text())
    assert compact == retained
    print('Alternate AddPoint stream and draw uniforms match retained receipt')
