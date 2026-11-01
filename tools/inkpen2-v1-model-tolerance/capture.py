"""Capture V1 Model tolerance controls on a translated saved trajectory."""
import hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[2]
pins={'crates/sdocx/tests/assets/fountain-v14/saved-source-29.json':'022d75f1dcfe52bb669280062cf95ac77e3818354d1cbb6edb5cb7265898e9dd','scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py':'4029c24aa69849941b124df47dff9bed117c413eebd66903b3414d7fffd35c1a','scratch/apk-analysis-native/arm64-v8a/libSPenInkPen2.so':'61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c'}
for p,h in pins.items(): assert hashlib.sha256((root/p).read_bytes()).hexdigest()==h,p
s=json.loads((root/next(iter(pins))).read_text()); n=len(s['points'])
case={'points':[[p['x']-96,p['y']-268] for p in s['points']],'size':9,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':s['pressures'],'timestamps':s['timestamps'],'tilt':s['tilts'][:n],'orientation':s['orientations'][:n]}
env=os.environ.copy(); env['PYTHONPATH']=os.pathsep.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
results={}
with tempfile.TemporaryDirectory() as d:
 for value in (None,0,0.5,2,100):
  case['tolerance']=value; inp=Path(d)/'input.json'; inp.write_text(json.dumps(case))
  raw=subprocess.check_output([sys.executable,str(root/'tools/inkpen2-v1-model-tolerance/caller.py'),'--print-capture','--pen-name','InkPen2','--advanced-setting','null','--input-json',str(inp)],cwd=root,env=env)
  x=json.loads(raw); assert x['native_calls']['route']['drawing_selected_drawable_version']=='1'
  streams={}
  for phase in ('draw_object_stroke','saved_redraw'):
   b=bytes.fromhex([a for a in x['callback_buffer_events'] if a['phase']==phase][-1]['after_bytes'])
   streams[phase]={'records':len(b)//12,'hex':b.hex(),'sha256':hashlib.sha256(b).hexdigest()}
  results[str(value)]=streams
print(json.dumps({'schema':'sdocx-inkpen2-v1-model-tolerance-v1','pins':pins,'source_points':n,'profile':[1920,1080],'tool':2,'size':9,'tolerances':results},indent=2))
