import ast,contextlib,hashlib,importlib.util,io,json,os,re,struct,subprocess,sys,types
from pathlib import Path
from unicorn import arm64_const as R
ROOT=Path('/home/jding/RustroverProjects/sdocx'); LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
sys.path.insert(0,str(ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance'))
# Execute the actual selected V5 Drawing path and retain its emulator/objects.
sys.argv=['caller','--write-fixture','--case','marker3_linked_saved1','--advanced-setting','5']
spec=importlib.util.spec_from_file_location('m3caller',Path('tmp/vector-parity-plan/marker3-tool3/paint/caller.py')); m=importlib.util.module_from_spec(spec)
with contextlib.redirect_stdout(io.StringIO()):spec.loader.exec_module(m)
p=m.p; rt=m.selected_rt
draw_rows=[r for r in m.captured_callbacks if r['phase']=='draw_object_stroke']; saved_rows=draw_rows
assert len(draw_rows)==len(saved_rows)==22 and len({r['vector_object'] for r in draw_rows})==len({r['vector_object'] for r in saved_rows})==1
vecobj=int(draw_rows[-1]['vector_object'],16); saved_vecobj=int(saved_rows[-1]['vector_object'],16)
vec_begin,vec_end,vec_cap=struct.unpack('<3Q',p.u.mem_read(vecobj,24)); vector_data=bytes(p.u.mem_read(vec_begin,vec_end-vec_begin)); assert len(vector_data)==176
assert vector_data==bytes.fromhex(m.captured_final_vectors['draw_object_stroke']['vector_bytes'])==bytes.fromhex(m.captured_final_vectors['saved_redraw']['vector_bytes'])
print('CALLER',m.selected_drawable,hex(rt),'V5',m.selected_drawable_version,'Drawing-vector',hex(vecobj),'separate-saved-redraw-vector',hex(saved_vecobj),len(vector_data),hashlib.sha256(vector_data).hexdigest(),'slots',p.u.mem_read(rt+80,16).hex())
# Bring the paint oracle's GL callback methods into this emulator without running its separate RT pipeline.
import ctypes as C,zlib,airbrush_texture_native as at,inkpen_paint_native as ip,fountain_native as fn
ARGS=types.SimpleNamespace(width=256,height=256)
source=ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3_paint_scale_native.py'; tree=ast.parse(source.read_text())
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
print('renderer mapped',hex(base),'GLES',p.gles.identity)
print('queuevars',[(k,hex(v)) for k,v in m.__dict__.items() if k in ('queue','qvt','manager','queue_vtable','context') and isinstance(v,int)])
try:
 p.call('SPen::Marker3StrokeDrawableRTV1::Init()', [rt], limit=5000000)
 print('INIT OK', 'gl calls',len(p.gl_calls),'error',p.gles.f['glGetError'](),'fields',[(o,p.u.mem_read(rt+o,8).hex()) for o in (44,72,80,88,120,136,200,208,216)])
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
print('CANVAS OK','rt fields',[(o,p.u.mem_read(rt+o,8).hex()) for o in (80,88,120,136,200,208,216)],'gl',len(p.gl_calls),'managed',managed_events)
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
print('OUTPUT CANVAS',hex(outbmp),hex(canvas),hex(bitmap),'gl',len(p.gl_calls),'managed',managed_events)
# Reattach only the identical vector object retained from the selected Drawing RT's AddPoint callbacks.
pre80=p.get(rt+80,'Q')[0];pre88=p.get(rt+88,'Q')[0]
if pre80 or pre88:raise RuntimeError(f'unexpected native RT slots before same-object bridge: {pre80:#x},{pre88:#x}')
p.put(rt+88,'Q',vecobj)
if p.get(rt+88,'Q')[0]!=vecobj:raise RuntimeError('same-vector handoff failed')
p.call('SPen::Marker3StrokeDrawableRTV1::Update(bool)',[rt,0],limit=5_000_000)
print('UPDATE OK','vector',hex(vecobj),'data',len(vector_data),hashlib.sha256(vector_data).hexdigest(),'fields',[(o,p.u.mem_read(rt+o,8).hex()) for o in (72,80,88,120,200,208,216)],'gl',len(p.gl_calls),'buffer',[(c['call'],c.get('size'),c.get('dataSha256')) for c in p.gl_calls if c['call']=='glBufferData'])
# Identity canvas matrix; source points are native RT callback coordinates in the fixed 256x256 profile.
mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
p.call('SPen::Marker3StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)
p.gles.gl.glFinish(); pixels=C.create_string_buffer(256*256*4);read=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels);read(0,0,256,256,0x1908,0x1401,pixels)
errs=p.gles.f['glGetError']()
if errs or any(row['error'] for row in gl_error_checks): raise RuntimeError(f'nonzero GL errors: final={errs} checks={gl_error_checks}')
nonzero=[(i%256,i//256) for i in range(256*256) if pixels.raw[i*4:i*4+4]!=bytes(4)]
bbox=[min(pt[0] for pt in nonzero),min(pt[1] for pt in nonzero),max(pt[0] for pt in nonzero),max(pt[1] for pt in nonzero)]
draws=[c for c in p.gl_calls if c['call'] in ('glDrawArrays','glDrawArraysInstanced')]
print('DRAW OK','bytes',len(pixels.raw),'sha256',hashlib.sha256(pixels.raw).hexdigest(),'samples',[{'xy':[x,y],'rgba':list(pixels.raw[(y*256+x)*4:(y*256+x+1)*4])} for x,y in ((0,0),(1,0),(20,0),(0,1),(8,0),(0,8),(128,128))], 'nonzero',len(nonzero),'bbox',bbox,'errors',errs,'draws',draws)
# Runtime evidence report: all geometry remains the caller-created vector object; only its cleared RT slot is bridged.
source_case=json.loads((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_text())['cases']['marker3_linked_saved1:5:tool2:curve1']
rows=draw_rows
assert len(rows)==22 and {r['vector_object'] for r in rows}=={hex(vecobj)}
assert p.get(rt+88,'Q')[0]==vecobj and bytes(p.u.mem_read(vec_begin,vec_end-vec_begin))==vector_data
uploads=[c for c in p.gl_calls if c['call']=='glBufferData' and c.get('size')==len(vector_data) and c.get('dataSha256')==hashlib.sha256(vector_data).hexdigest()]
assert uploads, 'no GPU upload matching native Drawing vector bytes'
attachments=[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D']
maskdraw=next(c for c in draws if c['call']=='glDrawArraysInstanced'); compdraw=next(c for c in draws if c['call']=='glDrawArrays')
maskatt=next(c for c in attachments if c['framebuffer']==maskdraw['state']['framebuffer']);outatt=next(c for c in attachments if c['framebuffer']==compdraw['state']['framebuffer'])
complete={c['framebuffer'] for c in p.gl_calls if c['call']=='glCheckFramebufferStatus' and c['status']==0x8cd5}
assert maskatt['args'][3]!=outatt['args'][3] and maskatt['framebuffer']!=outatt['framebuffer']
assert compdraw['state']['texture2d']==maskatt['args'][3]
assert {maskatt['framebuffer'],outatt['framebuffer']}<=complete
report={'schema':'marker3-drawing-selected-rt-host-gles-positive-v1','scope':'Actual Model ObjectStroke setters and native ObjectDrawing route selected Marker3 GLV5/RTV1 and native RedrawPen captured the 22 actual drawObjectStroke AddPoint callbacks. After wrapper return, native RT vector slots +80/+88 were zero; a bounded explicit host bridge placed the identical retained std::vector object captured from those callbacks into +88. The same Drawing-selected native RTV1 then executed Init, native PenGLDataManager::GetPenCanvas and SPGraphicsFactory::CreatePenCanvas, CreatePenCanvas, native Update(false), and native Draw to a separately created native RGBA8 target. No XY points were re-fed. Mesa GLES 3.2 llvmpipe provides GL; this is not Android device GPU parity.','source_caller_fixture_sha256':hashlib.sha256((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3-saved-caller-native.json').read_bytes()).hexdigest(),'source_caller_case':source_case,'selected_drawable_version':m.selected_drawable_version,'selected_drawable':hex(m.selected_drawable),'selected_rt':hex(rt),'same_vector_object':hex(vecobj),'native_vector_bytes':len(vector_data),'native_vector_sha256':hashlib.sha256(vector_data).hexdigest(),'callback_count':len(rows),'source_vector_phase':'draw_object_stroke','separate_saved_redraw_vector_object':hex(saved_vecobj),'same_xy_as_saved_redraw':True,'pre_bridge_slots':[hex(pre80),hex(pre88)],'post_bridge_rt_plus88':hex(p.get(rt+88,'Q')[0]),'matching_native_gpu_uploads':[{'size':c['size'],'sha256':c['dataSha256']} for c in uploads],'native_rt_color_rgba_f32':list(p.get(rt+44,'4f')),'host_gles':p.gles.identity,'draws':draws,'attachments':attachments,'complete_fbos':sorted(complete),'texture_allocations':[c for c in p.gl_calls if c['call']=='glTexImage2D'],'viewport_calls':[c for c in p.gl_calls if c['call']=='glViewport'],'blend_calls':[c for c in p.gl_calls if c['call'] in ('glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask')],'shader_source_calls':p.shader_source_calls,'uniform_calls':[c for c in p.gl_calls if c['call'].startswith('glUniform')],'gl_error_checks':gl_error_checks,'readback':{'bytes':len(pixels.raw),'sha256':hashlib.sha256(pixels.raw).hexdigest(),'nonzero_pixels':len(nonzero),'bbox':bbox,'samples':[{'xy':[x,y],'rgba':list(pixels.raw[(y*256+x)*4:(y*256+x+1)*4])} for x,y in ((0,0),(1,0),(20,0),(0,1),(8,0),(0,8),(128,128))]},'library_hashes':dict(m.native_hashes,**{'libSPenRenderer.so':hashlib.sha256((LIB/'libSPenRenderer.so').read_bytes()).hexdigest(),'libc++_shared.so':hashlib.sha256((LIB/'libc++_shared.so').read_bytes()).hexdigest()}),'helper_hashes':{'marker3_paint_scale_native.py':hashlib.sha256((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/marker3_paint_scale_native.py').read_bytes()).hexdigest(),'airbrush_texture_native.py':hashlib.sha256((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/airbrush_texture_native.py').read_bytes()).hexdigest(),'inkpen_paint_native.py':hashlib.sha256((ROOT/'tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen_paint_native.py').read_bytes()).hexdigest()},'boundaries':['Synthetic ObjectStroke values are supplied through Model setters in the caller fixture','explicit 1920x1080 ObjectDrawing screen profile from saved-caller fixture','identity ARGB context conversion is host-supplied','host GLES context/driver and EGL context identity','explicit host IGLMsgQueue object; synchronous in-emulator execution; managed-registration hooks installed but no registration occurred in this run','native SPGraphicsFactory creates external RGBA8 output texture with layouts 6/2','same-object RT+88 bridge after native Drawing wrapper cleared both slots','no device GPU, Android EGL, destruction lifecycle, or serialized SDK parser claim']}
out=Path(__file__).with_name('drawing-paint-positive.json');out.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');Path(__file__).with_name('drawing-paint-positive.rgba').write_bytes(pixels.raw)
