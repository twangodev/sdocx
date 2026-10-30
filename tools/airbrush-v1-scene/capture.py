#!/usr/bin/env python3
"""Experimental same-target two-draw capture; pixels are comparison evidence only."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
runner = root / 'tmp/vector-parity-plan/pen-family-audit/airbrush-v1-short-native/frame-capture.py'
manifest = json.loads((root / 'tools/pen-capture/airbrush_v1_curved_prerequisites.json').read_text())
for path, expected in manifest.items():
    if hashlib.sha256((root / path).read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'prerequisite mismatch: {path}')
source = runner.read_text()
if hashlib.sha256(source.encode()).hexdigest() != 'd38a83f3cf71b228f58654c08a135d3f91d3bda0349df1d6014926752dc3b73e':
    raise RuntimeError('runner mismatch')
def replace(old, new):
    global source
    if source.count(old) != 1:
        raise RuntimeError(f'anchor mismatch: {old}')
    source = source.replace(old, new)
replace("'points':[_probe_start]*n", "'points':[[120,120],[120.125,120.25],[122,121],[124,118],[124.0625,118],[126,123]]")
replace("'pressure':[.3]*n", "'pressure':[0,.1,.9,.3,1,.5]")
replace("'timestamps':[300+15*i for i in range(n)]", "'timestamps':[300,301,310,450,451,900]")
replace("'tilt':[.12]*n", "'tilt':[0,.1,.5,.2,.7,.3]")
replace("'orientation':[.2]*n", "'orientation':[0,.2,.9,.4,1.2,.6]")
replace("expected_pre=bytes(initial_rgba)*(args.width*args.height)", "expected_pre=globals().get('first_pixels', bytes(initial_rgba)*(args.width*args.height))")
anchor = " draw_result=p.x(0); draw_rect=bytes(p.u.mem_read(rect,16)).hex()"
replace(anchor, '''
 first = airbrush_draw_observer['draw_readback']
 first_pixels = zlib.decompress(base64.b64decode(first['post_rgba8']['data_base64']))
 first_events = len(airbrush_buffer_events)
 obj2=p.alloc(0x100)
 p.call('SPen::ObjectStroke::ObjectStroke()', [obj2])
 p.call('SPen::ObjectStroke::Construct_DrawData()', [obj2],limit=1_000_000)
 if p.x(0)!=1: raise RuntimeError('second native ObjectStroke construction failed')
 p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2])
 p.call('SPen::ObjectStroke::SetSizeLevel(int)',[obj2,30])
 p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,input_case['tool_type']])
 p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,input_case['stroke_type']])
 p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,int(input_case['curve_enabled'])])
 p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1])
 p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname])
 shifted = [[x+2,y+1] for x,y in points]
 p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in shifted for v in xy)))
 p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,pts,prs,ts,tilt,orient,count])
 p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,0xffd93672])
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj2,1],limit=5000000)
 second = airbrush_draw_observer['draw_readback']
 print('TWO_DRAW',json.dumps({'first':first['post_rgba8'],'second_pre':second['pre_rgba8'],'second':second['post_rgba8'],'first_event_count':first_events,'second_event_count':len(airbrush_buffer_events)-first_events,'first_call':first['draw_call'],'second_call':second['draw_call']}),_always=True)
 raise SystemExit(0)
''' + anchor)
env = os.environ.copy()
env.pop('PYTHONOPTIMIZE', None)
env['AB_N'] = '6'
env['PYTHONPATH'] = ':'.join(str(root / p) for p in ('scratch/apk-analysis-runtime/python', 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin', 'tmp/conformance-cleanup/native-capture-a241793/conformance'))
env['LD_LIBRARY_PATH'] = str(root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
with tempfile.NamedTemporaryFile(mode='w', suffix='.py') as script:
    script.write(source)
    script.flush()
    subprocess.run([sys.executable, script.name, '--single-case', '--print-capture', '--advanced-mode', 'unset', '--width', '256', '--height', '256', '--initial-rgba', os.environ.get('AIRBRUSH_SCENE_BACKGROUND', '0,0,0,0'), '--source-size', '8.75', '--color-argb', '0x8044ee11'], cwd=root, env=env, check=True)
