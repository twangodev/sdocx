import pathlib,tempfile,hashlib,json,subprocess,sys,zlib,base64
root=pathlib.Path(__file__).resolve().parents[2]; src=root/'scratch/oilbrush3-public-drawing-v17'; original_path=src/'oilbrush3_public_drawing_capture.py'; original=original_path.read_text(); assert hashlib.sha256(original_path.read_bytes()).hexdigest()==json.loads((root/'tools/oilbrush3-cadence/receipt.json').read_text())['original_runner_sha256']; old='[[0.0,0.0],[10.0,0.0],[20.0,5.0],[30.0,5.0]]'; new='[[500.0,500.0],[510.0,500.0],[520.0,505.0],[530.0,505.0]]'; assert original.count(old)==1
with tempfile.TemporaryDirectory(dir='/tmp') as tmp:
 p=pathlib.Path(tmp); runner=p/'oilbrush3_public_drawing_capture.py'; injection='''
 if capture and index==0:
  assert before['viewport']==[0,0,1920,1080]
  attachments=before['draw_framebuffer']['color_attachments']; assert attachments[0]['object_name']==1
  bind=ip.cfunc(_gl,'glBindTexture',None,[ctypes.c_uint,ctypes.c_uint])
  active=_oil_gl_int(0x84E0); _gl_active_texture(0x84C0); previous=_oil_gl_int(0x8069); bind(0x0DE1,1)
  assert _oil_gl_int(0x88EF)==0 and _oil_gl_int(0x0CF5) in (1,2,4,8) and all(_oil_gl_int(x)==0 for x in (0x0CF2,0x0CF3,0x0CF4))
  raw=bytes(v for y in range(64) for x in range(64) for v in (x*3,y*3,(x+y)*2,255))
  data=ctypes.create_string_buffer(raw); _gl_tex_sub_image_2d(0x0DE1,0,480,480,64,64,0x1908,0x1401,ctypes.cast(data,ctypes.c_void_p))
  bind(0x0DE1,previous); _gl_active_texture(active)
  before=_oil_draw_snapshot(index,call,args)
'''; needle=' # The original inherited callback runs once and forwards the original native'; assert original.count(needle)==1; original=original.replace(needle,injection+needle); runner.write_text(original.replace(old,new)); (p/'oilbrush3_canvas_native.py').write_bytes((src/'oilbrush3_canvas_native.py').read_bytes()); manifest_raw=(src/'preflight.json').read_bytes(); assert hashlib.sha256(manifest_raw).hexdigest()==json.loads((root/'conformance/oilbrush3-nonuniform-feedback/receipt.json').read_text())['preflight_sha256']; manifest=json.loads(manifest_raw); manifest['runner_sha256']=hashlib.sha256(runner.read_bytes()).hexdigest(); m=p/'preflight.json'; m.write_text(json.dumps(manifest)); cmd=[sys.executable,str(runner),'--preflight-manifest',str(m),'--preflight-sha256',hashlib.sha256(m.read_bytes()).hexdigest(),'--compact','--failure-json',str(p/'failure.json')]; result=subprocess.run(cmd,cwd=root,capture_output=True); print('return',result.returncode); print(result.stderr.decode()[-1200:]);
 if result.returncode: print(result.stdout.decode()[-1000:]); sys.exit(1)
 c=json.loads(result.stdout); ds=c['mesa_draw_observations'];
 fixture=root/'conformance/oilbrush3-nonuniform-feedback'
 for i in range(3):
  raw=zlib.decompress(base64.b64decode(ds[i]['after']['pixels']['payload_base64']))
  name = f'p4-{i+1}.rgba.zlib' if i < 2 else 'page-before.rgba.zlib'
  assert raw == zlib.decompress((fixture/name).read_bytes())
 records=b''.join(bytes.fromhex(x['append_bytes_hex']) for x in c['native_abi']['chunks'])
 assert records == (fixture/'addpoints.bin').read_bytes()
 uniforms=[]
 for d in ds[:3]:
  u={e['uniform_location_binding']['name']:base64.b64decode(e['payload_base64']) for e in d['uniform_events_since_prior_draw'] if e.get('payload_base64') and e.get('uniform_location_binding',{}).get('name')}
  uniforms.append({k:v.hex() for k,v in u.items()})
 assert uniforms == json.loads((fixture/'p4-uniforms.json').read_text())
 print('records',len(records),'hash',hashlib.sha256(records).hexdigest())

 for d in ds:
  if d['before']['viewport']==[0,0,16,16]:
   pix=d['after']['pixels']; raw=zlib.decompress(base64.b64decode(pix['payload_base64'])); print('P3',d['index'],'unique',len(set(zip(*[iter(raw)]*4))), 'sha',pix['sha256'])
 assert c['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error']==0
 assert ds[0]['before']['pixels']['nonzero_pixels']==4096
 initial=zlib.decompress(base64.b64decode(ds[0]['before']['pixels']['payload_base64']))
 assert all(initial[((480+y)*1920+480+x)*4:((480+y)*1920+480+x)*4+4]==bytes((3*x,3*y,2*(x+y),255)) for y in range(64) for x in range(64))
 assert ds[0]['before']['draw_framebuffer']['color_attachments'][0]['object_name']==ds[3]['before']['bound_texture_units']['captured_units'][0]['texture_2d']==1
 assert ds[3]['before']['draw_framebuffer']['color_attachments'][0]['object_name']==10
 assert [len(set(zip(*[iter(zlib.decompress(base64.b64decode(d['after']['pixels']['payload_base64'])))]*4))) for d in ds if d['before']['viewport']==[0,0,16,16]]==[49,71,73]
 for name,pixels in [('page-before',ds[2]['after']['pixels']),('feedback-after',ds[3]['after']['pixels'])]:
  actual=zlib.decompress(base64.b64decode(pixels['payload_base64'])); retained=zlib.decompress((root/'conformance/oilbrush3-nonuniform-feedback'/f'{name}.rgba.zlib').read_bytes()); assert actual==retained, name
 uniforms={e['uniform_location_binding']['name']:base64.b64decode(e['payload_base64']) for e in ds[3]['uniform_events_since_prior_draw'] if e.get('payload_base64') and e.get('uniform_location_binding',{}).get('name')}
 assert __import__('struct').unpack_from('<f',(root/'conformance/oilbrush3-v17-native-paint/oilbrush3-v17-draw-state-f32le.bin').read_bytes(),64+15*72+136)[0]==__import__('struct').unpack('<f',__import__('struct').pack('<f',0.4))[0]
 state=uniforms['uProjectionMatrix']+uniforms['uModelMatrix']+uniforms['uDepthMapScale']+__import__('struct').pack('<f',0.4)
 assert state==(root/'conformance/oilbrush3-nonuniform-feedback/p3-state.bin').read_bytes()
 print('Native post-P4 page and first P3 readback match retained fixtures')
