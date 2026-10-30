#!/usr/bin/env python3
"""Compare hash-verified native display controls' bound vector uniforms."""
import base64
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys

here = Path(__file__).resolve().parent
assert len(sys.argv) == 3, 'pass the 256 and 512 capture logs'
results = {}
provenance = {}
pins = {256: '79631bd2adb07adb3e8e7975f4d1937328d2c1340f4b2b5f59515458fa8ecf8f', 512: '026bb206b55207e9b3e358088068762b91214f1f3e7aa826744d24a12f0cd31b'}
for display, filename in zip((256, 512), sys.argv[1:], strict=True):
    subprocess.run([sys.executable, str(here / f'verify-identity-screen-{display}.py'), filename], check=True)
    raw = Path(filename).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == pins[display], 'capture log identity differs'
    text = raw.decode()
    capture, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
    uses = [e for e in capture['mesa_program_lifecycle']['events'] if e['call'] == 'glUseProgram']
    assert [e['program'] for e in uses] == [4, 5, 8]
    assert [e['sequence'] for e in uses] == sorted(e['sequence'] for e in uses)
    assert all(e['phase'] == 'native_public_queue_run_23' for e in uses)
    draws = capture['mesa_draw_observations']
    assert [(d['index'], d['api'], d['before']['current_program']) for d in draws] == [(0, 'glDrawArraysInstanced', 4), (1, 'glDrawArrays', 5), (2, 'glDrawArrays', 8)]
    expected = {0: {'uVertexHalfExtent': 1, 'uEllipseRadii': 5, 'uPosOffset': 2}, 1: {'patternSize': 1, 'grainOffset': 2, 'uShadowMaskOffsetUV': 7}}
    values = {}
    selected = []
    for draw_index, names in expected.items():
        draw = draws[draw_index]
        program = draw['before']['current_program']
        for name, location in names.items():
            bindings = [e for e in capture['mesa_uniform_location_events'] if e['program'] == program and e['location'] == location and e['name_utf8'] == name]
            assert len(bindings) == 1
            assert bindings[0]['name_bytes_hex'] == name.encode().hex()
            events = [e for e in draw['uniform_events_since_prior_draw'] if e['call'] == 'glUniform2fv' and e['location'] == location]
            assert len(events) == 1
            event = events[0]
            assert event['current_mesa_program'] == program
            assert event['uniform_location_binding']['name'] == name
            assert event['uniform_location_binding']['program'] == program
            assert event['uniform_location_binding']['location'] == location
            payload = base64.b64decode(event['payload_base64'], validate=True)
            assert hashlib.sha256(payload).hexdigest() == event['payload_sha256']
            values[name] = list(struct.unpack('<2f', payload))
            selected.append({'draw_index': draw_index, 'program': program, 'location': location, 'name': name, 'payload_base64': event['payload_base64'], 'payload_sha256': event['payload_sha256'], 'binding': bindings[0]})
    provenance[str(display)] = {'capture_log_sha256': pins[display], 'use_program_events': uses, 'draws': [{'index': d['index'], 'api': d['api'], 'current_program': d['before']['current_program'], 'phase': d['before']['phase']} for d in draws], 'uniforms': selected}
    keys = ('uVertexHalfExtent', 'uEllipseRadii', 'uPosOffset', 'patternSize', 'grainOffset', 'uShadowMaskOffsetUV')
    results[str(display)] = {key: values[key] for key in keys}
assert results['256']['uVertexHalfExtent'] == [1.6800000667572021, 6.720000267028809]
assert results['512']['uVertexHalfExtent'] == [1.7754546403884888, 7.101818561553955]
assert results['256']['patternSize'] == [64.0, 64.0]
assert results['512']['patternSize'] == [128.0, 128.0]
for key in ('uEllipseRadii', 'uPosOffset', 'grainOffset', 'uShadowMaskOffsetUV'):
    assert results['256'][key] == results['512'][key]
receipt = {'scope': 'Paired synthetic identity-matrix host-Mesa Drawing controls; native GLSL uniform calls; not saved-document or device parity', 'uniforms': results, 'provenance': provenance, 'interpretation': 'The AA-only source approximation holds mask quad extent and pattern size fixed; both change with display in these captures. These observations do not alone establish the full native mapping or compositing.', 'capture_verification': 'verify-identity-screen-256.py and verify-identity-screen-512.py; each checks input, native library hashes and frame bytes', 'replay': 'python3 tools/tapepen-public-drawing/identity-screen-state.py /path/to/256.log /path/to/512.log'}
(here / 'identity-screen-state-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('verified pinned logs, program sequence, draw association and uniform bindings')
