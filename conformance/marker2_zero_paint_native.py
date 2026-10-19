"""Reconstruct the bounded Marker2 V1/V2 zero-versus-positive-width paint path.

Run from the repository root or by absolute path with
`PYTHONPATH=scratch/apk-analysis-runtime/python:conformance python3 conformance/marker2_zero_paint_native.py --advanced-setting 2 --size 10`.
Default mode checks this case against the durable fixture; `--write-fixture` updates it.
"""
import argparse, ast, base64, contextlib, hashlib, importlib.util, io, json, os, re, struct, subprocess, sys, tempfile, types, zlib
from pathlib import Path
from unicorn import arm64_const as R
ROOT=Path(__file__).resolve().parents[1]; LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
PINNED_LIBRARIES={
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenMarker2.so':'f1b2ceea921baac78b722cbd5423732f42d7bec19ccb63d428cf429582412269',
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
PINNED_HELPERS={
 'marker2-saved-caller-native.json':'3f8ae6cc90ddc6bcef07809ea22ad9bd5b41b55b6fc53804b3a4a296972cfd43',
 'marker2_saved_caller_native.py':'01dc7f77dff725d5959d0c4a7533bee0cd0c8df948a5b1dbf7203569eb29a2c9',
 'marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
 'oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
 'elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
 'fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'airbrush_texture_native.py':'76bb130daef795be53296cff4f778be6e7be5f487c8b1d4c4b6d159bb688e470',
 'inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
}
for filename,digest in PINNED_LIBRARIES.items():
 actual=hashlib.sha256((LIB/filename).read_bytes()).hexdigest()
 if actual!=digest: raise RuntimeError(f'unpinned native library {filename}: {actual}')
for filename,digest in PINNED_HELPERS.items():
 actual=hashlib.sha256((ROOT/'conformance'/filename).read_bytes()).hexdigest()
 if actual!=digest: raise RuntimeError(f'unpinned helper {filename}: {actual}')
sys.path.insert(0,str(ROOT/'conformance'))
FIXTURE=ROOT/'conformance/marker2-zero-paint-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group()
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
parser.add_argument('--advanced-setting',choices=('null','2'))
parser.add_argument('--size',choices=('0','10'))
parser.add_argument('--tool',choices=('0','1','2'),default='2')
CLI=parser.parse_args()
if (CLI.advanced_setting is None) != (CLI.size is None):
 parser.error('--advanced-setting and --size must be supplied together')
if CLI.advanced_setting is None:
 mode_arg='--write-fixture' if CLI.write_fixture else '--check-fixture'
 for advanced,size in [('null','0'),('null','10'),('2','0'),('2','10')]:
  subprocess.run([sys.executable,str(Path(__file__).resolve()),mode_arg,'--advanced-setting',advanced,'--size',size,'--tool',CLI.tool],check=True,env=os.environ.copy())
 aggregate=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else {'cases':{}}
 expected={f'{advanced}-size{size}-tool{CLI.tool}' for advanced,size in [('null','0'),('null','10'),('2','0'),('2','10')]}
 if set(aggregate.get('cases',{}))!=expected: raise SystemExit(f'Marker2 matrix cases mismatch: expected {sorted(expected)}, found {sorted(aggregate.get("cases",{}))}')
 print(json.dumps({'fixture':str(FIXTURE),'mode':'write' if CLI.write_fixture else 'check','cases':sorted(expected)},sort_keys=True))
 raise SystemExit(0)
TOOL=CLI.tool; ADVANCED_SETTING=CLI.advanced_setting
CASE_KEY=f'{ADVANCED_SETTING}-size{CLI.size}-tool{TOOL}'
# Keep the one-case source caller's generated output outside the repository and
# remove it automatically; it is not a conformance dependency or sidecar.
source_tmp=tempfile.TemporaryDirectory(prefix='marker2-source-case-')
source_fixture=Path(source_tmp.name)/'source-case.json'
caller_path=ROOT/'conformance/marker2_saved_caller_native.py'
caller_source=caller_path.read_text().replace("FIXTURE=ROOT/'conformance/marker2-saved-caller-native.json'", "FIXTURE=SOURCE_FIXTURE").replace("print(f'wrote {FIXTURE.relative_to(ROOT)} case {slot}',_always=True)", "print(f'wrote temporary source case {slot}',_always=True)")
# Execute the selected Marker2 caller into its pinned native ObjectDrawing and RTV.
sys.argv=['marker2-saved-caller','--write-fixture','--case','marker2_nontrivial_saved3','--advanced-setting',ADVANCED_SETTING,'--size',CLI.size,'--tool',TOOL,'--curve','1']
spec=importlib.util.spec_from_file_location('m2caller',caller_path); m=importlib.util.module_from_spec(spec); m.__file__=str(caller_path); m.SOURCE_FIXTURE=source_fixture
with contextlib.redirect_stdout(io.StringIO()): exec(compile(caller_source,str(caller_path),'exec'),m.__dict__)
p=m.p; rt=m.selected_rt
rows=m.captured_callbacks
vector_data=bytes.fromhex(m.captured_final_vectors['saved_redraw']['vector_bytes'])
# Bring the paint oracle's GL callback methods into this emulator without running its separate RT pipeline.
import ctypes as C,zlib,airbrush_texture_native as at,inkpen_paint_native as ip,fountain_native as fn
ARGS=types.SimpleNamespace(width=256,height=256)
source=ROOT/'conformance/marker3_paint_scale_native.py'; tree=ast.parse(source.read_text())
klass=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='P')
env={'at':at,'fn':fn,'ip':ip,'C':C,'zlib':zlib,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':re,'json':json,'Path':Path,'LIB':LIB,'ARGS':ARGS,'SOURCE_HASHES':{},'SOURCE_FILES':{},'sys':sys,'ROOT':ROOT,'base64':__import__('base64')}
exec(compile(ast.Module(body=[klass],type_ignores=[]),str(source),'exec'),env)
Paint=env['P']
for owner in (at.TextureOracle,Paint):
 for name,value in owner.__dict__.items():
  if callable(value) and not name.startswith('__'): setattr(p,name,types.MethodType(value,p))
p.gles=ip.GLES(); p.errno_ptr=p.alloc(4); p.gl_calls=[]; p.shader_sources={}; p.shader_source_calls=[]; p.native_samplers={};p.native_syncs={};p.native_logs=[];p.next_gl_id=100
p.callbacks['glGetString']=p.get_string
p.callbacks['glGetIntegerv']=p.get_integer
p.callbacks['glGetError']=lambda:p.x(0,p.gles.f['glGetError']())
p.callbacks['glPixelStorei']=lambda:(p.gl_calls.append({'call':'glPixelStorei','pname':p.x(0),'value':p.x(1)}),p.gles.f['glPixelStorei'](p.x(0),p.x(1)))
p.callbacks['glDeleteTextures']=lambda:(lambda n,ptr: (lambda ids: (ip.cfunc(p.gles.gl,'glDeleteTextures',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*ids)),p.gl_calls.append({'call':'glDeleteTextures','ids':ids})))(list(p.get(ptr,'I'*n))))(p.x(0),p.x(1))
p.callbacks['glDeleteSamplers']=lambda:(lambda n,ptr: (lambda ids: (p._sync_proc('glDeleteSamplers',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*[p.native_samplers.pop(i) for i in ids])),p.gl_calls.append({'call':'glDeleteSamplers','ids':ids})))(list(p.get(ptr,'I'*n))))(p.x(0),p.x(1))
p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xabc)
p.callbacks['strstr']=p.strstr; p.callbacks['strncpy']=p.strncpy
p.callbacks['__strncpy_chk']=lambda: (p.u.mem_write(p.x(0),bytes(p.u.mem_read(p.x(1),p.x(2)))),p.x(0,p.x(0)))
# Evaluate the exact GL entrypoint callback map from the pinned paint adapter.
init=next(n for n in klass.body if isinstance(n,ast.FunctionDef) and n.name=='__init__')
updates=[]
for n in ast.walk(init):
 if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr=='update' and isinstance(n.func.value,ast.Attribute) and n.func.value.attr=='callbacks': updates.append(n)
entry=next(n.args[0] for n in updates if isinstance(n.args[0],ast.Dict) and any(isinstance(k,ast.Constant) and k.value=='glCreateShader' for k in n.args[0].keys)); callbacks=eval(compile(ast.Expression(entry),'paint-callback-map','eval'),env|{'self':p}); p.callbacks.update(callbacks)
for n in ast.walk(init):
 if isinstance(n,ast.Assign) and any(isinstance(t,ast.Subscript) and isinstance(t.value,ast.Attribute) and t.value.attr=='callbacks' for t in n.targets):
  key=n.targets[0].slice
  if isinstance(key,ast.Constant) and isinstance(key.value,str): p.callbacks[key.value]=eval(compile(ast.Expression(n.value),'paint-callback-assignment','eval'),env|{'self':p})
for name,method in [('glGenSamplers','gl_gen_samplers'),('glSamplerParameteri','gl_sampler_parameter'),('glFenceSync','gl_fence_sync'),('glClientWaitSync','gl_client_wait_sync'),('glWaitSync','gl_wait_sync'),('glDeleteSync','gl_delete_sync')]: p.callbacks[name]=getattr(p,method)
p.callbacks['glGetActiveUniform']=lambda:(setattr(p,'current_plt_symbol','glGetActiveUniform'),p.gl_get_active())
p.callbacks['glGetActiveAttrib']=lambda:(setattr(p,'current_plt_symbol','glGetActiveAttrib'),p.gl_get_active())
# Draw records include the actual Mesa state at each native mask/composite stage.
_basic_gl_state=p.gl_state
def _joined_gl_state(self):
 state=_basic_gl_state()
 get_i=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(self.gles.gl.glGetIntegerv)
 get_b=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_ubyte))(self.gles.gl.glGetBooleanv)
 get_enabled=C.CFUNCTYPE(C.c_ubyte,C.c_uint)(self.gles.gl.glIsEnabled)
 viewport=(C.c_int*4)();get_i(0x0BA2,viewport);state['viewport']=list(viewport)
 state['blend_enabled']=bool(get_enabled(0x0BE2))
 for label,pname in [('blend_equation_rgb',0x8009),('blend_equation_alpha',0x883D),('blend_src_rgb',0x80C9),('blend_dst_rgb',0x80C8),('blend_src_alpha',0x80CB),('blend_dst_alpha',0x80CA)]:
  value=C.c_int();get_i(pname,C.byref(value));state[label]=value.value
 mask=(C.c_ubyte*4)();get_b(0x0C23,mask);state['color_mask']=[bool(x) for x in mask]
 return state
p.gl_state=types.MethodType(_joined_gl_state,p)
# Add pinned Renderer at a disjoint address, use exact dynsym addresses for defined relocations.
renderer=LIB/'libSPenRenderer.so'; blob=renderer.read_bytes(); base=0x9000000
ph=struct.unpack_from('<Q',blob,32)[0]; ent,n=struct.unpack_from('<HH',blob,54); extent=0
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
 if t==1:extent=max(extent,va+ms)
p.u.mem_map(base,(extent+0xfff)&~0xfff)
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
 if t==1 and ms:p.u.mem_write(base+va,blob[o:o+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
envsub=dict(os.environ);envsub['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=envsub).splitlines():
 q=line.strip().split(' ',2)
 if len(q)==3 and q[1] in ('T','W'):p.syms[q[2]]=base+int(q[0],16)
objdump=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
p.syms['typeinfo for SPen::IGLManagedMsgQueue']=base+0x6b400
for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',subprocess.check_output([str(objdump),'-d','-C','-j','.plt',str(renderer)],text=True,env=envsub),re.M):p.plt[base+int(a,16)]=name
# Renderer external relocations are bound to pinned caller DSO exports; internal ABS64 use exact dynsym.
rawsyms={}
for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=envsub).splitlines():
 q=line.split()
 if len(q)==3 and q[1].upper() in ('T','W','D','B','R','V'):rawsyms[q[2]]=base+int(q[0],16)
shoff=struct.unpack_from('<Q',blob,40)[0]; se,sn,ss=struct.unpack_from('<HHH',blob,58);secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*se) for i in range(sn)];names=blob[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs};ds,st=by['.dynsym'],by['.dynstr'];strings=blob[st[4]:st[4]+st[5]];sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,inf,oth,ndx,val,size=struct.unpack_from('<IBBHQQ',blob,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',blob,off);typ,idx=info&0xffffffff,info>>32
  if typ==0x403:target=base+add
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx];target=base+val if val else rawsyms.get(name,0) or p.syms.get(name,0)
   if typ in (0x401,0x101):target+=add
  else:continue
  if target:p.u.mem_write(base+where,struct.pack('<Q',target))
# GL hosts/factory functions follow Marker3 paint adapter's normal setup.
p.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',None)
p.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)',None)
# Basic Renderer dependency init array.
sec=by['.init_array']
for off in range(0,sec[5],8):
 target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+off,8))[0]
 if target:
  for i in range(4):p.x(i,0)
  p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP);p.u.emu_start(target,m.oc.STOP,count=3000000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP:raise RuntimeError(f'Renderer init failed at {target:#x}')
# Enforce an empty GL error queue before and after every wrapped GL entrypoint.
gl_error_checks=[]
for glname,glfn in list(p.callbacks.items()):
 if glname.startswith('gl') and glname!='glGetError':
  def make_gl_guard(name,fn):
   def guarded():
    before=p.gles.f['glGetError']()
    if before: gl_error_checks.append({'call':name,'when':'before','error':before}); raise RuntimeError(f'pending GL error before {name}: {before:#x}')
    fn()
    after=p.gles.f['glGetError']()
    gl_error_checks.append({'call':name,'when':'after','error':after})
    if after: raise RuntimeError(f'GL error after {name}: {after:#x}; gl_tail={p.gl_calls[-8:]!r}; state={p.gl_state()!r}')
   return guarded
  p.callbacks[glname]=make_gl_guard(glname,glfn)
_native_tex_image=p.callbacks['glTexImage2D']
def _record_bound_texture_tex_image():
 bound=p.gl_state()['texture2d']
 _native_tex_image()
 p.gl_calls[-1]['boundTexture']=bound
p.callbacks['glTexImage2D']=_record_bound_texture_tex_image
try:
 vao=C.c_uint(); C.CFUNCTYPE(None,C.c_int,C.POINTER(C.c_uint))(p.gles.gl.glGenVertexArrays)(1,C.byref(vao)); C.CFUNCTYPE(None,C.c_uint)(p.gles.gl.glBindVertexArray)(vao.value)
 if not vao.value: raise RuntimeError('host GLES failed to allocate the required VAO')
 p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Init()', [rt], limit=5000000)

except Exception as e:
 print('INIT FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'entries',m.traced_methods[-20:]);raise
# Provide the task-queue base and managed-object subobject distinctly.
queue=m.queue; qvt=m.qvt
p.queue=queue; p.put(qvt-16,'qQ',0,p.syms['typeinfo for SPen::IGLManagedMsgQueue'])
queue_managed=p.alloc(16); managed_queue=p.alloc(32); managed_vt=p.alloc(64); managed_register=p.alloc(16); managed_unregister=p.alloc(16)
managed_tokens={}; managed_events=[]
p.put(queue_managed,'4I',0xd65f03c0,0,0,0) # function code ignored by address hook
p.put(managed_queue,'Q',managed_vt); p.put(managed_vt+16,'Q',managed_register);p.put(managed_vt+24,'Q',managed_unregister);p.put(qvt+56,'Q',queue_managed)
def queue_hooks(u,a,size,user):
 if a==queue_managed:
  p.x(0,managed_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_register:
  manager0,obj=p.x(0),p.x(1); token=p.alloc(8);p.put(token,'Q',obj);managed_tokens[token]=obj;managed_events.append(['Register',manager0,obj,token]);p.x(0,token);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_unregister:
  manager0,token=p.x(0),p.x(1);obj=managed_tokens.pop(token,None);managed_events.append(['Unregister',manager0,obj,token]);p.x(0,0);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(4,queue_hooks)
# Let native PenGLDataManager::GetPenCanvas create the actual internal canvas instead of CanvasProbe's matrix-only fake.
p.callbacks.pop('SPen::PenGLDataManager::GetPenCanvas(SPen::IGLMsgQueue*, SPen::String*, int, int, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',None)
p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None)
# Allocate Marker3's own mask canvas on the existing Drawing-selected RT.
try:p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,256,256,queue],limit=5_000_000)
except Exception as e: print('CREATE_CANVAS_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'x0x8',[hex(p.x(i)) for i in range(9)],'stack',p.u.mem_read(p.u.reg_read(R.UC_ARM64_REG_SP),64).hex()); raise

# Build the independent native RGBA8 destination exactly as the scale oracle does.
fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0'); initial=p.alloc(256*256*4);p.u.mem_write(initial,bytes(256*256*4))
factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
def call_stack(name,x,stack):
 for i,v in enumerate(x):p.x(i,v)
 sp=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack),*stack));p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP);p.u.emu_start(p.syms[name],m.oc.STOP,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP:raise RuntimeError('stack call failed '+name)
call_stack(factory,[queue,256,256,fmt,initial,1,1024,1],[6,2]); outbmp=p.x(0)
if not outbmp:raise RuntimeError('external native RGBA bitmap factory returned null')
# Capture source-caller vector slots before adding the explicitly labeled host bridge.
pre80=p.get(rt+80,'Q')[0];pre88=p.get(rt+88,'Q')[0]
# Host materialization bridge: build the std::vector descriptor from the exact native saved-redraw callback bytes because the caller fixture clears this field before return. Native Draw consumes RT+88.
vector_obj=p.alloc(24); vector_storage=p.alloc(len(vector_data)); p.u.mem_write(vector_storage,vector_data); p.put(vector_obj,'3Q',vector_storage,vector_storage+len(vector_data),vector_storage+len(vector_data)); p.put(rt+88,'Q',vector_obj)
if p.get(rt+88,'Q')[0]!=vector_obj:raise RuntimeError('Marker2 callback vector bridge failed')
canvas=p.alloc(512);p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1],limit=5_000_000)
p.call('SPen::SPCanvasImpl::GetBitmap()',[canvas]);bitmap=p.x(0);p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);bitmap=p.x(0)

# The actual native callback queue has already run the destructor-created SendDataToGPU WorkerTask.
# Re-submit the exact saved-redraw XY callback samples to the Drawing-selected native RT. This explicit host replay closes the cleared callback-vector boundary; no model points or alternate geometry are introduced.
addpoint=f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::AddPoint(SPen::Vector2<float>)'
replayed_samples=[]
for row in m.captured_callbacks:
 if row['phase']!='saved_redraw': continue
 for x,y in row['xy']:
  p.x(0,rt); p.f(0,x); p.f(1,y); p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000); p.u.reg_write(R.UC_ARM64_REG_LR,m.oc.STOP); p.u.emu_start(p.syms[addpoint],m.oc.STOP,count=500000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m.oc.STOP: raise RuntimeError('native Marker2 AddPoint replay failed')
  replayed_samples.append([x,y])
if not replayed_samples: raise RuntimeError('no saved-redraw samples available to replay')
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Update(bool)',[rt,0],limit=5_000_000)

# Identify and clear the actual external RGBA target attached by the native canvas.
output_uploads=[c for c in p.gl_calls if c['call']=='glTexImage2D' and c['hasPixels'] and c['format']==0x1908]
if len(output_uploads)!=1: raise RuntimeError(f'expected exactly one uploaded external RGBA target, saw {output_uploads!r}')
output_texture=output_uploads[0]['boundTexture']
output_attachments=[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D' and c['args'][3]==output_texture]
if len(output_attachments)!=1: raise RuntimeError(f'external texture must attach to exactly one FBO: {output_attachments!r}')
output_fbo=output_attachments[0]['framebuffer']
def host_gl_checked(name,fn,*args):
 before=p.gles.f['glGetError']();gl_error_checks.append({'call':name,'when':'before','error':before})
 if before: raise RuntimeError(f'pending GL error before {name}: {before:#x}')
 result=fn(*args)
 after=p.gles.f['glGetError']();gl_error_checks.append({'call':name,'when':'after','error':after})
 if after: raise RuntimeError(f'GL error after {name}: {after:#x}')
 return result
host_gl_checked('hostBindOutputFbo',p.gles.f['glBindFramebuffer'],0x8d40,output_fbo)
target_status=host_gl_checked('hostCheckOutputFbo',p.gles.f['glCheckFramebufferStatus'],0x8d40)
if target_status!=0x8cd5: raise RuntimeError(f'external target FBO {output_fbo} incomplete: {target_status:#x}')
host_gl_checked('hostOutputViewport',p.gles.f['glViewport'],0,0,256,256)
host_gl_checked('hostClearColor',p.gles.f['glClearColor'],0.0,0.0,0.0,0.0)
host_gl_checked('hostClearOutput',p.gles.f['glClear'],0x00004000)
preclear=C.create_string_buffer(256*256*4); _read_rgba=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)
host_gl_checked('hostReadOutputAfterClear',_read_rgba,0,0,256,256,0x1908,0x1401,preclear)
if any(preclear.raw): raise RuntimeError('external RGBA target was not fully transparent after clear')

# Identity canvas matrix; source points are native RT callback coordinates in the fixed 256x256 profile.
mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
# Mesa core profile requires a bound VAO. Native Renderer expects its app host context to supply this state.
_real_draw_cb=p.callbacks['glDrawArraysInstanced']
draw_attribute_state={}
def _draw_attr_diag():
 global draw_attribute_state
 _getattr_now=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetVertexAttribiv)
 _getprogram=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetProgramiv)
 _state={}
 for _idx in range(4):
  _state[str(_idx)]={}
  for _label,_pname in [('enabled',0x8622),('size',0x8623),('type',0x8625),('stride',0x8624),('buffer',0x889F),('divisor',0x88FE)]:
   _v=C.c_int();_getattr_now(_idx,_pname,C.byref(_v));_state[str(_idx)][_label]=_v.value
 _prog=C.c_int();C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetIntegerv)(0x8B8D,C.byref(_prog));_link=C.c_int();_nattr=C.c_int();_getprogram(_prog.value,0x8B82,C.byref(_link));_getprogram(_prog.value,0x8B89,C.byref(_nattr))
 query_error=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'draw-state-queries','when':'after','error':query_error})
 if query_error: raise RuntimeError(f'GL error while capturing draw state: {query_error:#x}')
 draw_attribute_state={'attributes':_state,'program':_prog.value,'program_linked':_link.value==1,'active_attribute_count':_nattr.value,'gl_state':p.gl_state()}
 args=[p.x(i) for i in range(4)]
 before=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'glDrawArraysInstanced','when':'before','error':before})
 if before: raise RuntimeError(f'pending GL error before glDrawArraysInstanced: {before:#x}')
 p.gl_calls.append({'call':'glDrawArraysInstanced','values':args,'state':p.gl_state()})
 p.gles.f['glDrawArraysInstanced'](*args)
 _draw_error=p.gles.f['glGetError']()
 gl_error_checks.append({'call':'glDrawArraysInstanced','when':'after','error':_draw_error})
 if _draw_error: raise RuntimeError(f'glDrawArraysInstanced itself returned {_draw_error:#x}')
 return
p.callbacks['glDrawArraysInstanced']=_draw_attr_diag
p.call(f'SPen::Marker2StrokeDrawableRTV{m.selected_drawable_version}::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)
post_draw_vec=p.get(rt+88,'Q')[0]
post_draw_framebuffer=p.gl_state()['framebuffer']
host_gl_checked('hostRebindOutputFboForReadback',p.gles.f['glBindFramebuffer'],0x8d40,output_fbo)
host_gl_checked('hostFinish',p.gles.gl.glFinish)
pixels=C.create_string_buffer(256*256*4);host_gl_checked('hostReadOutputRGBA',_read_rgba,0,0,256,256,0x1908,0x1401,pixels)
if any(row['error'] for row in gl_error_checks): raise RuntimeError(f'nonzero GL errors: {gl_error_checks}')
nonzero=[(i%256,i//256) for i in range(256*256) if pixels.raw[i*4:i*4+4]!=bytes(4)]
bbox=([min(pt[0] for pt in nonzero),min(pt[1] for pt in nonzero),max(pt[0] for pt in nonzero),max(pt[1] for pt in nonzero)] if nonzero else None)
draws=[c for c in p.gl_calls if c['call'] in ('glDrawArrays','glDrawArraysInstanced')]
if len(draws)!=2: raise RuntimeError(f'expected native mask and composite draws, observed {draws!r}')
mask_fbo=draws[0]['state']['framebuffer']; composite_fbo=draws[1]['state']['framebuffer']
mask_attachment=next((c for c in p.gl_calls if c['call']=='glFramebufferTexture2D' and c['framebuffer']==mask_fbo),None)
if mask_attachment is None: raise RuntimeError('native mask draw framebuffer has no recorded texture attachment')
mask_texture=mask_attachment['args'][3]; sampled_texture=draws[1]['state']['texture2d']
if mask_fbo==output_fbo or composite_fbo!=output_fbo or sampled_texture!=mask_texture or sampled_texture==output_texture:
 raise RuntimeError(f'Marker2 paint resources alias or mismatch: maskFbo={mask_fbo}, outputFbo={output_fbo}, compositeFbo={composite_fbo}, maskTexture={mask_texture}, sampleTexture={sampled_texture}, outputTexture={output_texture}')
if post_draw_framebuffer!=output_fbo: raise RuntimeError(f'native Draw ended on FBO {post_draw_framebuffer}, expected output FBO {output_fbo}')

# Record only bounded native caller and paint evidence; Marker2's vector transport is separate from this callback snapshot.
source_case=json.loads(source_fixture.read_text())['cases'][f'marker2_nontrivial_saved3:{ADVANCED_SETTING}:tool{TOOL}:curve1:size'+('00000000' if CLI.size=='0' else '00002041')]
rows=m.captured_callbacks
if not rows or not source_case: raise RuntimeError('caller-linked Marker2 source case or callback count missing')
if p.get(rt+60,'I')[0] != (0 if CLI.size=='0' else 10): raise RuntimeError('native RT width does not match SetPenData expected truncation')
expected_version='1' if ADVANCED_SETTING=='null' else '2'
if m.selected_drawable_version!=expected_version: raise RuntimeError(f'advanced setting {ADVANCED_SETTING} selected RTV{m.selected_drawable_version}, expected RTV{expected_version}')
if pre80!=0 or pre88!=0: raise RuntimeError('native Drawing caller did not leave the expected cleared RT vector slots')
if CLI.size=='0' and any(pixels.raw): raise RuntimeError('zero-width Marker2 draw produced nonzero target pixels')
if CLI.size=='10' and not any(pixels.raw): raise RuntimeError('positive-width Marker2 draw produced an empty target')
if not gl_error_checks or any(row['error'] for row in gl_error_checks): raise RuntimeError(f'expected all-zero GL error checks, got {gl_error_checks}')
report={'schema':'marker2-native-rt-paint-experiment-v1','case_key':CASE_KEY,'advanced_setting':ADVANCED_SETTING,'pen_size_input':float(CLI.size),'tool':int(TOOL),'scope':'Actual Marker2 Drawing factory selected RTV'+str(m.selected_drawable_version)+' for this ObjectStroke input and completed native Init/CreatePenCanvas/Update(false)/Draw into an RGBA8 Mesa target. Source callback stream is separately captured; saved callback XY and bytes are explicitly replayed/materialized at RT+88 by the host adapter, so native AddPoint ownership transfer is not claimed.','input_case':source_case,'selected_drawable_version':m.selected_drawable_version,'selected_rt':hex(rt),'rt_width_u32':p.get(rt+60,'I')[0],'callback_count':len(rows),'callback_vector_hex':vector_data.hex(),'callback_vector_sha256':hashlib.sha256(vector_data).hexdigest(),'callback_vector_bytes':len(vector_data),'replayed_saved_redraw_xy':replayed_samples,'replay_boundary':'host replays captured saved-redraw AddPoint XY into the selected RTV and materializes the captured native callback float bytes as an explicit std::vector descriptor at RT+88; native caller ownership/SendData transfer is not claimed','native_rt_color_rgba_f32':list(p.get(rt+44,'4f')),'rt_vector_slots_after_caller':[hex(pre80),hex(pre88)],'host_bridge_rt_plus88':hex(vector_obj),'output_target':{'fbo':output_fbo,'texture':output_texture,'preclear_readback_bytes':len(preclear.raw),'preclear_readback_sha256':hashlib.sha256(preclear.raw).hexdigest(),'preclear_readback_zlib_base64':base64.b64encode(zlib.compress(preclear.raw,9)).decode('ascii'),'preclear_all_zero':not any(preclear.raw),'post_draw_fbo':post_draw_framebuffer,'readback_fbo':output_fbo},'native_draw_calls':draws,'draw_vertex_state':draw_attribute_state,'buffer_uploads':[c for c in p.gl_calls if c['call']=='glBufferData'],'shader_sources':p.shader_source_calls,'program_calls':[c for c in p.gl_calls if c['call'] in ('glCreateProgram','glAttachShader','glLinkProgram','glUseProgram')],'attachments':[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D'],'framebuffer_checks':[c for c in p.gl_calls if c['call']=='glCheckFramebufferStatus'],'complete_fbos':sorted({c['framebuffer'] for c in p.gl_calls if c['call']=='glCheckFramebufferStatus' and c['status']==0x8cd5}),'texture_allocations':[c for c in p.gl_calls if c['call']=='glTexImage2D'],'viewport_calls':[c for c in p.gl_calls if c['call']=='glViewport'],'paint_execution':[f'Marker2StrokeDrawableRTV{m.selected_drawable_version}::Init()',f'Marker2StrokeDrawableRTV{m.selected_drawable_version}::CreatePenCanvas(256,256,queue)','SPGraphicsFactory::CreateBitmap(...,RGBA8,RGBA)','SPCanvasImpl(queue,outputBitmap,true)',f'Marker2StrokeDrawableRTV{m.selected_drawable_version}::Update(false)',f'Marker2StrokeDrawableRTV{m.selected_drawable_version}::Draw(outputSubBitmap,identityMatrix4)'],'gl_errors':gl_error_checks,'readback':{'width':256,'height':256,'bytes':len(pixels.raw),'sha256':hashlib.sha256(pixels.raw).hexdigest(),'all_zero':not any(pixels.raw),'nonzero_pixels':len(nonzero),'bbox':bbox,'zlib_base64':base64.b64encode(zlib.compress(pixels.raw,9)).decode('ascii')},'native_libraries':{name:hashlib.sha256((LIB/name).read_bytes()).hexdigest() for name in PINNED_LIBRARIES},'caller_fixture_sha256':hashlib.sha256((ROOT/'conformance/marker2-saved-caller-native.json').read_bytes()).hexdigest(),'caller_script_sha256':hashlib.sha256((ROOT/'conformance/marker2_saved_caller_native.py').read_bytes()).hexdigest(),'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'host_gles':p.gles.identity,'boundaries':['Native Marker2 Drawing/caller and RT methods execute in pinned Unicorn AArch64 DSOs.','Mesa GLES is host-provided; object/queue/canvas lifecycle adapters remain explicit.','The host adapter reads the final RGBA output; the direct intermediate GL_RED readback is omitted because the active attachment is RGBA8 and that readback format is not valid for it.','The full callback stream is not asserted to equal an RT GPU upload.','This is not Android device GPU parity or final application Canvas composition.']}
report['caller_execution_transform']={'source_sha256':hashlib.sha256(caller_path.read_bytes()).hexdigest(),'temporary_fixture_assignment':'FIXTURE=SOURCE_FIXTURE','source_fixture_location':'external per-run temporary directory, deleted when the runner exits','print_only_transform':'temporary source-case label replaces repo-relative path formatting'}
report['draw_resource_binding']={'mask_fbo':mask_fbo,'mask_texture':mask_texture,'mask_draw_fbo':mask_fbo,'composite_fbo':composite_fbo,'composite_sampler_texture':sampled_texture,'output_fbo':output_fbo,'output_texture':output_texture,'feedback_alias':False}
serialized=json.dumps(report,indent=2,sort_keys=True)+'\n'
if not gl_error_checks or any(row['error'] for row in gl_error_checks): raise RuntimeError(f'expected all-zero GL error checks, got {gl_error_checks}')
encoded=base64.b64decode(report['readback']['zlib_base64']); decoded=zlib.decompress(encoded)
if decoded!=pixels.raw or len(decoded)!=256*256*4: raise RuntimeError('embedded full readback failed round-trip validation')
predecoded=zlib.decompress(base64.b64decode(report['output_target']['preclear_readback_zlib_base64']))
if predecoded!=preclear.raw or len(predecoded)!=256*256*4: raise RuntimeError('embedded full preclear readback failed round-trip validation')
if CLI.write_fixture:
 aggregate=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else {'schema':'marker2-zero-width-native-paint-matrix-v1','cases':{}}
 if aggregate.get('schema')!='marker2-zero-width-native-paint-matrix-v1': raise RuntimeError('unexpected Marker2 fixture schema')
 for other_key,other in aggregate.get('cases',{}).items():
  if other.get('tool')==int(TOOL) and other.get('callback_vector_sha256')!=report['callback_vector_sha256']:
   raise RuntimeError(f'saved callback source stream changed between {other_key} and {CASE_KEY}')
 aggregate['cases'][CASE_KEY]=report
 FIXTURE.write_text(json.dumps(aggregate,indent=2,sort_keys=True)+'\n')
else:
 if not FIXTURE.exists(): raise SystemExit(f'missing fixture: {FIXTURE}; use --write-fixture')
 aggregate=json.loads(FIXTURE.read_text())
 expected=aggregate.get('cases',{}).get(CASE_KEY)
 if expected is None: raise SystemExit(f'missing fixture case {CASE_KEY}; use --write-fixture')
 if expected!=report:
  def changes(left,right,path=''):
   if isinstance(left,dict) and isinstance(right,dict):
    for key in sorted(set(left)|set(right)):
     if key not in left or key not in right: print('DIFF',path+'/'+key,'missing-side')
     else: changes(left[key],right[key],path+'/'+key)
   elif isinstance(left,list) and isinstance(right,list):
    if len(left)!=len(right): print('DIFF',path,'length',len(left),len(right))
    for index,(a,b) in enumerate(zip(left,right)): changes(a,b,f'{path}[{index}]')
   elif left!=right: print('DIFF',path,repr(left)[:160],repr(right)[:160])
  changes(expected,report)
  raise SystemExit(f'Marker2 fixture case {CASE_KEY} differs; inspect and use --write-fixture only for reviewed oracle updates')
print(json.dumps({'fixture':str(FIXTURE),'case':CASE_KEY,'mode':'write' if CLI.write_fixture else 'check','case_sha256':hashlib.sha256(serialized.encode()).hexdigest(),'vector_bytes':len(vector_data),'rgba_bytes':len(decoded),'rgba_sha256':hashlib.sha256(decoded).hexdigest(),'gl_error_checks':len(gl_error_checks)},sort_keys=True))
