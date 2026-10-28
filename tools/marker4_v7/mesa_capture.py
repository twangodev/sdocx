"""Capture V7 RTV3 paint on the native host with the retained GLES adapters.

Adapted from scratch/defaultpen-native/marker4_native_capture_full.py (SHA-256
55f01cb25276552d481d5096a5e797b28bbdd1bd64bd786bf4975c3ce64d1576).
Run from the project root with PYTHONPATH=scratch/apk-analysis-runtime/python:
tmp/conformance-cleanup/native-capture-a241793/conformance:scratch/defaultpen-native
and .venv/bin/python tools/marker4_v7/mesa_capture.py
--capture-output /tmp/m4v7-stroke30.json. The adapters and pinned
native inputs under ROOT are required; output frames are temporary.
"""
import ast,sys,os,hashlib,struct,subprocess,json,argparse,tempfile
from pathlib import Path
ROOT=Path('/home/jding/RustroverProjects/sdocx/tmp/vector-parity-plan/chinesebrush-paint-scale-work')
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
EXPECTED_REAL_FIXTURE='c6204b33441eb77a53932e1c475e931e0f9688a8ad40788d05a7f41c58400ca3'
def verify_pin(path,expected,label):
 actual=hashlib.sha256(path.read_bytes()).hexdigest()
 if actual!=expected: raise SystemExit(f'{label} hash mismatch: expected {expected}, got {actual} ({path})')
for name,digest in EXPECTED_HELPERS.items(): verify_pin(ROOT/'conformance'/name,digest,'helper')
if hashlib.sha256((ROOT/'conformance/fountain-v14-real-native.json').read_bytes()).hexdigest()!=EXPECTED_REAL_FIXTURE:
 raise SystemExit('real V14 native fixture hash mismatch before execution')
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
for name,digest in EXPECTED_DSOS.items(): verify_pin(LIB/f'libSPen{name}.so',digest,'native DSO')
verify_pin(LIB/'libc++_shared.so',EXPECTED_LIBCXX,'libc++')
verify_pin(LIB/'libSPenMarker4.so','23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff','Marker4')
cli=argparse.ArgumentParser(description='Capture the bounded native Marker4 V7 tool-0 paint control.')
cli.add_argument('--capture-output', required=True, help='JSON receipt path; RGBA output uses the same stem')
cli.add_argument('--case', default='turn-9.5')
cli.add_argument('--scale', type=float, default=1.0)
cli.add_argument('--phase-x', type=float, default=0.0)
cli.add_argument('--phase-y', type=float, default=0.0)
cli_args=cli.parse_args()
output_path=Path(cli_args.capture_output)
if output_path.suffix != '.json':
 raise SystemExit('capture output must have a .json suffix')
output_paths=[output_path,output_path.with_suffix('.rgba'),output_path.with_name(output_path.stem+'-mask.json'),output_path.with_name(output_path.stem+'-mask.rgba')]
if len({p.resolve() for p in output_paths}) != len(output_paths):
 raise SystemExit('capture output paths collide')
if any(p.exists() or p.is_symlink() for p in output_paths):
 raise SystemExit('capture output already exists')
output_path.parent.mkdir(parents=True, exist_ok=True)
fixture_path=Path('conformance/marker4-v7-finger-native.json')
verify_pin(fixture_path,'5e6150ffe049ae9775d26724beb25174361b7bf234f448c955e7039dcc6f3c77','finger geometry fixture')
def write_new(path,data):
 with path.open('xb') as stream: stream.write(data)
 if hashlib.sha256(path.read_bytes()).digest()!=hashlib.sha256(data).digest():
  raise RuntimeError(f'output hash mismatch: {path}')
sys.path[:0]=[str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
import airbrush_texture_native as at, inkpen_paint_native as ip, fountain_native as fn
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
ns={'at':at,'ip':ip,'fn':fn,'ROOT':ROOT,'LIB':LIB,'sys':sys,'os':os,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':__import__('re'),'zlib':__import__('zlib'),'C':__import__('ctypes'),'json':json,'base64':__import__('base64'),'argparse':__import__('argparse'),'Path':Path,'ARGS':__import__('argparse').Namespace(width=64,height=64,point_limit=0)}
helper=Path('scratch/defaultpen-native/marker3_paint_scale_marker4.py')
verify_pin(helper,'e0ca65960f16071d8f06db5705b66152a3335782bd0c3209bbd9f4be792710a1','Marker3 host class helper')
tree=ast.parse(helper.read_text())
node=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name=='P')
exec(compile(ast.Module(body=[node],type_ignores=[]),'<extract P>','exec'),ns)
p=ns['P']()
import math
p.callbacks['sinf']=lambda:p.f(0,math.sin(p.f(0)))
p.callbacks['cosf']=lambda:p.f(0,math.cos(p.f(0)))
p.callbacks['atan2f']=lambda:p.f(0,math.atan2(p.f(0),p.f(1)))
import ctypes as C
def _m4_strlen_debug():
 ptr=p.x(0)
 raw=bytes(p.u.mem_read(ptr,16384))
 nul=raw.find(b'\0')
 if nul<0: print('STRLEN_BAD_PTR',hex(ptr),raw.hex()); raise RuntimeError(f'strlen unterminated pointer {ptr:#x}')
 p.x(0,nul)
p.callbacks['strlen']=_m4_strlen_debug
_native_gubi_addr=p.gles.egl.eglGetProcAddress(b'glGetUniformBlockIndex')
_native_gubi=C.CFUNCTYPE(C.c_uint,C.c_uint,C.c_char_p)(_native_gubi_addr)
p.callbacks['glGetUniformBlockIndex']=lambda:p.x(0,_native_gubi(p.x(0),p.cstr(p.x(1))))
_native_u2fv=C.CFUNCTYPE(None,C.c_int,C.c_int,C.POINTER(C.c_float))(p.gles.egl.eglGetProcAddress(b'glUniform2fv'))
def _m4_u2fv():
 loc,count,ptr=p.x(0),p.x(1),p.x(2); vals=p.get(ptr,'%df'%(2*count)); arr=(C.c_float*len(vals))(*vals); _native_u2fv(loc,count,arr)
p.callbacks['glUniform2fv']=_m4_u2fv
_native_db=C.CFUNCTYPE(None,C.c_int,C.POINTER(C.c_uint))(p.gles.egl.eglGetProcAddress(b'glDeleteBuffers'))
def _m4_db():
 n=p.x(0); ptr=p.x(1); vals=p.get(ptr,'%dI'%n) if n else (); arr=(C.c_uint*n)(*vals); _native_db(n,arr)
p.callbacks['glDeleteBuffers']=_m4_db
_native_dp=C.CFUNCTYPE(None,C.c_uint)(p.gles.egl.eglGetProcAddress(b'glDeleteProgram'))
p.callbacks['glDeleteProgram']=lambda:_native_dp(p.x(0))
_native_ds=C.CFUNCTYPE(None,C.c_uint)(p.gles.egl.eglGetProcAddress(b'glDeleteShader'))
p.callbacks['glDeleteShader']=lambda:_native_ds(p.x(0))
watch={
 'ctor':'SPen::Marker4StrokeDrawableRTV3::Marker4StrokeDrawableRTV3(SPen::PenGLDataManager*)',
 'init':'SPen::Marker4StrokeDrawableRTV3::Init()',
 'create_canvas':'SPen::Marker4StrokeDrawableRTV3::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',
 'send_data_to_gpu':'SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'set_pen_data':'SPen::PenDrawableRT::SetPenData(float, int)',
 'update':'SPen::Marker4StrokeDrawableRTV3::Update(bool)',
 'draw':'SPen::Marker4StrokeDrawableRTV3::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
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
probe_args=argparse.Namespace(synthetic_control=True)
real_fixture=None
expected_real_bytes=None
draw_matrix=[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
pen_color=0xff336699
pen_size=2.0
frame_anchor=None
if False:
 real_fixture=json.loads((ROOT/'conformance/fountain-v14-real-native.json').read_text())
 real_dots=real_fixture['native_saved_traversal']['dots_f32_le_hex_words']
 caller_inputs=[tuple(struct.unpack('<f',bytes.fromhex(word))[0] for word in record)+(1.0,) for record in real_dots]
 native_records=real_fixture['native_addpoint'][0]['records_f32_le_hex_words']
 expected_real_bytes=b''.join(bytes.fromhex(word) for record in native_records for word in record)
 pen_size=real_fixture['source_stroke']['pen_width'];pen_color=0xff000000
 frame_anchor=[921.0,1435.0]
 draw_matrix=[1,0,0,0,0,-1,0,0,0,0,1,0,-889,1467,0,1]
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
rt=p.alloc(512);p.call('SPen::Marker4StrokeDrawableRTV3::Marker4StrokeDrawableRTV3(SPen::PenGLDataManager*)',[rt,mgr])
print('ctor',hex(rt))
try:
 p.call('SPen::Marker4StrokeDrawableRTV3::Init()',[rt],instruction_limit=5_000_000)
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
 p.call('SPen::Marker4StrokeDrawableRTV3::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,1024,512,queue],instruction_limit=5_000_000)
 print('canvas OK',hex(p.get(rt+200,'Q')[0]),'mask_bitmap',hex(p.get(rt+216,'Q')[0]),'dims',p.get(rt+340,'II'),'GLcalls',len(p.gl_calls))
 import json
 # Read the actual 200x200 texture produced by native Graphics canvas/roundrect tasks.
 import ctypes as C,hashlib,json
 g=p.gles.f; tex=2
 fb=C.c_uint();g['glGenFramebuffers'](1,C.byref(fb));g['glBindFramebuffer'](0x8D40,fb.value);g['glFramebufferTexture2D'](0x8D40,0x8CE0,0x0DE1,tex,0)
 status=g['glCheckFramebufferStatus'](0x8D40)
 raw=C.create_string_buffer(200*200*4);g['glReadPixels'](0,0,200,200,0x1908,0x1401,raw);err=g['glGetError']()
 out=output_path.with_name(output_path.stem + '-mask.rgba');write_new(out,raw.raw)
 events=[e for e in p.gl_calls if e.get('call') in ('glTexImage2D','glFramebufferTexture2D','glBindFramebuffer','glCheckFramebufferStatus','glBindTexture','glActiveTexture','glDrawArrays','glUniform1i','glUniform1f','glTexParameteri','glSamplerParameteri','glBindSampler')]
 meta={'schema':'marker4-v7-native-canvas-mask-readback-v1','DSO':{'Marker4':'23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff'},'mask_bitmap_pointer':hex(p.get(rt+216,'Q')[0]),'texture_id':tex,'fbo_readback':fb.value,'fbo_status':hex(status),'gl_error':err,'width':200,'height':200,'format':'RGBA/UNSIGNED_BYTE','row_origin':'GL bottom-left','bytes':len(raw.raw),'rgba_sha256':hashlib.sha256(raw.raw).hexdigest(),'file':str(out),'shader_sources':p.shader_source_calls,'texture_sampler_events':events,'host_gles':p.gles.identity}
 write_new(output_path.with_name(output_path.stem + '-mask.json'),(json.dumps(meta,indent=2)+'\n').encode())
 print('MASK_CAPTURE',json.dumps({k:v for k,v in meta.items() if k not in ('shader_sources','texture_sampler_events')}))
 print('TEXTURE_EVENTS',json.dumps(events,default=str))
 # Actual Marker4 task2: native saved V7 AddPoint stamps -> native Update/Draw -> Mesa.
 saved=json.loads(Path('conformance/marker4-v7-finger-native.json').read_text())['cases']
 case=next(x for x in saved if x['name']==cli_args.case)
 dots=case['dots']
 # Native producer/upload vectors are host-owned storage adapters; all point expansion and paint are native.
 producer=p.alloc(24);upload=p.alloc(24);pdata=p.alloc(65536);udata=p.alloc(65536)
 p.put(producer,'3Q',pdata,pdata,pdata+65536);p.put(upload,'3Q',udata,udata,udata+65536)
 p.call('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload])
 add='SPen::Marker4StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float, float)'
 for x,y,r,a in dots: p.call(add,[rt],[x,y,r,a])
 b,e,c=p.get(producer,'3Q');stream=bytes(p.u.mem_read(b,e-b))
 p.call('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,producer,upload])
 p.call('SPen::PenDrawableRT::SetPenData(float, int)',[rt,0x80ffcb30],[case['size']])
 p.call('SPen::Marker4StrokeDrawableRTV3::Update(bool)',[rt,0],instruction_limit=5_000_000)
 # Frame the saved horizontal stroke at native document coordinates, at 1 px/doc unit.
 w,h=1024,512; cx=sum(x for x,y,r,a in dots)/len(dots); cy=sum(y for x,y,r,a in dots)/len(dots)
 tag=p.alloc(32);p.u.mem_write(tag,b'm4-task2-output\0'); pixels=p.alloc(w*h*4);p.u.mem_write(pixels,bytes(w*h*4))
 fac='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
 p.call_stack_args(fac,[queue,w,h,tag,pixels,0,0,0],[6,2]);ob=p.x(0)
 p.call('SPen::SPTextureBitmap::AttachToCanvasRT(bool)',[ob,1],instruction_limit=5_000_000)
 p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[ob,0]);sub=p.x(0)
 # Matrix4 column-major transform: doc x remains +, doc y is flipped to raster top-down.
 mat=p.alloc(64);scale=cli_args.scale
 p.put(mat,'16f',scale,0,0,0,0,-scale,0,0,0,0,1,0,w/2-scale*cx+cli_args.phase_x,h/2+scale*cy-cli_args.phase_y,0,1)
 p.call('SPen::PenDrawableRT::SetRedrawState(bool)',[rt,1]);p.call('SPen::PenDrawableRT::SetEnhancedAntiAlias(bool)',[rt,1])
 before=len(p.gl_calls);p.call('SPen::Marker4StrokeDrawableRTV3::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,sub,mat],instruction_limit=5_000_000)
 g=p.gles.f; rf=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)
 raw=C.create_string_buffer(w*h*4);rf(0,0,w,h,0x1908,0x1401,raw);err=g['glGetError']()
 out=output_path.with_suffix('.rgba');write_new(out,raw.raw)
 fm={'schema':'marker4-v7-native-task2-mesa-frame-v1','source_json_sha256':hashlib.sha256(Path('conformance/marker4-v7-finger-native.json').read_bytes()).hexdigest(),'case':cli_args.case,'tool':0,'source_dot_count':len(dots),'source_dots_sha256':hashlib.sha256(json.dumps(dots,separators=(',',':')).encode()).hexdigest(),'producer_bytes':len(stream),'producer_sha256':hashlib.sha256(stream).hexdigest(),'producer_float_count':(e-b)//4,'set_pen_size_f32':case['size'],'set_pen_argb':hex(0x80ffcb30),'scale':cli_args.scale,'phase':[cli_args.phase_x,cli_args.phase_y],'target':[w,h],'frame_center_doc':[cx,cy],'matrix_f32':p.get(mat,'16f'),'texture_id':None,'rgba_sha256':hashlib.sha256(raw.raw).hexdigest(),'read_framebuffer':(lambda q:(p.gles.gl.glGetIntegerv(0x8CAA,C.byref(q)),q.value)[1])(C.c_int()),'gl_error':err,'draw_calls':p.gl_calls[before:],'image_path':str(out),'raw_input_boundary':'saved native V7 AddPoint outputs; GL/task callbacks use actual native render code'}
 fm['texture_id']=next((x.get('texture') for x in p.gl_calls[before:] if x.get('call')=='glFramebufferTexture2D'),None)
 fm['pre_draw_native_vbo_uploads']=[x for x in p.gl_calls if x.get('call')=='glBufferData' and x.get('size')==len(stream)]
 fm['target_texture_fbo_events']=[x for x in p.gl_calls if x.get('call') in ('glTexImage2D','glFramebufferTexture2D','glCheckFramebufferStatus')]
 fm['native_producer_first_12_f32']=list(p.get(b,'12f')) if (e-b)>=48 else []
 fm['native_producer_last_12_f32']=list(p.get(e-48,'12f')) if (e-b)>=48 else []
 fm['all_gl_buffer_data_small']=[x for x in p.gl_calls if x.get('call')=='glBufferData' and x.get('size',999999)<=1024]

 write_new(output_path,(json.dumps(fm,indent=2)+'\n').encode())
 print('TASK2_CAPTURE',json.dumps({k:v for k,v in fm.items() if k!='draw_calls'}))
 raise SystemExit(0)
except Exception as e:
 from unicorn import arm64_const as R
 print('canvas failed',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'entries',p.entries[-20:]);raise
