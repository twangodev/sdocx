#!/usr/bin/env python3
"""Capture endPen branch and interpolation registers for controlled strokes."""
import hashlib
import json
import os
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[3]
source = root / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
raw = source.read_bytes()
if hashlib.sha256(raw).hexdigest() != 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8':
    raise SystemExit('native harness hash mismatch')
s = raw.decode()
case = json.loads(os.environ['MB_CASE'])
s = s.replace('CASE_INPUT=CASE_INPUTS[CASE]', 'CASE_INPUT=' + repr(case))
a = s.index('# Exactly three sample positions')
b = s.index('model_arrays={}', a)
s = s[:a] + '''n=len(CASE_INPUT['points'])
pts=p.alloc(n*8);prs=p.alloc(n*4);ts=p.alloc(n*4);tilt=p.alloc(n*4);ori=p.alloc(n*4)
p.put(pts,str(n*2)+'f',*sum(CASE_INPUT['points'],[]));p.put(prs,str(n)+'f',*CASE_INPUT['pressure']);p.put(ts,str(n)+'i',*[100+10*i for i in range(n)]);p.put(tilt,str(n)+'f',*[0]*n);p.put(ori,str(n)+'f',*[0]*n)
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,n])
''' + s[b:]
s = s.replace('range(24)', 'range(25)')
s = s.replace('alpha_source_addrs={', '''ALPHA_SOURCE_RVAS={r: hex(r) for r in (0x1d890,0x1d8a0,0x1d8dc,0x1d8e4,0x1dc68,0x1dc8c,0x1dca0,0x1dcc0,0x1dcfc,0x1dd00)}
alpha_source_addrs={''')
marker = "p.u.hook_del(alpha_source_hook)\nmanager=p.get(drawing+16,'Q')[0]"
s = s[:s.index(marker)] + '''
result=[]
for row in alpha_source_rows:
 result.append({'rva':row['rva'],'x_registers':{k:row['x_registers'][k] for k in ('x0','x2','x8','x9','x24')},'s_registers':{k:row['s_registers'][k] for k in ('s0_f32_bits','s1_f32_bits','s8_f32_bits','s11_f32_bits','s13_f32_bits')},'drawable_member_bytes_offset_80_len_48':row['drawable_member_bytes_offset_80_len_48'],'loop_iteration':row['loop_iteration'],'loop_repeat':row['loop_repeat']})
'''
sys.argv = [str(source), '--case', 'baseline']
ns = {'__file__': str(source), '__name__': '__main__'}
exec(compile(s, str(source), 'exec'), ns)
print(json.dumps(ns['result']))
