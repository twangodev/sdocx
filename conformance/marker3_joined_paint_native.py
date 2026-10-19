"""Replay the selected Marker3 Drawing vector through its own RTV1 to host GLES.

Run from the repository root or by absolute path with
`PYTHONPATH=scratch/apk-analysis-runtime/python:conformance python3 conformance/marker3_joined_paint_native.py`.
No mode means exact fixture check; use `--write-fixture` only for reviewed updates.
"""
import argparse, ast, base64, contextlib, hashlib, importlib.util, io, json, os, re, struct, subprocess, sys, types, zlib
from pathlib import Path
from unicorn import arm64_const as R
ROOT=Path('/home/jding/RustroverProjects/sdocx'); LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
PINNED_LIBRARIES={
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenMarker3.so':'d9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5',
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
PINNED_HELPERS={
 'marker3-saved-caller-native.json':'6551bc4a2403079c54ddae2a8123b63189030596f9af79f5b7c5af1c2349d090',
 'marker3_saved_caller_native.py':'6237076d662e0390711b2a69b43bde4e07369297b9897a675c693550f297550d',
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
FIXTURE=ROOT/'conformance/marker3-joined-paint-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group()
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
CLI=parser.parse_args()
# Execute the checked-in selected V5 caller, instrumenting only a transient pointer
# observation so its returned vector can be handed to the same native RT.
caller_path=ROOT/'conformance/marker3_saved_caller_native.py'
caller_source=caller_path.read_text()
# Use the caller fixture's own IGLMsgQueue for its native callback tasks.
class_anchor='p=Marker3CanvasProbe()'
class_patch=r'''_marker3_old_canvas_hook=Marker3CanvasProbe.hook_canvas
def _marker3_queue_canvas_hook(self,uc,address,size,data):
 if address==0x200f2040 and getattr(self,'native_task_queue',0):
  self.x(0,self.native_task_queue); uc.reg_write(R.UC_ARM64_REG_PC,uc.reg_read(R.UC_ARM64_REG_LR)); return
 return _marker3_old_canvas_hook(self,uc,address,size,data)
Marker3CanvasProbe.hook_canvas=_marker3_queue_canvas_hook
'''
if caller_source.count(class_anchor)!=1: raise RuntimeError('caller class hook anchor changed')
caller_source=caller_source.replace(class_anchor,class_patch+class_anchor)
queue_anchor="p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']"
if caller_source.count(queue_anchor)!=1: raise RuntimeError('caller queue setup anchor changed')
caller_source=caller_source.replace(queue_anchor,"p._task_returns=[]; p.natural_queue_events=[]; p.deferred_native_tasks=[]; p.native_task_queue=queue; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']")
# Run the native SendData task but defer renderer and vector-delete WorkerTasks until a usable native paint target exists.
task_anchor="  if not method: raise RuntimeError('task has null Run')"
task_replace="""  p.natural_queue_events.append({'phase':phase,'task':hex(task),'vtable':hex(vt),'run':hex(method)})
  if vt in (0x10774c8,0x1077528):
   p.deferred_native_tasks.append({'task':task,'vtable':vt,'run':method})
   p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return
  if not method: raise RuntimeError('task has null Run')"""
if caller_source.count(task_anchor)!=1: raise RuntimeError('native task hook anchor changed')
caller_source=caller_source.replace(task_anchor,task_replace)
needle='pending_marker3_addpoint={}'
if caller_source.count(needle)!=1: raise RuntimeError('caller capture anchor changed')
caller_source=caller_source.replace(needle, 'transient_vector_objects={}\n'+needle)
needle2="  vec=struct.unpack('<Q',u.mem_read(this+80,8))[0]\n"
if caller_source.count(needle2)!=1: raise RuntimeError('vector capture anchor changed')
caller_source=caller_source.replace(needle2, needle2+"  transient_vector_objects.setdefault(record['phase'], []).append(vec)\n")
sys.argv=['marker3-saved-caller','--check-fixture','--case','marker3_linked_saved1','--advanced-setting','5']
spec=importlib.util.spec_from_file_location('m3caller',caller_path); m=importlib.util.module_from_spec(spec); m.__file__=str(caller_path)
with contextlib.redirect_stdout(io.StringIO()): exec(compile(caller_source,str(caller_path),'exec'),m.__dict__)
p=m.p; rt=m.selected_rt
draw_rows=[r for r in m.captured_callbacks if r['phase']=='draw_object_stroke']; saved_rows=[r for r in m.captured_callbacks if r['phase']=='saved_redraw']
assert len(draw_rows)==22 and len(saved_rows)==22
draw_vecobj=m.transient_vector_objects['draw_object_stroke'][-1]; saved_vecobj=m.transient_vector_objects['saved_redraw'][-1]; vecobj=saved_vecobj
assert len(set(m.transient_vector_objects['draw_object_stroke']))==len(set(m.transient_vector_objects['saved_redraw']))==1
vec_begin,vec_end,vec_cap=struct.unpack('<3Q',p.u.mem_read(vecobj,24)); vector_data=bytes(p.u.mem_read(vec_begin,vec_end-vec_begin)); assert len(vector_data)==176
assert vector_data==bytes.fromhex(m.captured_final_vectors['draw_object_stroke']['vector_bytes'])==bytes.fromhex(m.captured_final_vectors['saved_redraw']['vector_bytes'])
source_slots_after_caller=[hex(v) for v in p.get(rt+80,'2Q')]
if source_slots_after_caller != ['0x0',hex(saved_vecobj)]: raise RuntimeError(f'native SendDataToGPU task did not transfer the selected saved vector to RT+88: {source_slots_after_caller}; vector={saved_vecobj:#x}')
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
    if after: raise RuntimeError(f'GL error after {name}: {after:#x}')
   return guarded
  p.callbacks[glname]=make_gl_guard(glname,glfn)
try:
 p.call('SPen::Marker3StrokeDrawableRTV1::Init()', [rt], limit=5000000)

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
try:p.call('SPen::Marker3StrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,256,256,queue],limit=5_000_000)
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
canvas=p.alloc(512);p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1],limit=5_000_000)
p.call('SPen::SPCanvasImpl::GetBitmap()',[canvas]);bitmap=p.x(0);p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);bitmap=p.x(0)

# The actual native callback queue has already run the destructor-created SendDataToGPU WorkerTask.
pre80=p.get(rt+80,'Q')[0];pre88=p.get(rt+88,'Q')[0]
if pre80 or pre88!=vecobj:raise RuntimeError(f'native SendDataToGPU handoff mismatch: +80={pre80:#x}, +88={pre88:#x}, saved vector={vecobj:#x}')
p.call('SPen::Marker3StrokeDrawableRTV1::Update(bool)',[rt,0],limit=5_000_000)

# Identity canvas matrix; source points are native RT callback coordinates in the fixed 256x256 profile.
mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
p.call('SPen::Marker3StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)
post_draw_vec=p.get(rt+88,'Q')[0]
if post_draw_vec!=vecobj or bytes(p.u.mem_read(vec_begin,vec_end-vec_begin))!=vector_data:raise RuntimeError('native draw did not preserve the SendData vector through upload/paint')
p.gles.gl.glFinish(); pixels=C.create_string_buffer(256*256*4);read=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels);read(0,0,256,256,0x1908,0x1401,pixels)
errs=p.gles.f['glGetError']()
if errs or any(row['error'] for row in gl_error_checks): raise RuntimeError(f'nonzero GL errors: final={errs} checks={gl_error_checks}')
nonzero=[(i%256,i//256) for i in range(256*256) if pixels.raw[i*4:i*4+4]!=bytes(4)]
bbox=[min(pt[0] for pt in nonzero),min(pt[1] for pt in nonzero),max(pt[0] for pt in nonzero),max(pt[1] for pt in nonzero)]
draws=[c for c in p.gl_calls if c['call'] in ('glDrawArrays','glDrawArraysInstanced')]

# Runtime evidence report: geometry is transferred by the actual native SendDataToGPU WorkerTask.
source_case=json.loads((ROOT/'conformance/marker3-saved-caller-native.json').read_text())['cases']['marker3_linked_saved1:5:tool2:curve1']
rows=draw_rows
assert len(rows)==22 and len(set(m.transient_vector_objects['draw_object_stroke']))==1
assert p.get(rt+88,'Q')[0]==vecobj and bytes(p.u.mem_read(vec_begin,vec_end-vec_begin))==vector_data
send_tasks=[task for task in m.p.natural_queue_events if task['vtable']=='0x10774f8']
assert len(send_tasks)==2 and len(m.p.deferred_native_tasks)==4
native_routes_by_phase={phase:[event['vtable'] for event in m.p.natural_queue_events if event['phase']==phase and event['vtable'] in ('0x10774f8','0x10774c8','0x1077528')] for phase in ('draw_object_stroke','saved_redraw')}
assert all(route==['0x10774f8','0x10774c8','0x1077528'] for route in native_routes_by_phase.values())
assert len(vector_data)==176 and vector_data==bytes.fromhex(m.captured_final_vectors['saved_redraw']['vector_bytes'])
uploads=[c for c in p.gl_calls if c['call']=='glBufferData' and c.get('size')==len(vector_data) and c.get('dataSha256')==hashlib.sha256(vector_data).hexdigest()]
assert uploads, 'no GPU upload matching native Drawing vector bytes'
attachments=[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D']
maskdraw=next(c for c in draws if c['call']=='glDrawArraysInstanced'); compdraw=next(c for c in draws if c['call']=='glDrawArrays')
maskatt=next(c for c in attachments if c['framebuffer']==maskdraw['state']['framebuffer']);outatt=next(c for c in attachments if c['framebuffer']==compdraw['state']['framebuffer'])
complete={c['framebuffer'] for c in p.gl_calls if c['call']=='glCheckFramebufferStatus' and c['status']==0x8cd5}
assert maskatt['args'][3]!=outatt['args'][3] and maskatt['framebuffer']!=outatt['framebuffer']
assert compdraw['state']['texture2d']==maskatt['args'][3]
assert {maskatt['framebuffer'],outatt['framebuffer']}<=complete
assert maskdraw['state']['viewport']==compdraw['state']['viewport']==[0,0,256,256]
assert maskdraw['state']['framebuffer']!=compdraw['state']['framebuffer'] and maskatt['args'][3]!=outatt['args'][3]

report={'schema':'marker3-drawing-selected-rt-host-gles-positive-v2','scope':'Actual Model ObjectStroke setters and native ObjectDrawing route selected Marker3 GLV5/RTV1 and native RedrawPen. Its PenReturnCallback destructor posted a native WorkerTask::Member2<PenDrawableRT, SendDataToGPU>; the explicitly wired host queue ran both native SendData tasks, which populated the same selected RT +88 from the native callback vector after temporary +80 had been cleared. PenGLRenderMsg and vector-delete WorkerTasks were deferred at explicit host boundaries while the same RT executed native Init, native PenGLDataManager::GetPenCanvas/SPGraphicsFactory::CreatePenCanvas, native Update(false), and native Draw to the separately created native RGBA8 target. No vector bytes/points were re-fed and no RT slot was written by the host. Mesa GLES 3.2 llvmpipe provides GL; this is not Android device GPU parity.','source_caller_fixture_sha256':hashlib.sha256((ROOT/'conformance/marker3-saved-caller-native.json').read_bytes()).hexdigest(),'source_caller_case':source_case,'selected_drawable_version':m.selected_drawable_version,'selected_drawable':'Marker3StrokeDrawableGLV5','selected_rt':'drawing_selected_rt_1','same_vector_object':'drawing_selected_rt_saved_redraw_vector_1','same_vector_object_pointer_equal':True,'draw_phase_vector_pointer':hex(draw_vecobj),'saved_redraw_vector_pointer':hex(saved_vecobj),'native_queue_tasks':m.p.natural_queue_events,'native_task_vtable_order_by_phase':native_routes_by_phase,'rt_plus88_after_draw':hex(post_draw_vec),'native_rendered_vector_pointer_valid_after_draw':post_draw_vec==vecobj and bytes(p.u.mem_read(vec_begin,vec_end-vec_begin))==vector_data,'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'deferred_native_tasks':[{'vtable':hex(row['vtable']),'run':hex(row['run']),'task':hex(row['task'])} for row in m.p.deferred_native_tasks],'native_vector_bytes':len(vector_data),'native_vector_hex':vector_data.hex(),'uploaded_native_vector_hex':vector_data.hex(),'native_vector_sha256':hashlib.sha256(vector_data).hexdigest(),'callback_count':len(rows),'source_vector_phase':'saved_redraw','separate_saved_redraw_vector_object':'saved_redraw_vector_1','same_xy_as_saved_redraw':True,'original_rt_slots_after_drawing_caller_before_paint':source_slots_after_caller,'slots_after_native_send_task':[hex(pre80),hex(pre88)],'post_send_rt_plus88':'drawing_selected_rt_saved_redraw_vector_1','matching_native_gpu_uploads':[{'size':c['size'],'sha256':c['dataSha256'],'captured_bytes_hex':vector_data.hex()} for c in uploads],'native_rt_color_rgba_f32':list(p.get(rt+44,'4f')),'color_mapping':{'source_argb':source_case['input']['color_argb'],'context_color_conversion':'identity host callback (captured in source caller case)','native_rt_rgba_f32':list(p.get(rt+44,'4f')),'output_sample_0_0_rgba':list(pixels.raw[:4])},'host_gles':p.gles.identity,'draws':draws,'attachments':attachments,'complete_fbos':sorted(complete),'stage_resources':{'mask_framebuffer':maskatt['framebuffer'],'mask_texture':maskatt['args'][3],'composite_framebuffer':outatt['framebuffer'],'output_texture':outatt['args'][3],'sampler_texture_during_composite':compdraw['state']['texture2d'],'mask_output_alias':maskatt['args'][3]==outatt['args'][3],'complete':True},'texture_allocations':[c for c in p.gl_calls if c['call']=='glTexImage2D'],'viewport_calls':[c for c in p.gl_calls if c['call']=='glViewport'],'native_paint_execution':['Marker3StrokeDrawableRTV1::Init()','Marker3StrokeDrawableRTV1::CreatePenCanvas(256,256,queue)','SPGraphicsFactory::CreateBitmap(...,TextureMemoryLayout::RGBA8,TextureTargetLayout::RGBA)','SPCanvasImpl::SPCanvasImpl(queue,outputBitmap,true)','Marker3StrokeDrawableRTV1::Update(false)','Marker3StrokeDrawableRTV1::Draw(outputSubBitmap,identityMatrix4)'],'native_factory_route':{'internal_canvas':'PenGLDataManager::GetPenCanvas -> SPGraphicsFactory::CreatePenCanvas -> Marker3StrokeDrawableRTV1::CreatePenCanvas','external_target_factory':'SPGraphicsFactory::CreateBitmap(queue,256,256,RGBA,zeroed_RGBA8,true,1024,true,TextureMemoryLayout::RGBA8,TextureTargetLayout::RGBA)','target_layout_enum':2,'memory_layout_enum':6},'blend_calls':[c for c in p.gl_calls if c['call'] in ('glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask')],'shader_source_calls':p.shader_source_calls,'uniform_calls':[c for c in p.gl_calls if c['call'].startswith('glUniform')],'gl_error_checks':gl_error_checks,'readback':{'bytes':len(pixels.raw),'sha256':hashlib.sha256(pixels.raw).hexdigest(),'zlib_base64':base64.b64encode(zlib.compress(pixels.raw,9)).decode('ascii'),'nonzero_pixels':len(nonzero),'bbox':bbox,'samples':[{'xy':[x,y],'rgba':list(pixels.raw[(y*256+x)*4:(y*256+x+1)*4])} for x,y in ((0,0),(1,0),(20,0),(0,1),(8,0),(0,8),(128,128))]},'library_hashes':dict(m.native_hashes,**{'libSPenRenderer.so':hashlib.sha256((LIB/'libSPenRenderer.so').read_bytes()).hexdigest(),'libc++_shared.so':hashlib.sha256((LIB/'libc++_shared.so').read_bytes()).hexdigest()}),'helper_hashes':{'oblique_canvas_native.py':hashlib.sha256((ROOT/'conformance/oblique_canvas_native.py').read_bytes()).hexdigest(),'elf_plt.py':hashlib.sha256((ROOT/'conformance/elf_plt.py').read_bytes()).hexdigest(),'fountain_native.py':hashlib.sha256((ROOT/'conformance/fountain_native.py').read_bytes()).hexdigest(),'marker3-saved-caller-native.json':hashlib.sha256((ROOT/'conformance/marker3-saved-caller-native.json').read_bytes()).hexdigest(),'marker3_saved_caller_native.py':hashlib.sha256((ROOT/'conformance/marker3_saved_caller_native.py').read_bytes()).hexdigest(),'marker3_paint_scale_native.py':hashlib.sha256((ROOT/'conformance/marker3_paint_scale_native.py').read_bytes()).hexdigest(),'airbrush_texture_native.py':hashlib.sha256((ROOT/'conformance/airbrush_texture_native.py').read_bytes()).hexdigest(),'inkpen_paint_native.py':hashlib.sha256((ROOT/'conformance/inkpen_paint_native.py').read_bytes()).hexdigest()},'boundaries':['Synthetic ObjectStroke values are supplied through Model setters in the caller fixture','explicit 1920x1080 ObjectDrawing screen profile from saved-caller fixture','identity ARGB context conversion is host-supplied','host GLES context/driver and EGL context identity','actual IGLMsgQueue route is host-wired to synchronously run native WorkerTasks; managed queue registration hooks are installed','native SPGraphicsFactory creates external RGBA8 output texture with layouts 6/2','native SendDataToGPU transfers vector ownership into RT+88; no host RT slot write','PenGLRenderMsg deferred because matrix-only host canvas lacks native bitmap vtable required at its virtual call; vector-delete tasks remain pending after paint, so destruction lifecycle is unclaimed','no device GPU, Android EGL, complete renderer message/lifetime or serialized SDK parser claim']}
if len(draw_rows)!=22 or not source_case: raise RuntimeError('caller-linked source case or callback count missing')
serialized=json.dumps(report,indent=2,sort_keys=True)+'\n'
if len(gl_error_checks)!=239 or any(row['error'] for row in gl_error_checks): raise RuntimeError(f'expected 239 zero GL error checks, got {len(gl_error_checks)}')
encoded=base64.b64decode(report['readback']['zlib_base64']); decoded=zlib.decompress(encoded)
if decoded!=pixels.raw or len(decoded)!=256*256*4: raise RuntimeError('embedded full readback failed round-trip validation')
if CLI.write_fixture: FIXTURE.write_text(serialized)
else:
 if not FIXTURE.exists(): raise SystemExit(f'missing fixture: {FIXTURE}; use --write-fixture')
 expected=FIXTURE.read_text()
 if expected!=serialized: raise SystemExit('Marker3 joined paint fixture differs; use diff to inspect and --write-fixture only for reviewed oracle updates')
print(json.dumps({'fixture':str(FIXTURE),'mode':'write' if CLI.write_fixture else 'check','sha256':hashlib.sha256(serialized.encode()).hexdigest(),'vector_bytes':len(vector_data),'rgba_bytes':len(decoded),'rgba_sha256':hashlib.sha256(decoded).hexdigest(),'gl_error_checks':len(gl_error_checks)},sort_keys=True))
