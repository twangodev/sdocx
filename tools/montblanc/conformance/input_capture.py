#!/usr/bin/env python3
"""Probe saved-model setters through the pinned native Montblanc Drawing harness."""
import hashlib
import json
import os
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
EXPECTED = 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'
source = SOURCE.read_bytes()
if hashlib.sha256(source).hexdigest() != EXPECTED:
    raise SystemExit('native harness hash mismatch')
case = json.loads(os.environ['MONTBLANC_INPUT_CASE'])
script = source.decode()
script = script.replace("advanced_value='1;'", 'advanced_value=' + repr(case.get('setting', '1;')))
script = script.replace("[obj,2]);call('SPen::ObjectStroke::SetStrokeType", "[obj," + str(case.get('tool', 2)) + "]);call('SPen::ObjectStroke::SetStrokeType")
if case.get('omit_size'):
    script = script.replace("call('SPen::ObjectStroke::SetPenSize(float)',[obj],[CASE_INPUT['pen_size']])", '')
if 'color' in case:
    script = script.replace("# Exactly three sample positions", "call('SPen::ObjectStroke::SetColor(unsigned int)',[obj],[] )\n# Exactly three sample positions".replace('[obj],[]', '[obj,' + str(case['color']) + ']'))
marker = "p.u.hook_del(alpha_source_hook)\nmanager=p.get(drawing+16,'Q')[0]"
assert script.count(marker) == 1
script = script[:script.index(marker)] + '''
stream=[[row['entry'],row['float_words_s0_s5']] for row in add_calls]
result={'return':rc,'model_size_f32_le':model_size_bits,
        'set_size_f32_le':[row['input_s0_f32_bits'] for row in pen_set_calls],
        'redraw_entries':[row['entry'] for row in redraws], 'add_count':len(stream),
        'stream_sha256':hashlib.sha256(json.dumps(stream,separators=(',',':')).encode()).hexdigest(),
        'montblanc_factory_selected':any(row.get('handle')==hex(MB_HANDLE) and row.get('name')=='createPenInst' for row in loader)}
'''
sys.argv = [str(SOURCE), '--case', 'baseline']
namespace = {'__file__': str(SOURCE), '__name__': '__main__'}
exec(compile(script, str(SOURCE), 'exec'), namespace)
print(json.dumps(namespace['result'], sort_keys=True))
