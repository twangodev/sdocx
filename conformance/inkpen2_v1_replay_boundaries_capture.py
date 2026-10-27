import json,struct,subprocess,sys,os,tempfile,hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[1]; src=json.loads((root/'conformance/inkpen2-null-v1-controls-native.json').read_text())['cases']; out={}
def f(x):return struct.unpack('<f',bytes.fromhex(x))[0]
env=os.environ.copy();env['PYTHONPATH']=':'.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
for name in ['baseline','duplicate','short','two_points','turn','reverse']:
 p=src[name]['input'];case={'points':[[f(x) for x in p] for p in p['points_f32_bits']],'size':f(p['pen_size_f32_bits']),'curve_enabled':p['curve_enabled'],'tool_type':p['tool_type'],'stroke_type':p['stroke_type'],'pressure':[f(x) for x in p['pressure_f32_bits']],'timestamps':p['timestamps_i32'],'tilt':[f(x) for x in p['tilt_f32_bits']],'orientation':[f(x) for x in p['orientation_f32_bits']]}
 with tempfile.NamedTemporaryFile(mode='w') as t:
  json.dump(case,t);t.flush(); x=json.loads(subprocess.check_output([sys.executable,str(root/'conformance/inkpen2_v1_replay_trace_native.py'),'--print-capture','--pen-name','InkPen2','--advanced-setting','null','--input-json',t.name],cwd=root,env=env))
 ev=[e for e in x['native_method_entries'] if e['phase']=='saved_redraw'];base=ev[0]['callback_count']; calls=[{'method':next(n for n in ['drawLine','endPen'] if '::'+n+'(' in e['signatures'][0]),'count':e['callback_count']-base} for e in ev if any('::'+n+'(' in e['signatures'][0] for n in ['drawLine','endPen'])]; total=sum(1 for e in x['callback_buffer_events'] if e['phase']=='saved_redraw');out[name]={'calls':calls,'total':total}; print(name,calls,total,flush=True)
receipt={'schema':'sdocx-inkpen2-v1-saved-callback-boundaries-v1','scope':'Synthetic native saved redraw, method entry and AddPoint return counts; this is not a native replay-controller capture.','cases':out,'capture_script':'conformance/inkpen2_v1_replay_boundaries_capture.py','source_controls':'conformance/inkpen2-null-v1-controls-native.json'};(root/'conformance/inkpen2-v1-saved-callback-boundaries-native.json').write_text(json.dumps(receipt,indent=2)+'\n')
