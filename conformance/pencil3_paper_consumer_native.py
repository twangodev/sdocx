"""Bounded saved Drawing caller with one synthetic depth-map ISPBitmap.

This reuses the pinned Pencil3 native caller harness without modifying it. The
bitmap object is an explicit host adapter; Drawing, DrawingUtil, Pencil3 V3 and
RTV1 setters execute as native AArch64 code in Unicorn.
"""
from pathlib import Path
import contextlib, hashlib, io, json, os, struct, sys

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0, str(ROOT / "conformance"))
sys.path.insert(0, str(ROOT / "scratch/apk-analysis-runtime/python"))

source_path = ROOT / "conformance/pencil3_saved_caller_native.py"
source = source_path.read_text()
SOURCE_PINS={
 "pencil3_saved_caller_native.py":"129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e",
 "oblique_canvas_native.py":"1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
 "calligraphy_draw_native.py":"e98afde895bc83d3a4667578383b43e3f648a337b463a95c6a0451e17ff4a788",
 "fountain_native.py":"40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
 "elf_plt.py":"816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
 "pencil_native.py":"b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c",
 "calligraphy_saved_native.py":"663f15668825ce6ed219830f3e671c408b85ad103477b444201f68f0b12c6517",
}
for name,digest in SOURCE_PINS.items():
 path=ROOT/'conformance'/name
 if hashlib.sha256(path.read_bytes()).hexdigest()!=digest: raise RuntimeError(f'pinned source mismatch before patch: {name}')
source_loader = "oc.native.NativeFountain.__init__(self,oc.LIBRARY_DIR,oc.OBJDUMP,('Drawing','Graphics','Pencil3'))"
if source.count(source_loader) != 1:
    raise RuntimeError("Pencil3 loader anchor changed")
source = source.replace(source_loader, source_loader.replace("'Pencil3'))", "'Pencil3','Renderer'))"))
# Renderer is appended at the non-overlapping base 0x6000000. The caller
# harness separately maps Composer; move Composer to 0x9000000 below.
source = source.replace("'Composer':0x6000000", "'Composer':0x9000000")
import oblique_canvas_native as _oc
import calligraphy_draw_native as _cd
_oc.native.HASHES['Renderer'] = 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
_oc.native.HASHES['Graphics'] = 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621'
DSO_PINS={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Pencil3':'41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
LIBDIR=ROOT/'scratch/apk-analysis-native/arm64-v8a'
for name,digest in DSO_PINS.items():
 path=LIBDIR/('libSPen'+name+'.so' if name not in ('libc++_shared.so',) else name)
 if hashlib.sha256(path.read_bytes()).hexdigest()!=digest: raise RuntimeError(f'pinned native library mismatch before execution: {name}')
def _load_composer_at_nine(self):
 import hashlib, re, struct, subprocess
 path=_oc.LIBRARY_DIR/'libSPenComposer.so'; blob=path.read_bytes(); base=0x9000000
 if hashlib.sha256(blob).hexdigest()!=_oc.COMPOSER_SHA256: raise RuntimeError('unverified Composer DSO')
 off=struct.unpack_from('<Q',blob,32)[0]; ent,num=struct.unpack_from('<HH',blob,54); extent=0
 for i in range(num):
  typ,flags,fo,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,off+i*ent)
  if typ==1: extent=max(extent,va+ms)
 self.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(num):
  typ,flags,fo,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,off+i*ent)
  if typ==1 and ms:
   self.u.mem_write(base+va,blob[fo:fo+fs])
   if ms>fs:self.u.mem_write(base+va+fs,bytes(ms-fs))
 env=dict(os.environ); runtime=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'
 if runtime.is_dir():env['LD_LIBRARY_PATH']=str(runtime.resolve())
 for line in subprocess.check_output(['nm','-D','-C',path],text=True,env=env).splitlines():
  z=line.strip().split(' ',2)
  if len(z)==3 and z[1] in ('T','W'):self.syms[z[2]]=base+int(z[0],16)
 dis=subprocess.check_output([str(_oc.OBJDUMP),'-d','-C','-j','.plt',path],text=True,env=env)
 for a,n in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):self.plt[base+int(a,16)]=n
_oc.CanvasProbe.load_composer = _load_composer_at_nine
needle = "phase='draw_object_stroke';"
inject = r'''
# Explicit synthetic ISPBitmap adapter: GetBitmapReadable returns a host
# readable-bitmap adapter, Ref is a no-op, and dimension getters return 320x180.
paper_bitmap=p.alloc(64); paper_vtable=p.alloc(192); paper_texture=p.alloc(320)
p.put(paper_bitmap,'Q',paper_vtable)
p.put(paper_texture,'Q',pencil_texture_vtable)
p.put(paper_texture+40,'II',0x0de1,1); p.put(paper_texture+64,'II',320,180); p.put(paper_texture+76,'II',320,180)
def literal_return(address, value):
 p.u.mem_write(address, struct.pack('<IIQ', 0x58000040, 0xd65f03c0, value))
literal_return(0x200f2100,paper_texture)
literal_return(0x200f2110,0)
literal_return(0x200f2120,320)
literal_return(0x200f2130,180)
p.put(paper_vtable+144,'Q',0x200f2100) # GetBitmapReadable
p.put(paper_vtable+128,'Q',0x200f2110) # Ref
p.put(paper_vtable+72,'Q',0x200f2120)  # GetWidth
p.put(paper_vtable+80,'Q',0x200f2130)  # GetHeight
p.call('SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)',[p.drawing,paper_bitmap])
p.call('SPen::ObjectDrawing::SetBackground(SPen::ISPBitmap*)',[p.drawing,native_pencil_bitmap])
background_bitmap=p.alloc(64); background_vtable=p.alloc(192); background_texture=p.alloc(32)
p.put(background_bitmap,'Q',background_vtable)
literal_return(0x200f2140,background_texture)
literal_return(0x200f2150,0)
literal_return(0x200f2160,640)
literal_return(0x200f2170,360)
p.put(background_vtable+144,'Q',0x200f2140)
p.put(background_vtable+128,'Q',0x200f2150)
p.put(background_vtable+72,'Q',0x200f2160)
p.put(background_vtable+80,'Q',0x200f2170)
p.call('SPen::ObjectDrawing::SetBackground(SPen::ISPBitmap*)',[p.drawing,background_bitmap])
paper_depthmap_injected={'drawing_depthmap_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+48,8))[0]),'depthmap_bitmap':hex(paper_bitmap),'depthmap_texture':hex(paper_texture),'depthmap_dimensions':[320,180],'drawing_background_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+40,8))[0]),'background_bitmap':hex(background_bitmap),'background_texture':hex(background_texture),'background_dimensions':[640,360]}
from unicorn import UC_HOOK_MEM_WRITE
penreturn_symbols={n:a for n,a in p.syms.items() if 'SPen::PenReturnCallback::' in n and '@plt' not in n}
penreturn_ctor_pcs={a for n,a in penreturn_symbols.items() if 'PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)' in n}
penreturn_dtor_pcs={a for n,a in penreturn_symbols.items() if '~PenReturnCallback()' in n and '@plt' not in n}
penreturn_callback_objects=[]
penreturn_callback_events=[]
def trace_penreturn_callbacks(_uc,address,_size,_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj.get('return_pc')==address:
   obj['active']=False
   penreturn_callback_events.append({'event':'destructor_return','phase':phase,'pc':hex(address),'this':hex(obj['this'])})
 if address in penreturn_ctor_pcs:
  this=p.x(0); penreturn_callback_objects.append({'this':this,'phase':phase,'active':True,'return_pc':None}); penreturn_callback_events.append({'event':'constructor','phase':phase,'pc':hex(address),'this':hex(this)})
 elif address in penreturn_dtor_pcs:
  this=p.x(0)
  matches=[row for row in penreturn_callback_objects if row['this']==this and row.get('active')]
  if matches:
   retpc=p.u.reg_read(R.UC_ARM64_REG_LR)
   matches[-1]['return_pc']=retpc
   penreturn_callback_events.append({'event':'destructor_entry','phase':phase,'pc':hex(address),'return_pc':hex(retpc),'this':hex(this),'bytes_0_80':p.u.mem_read(this,80).hex()})
def trace_callback_writes(_uc,_access,address,size,value,_user_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj['this'] <= address < obj['this']+80:
   penreturn_callback_events.append({'event':'field_write','phase':phase,'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'this':hex(obj['this']),'offset':address-obj['this'],'size':size,'value_hex':(int(value) & ((1 << (8*size))-1)).to_bytes(size,'little').hex()})
p.u.hook_add(UC_HOOK_CODE,trace_penreturn_callbacks)
p.u.hook_add(UC_HOOK_MEM_WRITE,trace_callback_writes)
paper_setter_trace=[]
paper_setter_pcs={p.syms['SPen::Pencil3StrokeDrawableGLV3::SetDepthMapBitmap(SPen::ISPBitmap*)'],p.syms['SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)'],0x510d48c}
def trace_paper_setters(_uc,address,_size,_data):
 if address in paper_setter_pcs:
  row={'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]}; paper_setter_trace.append(row)
p.u.hook_add(UC_HOOK_CODE,trace_paper_setters)
scale_after=[]
def trace_scale_return(_uc,address,_size,_data):
 if address==0x510d490: scale_after.append({'phase':phase,'x0':hex(p.x(0)),'scale_bits':p.u.mem_read(p.x(0)+264,4).hex()})
p.u.hook_add(UC_HOOK_CODE,trace_scale_return)
paint_entries=[]
queued_tasks=[]
executed_task_runs=[]
task_run_seen=[]
def trace_queue_enqueue(_uc,address,_size,_data):
 if address in (0x510a694,0x510cce0): paint_entries.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2))})
 if address in (0x51089d4,0x5108a00,0x5108a28,0x5108a54,0x510d48c,0x510c7e4): task_run_seen.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]})
 elif address==0x20001000:
  task=p.x(1); vt=struct.unpack('<Q',p.u.mem_read(task,8))[0]
  slots=struct.unpack('<6Q',p.u.mem_read(vt,48))
  queued_tasks.append({'phase':phase,'task':hex(task),'vtable':hex(vt),'vtable_slots':[hex(v) for v in slots],'raw_qwords_0_64':[hex(v) for v in struct.unpack('<8Q',p.u.mem_read(task,64))]})

p.u.hook_add(UC_HOOK_CODE,trace_queue_enqueue)
# Intercept only the PenReturnCallback's native queue-enqueue call on the
# existing IContext vslot. Non-message profile queries execute the original
# host getter bytes. For the native Member2 task, synchronously call its native
# vtable+16 Run method and leave the RT target/args untouched.
MESSAGE_CONTEXT_CALL=0x200f2400; MESSAGE_RETURN=0x200f2220; RT_INIT_RETURN=0x200f2240
p.u.mem_write(MESSAGE_CONTEXT_CALL,struct.pack('<II',0xd503201f,0xd65f03c0))
p.put(qvt+16,'Q',MESSAGE_CONTEXT_CALL)
p.u.mem_write(MESSAGE_RETURN,struct.pack('<II',0xd503201f,0xd65f03c0))
native_message_dispatch=[]; message_return_stack=[]; context_enqueue_calls=[]
EXPECTED_MESSAGE_VPTRS={0x10774c8,0x10774f8,0x1077528}

def trace_native_message_dispatch(_uc,address,_size,_data):
 if address==MESSAGE_CONTEXT_CALL:
  message=p.x(1)
  if not message: return
  vptr=struct.unpack('<Q',p.u.mem_read(message,8))[0]
  context_enqueue_calls.append({'phase':phase,'context':hex(p.x(0)),'x1':hex(message),'candidate_vptr':hex(vptr),'bytes_0_144':bytes(p.u.mem_read(message,144)).hex() if vptr in {0x10774f8,0x10774c8,0x1077528} else None})
  if vptr not in EXPECTED_MESSAGE_VPTRS: return
  slots=struct.unpack('<6Q',p.u.mem_read(vptr,48)); method=slots[2]
  if not method: raise RuntimeError(f'native queued message has null vslot+16: {message:#x} {slots!r}')
  caller=p.u.reg_read(R.UC_ARM64_REG_LR)
  record={'phase':phase,'context':hex(p.x(0)),'message':hex(message),'vtable':hex(vptr),'vtable_slots':[hex(v) for v in slots],'native_execute_slot_plus16':hex(method),'message_bytes_0_144':bytes(p.u.mem_read(message,144)).hex(),'caller':hex(caller)}
  native_message_dispatch.append(record); message_return_stack.append({'caller':caller,'record':record})
  if vptr==0x10774c8:
   rtptr=struct.unpack('<Q',p.u.mem_read(message+32,8))[0]
   init=p.syms['SPen::Pencil3StrokeDrawableRTV1::Init()']
   frame=message_return_stack[-1]; frame.update({'message':message,'method':method,'rt':rtptr,'init':init})
   record['selected_rt']=hex(rtptr); record['host_lifecycle_intervention']='call actual RTV1::Init on this native Drawing-selected receiver before native queued Run; no RT fields patched'
   p.x(0,rtptr); p.u.reg_write(R.UC_ARM64_REG_LR,RT_INIT_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,init)
   return
  p.x(0,message); p.u.reg_write(R.UC_ARM64_REG_LR,MESSAGE_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,method)
 elif address==RT_INIT_RETURN:
  frame=message_return_stack[-1]; rtptr=frame['rt']; frame['record']['native_init_return']={'x0':hex(p.x(0)),'field208':bytes(p.u.mem_read(rtptr+208,8)).hex(),'field216':bytes(p.u.mem_read(rtptr+216,8)).hex()}
  p.x(0,frame['message']); p.u.reg_write(R.UC_ARM64_REG_LR,MESSAGE_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,frame['method'])
 elif address==MESSAGE_RETURN:
  if not message_return_stack: raise RuntimeError('native message return without enqueue frame')
  frame=message_return_stack.pop(); frame['record']['native_execute_return_x0']=hex(p.x(0))
  p.x(0,1); p.u.reg_write(R.UC_ARM64_REG_PC,frame['caller'])
p.u.hook_add(UC_HOOK_CODE,trace_native_message_dispatch)

'''
inject = "\n".join(" " + line if line else line for line in inject.splitlines())
if source.count(needle) != 1:
    raise RuntimeError("native caller injection point changed")
source = source.replace(needle, needle + inject)
native_setup = r'''p.u.mem_write(0x200ef000,struct.pack('<II',6,2))
 # Renderer-aware, source-pinned ShaderManager setup adapted from the existing
 # Calligraphy native Draw probe. EGL/GLES remain host resource boundaries.
 cxa_guard_seen=set()
 def _guard_acquire():
  guard=p.x(0); p.x(0,0 if guard in cxa_guard_seen else 1); cxa_guard_seen.add(guard)
 for _name in ('pthread_mutex_init','pthread_mutex_lock','pthread_mutex_unlock','pthread_mutex_destroy','__cxa_guard_release','__cxa_guard_abort'):
  p.callbacks[_name]=lambda:p.x(0,0)
 p.callbacks['__cxa_guard_acquire']=_guard_acquire
 p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xE611)
 p.callbacks['crc32']=lambda:p.x(0,__import__('zlib').crc32(bytes(p.u.mem_read(p.x(1),p.x(2))) if p.x(1) and p.x(2) else b'') & 0xffffffff)
 p.callbacks['__android_log_print']=lambda:p.x(0,0)
 def _bytes_until_nul(pointer,limit=4096):
  out=bytearray()
  for i in range(limit):
   b=p.u.mem_read(pointer+i,1)[0]
   if not b:return bytes(out)
   out.append(b)
  raise RuntimeError('bounded native C string exceeded')
 p.callbacks['strlen']=lambda:p.x(0,len(_bytes_until_nul(p.x(0))))
 p.callbacks['__strlen_chk']=p.callbacks['strlen']
 p.callbacks['strcmp']=lambda:p.x(0,( _bytes_until_nul(p.x(0)) > _bytes_until_nul(p.x(1))) - (_bytes_until_nul(p.x(0)) < _bytes_until_nul(p.x(1))))
 def _strstr():
  hay,needle=p.x(0),p.x(1); h=_bytes_until_nul(hay); n=_bytes_until_nul(needle); i=h.find(n); p.x(0,0 if i<0 else hay+i)
 p.callbacks['strstr']=_strstr
 def _strncpy_chk():
  dst,src,count,capacity=(p.x(i) for i in range(4))
  if count>capacity or count>(1<<20):raise RuntimeError(f'bounded strncpy overflow {count=} {capacity=}')
  data=bytes(p.u.mem_read(src,count)); end=data.find(b'\0'); data=data[:end]+bytes(count-end) if end>=0 else data; p.u.mem_write(dst,data); p.x(0,dst)
 p.callbacks['__strncpy_chk']=_strncpy_chk; p.callbacks['__strncpy_chk2']=_strncpy_chk
 # Reuse the existing bounded shader-source/reflection GLES adapter, with its
 # state hosted over this probe's actual Unicorn memory/register interface.
 glhost=object.__new__(_cd.NativeDrawCalligraphy)
 for _name in ('x','f','get','put','alloc'):
  setattr(glhost,_name,getattr(p,_name))
 def _read_gl_string(pointer):
  data=bytearray()
  for offset in range(1<<16):
   byte=p.u.mem_read(pointer+offset,1)[0]
   if not byte: return data.decode('utf-8','replace')
   data.append(byte)
  raise RuntimeError('GL string exceeded bound')
 def _read_gl_bytes(pointer): return bytes(p.u.mem_read(pointer,1<<16)).split(b'\0',1)[0]
 glhost.read_c_string=_read_gl_string; glhost.read_c_bytes=_read_gl_bytes
 glhost.u=p.u; glhost.gl_calls=[]; glhost.gl_next_id=100; glhost.gl_strings={}; glhost.gl_locations={}; glhost.gl_shader_sources={}; glhost.gl_program_shaders={}
 glhost.program_active_uniforms=_cd.NativeDrawCalligraphy.program_active_uniforms.__get__(glhost)
 for _name in set(p.plt.values()):
  if _name.startswith('gl'):
   def _gl_callback(n=_name):
    if n=='glVertexAttribDivisor':
     glhost.gl_calls.append({'name':n,'args':[p.x(i) for i in range(4)],'host_boundary':'synthetic GLES state adapter'}); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
    else: glhost.gl_host_import(n)
   p.callbacks[_name]=_gl_callback
 renderer_base=0x6000000
 # Resolve the real ContextHolder and GLES implementation vtables required
 # by ShaderManager::Key and native Pencil3 RT initialization.
 for off,val in ((0x6d630,0x6a0a0),(0x6a0c0,0x5157c),(0x6d4a8,0x73288),(0x6d4b0,0x73290),(0x6d4a0,0x302c8),(0x6d620,0x6a068),(0x6d650,0x6a210),(0x6d428,0x69ca0),(0x6d6d0,0x6a608),(0x6a618,0x55954),(0x6a620,0x55ca0),(0x6d890,0x6ae90)):
  p.put(renderer_base+off,'Q',renderer_base+val)
 for slot,method in enumerate((0x530a0,0x530e8,0x52218,0x52380,0x52a10,0x52824,0x52464,0x5264c,0x53134,0x53140,0x53224)):
  p.put(renderer_base+0x6a220+slot*8,'Q',renderer_base+method)
 for slot,method in enumerate((0x51490,0x51494,0x514b8,0x514cc,0x51570)):
  p.put(renderer_base+0x6a078+slot*8,'Q',renderer_base+method)
 for table,method in ((0x6aea0,0x5b9c8),(0x6aea8,0x5ba20),(0x6aeb0,0x5b824),(0x6aeb8,0x5b9bc),(0x6aec0,0x5b9c4),(0x6aec8,0x5ba6c),(0x6aed0,0x5b2c4),(0x6aed8,0x5c5a8),(0x6aee0,0x5c600),(0x6af08,0x45600),(0x6af10,0x4563c),(0x6af18,0x5c5d4),(0x6af20,0x5c604)):
  p.put(renderer_base+table,'Q',renderer_base+method)
 shader_manager=p.alloc(512); p.put(renderer_base+0x6d480,'Q',shader_manager)
 p.call('SPen::ShaderManager::ShaderManager()',[shader_manager],limit=2000000)
 p.call('SPen::ShaderManager::GetInstance()',[])
 native_resource_calls_shader={'manager':hex(shader_manager),'singleton':hex(p.x(0)),'host_egl_context':0xE611}
 native_resource_calls=[]
 native_offscreen_factory_calls=[]
 _offscreen_entry=p.syms.get('SPen::GLES::RendererFactoryImpl::CreateOffscreenRenderTarget(SPen::ITextureObject*)')
 def _trace_native_offscreen_factory(_uc,address,_size,_data):
  if address==_offscreen_entry: native_offscreen_factory_calls.append({'entry':hex(address),'input_texture_object':hex(p.x(0)),'route':'native RendererFactory/RendererFactoryImpl/OffscreenRenderTarget; explicit synthetic ITextureObject and host EGL/GLES'})
 p.u.hook_add(UC_HOOK_CODE,_trace_native_offscreen_factory)
 native_graphics_factory_calls=[]; native_graphics_factory_returns=[]; graphics_init_entries=[]; pending_graphics_init=[]
 _gf_names=('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::GLES::RendererFactoryImpl::GreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::GLES::GraphicsObject::GraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)')
 _gf_entries={p.syms[n]:n for n in _gf_names if n in p.syms}
 _rtinit=p.syms.get('SPen::Pencil3StrokeDrawableRTV1::Init()')
 native_rt_calls=[]
 _rt_points={p.syms[n]:n for n in ('SPen::Pencil3StrokeDrawableRTV1::Update(bool)','SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)','non-virtual thunk to SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)') if n in p.syms}
 def _trace_graphics_init(_uc,address,_size,_data):
  if address==0x510c3b0: native_graphics_factory_returns.append({'return_pc':hex(address),'returned_graphics_object':hex(p.x(0)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address in _gf_entries: native_graphics_factory_calls.append({'entry':hex(address),'symbol':_gf_entries[address],'x0_x4':[hex(p.x(i)) for i in range(5)],'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address==_rtinit:
   obj=p.x(0); lr=p.u.reg_read(R.UC_ARM64_REG_LR); graphics_init_entries.append({'event':'init-entry','object':hex(obj),'lr':hex(lr),'field208_before':bytes(p.u.mem_read(obj+208,8)).hex()}); pending_graphics_init.append((lr,obj))
  if address in _rt_points:
   obj=p.x(0); record={'entry':hex(address),'symbol':_rt_points[address],'x0':hex(obj),'x1':hex(p.x(1)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))}
   for off in (0,8,16,24,32,40,48,56,64,72,80,88,96,104,112,120,128,136,144,152,160,168,176,184,192,200,208,216,224,232,240,248,256):
    try: record[f'plus_{off}']=bytes(p.u.mem_read(obj+off,8)).hex()
    except Exception: pass
   native_rt_calls.append(record)
  for lr,obj in list(pending_graphics_init):
   if address==lr:
    graphics_init_entries.append({'event':'init-return','object':hex(obj),'field208_after':bytes(p.u.mem_read(obj+208,8)).hex(),'field216_after':bytes(p.u.mem_read(obj+216,8)).hex()}); pending_graphics_init.remove((lr,obj))
 p.u.hook_add(UC_HOOK_CODE,_trace_graphics_init)
 native_caps=p.alloc(16); native_caps_vtable=p.alloc(32); p.put(native_caps,'Q',native_caps_vtable)
 p.u.mem_write(0x200f2300,struct.pack('<II',0x52820000,0xd65f03c0)); p.put(native_caps_vtable+16,'Q',0x200f2300)
 def _pencil_caps():
  native_resource_calls.append({'call':'RendererFactory::GetRendererDeviceCaps','returned':hex(native_caps),'max_texture_size':4096,'host_adapter':'4096 IRendererDeviceCaps slot+16 contract reused from BrushPen TextureProbe'})
  p.x(0,native_caps)
 pencil_texture_vtable=p.alloc(128)
 for _offset in range(0,128,8): p.put(pencil_texture_vtable+_offset,'Q',0x200f2310)
 p.u.mem_write(0x200f2310,bytes.fromhex('e00300aac0035fd6'))
 p.u.mem_write(0x200f2320,struct.pack('<II',0x52802800,0xd65f03c0)) # GetWidth -> 320 synthetic source texture
 p.u.mem_write(0x200f2330,struct.pack('<II',0x52801680,0xd65f03c0)) # GetHeight -> 180 synthetic source texture
 p.put(pencil_texture_vtable+32,'Q',0x200f2320)
 p.put(pencil_texture_vtable+40,'Q',0x200f2330)
 p.put(pencil_texture_vtable+64,'Q',0x200f2310)
 def _pencil_texture_object():
  descriptor=p.x(0); label=p.x(1); initial=p.x(2); pairs=p.x(3); pair_count=p.x(4); info=p.x(5)
  words=struct.unpack('<8I',p.u.mem_read(descriptor,32)); info_words=struct.unpack('<4I',p.u.mem_read(info,16)); pair_words=struct.unpack('<'+'I'*(pair_count*2),p.u.mem_read(pairs,pair_count*8)) if pair_count else ()
  label_bytes=bytes(p.u.mem_read(label,128)).split(b'\\0',1)[0].decode('ascii','replace')
  tex=p.alloc(320); p.put(tex,'Q',pencil_texture_vtable); p.put(tex+40,'II',0x0de1,1); p.put(tex+64,'II',words[0],words[1]); p.put(tex+76,'II',words[3],words[4]); p.put(tex+268,'I',0)
  native_resource_calls.append({'call':'RendererFactory::CreateTextureObject','descriptor_u32':list(words),'buffer_info_u32':list(info_words),'u32_pairs':list(pair_words),'label':label_bytes,'initial_source_is_null':initial==0,'returned_native_interface_adapter':hex(tex),'adapter_fields':{'gl_target':0x0de1,'dimensions':[words[0],words[1]]}})
  p.x(0,tex)
 # Leave CreateOffscreenRenderTarget unresolved to host callbacks so the
 # pinned RendererFactory wrapper/OffscreenRenderTarget constructor execute.
 # Its ITextureObject and EGL/GLES dependencies remain explicit host adapters.
 p.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=_pencil_caps
 p.callbacks['SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']=_pencil_texture_object
 p.callbacks['strncpy']=lambda:(p.x(0,p.x(0)))
 # Offscreen target creation and RTV1 Init run natively. No framebuffer target
 # fields are patched; the later Draw fault records the remaining resource gap.
 clip_boundary_calls=[]; clip_entry=p.syms.get('SPen::SPCanvasClipping::EnableClipRT(int)'); disable_entry=p.syms.get('SPen::PenDrawableRT::DisableDepthStencil()'); activate_entry=p.syms.get('SPen::SPSubBitmapRT::ActivateFrameBufferRT(bool)')
 def _clip_resource_boundary(_uc,address,_size,_data):
  if address==clip_entry:
   clip_boundary_calls.append({'entry':hex(address),'this':hex(p.x(0)),'enabled':p.x(1),'result':0,'boundary':'host clip-target operation adapter; no drawing state asserted'})
   p.x(0,0); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
  elif address==disable_entry:
   clip_boundary_calls.append({'entry':hex(address),'this':hex(p.x(0)),'boundary':'host depth/stencil operation adapter'})
   p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
  elif address==activate_entry:
   clip_boundary_calls.append({'entry':hex(address),'this':hex(p.x(0)),'enable':p.x(1),'boundary':'host framebuffer activation adapter'})
   p.x(0,0); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
 p.u.hook_add(UC_HOOK_CODE,_clip_resource_boundary)
 native_surface=bytes(320*180*4); native_surface_ptr=p.alloc(len(native_surface)); p.u.mem_write(native_surface_ptr,native_surface)
 native_surface_tag=p.alloc(32); p.u.mem_write(native_surface_tag,b'pencil-consumer-synthetic\\0')
 native_bitmap_factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
 native_bitmap_factory_calls=[]
 _bitmap_factory_entry=p.syms[native_bitmap_factory]
 def _trace_bitmap_factory(_uc,address,_size,_data):
  if address==_bitmap_factory_entry:
   sp=p.u.reg_read(R.UC_ARM64_REG_SP)
   native_bitmap_factory_calls.append({'entry':hex(address),'x0_x7':[hex(p.x(i)) for i in range(8)],'stack_args_8_9_u32':list(struct.unpack('<II',p.u.mem_read(sp,8))),'stack_32_bytes':bytes(p.u.mem_read(sp,32)).hex()})
 p.u.hook_add(UC_HOOK_CODE,_trace_bitmap_factory)
 # CreateBitmap has ten AAPCS64 arguments; enum arguments 8 and 9 are at SP
 # and SP+8. CanvasProbe.call sets SP to 0x200ef000.
 p.u.mem_write(0x200ef000,struct.pack('<II',0,0)+bytes(24))
 p.call(native_bitmap_factory,[queue,320,180,native_surface_tag,native_surface_ptr,0,0,0],limit=5000000)
 native_pencil_bitmap=p.x(0)
 native_pencil_canvas=p.alloc(256)
 p.call('SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)',[native_pencil_canvas,queue,native_pencil_bitmap],limit=5000000)
 _bitmap_vptr=struct.unpack('<Q',p.u.mem_read(native_pencil_bitmap,8))[0]
 native_pencil_canvas_info={'bitmap':hex(native_pencil_bitmap),'bitmap_vptr':hex(_bitmap_vptr),'canvas':hex(native_pencil_canvas),'canvas_vptr':hex(struct.unpack('<Q',p.u.mem_read(native_pencil_canvas,8))[0]),'canvas_bitmap_getter':hex(struct.unpack('<Q',p.u.mem_read(struct.unpack('<Q',p.u.mem_read(native_pencil_canvas,8))[0]+192,8))[0]),'expected_getbitmap':hex(p.syms['SPen::SPPenCanvas::GetBitmap()']),'bitmap_get_writable':hex(struct.unpack('<Q',p.u.mem_read(_bitmap_vptr+152,8))[0]),'expected_writable':hex(p.syms['SPen::SPTextureBitmap::GetBitmapWritable()']),'bitmap_get_texture':hex(struct.unpack('<Q',p.u.mem_read(_bitmap_vptr+168,8))[0]),'expected_get_texture':hex(p.syms['SPen::SPTextureBitmap::GetTextureBitmap()'])}
 print('NATIVE_CANVAS_CHECK',native_pencil_canvas_info)
 if native_pencil_canvas_info['canvas_bitmap_getter'] != native_pencil_canvas_info['expected_getbitmap'] or native_pencil_canvas_info['bitmap_get_writable'] != native_pencil_canvas_info['expected_writable'] or native_pencil_canvas_info['bitmap_get_texture'] != native_pencil_canvas_info['expected_get_texture']:
  raise RuntimeError('native Pencil canvas/bitmap vtable contract mismatch: '+repr(native_pencil_canvas_info))
 p.pen_canvas=native_pencil_canvas
'''
setup_anchor = "p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]);"
if source.count(setup_anchor) != 1:
    raise RuntimeError("native Drawing setup anchor changed")
source = source.replace(setup_anchor, native_setup + "\n " + setup_anchor)
old = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.'}"
new = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.','paper_depthmap_injected':paper_depthmap_injected,'paper_setter_trace':paper_setter_trace,'penreturn_callback_events':penreturn_callback_events,'penreturn_symbols':{n:hex(a) for n,a in penreturn_symbols.items()},'queued_tasks':queued_tasks,'executed_task_runs':executed_task_runs,'task_run_seen':task_run_seen,'scale_after':scale_after,'screen_resolution_calls':screen_resolution_calls,'paint_entries':paint_entries,'native_message_dispatch':native_message_dispatch,'context_enqueue_calls':context_enqueue_calls,'message_queue_contract':{'context_enqueue_entry':hex(MESSAGE_CONTEXT_CALL),'expected_native_message_vptrs':[hex(v) for v in sorted(EXPECTED_MESSAGE_VPTRS)],'return_stub':hex(MESSAGE_RETURN)},'glv3_depthmap_ptr_after_saved_redraw':hex(struct.unpack('<Q',p.u.mem_read(actual_drawable+488,8))[0]),'rtv1_paper_state_after_saved_redraw':{'depthmap_ptr':hex(struct.unpack('<Q',p.u.mem_read(rt+248,8))[0]),'depthmap_dimensions':list(struct.unpack('<II',p.u.mem_read(rt+256,8))),'background_scale_f32_bits':p.u.mem_read(rt+264,4).hex()}}"
if source.count(old) != 1:
    raise RuntimeError("native caller result assembly changed")
source = source.replace(old,new)
context_width=int(os.environ.get('PENCIL_CONTEXT_WIDTH','1920')); context_height=int(os.environ.get('PENCIL_CONTEXT_HEIGHT','1080'))
import argparse
parser=argparse.ArgumentParser()
mode=parser.add_mutually_exclusive_group()
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
args_cli=parser.parse_args()
FIXTURE=ROOT/'conformance/pencil3-paper-consumer-native.json'
sys.argv = [str(source_path), "--print-capture", "--advanced-setting", "empty", "--width", str(context_width), "--height", str(context_height)]
output = io.StringIO()
expected_native_fault=None
try:
    with contextlib.redirect_stdout(output):
        state = {"__name__": "__main__", "__file__": str(source_path), "_cd": _cd}
        exec(compile(source, str(source_path), "exec"), state)
except Exception as error:
    if not ('state' in locals() and 'p' in state): raise
    p=state['p']; pc=p.u.reg_read(_cd.R.UC_ARM64_REG_PC); x0=p.x(0)
    target=struct.unpack('<Q',p.u.mem_read(x0+88,8))[0]
    lr=p.u.reg_read(_cd.R.UC_ARM64_REG_LR)
    if type(error).__name__!='UcError' or pc!=0x408e0d0 or lr!=0x510cf24 or target!=0:
        raise RuntimeError(f'unexpected native failure: {type(error).__name__}: {error}; pc={pc:#x}; lr={lr:#x}; x0={x0:#x}; target={target:#x}') from error
    expected_native_fault={'exception':type(error).__name__,'message':str(error),'pc':hex(pc),'lr':hex(p.u.reg_read(_cd.R.UC_ARM64_REG_LR)),'x0':hex(x0),'x1':hex(p.x(1)),'x2':hex(p.x(2)),'subbitmap_target_offset':88,'subbitmap_target':hex(target),'boundary':'native Draw reached SPSubBitmapRT::DeactivateFrameBufferRT and faulted because the host-created subbitmap has no writable framebuffer target'}
if expected_native_fault is None:
    raise RuntimeError('expected native subbitmap target fault did not occur')
messages=state['native_message_dispatch']; render=next(r for r in messages if r['vtable']=='0x10774c8')
rt=int(render['selected_rt'],16); init=render['native_init_return']
producer_events=[r for r in state['pencil_buffer_events'] if r['phase']=='draw_object_stroke']
producer_redraw_entries=[{k:r[k] for k in ('phase','entry_pc','receiver_address','receiver_role')} for r in state['drawable_entry_snapshots'] if r['phase']=='draw_object_stroke']
producer_stream=bytes.fromhex(producer_events[-1]['after_bytes']) if producer_events else b''
render_bytes=bytes.fromhex(render['message_bytes_0_144'])
render_bitmap=int.from_bytes(render_bytes[16:24],'little')
render_rt=int.from_bytes(render_bytes[32:40],'little')
render_canvas=int.from_bytes(render_bytes[40:48],'little')
setter=next(r for r in state['paper_setter_trace'] if r['pc']=='0x510c7e4')
raw_setter_thunk=next(r for r in state['native_rt_calls'] if r['entry']=='0x510c85c')
setter_thunk={k:raw_setter_thunk[k] for k in ('entry','symbol','x0','x1','lr')}
raw_update_entry=next(r for r in state['native_rt_calls'] if r['entry']=='0x510cc94')
update_entry={k:raw_update_entry[k] for k in ('entry','symbol','x0','x1','lr','plus_208','plus_216','plus_248','plus_256')}
rt_after=bytes(p.u.mem_read(rt+248,8)).hex(); graphics_ptr=int.from_bytes(bytes.fromhex(init['field208']),'little')
gfx_factory_return=next(r for r in state['native_graphics_factory_returns'] if r['returned_graphics_object']==hex(graphics_ptr))
init_entry=next(r for r in state['graphics_init_entries'] if r['event']=='init-entry' and r['object']==hex(rt) and r['lr']=='0x200f2240')
def require(condition,detail):
 if not condition: raise RuntimeError(f'native consumer assertion failed: {detail}')
require(int.from_bytes(bytes.fromhex(render['message_bytes_0_144'])[32:40],'little')==rt,'RenderMsg receiver equals queued message+32')
require(render_bitmap==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'RenderMsg +16 is the Drawing depthmap input')
require(render_canvas==int(state['native_pencil_canvas_info']['canvas'],16),'RenderMsg +40 is the constructed native SPenCanvas')
require(bool(producer_events) and all(int(r['rt'],16)==rt for r in producer_events),'queued RT equals native AddPoint callback producer receivers')
require(bool(producer_redraw_entries),'native selected GLV3 RedrawPen entry occurred before queue submission')
require(bool(producer_stream) and len(producer_stream)%20==0,'native AddPoint producer stream is a complete sequence of 20-byte records')
producer={'redraw_entries':producer_redraw_entries,
 'addpoint_callbacks':[{'rt':r['rt'],'vector_object':r['vector_object'],'before_count_bytes':r['before_count_bytes'],'after_count_bytes':r['after_count_bytes'],'after_record_count':r['after_count_bytes']//20,'float_args':r['float_args'],'return_pc':r['return_pc']} for r in producer_events],
 'rt':producer_events[0]['rt'],'callback_count':len(producer_events),'final_stream':{'record_bytes':20,'record_count':len(producer_stream)//20,'byte_count':len(producer_stream),'sha256':hashlib.sha256(producer_stream).hexdigest(),'bytes_hex':producer_stream.hex()}}
require(init['x0']=='0x0' and int.from_bytes(bytes.fromhex(init['field208']),'little')==graphics_ptr!=0,'native RTV1 Init populated nonnull +208 graphics object')
require(int(setter['x0'],16)==rt and int(setter['x1'],16)==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'native setter body receiver and input bitmap')
require(int(setter_thunk['x0'],16)==rt+0xc8 and int(setter_thunk['x1'],16)==int(setter['x1'],16),'native interface thunk resolves the actual RT receiver')
require(int(update_entry['x0'],16)==rt,'native RenderMsg Update targets the same RT')
require(int.from_bytes(bytes.fromhex(rt_after),'little')==int(setter['x1'],16),'native setter stored bitmap in RT+248')
rt_dimensions=struct.unpack('<II',p.u.mem_read(rt+256,8))
require(rt_dimensions==(320,180),'native setter cached input bitmap dimensions in RT+256/+260')
require(int.from_bytes(bytes.fromhex(update_entry['plus_248']),'little')==0,'RT+248 was null before native setter')
require(int.from_bytes(bytes.fromhex(init_entry['field208_before']),'little')==0,'Init ran on initially uninitialized Drawing-selected RT')
require(int(gfx_factory_return['returned_graphics_object'],16)==graphics_ptr,'native factory return joins RT+208')
require(state['native_bitmap_factory_calls'][0]['stack_args_8_9_u32']==[0,0],'CreateBitmap arguments 8 and 9 were explicitly initialized')
libs={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in state['paths'].items()}
libs['Renderer']=hashlib.sha256((ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so').read_bytes()).hexdigest()
libs['libc++_shared.so']=hashlib.sha256((ROOT/'scratch/apk-analysis-native/arm64-v8a/libc++_shared.so').read_bytes()).hexdigest()
result={'schema':'pencil3-paper-consumer-native-v1','runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(source_path.read_bytes()).hexdigest(),'pinned_source_hashes':SOURCE_PINS,'libraries':libs,'bitmap_factory_call':state['native_bitmap_factory_calls'],'native_assertions':{'render_message_vtable':'0x10774c8','run_slot':render['native_execute_slot_plus16'],'queued_message_bitmap':hex(render_bitmap),'queued_message_rt':render['selected_rt'],'queued_message_canvas':hex(render_canvas),'init_receiver':init_entry['object'],'init_return_x0':init['x0'],'rt_field_208_graphics_object':init['field208'],'graphics_object_factory_return':gfx_factory_return,'rt_field_216_shader':init['field216'],'rt_field_248_before_update':update_entry['plus_248'],'native_update_entry':update_entry,'native_setter_thunk':setter_thunk,'rt_setter_body_pc':setter['pc'],'rt_setter_this':setter['x0'],'rt_setter_bitmap':setter['x1'],'rt_field_248_after':rt_after,'rt_dimensions_after_setter':list(rt_dimensions),'setter_matches_input_bitmap':int(setter['x1'],16)==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'setter_matches_rt_field':int.from_bytes(bytes.fromhex(rt_after),'little')==int(setter['x1'],16)},'explicit_native_init_intervention':render['host_lifecycle_intervention'],'queue_adapter':{'kind':'synthetic IGLMsgQueue','execution':'synchronously invokes the actual queued ITask vtable+16 Run body; async scheduling and ownership lifecycle are not modeled','native_task_vptrs':[r['vtable'] for r in messages]},'inputs':{'context':[context_width,context_height],'synthetic_depthmap_dimensions':[320,180],'synthetic_background_dimensions':[640,360],'depthmap_bitmap':state['paper_depthmap_injected']['depthmap_bitmap'],'depthmap_texture':state['paper_depthmap_injected']['depthmap_texture'],'native_pen_canvas':state['native_pencil_canvas_info']},'producer':producer,'host_resources':state['native_resource_calls'],'host_operation_adapters':state['clip_boundary_calls'],'expected_native_draw_boundary':expected_native_fault,'scope':'Native Drawing caller, queued RenderMsg, actual selected RTV1 Init and SetDepthMapBitmap body are executed. The synthetic ISPBitmap adapter supplies GetBitmapReadable, Ref, and dimensions; its readable interface and texture objects, EGL/GLES interfaces, caps and queue scheduling are host supplied. Native Draw proceeds until the missing subbitmap framebuffer target faults; this is not completed paint or device output.'}
encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
if args_cli.write_fixture: FIXTURE.write_text(encoded); print(f'wrote {FIXTURE.relative_to(ROOT)}')
else:
    if not FIXTURE.exists(): raise SystemExit('Pencil3 paper consumer fixture missing; review then use --write-fixture')
    if FIXTURE.read_text()!=encoded: raise SystemExit('Pencil3 paper consumer fixture mismatch')
    print(f'verified {FIXTURE.relative_to(ROOT)} (native setter reached; expected Draw resource fault asserted)')
