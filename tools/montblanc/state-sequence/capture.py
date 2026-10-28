#!/usr/bin/env python3
"""Replay a controlled A/B/A sequence through the pinned native Drawing harness."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
EXPECTED = 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'
source = SOURCE.read_bytes()
if hashlib.sha256(source).hexdigest() != EXPECTED:
    raise SystemExit('native harness hash mismatch')
script = source.decode()
marker = "p.u.hook_del(alpha_source_hook)\nmanager=p.get(drawing+16,'Q')[0]"
replacement = '''first_add_count=len(add_calls); first_redraw_count=len(redraws); first_size_count=len(pen_set_calls)
p.u.hook_del(alpha_source_hook)
p.put(prs,'3f',0.3,0.85,0.9)
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])
rc2=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])
second_add_end=len(add_calls); second_redraw_end=len(redraws); second_size_end=len(pen_set_calls)
p.put(prs,'3f',*CASE_INPUT['pressure'])
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])
rc3=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])
manager=p.get(drawing+16,'Q')[0]'''
if script.count(marker) != 1:
    raise SystemExit('harness insertion point changed')
script = script.replace(marker, replacement)
script = script[:script.index('records=[{')] + '''
result={'first_add':add_calls[:first_add_count], 'second_add':add_calls[first_add_count:second_add_end], 'third_add':add_calls[second_add_end:], 'redraws':[redraws[:first_redraw_count],redraws[first_redraw_count:second_redraw_end],redraws[second_redraw_end:]], 'sizes':[pen_set_calls[:first_size_count],pen_set_calls[first_size_count:second_size_end],pen_set_calls[second_size_end:]], 'returns':[rc,rc2,rc3]}
'''
sys.argv = [str(SOURCE), '--case', 'baseline']
namespace = {'__file__': str(SOURCE), '__name__': '__main__'}
exec(compile(script, str(SOURCE), 'exec'), namespace)
result = namespace['result']
def words(records):
    return [[row['entry'], row['float_words_s0_s5']] for row in records]
def digest(value):
    return hashlib.sha256(json.dumps(value, separators=(',', ':')).encode()).hexdigest()
fresh = {}
for case in ('baseline', 'pressure'):
    path = SOURCE.parent / 'captures' / (case + '.json')
    fresh[case] = words(json.loads(path.read_bytes())['cases'][0]['actual_montblanc_rtv1_addpoint_records'])
a, b, c = (words(result[key + '_add']) for key in ('first', 'second', 'third'))
summary = {'counts': list(map(len, (a, b, c))), 'returns': result['returns'],
           'same_gl_receiver': len({r['this'] for batch in result['redraws'] for r in batch}) == 1,
           'same_rtv_receiver': len({r['receiver'] for key in ('first', 'second', 'third') for r in result[key + '_add']}) == 1,
           'first_equals_fresh_baseline': a == fresh['baseline'], 'second_equals_fresh_pressure': b == fresh['pressure'],
           'third_equals_fresh_baseline': c == fresh['baseline'], 'stream_sha256': list(map(digest, (a, b, c))),
           'pen_size_f32_bits': [[r['input_s0_f32_bits'] for r in batch] for batch in result['sizes']]}
print(json.dumps(summary, sort_keys=True, indent=2))
if not (summary['same_gl_receiver'] and summary['same_rtv_receiver'] and summary['first_equals_fresh_baseline'] and summary['second_equals_fresh_pressure'] and summary['third_equals_fresh_baseline']):
    raise SystemExit('sequence differs from frozen fresh controls')
