import ast,sys,os,hashlib,struct,subprocess,json,argparse,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
EXPECTED_HELPERS={
 'airbrush_texture_native.py':'76bb130daef795be53296cff4f778be6e7be5f487c8b1d4c4b6d159bb688e470',
 'airbrush_saved_native.py':'cad2bd9ec88a393f0b0b4a7dbe58c7b469f9b00b283d4c544f491a8ab190999a',
 'inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
 'fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
 'fountain-saved-caller-native.json':'552dbabd5cf1b760cf63c8fba967c7a6fe14ca124155ccc929d0587a7ffa4695',
}
EXPECTED_DSOS={
 'Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'Marker3':'d9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5',
 'PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
}
EXPECTED_LIBCXX='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
def verify_pin(path,expected,label):
 actual=hashlib.sha256(path.read_bytes()).hexdigest()
 if actual!=expected: raise SystemExit(f'{label} hash mismatch: expected {expected}, got {actual} ({path})')
for name,digest in EXPECTED_HELPERS.items(): verify_pin(ROOT/'conformance'/name,digest,'helper')
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
for name,digest in EXPECTED_DSOS.items(): verify_pin(LIB/f'libSPen{name}.so',digest,'native DSO')
verify_pin(LIB/'libc++_shared.so',EXPECTED_LIBCXX,'libc++')
cli=argparse.ArgumentParser(description='Replay the bounded native Fountain RTV4 paint oracle.')
mode=cli.add_mutually_exclusive_group()
mode.add_argument('--write-fixture',action='store_true',help='explicitly regenerate the fixture after review')
mode.add_argument('--check-fixture',action='store_true',help='replay and compare with the checked-in fixture (default)')
cli.add_argument('--capture-case',choices=('saved-caller','synthetic-control'),help=argparse.SUPPRESS)
cli.add_argument('--capture-output',help=argparse.SUPPRESS)
cli_args=cli.parse_args()
FIXTURE=ROOT/'conformance/fountain-rtv4-paint-native.json'
RUNNER=Path(__file__).resolve()
if not cli_args.capture_case:
 expected=None
 if not cli_args.write_fixture:
  if not FIXTURE.is_file():
   print(f'fixture missing: {FIXTURE}; rerun with --write-fixture after review',file=sys.stderr);sys.exit(1)
  expected=json.loads(FIXTURE.read_text())
  expected_runner=expected.get('runner_sha256')
  actual_runner=hashlib.sha256(RUNNER.read_bytes()).hexdigest()
  if expected_runner!=actual_runner:
   print(f'runner pin mismatch before native execution: expected {expected_runner}, got {actual_runner}; use --write-fixture only after review',file=sys.stderr);sys.exit(1)
 with tempfile.TemporaryDirectory(prefix='sdocx-fountain-rtv4-') as td:
  captures={}
  for case in ('saved-caller','synthetic-control'):
   output=Path(td)/(case+'.json')
   env=os.environ.copy()
   env['PYTHONPATH']=os.pathsep.join((str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python'),env.get('PYTHONPATH','')))
   run=subprocess.run([sys.executable,str(RUNNER),'--capture-case',case,'--capture-output',str(output)],cwd=ROOT,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
   if run.returncode or not output.is_file():
    print(f'CAPTURE FAILED: {case} exit={run.returncode}\n{run.stdout[-12000:]}',file=sys.stderr);sys.exit(run.returncode or 1)
   captures[case]=json.loads(output.read_text())
 document={'schema':'sdocx-fountain-rtv4-paint-native-v1','runner_sha256':hashlib.sha256(RUNNER.read_bytes()).hexdigest(),'captures':captures}
 encoded=json.dumps(document,indent=2,sort_keys=True)+'\n'
 if cli_args.write_fixture:
  FIXTURE.write_text(encoded);print(f'WROTE {FIXTURE} sha256={hashlib.sha256(FIXTURE.read_bytes()).hexdigest()}');sys.exit(0)
 if not FIXTURE.is_file():
  print(f'fixture missing: {FIXTURE}; rerun with --write-fixture after review',file=sys.stderr);sys.exit(1)
 if document!=expected:
  print('FIXTURE MISMATCH: native capture or pinned source inputs changed',file=sys.stderr)
  print('expected runner/source hashes:',expected.get('runner_sha256'),file=sys.stderr)
  print('actual runner hash:',document['runner_sha256'],file=sys.stderr)
  sys.exit(1)
 print(f'PASS {FIXTURE} saved-caller + synthetic controls; runner_sha256={document["runner_sha256"]}')
 sys.exit(0)
if not cli_args.capture_output:
 cli.error('internal capture mode requires --capture-output')
sys.path[:0]=[str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
import airbrush_texture_native as at, inkpen_paint_native as ip, fountain_native as fn
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
ns={'at':at,'ip':ip,'fn':fn,'ROOT':ROOT,'LIB':LIB,'sys':sys,'os':os,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':__import__('re'),'zlib':__import__('zlib'),'C':__import__('ctypes'),'json':json,'base64':__import__('base64'),'argparse':__import__('argparse'),'Path':Path,'ARGS':__import__('argparse').Namespace(width=64,height=64,point_limit=0)}
tree=ast.parse((ROOT/'conformance/marker3_paint_scale_native.py').read_text())
node=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name=='P')
exec(compile(ast.Module(body=[node],type_ignores=[]),'<extract P>','exec'),ns)
p=ns['P']()
import ctypes as C
watch={
 'ctor':'SPen::FountainPenStrokeDrawableRTV4::FountainPenStrokeDrawableRTV4(SPen::PenGLDataManager*)',
 'init':'SPen::FountainPenStrokeDrawableRTV4::Init()',
 'create_canvas':'SPen::FountainPenStrokeDrawableRTV4::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',
 'set_inverse_scale':'SPen::FountainPenStrokeDrawableRTV4::SetInverseScale(SPen::Vector2<float> const&)',
 'add_point':'SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'send_data_to_gpu':'SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'set_pen_data':'SPen::PenDrawableRT::SetPenData(float, int)',
 'update':'SPen::FountainPenStrokeDrawableRTV4::Update(bool)',
 'draw':'SPen::FountainPenStrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
}
method_counts={label:0 for label in watch}
missing_symbols=[symbol for symbol in watch.values() if symbol not in p.syms]
if missing_symbols: raise RuntimeError(f'native entry symbols missing from pinned DSO: {missing_symbols}')
entry_address_to_label={p.syms[symbol]:label for label,symbol in watch.items()}
from unicorn import UC_HOOK_CODE
def trace_native_entries(u,address,size,data):
 label=entry_address_to_label.get(address)
 if label: method_counts[label]+=1
p.u.hook_add(UC_HOOK_CODE,trace_native_entries)
orig_buffer_data=p.callbacks['glBufferData']
def trace_buffer_data():
 binding=C.c_int();p.gles.gl.glGetIntegerv(0x8894,C.byref(binding));before=len(p.gl_calls)
 orig_buffer_data()
 for event in p.gl_calls[before:]:
  if event.get('call')=='glBufferData': event['array_buffer_binding']=binding.value
p.callbacks['glBufferData']=trace_buffer_data
program_build_events=[]
def trace_create_shader():
 shader_type=p.x(0);p.callbacks['_native_glCreateShader']();shader=p.x(0)
 program_build_events.append({'event':'glCreateShader','shader':shader,'shader_type':shader_type})
def trace_shader_source():
 shader=p.x(0);before=len(p.shader_source_calls);p.callbacks['_native_glShaderSource']()
 sources=p.shader_source_calls[before:]
 raw=''.join(''.join(item.get('source',[])) for item in sources).encode()
 program_build_events.append({'event':'glShaderSource','shader':shader,'source_sha256':hashlib.sha256(raw).hexdigest(),'source_call_count':len(sources)})
def trace_compile_shader():
 shader=p.x(0);p.callbacks['_native_glCompileShader']();program_build_events.append({'event':'glCompileShader','shader':shader})
def trace_create_program():
 p.callbacks['_native_glCreateProgram']();program=p.x(0);program_build_events.append({'event':'glCreateProgram','program':program})
def trace_attach_shader():
 program,shader=p.x(0),p.x(1);p.callbacks['_native_glAttachShader']();program_build_events.append({'event':'glAttachShader','program':program,'shader':shader})
def trace_link_program():
 program=p.x(0);p.callbacks['_native_glLinkProgram']();program_build_events.append({'event':'glLinkProgram','program':program})
def trace_use_program():
 program=p.x(0);p.callbacks['_native_glUseProgram']();program_build_events.append({'event':'glUseProgram','program':program})
for public,trace in [('glCreateShader',trace_create_shader),('glShaderSource',trace_shader_source),('glCompileShader',trace_compile_shader),('glCreateProgram',trace_create_program),('glAttachShader',trace_attach_shader),('glLinkProgram',trace_link_program),('glUseProgram',trace_use_program)]:
 p.callbacks['_native_'+public]=p.callbacks[public];p.callbacks[public]=trace
caller_fixture=json.loads((ROOT/'conformance/fountain-saved-caller-native.json').read_text())
caller_case=next(c for c in caller_fixture['cases'] if c['case']=='v14_omitted')
caller_inputs=[]
for entry in caller_case['rtv_addpoint_entries'][:3]:
 words=entry['float_words_s0_s5']; caller_inputs.append(tuple(struct.unpack('<f',bytes.fromhex(w))[0] for w in words))
probe_args=argparse.Namespace(synthetic_control=cli_args.capture_case=='synthetic-control')
if probe_args.synthetic_control:
 caller_inputs=[(0.,0.,0.1,0.,1.,1.),(8.,4.,0.1,1.,0.,1.),(16.,8.,0.1,1.,0.,1.)]
attr_snapshot=[]
def query_attr(index,pname):
 value=C.c_int(); p.gles.gl.glGetVertexAttribiv(index,pname,C.byref(value)); return value.value
orig_instanced=p.callbacks['glDrawArraysInstanced']
def trace_instanced():
 current=C.c_int(); vao=C.c_int(); fbo=C.c_int()
 p.gles.gl.glGetIntegerv(0x8B8D,C.byref(current));p.gles.gl.glGetIntegerv(0x85B5,C.byref(vao));p.gles.gl.glGetIntegerv(0x8CA6,C.byref(fbo))
 for index in range(6):
  attr_snapshot.append({'index':index,'enabled':query_attr(index,0x8622),'size':query_attr(index,0x8623),'type':query_attr(index,0x8625),'stride':query_attr(index,0x8624),'buffer':query_attr(index,0x889F),'divisor':query_attr(index,0x88FE)})
 print('PRE_DRAW_ATTRIBS',{'program':current.value,'vao':vao.value,'fbo':fbo.value,'attrs':attr_snapshot})
 vals=[p.x(i) for i in range(4)];p.gl_calls.append({'call':'glDrawArraysInstanced','values':vals,'state':p.gl_state()});p.gles.f['glDrawArraysInstanced'](*vals)
p.callbacks['glDrawArraysInstanced']=trace_instanced
queue=p.alloc(32);qv=p.alloc(64);p.queue_post=p.alloc(16);p.queue_task_return=p.alloc(16);p.pending_queue_return=None;p.queue_managed=p.alloc(16);p.managed_queue=p.alloc(32);p.managed_queue_vtable=p.alloc(64);p.managed_register=p.alloc(16);p.managed_unregister=p.alloc(16);p.managed_tokens={};p.managed_events=[]
typeinfo=p.syms.get('typeinfo for SPen::IGLManagedMsgQueue');p.put(queue,'Q',qv);p.put(qv-16,'qQ',0,typeinfo);p.put(qv+16,'Q',p.queue_post);p.put(qv+56,'Q',p.queue_managed);p.put(p.managed_queue,'Q',p.managed_queue_vtable);p.put(p.managed_queue_vtable+16,'Q',p.managed_register);p.put(p.managed_queue_vtable+24,'Q',p.managed_unregister);p.queue=queue
mgr=p.alloc(256);p.call('SPen::PenGLDataManager::PenGLDataManager()',[mgr])
rt=p.alloc(512);p.call('SPen::FountainPenStrokeDrawableRTV4::FountainPenStrokeDrawableRTV4(SPen::PenGLDataManager*)',[rt,mgr])
print('ctor',hex(rt))
try:
 p.call('SPen::FountainPenStrokeDrawableRTV4::Init()',[rt],instruction_limit=5_000_000)
 print('init OK',[(o,hex(p.get(rt+o,'Q')[0])) for o in (208,216,224,232,240,248,256)])
except Exception as e:
 from unicorn import arm64_const as R
 print('init failed',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'calls',p.entries[-20:],'trace',p.native_logs[-2:]);raise
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R
qret=[]
def queue_hook(u,a,size,data):
 if a==p.queue_post:
  task=p.x(1); vt=p.get(task,'Q')[0]; method=p.get(vt+16,'Q')[0]
  if not method: raise RuntimeError(f'queue task vtable+16 null task={task:#x}')
  qret.append(u.reg_read(R.UC_ARM64_REG_LR));p.x(0,task);u.reg_write(R.UC_ARM64_REG_LR,p.queue_task_return);u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==p.queue_task_return:
  u.reg_write(R.UC_ARM64_REG_PC,qret.pop())
p.u.hook_add(UC_HOOK_CODE,queue_hook)
p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:p.x(0,queue)
try:
 p.call('SPen::FountainPenStrokeDrawableRTV4::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,64,64,queue],instruction_limit=5_000_000)
 print('canvas OK',hex(p.get(rt+200,'Q')[0]),'dims',p.get(rt+276,'II'),'GLcalls',len(p.gl_calls))
except Exception as e:
 from unicorn import arm64_const as R
 print('canvas failed',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'entries',p.entries[-20:]);raise
# Synthetic input, explicit inverse-scale pair. Actual RT producer/update/draw bodies execute.
inverse=p.alloc(8);p.put(inverse,'2f',1.0,1.0)
p.call('SPen::FountainPenStrokeDrawableRTV4::SetInverseScale(SPen::Vector2<float> const&)',[rt,inverse])
# Actual AddPoint's +80 vector is supplied using native SetBuffer; this is the bounded app-side storage adapter.
producer=p.alloc(24);upload=p.alloc(24);pdata=p.alloc(512);udata=p.alloc(512)
p.put(producer,'3Q',pdata,pdata,pdata+512);p.put(upload,'3Q',udata,udata,udata+512)
p.call('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload])
add='SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)'
for vals in caller_inputs:p.call(add,[rt],vals)
prod_begin,prod_end,prod_cap=p.get(producer,'3Q');native_base_producer_bytes=bytes(p.u.mem_read(prod_begin,prod_end-prod_begin));native_base_producer_f32=list(p.get(prod_begin,'%df'%((prod_end-prod_begin)//4)));print('producerfloatcount',(prod_end-prod_begin)//4,'f32',native_base_producer_f32)
p.call('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload])
p.call('SPen::PenDrawableRT::SetPenData(float, int)',[rt,0xff336699],[2.0])
p.call('SPen::FountainPenStrokeDrawableRTV4::Update(bool)',[rt,0],instruction_limit=5_000_000)
bmp=p.get(rt+200,'Q')[0];p.call('SPen::SPPenCanvasRT::GetBitmap()',[bmp]); bitmap=p.x(0)
print('bitmap',hex(bitmap),'vptr',hex(p.get(bitmap,'Q')[0]))
p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);subbitmap=p.x(0);print('subbitmap',hex(subbitmap),'vptr',hex(p.get(subbitmap,'Q')[0]))
tag=p.alloc(32);p.u.mem_write(tag,b'fountain-output\0')
pixels=p.alloc(64*64*4);p.u.mem_write(pixels,bytes(64*64*4))
fac='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
p.call_stack_args(fac,[queue,64,64,tag,pixels,0,0,0],[6,2])
output_bitmap=p.x(0);p.call('SPen::SPTextureBitmap::AttachToCanvasRT(bool)',[output_bitmap,1],instruction_limit=5_000_000);print('output_bitmap',hex(output_bitmap),'vptr',hex(p.get(output_bitmap,'Q')[0]))
p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[output_bitmap,0]);output_subbitmap=p.x(0);print('output_subbitmap',hex(output_subbitmap),'vptr',hex(p.get(output_subbitmap,'Q')[0]))
mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
p.call('SPen::PenDrawableRT::SetRedrawState(bool)',[rt,1])
p.call('SPen::PenDrawableRT::SetEnhancedAntiAlias(bool)',[rt,1])
start=len(p.gl_calls)
p.call('SPen::FountainPenStrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,output_subbitmap,mat],instruction_limit=5_000_000)
print('draw GL calls',len(p.gl_calls)-start,'native draws',[c for c in p.gl_calls[start:] if c.get('call') in ('glDrawArrays','glDrawArraysInstanced')])
print('native shader source count',len(p.shader_source_calls),'blend',[c for c in p.gl_calls[start:] if c.get('call') in ('glBlendFuncSeparate','glBlendEquationSeparate','glEnable','glDisable')][-20:])
print('gl errors',[c for c in p.gl_calls if c.get('call','').startswith('errorAfter') and c.get('error')])
print('draw_uniforms',[c for c in p.gl_calls[start:] if c.get('call','').startswith('glUniform')])
print('draw_binding_calls',[c for c in p.gl_calls[start:] if c.get('call') in ('glBindFramebuffer','glBindVertexArray','glBindTexture','glViewport','glActiveTexture','glDrawBuffers')])
print('shader_sources',p.shader_source_calls)
print('draw_gl_sequence',[(c.get('call'),{k:v for k,v in c.items() if k not in ('state',)}) for c in p.gl_calls[start:] if c.get('call') not in ('errorAfter:glGetError',)])
print('allbufferdata',[c for c in p.gl_calls if c.get('call')=='glBufferData'])
print('allbindvao',[c for c in p.gl_calls if c.get('call')=='glBindVertexArray'])
print('attribdraw',[c for c in p.gl_calls[start:] if c.get('call') in ('errorAfter:glVertexAttribPointer','errorAfter:glEnableVertexAttribArray','glBindBuffer')])

read=C.create_string_buffer(64*64*4);read_fbo=C.c_int();draw_fbo=C.c_int();p.gles.gl.glGetIntegerv(0x8CAA,C.byref(read_fbo));p.gles.gl.glGetIntegerv(0x8CA6,C.byref(draw_fbo));rf=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels);rf(0,0,64,64,0x1908,0x1401,read);readback_gl_error=p.gles.f['glGetError']();print('RGBA_READBACK',{'bytes':len(read.raw),'sha256':hashlib.sha256(read.raw).hexdigest(),'nonzero_bytes':sum(1 for b in read.raw if b),'nonzero_alpha':sum(1 for b in read.raw[3::4] if b),'max_alpha':max(read.raw[3::4]),'read_framebuffer_binding':read_fbo.value,'draw_framebuffer_binding':draw_fbo.value,'error':readback_gl_error})
# Independent controls: replay exact saved-caller V14 AddPoint tuples under controlled inverse scale,
# color alpha, and Draw matrix. These use the same native caller records; only the named context
# inputs vary. Clear the two distinct RGBA attachments as explicit host setup between observations.
def clear_target(fbo):
 p.gles.f['glBindFramebuffer'](0x8D40,fbo)
 p.gles.f['glClearColor'](0.0,0.0,0.0,0.0)
 p.gles.f['glClear'](0x4000)
def read_target():
 buf=C.create_string_buffer(64*64*4)
 read_fbo=C.c_int();draw_fbo=C.c_int()
 p.gles.gl.glGetIntegerv(0x8CAA,C.byref(read_fbo));p.gles.gl.glGetIntegerv(0x8CA6,C.byref(draw_fbo))
 C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)(0,0,64,64,0x1908,0x1401,buf)
 read_error=p.gles.f['glGetError']()
 if read_error: raise RuntimeError(f'control readback produced GL error {read_error:#x}')
 return {'sha256':hashlib.sha256(buf.raw).hexdigest(),'bytes':len(buf.raw),'nonzero_alpha_pixels':sum(1 for b in buf.raw[3::4] if b),'max_alpha':max(buf.raw[3::4]),'read_framebuffer_binding':read_fbo.value,'draw_framebuffer_binding':draw_fbo.value,'gl_error':read_error,'rgba_zlib_base64':__import__('base64').b64encode(__import__('zlib').compress(buf.raw,9)).decode()}
def clear_both():
 clear_target(1);clear_target(2)
def draw_observation(label,argb,matrix_values):
 clear_both()
 p.call('SPen::PenDrawableRT::SetPenData(float, int)',[rt,argb],[2.0])
 mm=p.alloc(64);p.put(mm,'16f',*matrix_values)
 before=len(p.gl_calls)
 p.call('SPen::FountainPenStrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,output_subbitmap,mm],instruction_limit=5_000_000)
 calls=p.gl_calls[before:]
 projection=next((c['values'] for c in calls if c.get('call')=='glUniformMatrix4fv'),None)
 color_uniform=next((c for c in calls if c.get('call')=='glUniform4fv'),None)
 return {'label':label,'argb':f'0x{argb:08x}','draw_matrix_f32':matrix_values,'projection_uniform':projection,'color_uniform_call':color_uniform,'readback':read_target(),'gl_errors':[c for c in calls if c.get('call','').startswith('errorAfter:') and c.get('error')]}
control_observations=[]
identity=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
for a in (0x00336699,0x80336699,0xff336699):
 control_observations.append(draw_observation('color_alpha_'+str((a>>24)&255),a,identity))
for label,matrix in [('draw_identity',identity),('draw_translate_x8',[1,0,0,0,0,1,0,0,0,0,1,0,8,0,0,1]),('draw_scale_2',[2,0,0,0,0,2,0,0,0,0,1,0,0,0,0,1])]:
 control_observations.append(draw_observation(label,0xff336699,matrix))
inv2=p.alloc(8);p.put(inv2,'2f',0.5,0.25);p.call('SPen::FountainPenStrokeDrawableRTV4::SetInverseScale(SPen::Vector2<float> const&)',[rt,inv2])
prod2=p.alloc(24);upl2=p.alloc(24);pd2=p.alloc(512);ud2=p.alloc(512);p.put(prod2,'3Q',pd2,pd2,pd2+512);p.put(upl2,'3Q',ud2,ud2,ud2+512)
p.call('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,prod2,upl2])
for vals in caller_inputs:p.call(add,[rt],vals)
p2b,p2e,p2c=p.get(prod2,'3Q');inverse_changed_bytes=bytes(p.u.mem_read(p2b,p2e-p2b));inverse_changed_f32=list(p.get(p2b,'%df'%((p2e-p2b)//4)))
mat2=p.alloc(64);p.put(mat2,'16f',2,0,0,0,0,2,0,0,0,0,1,0,0,0,0,1)
# Restore only the original upload VBO pointer by retaining the already uploaded native buffer; do not call SendDataToGPU for inverse-scale control.
second_draw_start=len(p.gl_calls);p.call('SPen::FountainPenStrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,output_subbitmap,mat2],instruction_limit=5_000_000)
base_projection=next(c['values'] for c in p.gl_calls[start:] if c.get('call')=='glUniformMatrix4fv')
scaled_projection=next(c['values'] for c in p.gl_calls[second_draw_start:] if c.get('call')=='glUniformMatrix4fv')
print('SCALE_CONTROLS',{'base_inverse_scale':[1,1],'changed_inverse_scale':[0.5,0.25],'base_record_bytes_sha256':hashlib.sha256(native_base_producer_bytes).hexdigest(),'changed_record_bytes_sha256':hashlib.sha256(inverse_changed_bytes).hexdigest(),'base_projection':base_projection,'scaled_draw_projection':scaled_projection,'gl_errors_after_controls':[c for c in p.gl_calls[second_draw_start:] if c.get('call','').startswith('errorAfter:') and c.get('error')]})
# Persist the narrow joined producer -> native RT Draw -> host GLES observation.
import base64,zlib
expected_method_counts={'ctor':1,'init':1,'create_canvas':1,'set_inverse_scale':2,'add_point':6,'send_data_to_gpu':1,'set_pen_data':7,'update':1,'draw':8}
if method_counts!=expected_method_counts: raise RuntimeError(f'native method entry trace changed: expected {expected_method_counts}, got {method_counts}')
draws=[c for c in p.gl_calls if c.get('call') in ('glDrawArrays','glDrawArraysInstanced')]
program_calls=program_build_events
attachments=[c for c in p.gl_calls if c.get('call')=='glFramebufferTexture2D']
errors=[c for c in p.gl_calls if c.get('call','').startswith('errorAfter:') and c.get('error')]
buffer_data=[c for c in p.gl_calls if c.get('call')=='glBufferData']
instance_uploads=[c for c in buffer_data if c.get('size')==len(native_base_producer_bytes) and c.get('dataSha256')==hashlib.sha256(native_base_producer_bytes).hexdigest()]
if len(instance_uploads)!=1: raise RuntimeError(f'expected one captured 108B native instance upload, found {instance_uploads}')
instance_buffer_id=instance_uploads[0].get('array_buffer_binding')
draw_instance_buffer_id=next((a['buffer'] for a in attr_snapshot if a['index']==2),None)
if instance_buffer_id is None or instance_buffer_id!=draw_instance_buffer_id: raise RuntimeError(f'producer upload buffer {instance_buffer_id} differs from mask draw instance buffer {draw_instance_buffer_id}')
shader_source_by_id={};attached_by_program={};linked_sources={}
for event in program_calls:
 if event['event']=='glShaderSource': shader_source_by_id[event['shader']]=event['source_sha256']
 elif event['event']=='glCreateProgram': attached_by_program[event['program']]=[]
 elif event['event']=='glAttachShader':
  if event['program'] not in attached_by_program or event['shader'] not in shader_source_by_id: raise RuntimeError(f'program/shader source association missing: {event}')
  attached_by_program[event['program']].append(shader_source_by_id[event['shader']])
 elif event['event']=='glLinkProgram': linked_sources[event['program']]=list(attached_by_program.get(event['program'],[]))
draw_program_sequence=[event['program'] for event in program_calls if event['event']=='glUseProgram']
if draw_program_sequence != [3,4]*8: raise RuntimeError(f'native Draw pass program sequence changed: {draw_program_sequence}')
if any(len(linked_sources.get(program,[]))!=2 for program in (3,4)): raise RuntimeError(f'native mask/composite shader linkage not captured: {linked_sources}')
mask_texture=next((e['args'][3] for e in attachments if e.get('framebuffer')==1),None)
output_texture=next((e['args'][3] for e in attachments if e.get('framebuffer')==2),None)
if mask_texture is None or output_texture is None or mask_texture==output_texture: raise RuntimeError(f'mask/output attachments are not distinct: {attachments}')
if errors: raise RuntimeError(f'GL error after native Draw callbacks: {errors}')
if read_fbo.value!=2 or draw_fbo.value!=2: raise RuntimeError(f'readback did not come from output FBO2: read={read_fbo.value} draw={draw_fbo.value}')
if readback_gl_error: raise RuntimeError(f'host readback produced GL error {readback_gl_error:#x}')
if not any(read.raw[3::4]): raise RuntimeError('native paint readback has no alpha support')
expected_control_names={'color_alpha_0','color_alpha_128','color_alpha_255','draw_identity','draw_translate_x8','draw_scale_2'}
if {o['label'] for o in control_observations}!=expected_control_names: raise RuntimeError('required color/matrix control cases missing')
if any(o['gl_errors'] or o['readback']['gl_error'] or o['readback']['read_framebuffer_binding']!=2 or o['readback']['draw_framebuffer_binding']!=2 for o in control_observations): raise RuntimeError('control Draw/readback failed its GL/FBO assertions')
if next(o for o in control_observations if o['label']=='color_alpha_0')['readback']['nonzero_alpha_pixels']!=0: raise RuntimeError('zero-alpha control produced nonzero output')
if any(next(o for o in control_observations if o['label']==f'color_alpha_{alpha}')['readback']['nonzero_alpha_pixels']==0 for alpha in (128,255)): raise RuntimeError('nonzero-alpha control lost all paint support')
report={
 'schema':'sdocx-fountain-v14-rtv4-native-draw-hostgles-v1',
 'evidence_boundary':'Actual pinned FountainPen RTV4 constructor, Init, CreatePenCanvas, SetInverseScale, AddPoint, SendDataToGPU, Update, and Draw execute in Unicorn. The saved-caller capture replays the first three native AddPoint tuples from fixture v14_omitted; the separately labeled control uses explicit synthetic tuples. The probe constructs RTV4 directly and does not execute Drawing/Model admission. Color, canvas, queue/context/bitmap, and Draw matrices are explicit host inputs. GLES is Mesa via the existing host adapter. Readback is host-Mesa only, not Samsung device parity.',
 'native_sources':{n:hashlib.sha256((LIB/f'libSPen{n}.so').read_bytes()).hexdigest() for n in EXPECTED_DSOS},
 'libcxx_sha256':hashlib.sha256((LIB/'libc++_shared.so').read_bytes()).hexdigest(),
 'helper_sources':{n:hashlib.sha256((ROOT/'conformance'/n).read_bytes()).hexdigest() for n in EXPECTED_HELPERS},
 'selected_v14_context':{'source_fixture':'conformance/fountain-saved-caller-native.json','case':'v14_omitted','caller_selected_rt':'SPen::FountainPenStrokeDrawableRTV4','selection_is_separate_from_direct_construction':True},
 'input':{'kind':'explicit synthetic native AddPoint arguments; not captured device data' if probe_args.synthetic_control else 'exact argument replay from native saved-caller fixture; not a captured device paint','source_fixture':None if probe_args.synthetic_control else 'conformance/fountain-saved-caller-native.json','source_fixture_sha256':None if probe_args.synthetic_control else hashlib.sha256((ROOT/'conformance/fountain-saved-caller-native.json').read_bytes()).hexdigest(),'source_case':None if probe_args.synthetic_control else 'v14_omitted','source_addpoint_entry_indices':None if probe_args.synthetic_control else [0,1,2],'addpoint_argument_bytes_sha256':hashlib.sha256(struct.pack('<%df'%(len(caller_inputs)*6),*(v for row in caller_inputs for v in row))).hexdigest(),'canvas':[64,64],'inverse_scale_f32':[1.0,1.0],'draw_matrix_f32':[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],'SetPenData_size_f32':2.0,'SetPenData_argb_u32':'0xff336699','addpoint_s0_s5_f32':[list(x) for x in caller_inputs],'native_producer_float_count':(prod_end-prod_begin)//4,'native_producer_f32':native_base_producer_f32,'producer_upload_bytes':len(native_base_producer_bytes),'producer_upload_sha256':hashlib.sha256(native_base_producer_bytes).hexdigest(),'queue':'synchronous explicit host callback dispatching task vtable+16'},
 'scale_controls':{'inverse_scale_only_changed_addpoint_record_bytes':{'inverse_scale':[0.5,0.25],'bytes':len(inverse_changed_bytes),'sha256':hashlib.sha256(inverse_changed_bytes).hexdigest(),'f32':inverse_changed_f32,'draw_replayed_with_this_new_stream':False},'draw_matrix_only_changed_projection':{'matrix_f32':[2,0,0,0,0,2,0,0,0,0,1,0,0,0,0,1],'projection_before':list(base_projection),'projection_after':list(scaled_projection),'original_uploaded_instance_stream_retained':True},'color_alpha_and_draw_matrix_controls':control_observations},
 'rt_fields':{'canvas_width_height':[64,64],'inverse_scale_field_plus264_f32':list(p.get(rt+264,'2f')),'rgba_color_plus44_f32':list(p.get(rt+44,'4f')),'enhanced_antialias':bool(p.get(rt+68,'B')[0]),'redraw_state':bool(p.get(rt+25,'B')[0]),'buffers':{'producer_plus80':hex(producer),'upload_plus88':hex(upload)}},
 'native_method_entry_trace':[{'method':label,'symbol':symbol,'mapped_va':hex(p.syms[symbol]),'entry_count':method_counts[label]} for label,symbol in watch.items()],
 'shader_sources_from_native_glShaderSource':p.shader_source_calls,
 'program_shader_link_trace':program_calls,
 'draw_program_binding':{'use_sequence':draw_program_sequence,'linked_shader_source_hashes':{str(program):linked_sources[program] for program in (3,4)},'mask_program':3,'composite_program':4},
 'draw_calls':draws,
 'draw_uniforms':[c for c in p.gl_calls if c.get('call','').startswith('glUniform')],
 'blend_state_calls':[c for c in p.gl_calls if c.get('call') in ('glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask')],
 'framebuffer_events':[c for c in p.gl_calls if c.get('call') in ('glBindFramebuffer','glFramebufferTexture2D','glCheckFramebufferStatus','glViewport')],
 'texture_events':[c for c in p.gl_calls if c.get('call') in ('glBindTexture','glActiveTexture','glTexParameteri','glTexImage2D')],
 'attribute_state_at_native_mask_draw':attr_snapshot,
 'gl_buffer_data_events':buffer_data,
 'native_instance_upload':{'buffer_id':instance_buffer_id,'draw_attribute_location_2_buffer_id':draw_instance_buffer_id,'bytes':instance_uploads[0]['size'],'sha256':instance_uploads[0]['dataSha256'],'matches_native_producer_bytes':instance_uploads[0]['dataSha256']==hashlib.sha256(native_base_producer_bytes).hexdigest()},
 'paint_interpretation':'The captured composite fragment shader writes vec4(uInputColor.rgb * alpha, alpha), so its RGB is premultiplied by the mask and color alpha; native Draw sets GL_BLEND with ONE, ONE_MINUS_SRC_ALPHA. This describes the captured shader and GLES state, not a Samsung device compositing observation.',
 'gl_errors':errors,
 'host_gles':p.gles.identity,
 'readback':{'origin':'after actual native RTV4::Draw return, RGBA/UNSIGNED_BYTE read from host Mesa current FBO2; rows are GL bottom-up order','width':64,'height':64,'format':'RGBA/UNSIGNED_BYTE','bytes':len(read.raw),'sha256':hashlib.sha256(read.raw).hexdigest(),'nonzero_bytes':sum(1 for b in read.raw if b),'nonzero_alpha_pixels':sum(1 for b in read.raw[3::4] if b),'max_alpha':max(read.raw[3::4]),'read_framebuffer_binding':read_fbo.value,'draw_framebuffer_binding':draw_fbo.value,'gl_error':readback_gl_error,'zlib_base64':base64.b64encode(zlib.compress(read.raw,9)).decode()},
 'host_boundaries':['Output bitmap comes from native SPGraphicsFactory::CreateBitmap with explicit RGBA8/RGBA target args, then actual SPTextureBitmap::AttachToCanvasRT(true).','The GL queue executes posted task vtable+16 synchronously in a host callback.','The Renderer-aware Unicorn loader and all GLES forwarding use host adapters.','The positive native Draw run replaces only the inherited host glDrawArraysInstanced diagnostic callback because that callback issues an invalid GL_RED read on this RGBA8 target; the replacement forwards the native draw and captures attribute state.','No Android app GL thread, natural object destruction, full Model/Drawing->RT lifecycle, or device GPU pixels are claimed.']}
path=Path(cli_args.capture_output)
path.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
print('CAPTURE',cli_args.capture_case,'sha256',hashlib.sha256(path.read_bytes()).hexdigest(),'draws',len(draws),'attachments',attachments,'errors',len(errors))
