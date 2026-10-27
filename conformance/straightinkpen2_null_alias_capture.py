"""Reproduce the synthetic null-setting InkPen2/StraightInkPen2 dispatch control.

Requires the retained scratch/straightinkpen2-alias harness and its pinned native
runtime. This checks Drawing and callback geometry, not device paint or parsing.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / 'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py'
EXPECTED = '773e365a08bcb63fd25b1d5895a7d8f5d5f6a56820678dc8a93e2207e0a15d01'
STREAM = '4c44864e52c01d3134712d1975692b2f6d29ce2a8e06ce141f492ea0d2ecd6fb'

def capture(name):
    receipt = json.loads((ROOT / 'conformance/straightinkpen2-null-alias-native.json').read_text())
    assert hashlib.sha256(HARNESS.read_bytes()).hexdigest() == receipt['harness_sha256']
    env = os.environ.copy()
    env['PYTHONPATH'] = os.pathsep.join(str(ROOT / p) for p in (
        'scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python',
        'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
    output = subprocess.check_output([sys.executable, str(HARNESS), '--print-capture',
        '--pen-name', name, '--case', 'line0_curve_short_history', '--advanced-setting', 'null'],
        cwd=ROOT, env=env)
    return json.loads(output)

def normalize(capture):
    route = capture['native_calls']['route']
    assert route['advanced_getter_pointer_nonnull'] is False
    assert route['drawing_selected_drawable_version'] == '1'
    assert route['saved_redraw_invoked_version'] == '1'
    events = [{k: v for k, v in event.items() if k not in ('rt', 'vector_object')}
              for event in capture['callback_buffer_events']]
    assert len(events) == 50
    encoded = json.dumps(events, sort_keys=True, separators=(',', ':')).encode()
    assert hashlib.sha256(encoded).hexdigest() == EXPECTED
    assert hashlib.sha256(bytes.fromhex(events[-1]['after_bytes'])).hexdigest() == STREAM
    return events

if __name__ == '__main__':
    first = normalize(capture('InkPen2'))
    second = normalize(capture('StraightInkPen2'))
    assert first == second
    print('verified null-setting V1 selection and 50 callback events for both names')
