import pathlib,tempfile,hashlib,json,subprocess,sys,zlib,base64,struct
root=pathlib.Path(__file__).resolve().parents[2]; src=root/'scratch/oilbrush3-public-drawing-v17'; original=(src/'oilbrush3_public_drawing_capture.py').read_text(); assert hashlib.sha256(original.encode()).hexdigest()==json.loads((root/'conformance/oilbrush3-independent-feedback/receipt.json').read_text())['native_runner_sha256']; old='[[0.0,0.0],[10.0,0.0],[20.0,5.0],[30.0,5.0]]'; new='[[500.0,500.0],[504.0,510.0],[515.0,515.0],[525.0,503.0]]'; assert original.count(old)==1
injection='''
 if capture and index==0:
  assert before['viewport']==[0,0,1920,1080]
  bind=ip.cfunc(_gl,'glBindTexture',None,[ctypes.c_uint,ctypes.c_uint]); active=_oil_gl_int(0x84E0); _gl_active_texture(0x84C0); previous=_oil_gl_int(0x8069); bind(0x0DE1,1)
  raw=bytes(v for y in range(64) for x in range(64) for v in ((x*7)%256,(y*5)%256,((x^y)*3)%256,255))
  data=ctypes.create_string_buffer(raw); _gl_tex_sub_image_2d(0x0DE1,0,480,480,64,64,0x1908,0x1401,ctypes.cast(data,ctypes.c_void_p)); bind(0x0DE1,previous); _gl_active_texture(active); before=_oil_draw_snapshot(index,call,args)
'''
needle=' # The original inherited callback runs once and forwards the original native'; assert original.count(needle)==1
with tempfile.TemporaryDirectory(dir='/tmp') as tmp:
 p=pathlib.Path(tmp); runner=p/'oilbrush3_public_drawing_capture.py'; runner.write_text(original.replace(needle,injection+needle).replace(old,new)); (p/'oilbrush3_canvas_native.py').write_bytes((src/'oilbrush3_canvas_native.py').read_bytes()); manifest_raw=(src/'preflight.json').read_bytes(); assert hashlib.sha256(manifest_raw).hexdigest()==json.loads((root/'conformance/oilbrush3-independent-feedback/receipt.json').read_text())['preflight_sha256']; manifest=json.loads(manifest_raw); manifest['runner_sha256']=hashlib.sha256(runner.read_bytes()).hexdigest(); m=p/'preflight.json'; m.write_text(json.dumps(manifest)); result=subprocess.run([sys.executable,str(runner),'--preflight-manifest',str(m),'--preflight-sha256',hashlib.sha256(m.read_bytes()).hexdigest(),'--compact','--failure-json',str(p/'failure.json')],cwd=root,capture_output=True); print('return',result.returncode, result.stderr.decode()[-1000:]);
 if result.returncode: print(result.stdout.decode()[-1000:]);sys.exit(1)
 c=json.loads(result.stdout); ds=c['mesa_draw_observations']; receipt=json.loads((root/'conformance/oilbrush3-independent-feedback/receipt.json').read_text()); out=root/'conformance/oilbrush3-independent-feedback';out.mkdir(exist_ok=True); records=b''.join(bytes.fromhex(x['append_bytes_hex']) for x in c['native_abi']['chunks']); assert (out/'addpoints.bin').read_bytes()==records
 for name, digest in receipt['input_sha256'].items(): assert hashlib.sha256((out/name).read_bytes()).hexdigest() == digest
 for i in range(5):
  identity=receipt['draw_identity'][i]
  for phase in ('before','after'):
   snapshot=ds[i][phase]; pixels=snapshot['pixels']
   assert snapshot['current_program']==identity['program'] and snapshot['viewport']==identity['viewport']
   for target in ('draw_framebuffer','read_framebuffer'):
    framebuffer=snapshot[target]; assert framebuffer['framebuffer']==identity['framebuffer'] and framebuffer['status']==0x8CD5
    assert any(a['object_name']==identity['texture'] and a['object_type']==0x1702 for a in framebuffer['color_attachments'])
   assert (pixels['status'],pixels['width'],pixels['height'],pixels['texture_id'],pixels['format'],pixels['origin']) == ('ok',*identity['size'],identity['texture'],'RGBA8/UNSIGNED_BYTE','GL lower-left')
  raw=zlib.decompress(base64.b64decode(ds[i]['after']['pixels']['payload_base64'])); assert hashlib.sha256(raw).hexdigest()==receipt['raw_sha256'][i]; assert zlib.decompress((out/f'after-{i}.zlib').read_bytes()) == raw; print(i,len(raw),hashlib.sha256(raw).hexdigest())
 uniforms=[]
 for d in ds[:4]:
  u={e['uniform_location_binding']['name']:base64.b64decode(e['payload_base64']).hex() for e in d['uniform_events_since_prior_draw'] if e.get('payload_base64') and e.get('uniform_location_binding',{}).get('name')};uniforms.append(u)
 assert json.loads((out/'uniforms.json').read_text()) == uniforms; assert c['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error']==0; print('records',len(records),'Mesa error 0')
