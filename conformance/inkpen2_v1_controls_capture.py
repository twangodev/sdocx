"""Capture compact InkPen2 null-setting synthetic native geometry controls."""
import copy, hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
harness=root/'conformance/inkpen2_v1_controls_native.py'
base={'points':[[0,0],[8,0],[16,3],[24,3]], 'size':4, 'curve_enabled':True, 'tool_type':2, 'stroke_type':0, 'pressure':[.2,.4,.6,.8], 'timestamps':[0,10,20,30], 'tilt':[0,0,0,0], 'orientation':[0,0,0,0]}
cases={'baseline':{}, 'pressure_constant':{'pressure':[.5]*4}, 'size_two':{'size':2}, 'curve_disabled':{'curve_enabled':False}, 'tool_zero':{'tool_type':0}, 'translated':{'points':[[10,20],[18,20],[26,23],[34,23]]}}
env=os.environ.copy(); env['PYTHONPATH']=os.pathsep.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
out={}
with tempfile.TemporaryDirectory() as temp:
 for name,changes in cases.items():
  case=copy.deepcopy(base);case.update(changes); p=Path(temp)/'input.json';p.write_text(json.dumps(case))
  x=json.loads(subprocess.check_output([sys.executable,str(harness),'--print-capture','--pen-name','InkPen2','--advanced-setting','null','--input-json',str(p)],cwd=root,env=env))
  assert x['native_calls']['route']['drawing_selected_drawable_version']=='1'
  events=x['callback_buffer_events']; draw=[e for e in events if e['phase']=='draw_object_stroke']; saved=[e for e in events if e['phase']=='saved_redraw']; assert draw and saved and draw[-1]['after_bytes']==saved[-1]['after_bytes']
  data=bytes.fromhex(saved[-1]['after_bytes'])
  out[name]={'input':x['input'],'stream_hex':data.hex(),'stream_sha256':hashlib.sha256(data).hexdigest(),'record_count':len(data)//12,'redraw_rect_f32_bits':x['native_calls']['separate_saved_RedrawPen_rect_f32_bits']}
  print(name,len(data)//12,file=sys.stderr,flush=True)
receipt={'schema':'sdocx-inkpen2-null-v1-controls-native-v1','profile':{'width':1920,'height':1080,'identity_canvas_matrix':True},'scope':'Synthetic ObjectStroke setters through native Drawing and saved RedrawPen; callback records only, no paint or deserialization.','source_harness_sha256':hashlib.sha256((root/'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py').read_bytes()).hexdigest(),'capture_script':'conformance/inkpen2_v1_controls_capture.py','cases':out}
(root/'conformance/inkpen2-null-v1-controls-native.json').write_text(json.dumps(receipt,indent=2)+'\n')
