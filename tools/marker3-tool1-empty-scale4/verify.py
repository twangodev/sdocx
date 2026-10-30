import ast,hashlib,json,re,struct,sys
from pathlib import Path
root=Path(sys.argv[1]); cases={}
for case in ('short4','short6','fractional','crossing'):
 a,b=[json.loads((root/f'{case}-{setting}'/'drawing-paint-positive.json').read_text()) for setting in ('5','empty')]
 fa,fb=[(root/f'{case}-{setting}'/'drawing-paint-positive.rgba').read_bytes() for setting in ('5','empty')]
 ia,ib=[r['source_caller_case']['input'].copy() for r in (a,b)]
 assert ia.pop('advanced_setting')=='5;' and ib.pop('advanced_setting')=='' and ia==ib
 source=Path(__file__).with_name('export.rs').read_text()
 assert 'preload.StraightMarker' in source and '"tool_type_raw":1' in source and '"advanced_settings":""' in source
 branch='short4' if case.startswith('short') else case
 line=next(line for line in source.splitlines() if line.lstrip().startswith('"'+branch+'"'))
 vectors=re.findall(r'vec!(\[.*?\])(?=,vec!|,0x|,if)',line)
 points,pressures=map(ast.literal_eval,vectors[:2]); shift=-0.0625 if case=='short4' else 0.0625 if case=='short6' else 0; phase=0 if case=='short4' else 0.5 if case=='short6' else 0.25
 bits=lambda v:struct.pack('<f',v).hex()
 assert [[bits(x+shift),bits(y+shift)] for x,y in points]==[[bits(struct.unpack('<f',bytes.fromhex(x))[0]+phase/4),bits(struct.unpack('<f',bytes.fromhex(y))[0]+phase/4)] for x,y in ia['points_f32_bits']]
 assert list(map(bits,pressures))==ia['pressure_f32_bits'] and ia['pen_size_f32_bits']==bits(4 if case=='short4' else 6 if case=='short6' else 8)
 assert ia['color_argb'][2:] in line.lower() and ia['tool_type']==1 and ia['timestamps_i32']==[8*i for i in range(len(points))]
 assert ia['tilt_f32_bits']==ia['orientation_f32_bits']==[bits(0)]*len(points) and ia['curve_enabled'] and ia['stroke_type']==0
 for r,value in ((a,'5;'),(b,'')):
  assert r['source_caller_case']['native_calls']['ObjectStroke_GetAdvancedPenSetting']=={'is_null':False,'utf8':value}
  assert r['selected_drawable_version']=='5' and all(c['error']==0 for c in r['gl_error_checks'])
 for key in ('native_vector_sha256','native_vector_bytes','callback_count','readback','draws','native_rt_color_rgba_f32','matching_native_gpu_uploads','library_hashes','helper_hashes','host_gles'):
  assert a[key]==b[key],(case,key)
 assert a['source_caller_case']['phase_final_vectors']==b['source_caller_case']['phase_final_vectors']
 def callbacks(r):
  return [{k:v for k,v in row.items() if k not in ('vector_begin','vector_object')} for row in r['source_caller_case']['callback_add_point_calls']]
 assert callbacks(a)==callbacks(b),(case,'callbacks')
 assert fa==fb and len(fa)==512*512*4
 cases[case]={'input':ia,'native_settings':[a['source_caller_case']['native_calls']['ObjectStroke_GetAdvancedPenSetting'],b['source_caller_case']['native_calls']['ObjectStroke_GetAdvancedPenSetting']],'native_vector_sha256':a['native_vector_sha256'],'native_vector_bytes':a['native_vector_bytes'],'callback_count':a['callback_count'],'frame_sha256':hashlib.sha256(fa).hexdigest(),'draws':a['draws'],'library_hashes':a['library_hashes'],'helper_hashes':a['helper_hashes'],'host_gles':a['host_gles']}
Path(sys.argv[2]).open('x').write(json.dumps({'scope':'Paired synthetic StraightMarker tool 1 4x native Drawing and RTV1 host Mesa; no saved note, device or opaque backdrop claim.','cases':cases},indent=2,sort_keys=True)+'\n')
