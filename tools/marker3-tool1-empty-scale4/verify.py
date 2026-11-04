import hashlib,json,struct,sys
from pathlib import Path
root=Path(sys.argv[1]); cases={}
short = [(25.25, 28.5), (32.25, 39.5), (42.25, 30.5), (49.25, 41.5), (55.25, 31.5)]
expected = {
    'short4': (short, [0.1, 0.9, 0.3, 0.8, 0.2], 4.0, '0x8033cc55'),
    'short6': (short, [0.1, 0.9, 0.3, 0.8, 0.2], 6.0, '0xffd14a87'),
    'fractional': ([(25.25, 28.5), (67.75, 46.125)], [0.2, 0.9], 8.0, '0x8033cc55'),
    'crossing': ([(35.25, 35.5), (85.75, 85.25), (35.25, 85.25), (85.75, 35.5)], [0.2, 0.9, 0.4, 1.0], 8.0, '0xffd14a87'),
}
for case in ('short4','short6','fractional','crossing'):
 a,b=[json.loads((root/f'{case}-{setting}'/'drawing-paint-positive.json').read_text()) for setting in ('5','empty')]
 fa,fb=[(root/f'{case}-{setting}'/'drawing-paint-positive.rgba').read_bytes() for setting in ('5','empty')]
 ia,ib=[r['source_caller_case']['input'].copy() for r in (a,b)]
 assert ia.pop('advanced_setting')=='5;' and ib.pop('advanced_setting')=='' and ia==ib
 points,pressures,size,color=expected[case]
 bits=lambda v:struct.pack('<f',v).hex()
 assert ia['pen_name'].endswith('.StraightMarker')
 assert ia['points_f32_bits']==[[bits(x),bits(y)] for x,y in points]
 assert ia['pressure_f32_bits']==list(map(bits,pressures)) and ia['pen_size_f32_bits']==bits(size)
 assert ia['color_argb']==color and ia['tool_type']==1 and ia['timestamps_i32']==[8*i for i in range(len(points))]
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
