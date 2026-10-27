"""Capture source color and stroke-type controls through native Drawing."""
import copy, hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
base={'points':[[0,0],[8,0],[16,3],[24,3]], 'size':4, 'curve_enabled':True, 'tool_type':2, 'stroke_type':0, 'pressure':[.2,.4,.6,.8], 'timestamps':[0,10,20,30], 'tilt':[0]*4, 'orientation':[0]*4}
cases={'opaque':{'color_argb':0xff336699}, 'half_alpha':{'color_argb':0x80336699}, 'zero_alpha':{'color_argb':0x00336699}}
env=os.environ.copy();env['PYTHONPATH']=os.pathsep.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
out={}
with tempfile.TemporaryDirectory() as tmp:
 for name,changes in cases.items():
  case=copy.deepcopy(base);case.update(changes);path=Path(tmp)/'input.json';path.write_text(json.dumps(case))
  raw=subprocess.check_output([sys.executable,str(root/'conformance/inkpen2_v1_style_native.py'),'--print-capture','--pen-name','InkPen2','--advanced-setting','null','--input-json',str(path)],cwd=root,env=env)
  x=json.loads(raw); result={'input':case,'factory_count':x.get('factory_count',1)}
  if result['factory_count']:
   result.update({'route':x['native_calls']['route'],'paint_color_calls':x['native_calls'].get('paint_color_calls',[]),'source_color_getter':x['native_calls'].get('source_ObjectStroke_GetColor_u32'),'stream_sha256':hashlib.sha256(bytes.fromhex([e for e in x['callback_buffer_events'] if e['phase']=='saved_redraw'][-1]['after_bytes'])).hexdigest(),'pen_size_bits':x['native_calls']['pen_GetSize_after_draw_f32_bits']})
  else: result['native_result']=x
  out[name]=result;print(name, result['factory_count'],file=sys.stderr,flush=True)
receipt={'schema':'sdocx-inkpen2-null-v1-style-native-v1','scope':'Synthetic ObjectStroke setters through native Drawing and direct saved redraw; no deserialization, paint readback or device execution.','capture_script':'conformance/inkpen2_v1_style_capture.py','source_harness_sha256':hashlib.sha256((root/'scratch/straightinkpen2-alias/inkpen2_saved_caller_native.py').read_bytes()).hexdigest(),'cases':out}
(root/'conformance/inkpen2-null-v1-style-native.json').write_text(json.dumps(receipt,indent=2)+'\n')
