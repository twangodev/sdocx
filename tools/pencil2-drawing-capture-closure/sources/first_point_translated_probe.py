"""Route the callback queue query to the synthetic task dispatcher as an intervention."""
import hashlib
import importlib.util
import sys
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[2]
source = root / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py'
if hashlib.sha256(source.read_bytes()).hexdigest() != '996ef6db3a376619e9e5d1d05b7d5e1a940726f3feb2ef6895275a705576ee3e':
    raise SystemExit('Pinned Drawing capture source changed')
spec = importlib.util.spec_from_file_location('pencil_handoff', source)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
m.CASES['handoff'] = {**m._CONTROL_BASELINE, 'pressure': [0.25, 0.5, 0.75, 1.0], 'tilt': [0.4] * 4, 'color_argb': 0x80336699, 'case_id': 'handoff'}
m.OUTPUTS = Path(tempfile.mkdtemp(prefix='pencil2-queue-intervention-', dir=root / 'tmp'))
original = m.adapt_source

def adapt(case):
    code = original(case)
    # Override only the callback destructor's canvas-data query; retain the
    # original host canvas behavior at all other call sites.
    needle = 'p=PencilCanvasProbe()'
    assert code.count(needle) == 1
    code = code.replace(needle, """_canvas_hook = PencilCanvasProbe.hook_canvas
def _callback_queue_hook(self,u,a,z,d):
 if a==oc.STUB_CANVAS_DATA and u.reg_read(R.UC_ARM64_REG_LR)==p.syms['SPen::PenReturnCallback::~PenReturnCallback()']+0x50:
  self.x(0,self.handoff_queue); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 else: _canvas_hook(self,u,a,z,d)
PencilCanvasProbe.hook_canvas = _callback_queue_hook
p=PencilCanvasProbe()""")
    needle = "manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64);"
    assert code.count(needle) == 1
    code = code.replace(needle, "manager=p.alloc(256); queue=p.alloc(64); p.handoff_queue=queue; qvt=p.alloc(64);")
    needle = 'event_order=[]\npressure_alpha_observations=[]'
    assert code.count(needle) == 1
    code = code.replace(needle, 'event_order=[]\nhandoff_events=[]\nhandoff_pending={}\nqueue_events=[]\ncallback_events=[]\npressure_alpha_observations=[]')
    needle = "  if not method: raise RuntimeError('task has null Run')"
    assert code.count(needle) == 1
    code = code.replace(needle, """  if vt in (0x10774c8,0x1077528):
   queue_events.append({'phase':phase,'event':'deferred','task':hex(task),'vtable':hex(vt),'run':hex(method)})
   p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller); return
  if not method: raise RuntimeError('task has null Run')""")
    needle = " if a==p.syms.get('SPen::Pencil2StrokeDrawableRTV4::AddPoint(SPen::Vector2<float>*, float, float, float, int, SPen::Vector3<float>)'):"
    assert code.count(needle) == 1
    hook = ''' if a==p.syms.get('SPen::PenReturnCallback::~PenReturnCallback()'):
  cb=u.reg_read(R.UC_ARM64_REG_X0); callback_events.append({'phase':phase,'callback':hex(cb),'rt':hex(struct.unpack('<Q',u.mem_read(cb+8,8))[0]),'canvas':hex(struct.unpack('<Q',u.mem_read(cb+16,8))[0]),'buffer':hex(struct.unpack('<Q',u.mem_read(cb+40,8))[0]),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'entry_pc':hex(a),'entry_bytes':bytes(u.mem_read(a,16)).hex(),'canvas_vtable':hex(struct.unpack('<Q',u.mem_read(struct.unpack('<Q',u.mem_read(cb+16,8))[0],8))[0])})
 if a==p.syms.get('SPen::PenReturnCallback::~PenReturnCallback()',0)+0x50:
  queue_events.append({'phase':phase,'event':'destructor_queue_return','queue':hex(u.reg_read(R.UC_ARM64_REG_X0))})
 if a==0x20001000:
  task=u.reg_read(R.UC_ARM64_REG_X1); print('INTERVENTION_QUEUE',phase,hex(task),hex(u.reg_read(R.UC_ARM64_REG_LR)),flush=True); vt=struct.unpack('<Q',u.mem_read(task,8))[0]; queue_events.append({'phase':phase,'task':hex(task),'vtable':hex(vt),'run':hex(struct.unpack('<Q',u.mem_read(vt+16,8))[0]),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
 for handoff_name in ('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)','SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'):
  if a==p.syms.get(handoff_name):
   receiver=u.reg_read(R.UC_ARM64_REG_X0); lr=u.reg_read(R.UC_ARM64_REG_LR)
   row={'phase':phase,'method':handoff_name,'receiver':hex(receiver),'arg1':hex(u.reg_read(R.UC_ARM64_REG_X1)),'arg2':hex(u.reg_read(R.UC_ARM64_REG_X2)),'before':list(struct.unpack('<4Q',u.mem_read(receiver+80,32)))}
   print('INTERVENTION_HANDOFF',phase,handoff_name,hex(receiver),flush=True); handoff_events.append(row); handoff_pending[lr]=row
 if a in handoff_pending:
  row=handoff_pending.pop(a); receiver=int(row['receiver'],16)
  row['after']=list(struct.unpack('<4Q',u.mem_read(receiver+80,32)))
'''
    code = code.replace(needle, hook + needle)
    needle = " encoded=json.dumps(result,indent=2,sort_keys=True)+'\\n'"
    assert code.count(needle) == 1
    code = code.replace(needle, " result['handoff_events']=handoff_events\n result['queue_events']=queue_events\n result['callback_events']=callback_events\n result['callback_adapter_keys']=[name for name in p.callbacks if 'PenReturnCallback' in name]\n result['handoff_symbols']={name:hex(address) for name,address in p.syms.items() if name.startswith('SPen::PenDrawableRT::SetBuffer(') or name.startswith('SPen::PenDrawableRT::SendDataToGPU(')}\n" + needle)
    needle = " encoded=json.dumps(result,indent=2,sort_keys=True)+'\\n'"
    assert code.count(needle) == 1
    code = code.replace(needle, """ import os,re,subprocess
 from pathlib import Path
 renderer=Path('/home/jding/RustroverProjects/sdocx/scratch/apk-analysis-native/arm64-v8a')/'libSPenRenderer.so'; blob=renderer.read_bytes(); base=0x9000000
 if hashlib.sha256(blob).hexdigest()!='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed': raise RuntimeError('Renderer DSO hash mismatch')
 ph=struct.unpack_from('<Q',blob,32)[0]; ent,n=struct.unpack_from('<HH',blob,54); extent=0
 for i in range(n):
  t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
  if t==1:extent=max(extent,va+ms)
 p.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(n):
  t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,ph+i*ent)
  if t==1 and ms:p.u.mem_write(base+va,blob[o:o+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
 envsub=dict(os.environ);envsub['LD_LIBRARY_PATH']=str((Path('/home/jding/RustroverProjects/sdocx')/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
 for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=envsub).splitlines():
  q=line.strip().split(' ',2)
  if len(q)==3 and q[1] in ('T','W'):p.syms[q[2]]=base+int(q[0],16)
 objdump=Path('/home/jding/RustroverProjects/sdocx')/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
 p.syms['typeinfo for SPen::IGLManagedMsgQueue']=base+0x6b400
 for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',subprocess.check_output([str(objdump),'-d','-C','-j','.plt',str(renderer)],text=True,env=envsub),re.M):p.plt[base+int(a,16)]=name
 # Renderer external relocations are bound to pinned caller DSO exports; internal ABS64 use exact dynsym.
 rawsyms={}
 for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=envsub).splitlines():
  q=line.split()
  if len(q)==3 and q[1].upper() in ('T','W','D','B','R','V'):rawsyms[q[2]]=base+int(q[0],16)
 shoff=struct.unpack_from('<Q',blob,40)[0]; se,sn,ss=struct.unpack_from('<HHH',blob,58);secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*se) for i in range(sn)];names=blob[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\\0',x[0])].decode():x for x in secs};ds,st=by['.dynsym'],by['.dynstr'];strings=blob[st[4]:st[4]+st[5]];sy=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,inf,oth,ndx,val,size=struct.unpack_from('<IBBHQQ',blob,off);sy.append((strings[no:strings.find(b'\\0',no)].decode() if no else '',val))
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
 import sys
 sys.path.insert(0,str(Path('/home/jding/RustroverProjects/sdocx/tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance')))
 import inkpen_paint_native as ip
 p.mesa=ip.GLES()
 gl_strings=[]
 def get_mesa_string():
  enum=p.x(0); raw=p.mesa.gl_string(enum)
  if raw is None: p.x(0,0); return
  ptr=p.alloc(len(raw)+1); p.u.mem_write(ptr,raw+bytes([0])); p.x(0,ptr); gl_strings.append({'enum':enum,'value':raw.decode(errors='replace')})
 p.callbacks['glGetString']=get_mesa_string
 def cstring(ptr):
  b=bytearray()
  for i in range(65536):
   c=bytes(p.u.mem_read(ptr+i,1))
   if c==bytes([0]): return bytes(b)
   b.extend(c)
  raise RuntimeError('unterminated guest string')
 def strstr():
  base=p.x(0); offset=cstring(base).find(cstring(p.x(1))); p.x(0,base+offset if offset>=0 else 0)
 p.callbacks['strstr']=strstr
 def strchr():
  ptr,byte=p.x(0),p.x(1)&255; raw=cstring(ptr)+bytes([0]); offset=raw.find(bytes([byte])); p.x(0,ptr+offset if offset>=0 else 0)
 p.callbacks['strchr']=strchr
 def strncpy_checked():
  dst,src,count,capacity=[p.x(i) for i in range(4)]
  if count>capacity or count>1048576: raise RuntimeError('invalid __strncpy_chk length')
  raw=bytearray()
  for i in range(count):
   byte=bytes(p.u.mem_read(src+i,1))
   if byte==bytes([0]): break
   raw.extend(byte)
  p.u.mem_write(dst,bytes(raw)+bytes(count-len(raw))); p.x(0,dst)
 p.callbacks['__strncpy_chk']=strncpy_checked
 def strlen_checked():
  ptr,capacity=p.x(0),p.x(1); raw=cstring(ptr)
  if len(raw)>=capacity: raise RuntimeError('invalid __strlen_chk length')
  p.x(0,len(raw))
 p.callbacks['__strlen_chk']=strlen_checked
 def strncpy_checked2():
  dst,src,count,capacity,source_capacity=[p.x(i) for i in range(5)]
  if count>capacity or count>1048576: raise RuntimeError('invalid __strncpy_chk2 destination length')
  raw=bytearray()
  for i in range(count):
   if i>=source_capacity: raise RuntimeError('invalid __strncpy_chk2 source length')
   byte=bytes(p.u.mem_read(src+i,1))
   if byte==bytes([0]): break
   raw.extend(byte)
  p.u.mem_write(dst,bytes(raw)+bytes(count-len(raw))); p.x(0,dst)
 p.callbacks['__strncpy_chk2']=strncpy_checked2
 def strncpy_guest():
  dst,src,count=[p.x(i) for i in range(3)]
  if count>1048576: raise RuntimeError('strncpy length out of bounds')
  raw=bytearray()
  for i in range(count):
   byte=bytes(p.u.mem_read(src+i,1))
   if byte==bytes([0]): break
   raw.extend(byte)
  p.u.mem_write(dst,bytes(raw)+bytes(count-len(raw))); p.x(0,dst)
 p.callbacks['strncpy']=strncpy_guest
 def strcmp_guest():
  a,b=cstring(p.x(0)),cstring(p.x(1)); p.x(0,((a>b)-(a<b)) & 0xffffffffffffffff)
 p.callbacks['strcmp']=strcmp_guest
 def clock_gettime_guest():
  import time
  clock,out=p.x(0),p.x(1)
  if clock not in (0,1): raise RuntimeError('unsupported clock_gettime clock')
  ns=time.clock_gettime_ns(clock); p.put(out,'qq',ns//1000000000,ns%1000000000); p.x(0,0)
 p.callbacks['clock_gettime']=clock_gettime_guest
 import ctypes as C
 gl_generated=[]
 def gen_buffers():
  count,out=p.x(0),p.x(1)
  if not 0<count<=32: raise RuntimeError('glGenBuffers count out of bounds')
  ids=(C.c_uint*count)(); ip.cfunc(p.mesa.gl,'glGenBuffers',None,[C.c_int,C.POINTER(C.c_uint)])(count,ids)
  p.put(out,'I'*count,*ids); gl_generated.extend(list(ids))
 p.callbacks['glGenBuffers']=gen_buffers
 def gen_textures():
  count,out=p.x(0),p.x(1)
  if not 0<count<=32: raise RuntimeError('glGenTextures count out of bounds')
  ids=(C.c_uint*count)(); ip.cfunc(p.mesa.gl,'glGenTextures',None,[C.c_int,C.POINTER(C.c_uint)])(count,ids)
  p.put(out,'I'*count,*ids); gl_simple.append(['glGenTextures',*ids])
 p.callbacks['glGenTextures']=gen_textures
 def gen_samplers():
  count,out=p.x(0),p.x(1)
  if not 0<count<=32: raise RuntimeError('glGenSamplers count out of bounds')
  ids=(C.c_uint*count)(); ip.cfunc(p.mesa.gl,'glGenSamplers',None,[C.c_int,C.POINTER(C.c_uint)])(count,ids)
  p.put(out,'I'*count,*ids); gl_simple.append(['glGenSamplers',*ids])
 p.callbacks['glGenSamplers']=gen_samplers
 gl_simple=[]
 def bind_buffer():
  target,buffer=p.x(0),p.x(1); ip.cfunc(p.mesa.gl,'glBindBuffer',None,[C.c_uint,C.c_uint])(target,buffer); gl_simple.append(['glBindBuffer',target,buffer])
 p.callbacks['glBindBuffer']=bind_buffer
 def buffer_data():
  target,size,ptr,usage=[p.x(i) for i in range(4)]
  if size>16*1024*1024: raise RuntimeError('glBufferData size exceeds bound')
  raw=bytes(p.u.mem_read(ptr,size)) if ptr and size else None
  host=C.create_string_buffer(raw) if raw is not None else None
  ip.cfunc(p.mesa.gl,'glBufferData',None,[C.c_uint,C.c_ssize_t,C.c_void_p,C.c_uint])(target,size,host,usage)
  gl_simple.append(['glBufferData',target,size,hex(ptr),usage,hashlib.sha256(raw).hexdigest() if raw is not None else None])
 p.callbacks['glBufferData']=buffer_data
 def scalar_gl(name, restype, argtypes):
  fn=ip.cfunc(p.mesa.gl,name,restype,argtypes)
  def call():
   args=[p.x(i) for i in range(len(argtypes))]; value=fn(*args)
   gl_simple.append([name,*args,value])
   if restype is not None: p.x(0,value & 0xffffffffffffffff)
  p.callbacks[name]=call
 for name,restype,argtypes in [
  ('glCreateShader',C.c_uint,[C.c_uint]),
  ('glGetError',C.c_uint,[]),
  ('glCompileShader',None,[C.c_uint]),
  ('glCreateProgram',C.c_uint,[]),
  ('glAttachShader',None,[C.c_uint,C.c_uint]),
  ('glDetachShader',None,[C.c_uint,C.c_uint]),
  ('glLinkProgram',None,[C.c_uint]),
  ('glDeleteShader',None,[C.c_uint]),
  ('glDeleteProgram',None,[C.c_uint]),
  ('glBindTexture',None,[C.c_uint,C.c_uint]),
  ('glTexParameteri',None,[C.c_uint,C.c_uint,C.c_int]),
  ('glSamplerParameteri',None,[C.c_uint,C.c_uint,C.c_int]),
  ('glBindSampler',None,[C.c_uint,C.c_uint]),
  ('glPixelStorei',None,[C.c_uint,C.c_int]),
  ('glUseProgram',None,[C.c_uint])]: scalar_gl(name,restype,argtypes)
 def tex_image_2d():
  args=[p.x(i) for i in range(8)]; target,level,internal,width,height,border,fmt,kind=args
  ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0]
  if width>4096 or height>4096 or fmt!=0x1908 or kind!=0x1401: raise RuntimeError('unsupported texture upload layout')
  # The caller configures UNPACK_ROW_LENGTH to this texture's width.
  if ptr:
   get=ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])
   state=[]
   for enum in (0x0cf2,0x0cf5,0x0cf4,0x0cf3):
    value=C.c_int(); get(enum,C.byref(value)); state.append(value.value)
   row,align,skip_pixels,skip_rows=state
   if row not in (0,width) or align not in (1,2,4,8) or skip_pixels or skip_rows: raise RuntimeError('unsupported unpack stride')
   raw=bytes(p.u.mem_read(ptr,width*height*4)); data=C.create_string_buffer(raw)
  else: raw=None; data=None
  ip.cfunc(p.mesa.gl,'glTexImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(*args,data)
  gl_simple.append(['glTexImage2D',*args,hex(ptr),hashlib.sha256(raw).hexdigest() if raw is not None else None])
 p.callbacks['glTexImage2D']=tex_image_2d
 def tex_sub_image_2d():
  args=[p.x(i) for i in range(8)]; target,level,x,y,width,height,fmt,kind=args
  ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0]
  if not ptr or width>4096 or height>4096 or fmt!=0x1908 or kind!=0x1401: raise RuntimeError('unsupported subimage layout')
  get=ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])
  state=[]
  for enum in (0x0cf2,0x0cf5,0x0cf4,0x0cf3):
   value=C.c_int(); get(enum,C.byref(value)); state.append(value.value)
  row,align,skip_pixels,skip_rows=state
  if row not in (0,width) or align not in (1,2,4,8) or skip_pixels or skip_rows: raise RuntimeError('unsupported unpack subimage stride')
  raw=bytes(p.u.mem_read(ptr,width*height*4)); data=C.create_string_buffer(raw)
  ip.cfunc(p.mesa.gl,'glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(*args,data)
  gl_simple.append(['glTexSubImage2D',*args,hex(ptr),hashlib.sha256(raw).hexdigest()])
 p.callbacks['glTexSubImage2D']=tex_sub_image_2d
 def shader_source():
  shader,count,strings,lengths=[p.x(i) for i in range(4)]
  if count>64: raise RuntimeError('shader source count out of bounds')
  source=[]
  for i in range(count):
   ptr=p.get(strings+8*i,'Q')[0]
   length=p.get(lengths+4*i,'i')[0] if lengths else -1
   if length>1048576: raise RuntimeError('shader source length out of bounds')
   source.append(bytes(p.u.mem_read(ptr,length)) if length>=0 else cstring(ptr))
  arr=(C.c_char_p*count)(*source); sizes=(C.c_int*count)(*[len(s) for s in source])
  ip.cfunc(p.mesa.gl,'glShaderSource',None,[C.c_uint,C.c_int,C.POINTER(C.c_char_p),C.POINTER(C.c_int)])(shader,count,arr,sizes)
  gl_simple.append(['glShaderSource',shader,[hashlib.sha256(s).hexdigest() for s in source]])
 p.callbacks['glShaderSource']=shader_source
 def get_integer():
  enum,out=p.x(0),p.x(1); value=C.c_int()
  ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(enum,C.byref(value))
  p.put(out,'i',value.value); gl_simple.append(['glGetIntegerv',enum,value.value])
 p.callbacks['glGetIntegerv']=get_integer
 def bind_attrib():
  program,index,ptr=p.x(0),p.x(1),p.x(2); name=cstring(ptr)
  ip.cfunc(p.mesa.gl,'glBindAttribLocation',None,[C.c_uint,C.c_uint,C.c_char_p])(program,index,name)
  gl_simple.append(['glBindAttribLocation',program,index,name.decode(errors='replace')])
 p.callbacks['glBindAttribLocation']=bind_attrib
 def active_uniform():
  program,index,size,length,count,kind,out=[p.x(i) for i in range(7)]
  if size>1048576: raise RuntimeError('active uniform name length out of bounds')
  written=C.c_int(); elements=C.c_int(); uniform_type=C.c_uint(); buf=C.create_string_buffer(max(size,1))
  ip.cfunc(p.mesa.gl,'glGetActiveUniform',None,[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p])(program,index,size,C.byref(written),C.byref(elements),C.byref(uniform_type),buf if out else None)
  if length: p.put(length,'i',written.value)
  if count: p.put(count,'i',elements.value)
  if kind: p.put(kind,'I',uniform_type.value)
  if out and size: p.u.mem_write(out,buf.raw[:min(size,written.value+1)])
  gl_simple.append(['glGetActiveUniform',program,index,buf.value.decode(errors='replace'),elements.value,uniform_type.value])
 p.callbacks['glGetActiveUniform']=active_uniform
 def get_location(name):
  fn=ip.cfunc(p.mesa.gl,name,C.c_int,[C.c_uint,C.c_char_p])
  def call():
   program,ptr=p.x(0),p.x(1); label=cstring(ptr); value=fn(program,label)
   p.x(0,value & 0xffffffffffffffff); gl_simple.append([name,program,label.decode(errors='replace'),value])
  p.callbacks[name]=call
 for name in ('glGetUniformLocation','glGetAttribLocation'): get_location(name)
 def program_binary():
  program,size,length,fmt,out=[p.x(i) for i in range(5)]
  if size>16*1024*1024: raise RuntimeError('program binary length out of bounds')
  written=C.c_int(); binary_format=C.c_uint(); buf=C.create_string_buffer(max(size,1))
  ip.cfunc(p.mesa.gl,'glGetProgramBinary',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p])(program,size,C.byref(written),C.byref(binary_format),buf)
  if length: p.put(length,'i',written.value)
  if fmt: p.put(fmt,'I',binary_format.value)
  if out and written.value: p.u.mem_write(out,buf.raw[:written.value])
  gl_simple.append(['glGetProgramBinary',program,written.value,binary_format.value,hashlib.sha256(buf.raw[:written.value]).hexdigest()])
 p.callbacks['glGetProgramBinary']=program_binary
 def format_cache_path():
  dst,cap,fmt,va=[p.x(i) for i in range(4)]
  stack,gr,vr,go,vo=struct.unpack('<QQQii',p.u.mem_read(va,32))
  format_string=cstring(fmt)
  if format_string!=b'/sh%0lu.dat' or cap>4096: raise RuntimeError('unsupported vsnprintf format '+repr(format_string))
  value=p.get(gr+go if go<0 else stack,'Q')[0]
  rendered=b'/sh'+str(value).encode()+b'.dat'
  if cap: p.u.mem_write(dst,rendered[:cap-1]+bytes([0]))
  p.x(0,len(rendered)); gl_simple.append(['vsnprintf',format_string.decode(),value,rendered.decode()])
 p.callbacks['vsnprintf']=format_cache_path
 import tempfile
 cache_streams={}
 def cache_fopen():
  path,mode=cstring(p.x(0)),cstring(p.x(1))
  if not re.fullmatch(rb'/sh[0-9]+[.]dat',path) or mode!=b'wb': raise RuntimeError('unsupported fopen '+repr((path,mode)))
  stream=p.alloc(8); cache_streams[stream]={'path':path.decode(),'file':tempfile.TemporaryFile(mode='w+b',buffering=0)}; p.x(0,stream)
  gl_simple.append(['fopen',path.decode(),mode.decode()])
 def cache_fwrite():
  ptr,size,count,stream=[p.x(i) for i in range(4)]
  if stream not in cache_streams or size*count>16*1024*1024: raise RuntimeError('unsupported fwrite')
  written=cache_streams[stream]['file'].write(bytes(p.u.mem_read(ptr,size*count)))
  p.x(0,written//size if size else 0); gl_simple.append(['fwrite',size,count,written])
 def cache_fclose():
  stream=p.x(0)
  if stream not in cache_streams: raise RuntimeError('unsupported fclose')
  item=cache_streams.pop(stream); f=item['file']; f.seek(0); data=f.read(); f.close()
  gl_simple.append(['fclose',item['path'],len(data),hashlib.sha256(data).hexdigest()]); p.x(0,0)
 def cache_setbuf():
  stream,buf=p.x(0),p.x(1)
  if stream not in cache_streams or buf: raise RuntimeError('unsupported setbuf')
  gl_simple.append(['setbuf','unbuffered'])
 def cache_fileno():
  stream=p.x(0)
  if stream not in cache_streams: raise RuntimeError('unsupported fileno')
  fd=cache_streams[stream]['file'].fileno(); p.x(0,fd); gl_simple.append(['fileno',fd])
 def cache_fflush():
  stream=p.x(0)
  if stream not in cache_streams: raise RuntimeError('unsupported fflush')
  cache_streams[stream]['file'].flush(); p.x(0,0); gl_simple.append(['fflush'])
 def cache_fsync():
  fd=p.x(0)
  if fd not in [item['file'].fileno() for item in cache_streams.values()]: raise RuntimeError('unsupported fsync')
  os.fsync(fd); p.x(0,0); gl_simple.append(['fsync',fd])
 p.callbacks.update({'fopen':cache_fopen,'fwrite':cache_fwrite,'fclose':cache_fclose,'setbuf':cache_setbuf,'fileno':cache_fileno,'fflush':cache_fflush,'fsync':cache_fsync})
 for name in ('glGetShaderiv','glGetProgramiv'):
  def query(name=name):
   obj,enum,out=[p.x(i) for i in range(3)]; value=C.c_int()
   ip.cfunc(p.mesa.gl,name,None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)])(obj,enum,C.byref(value))
   p.put(out,'i',value.value); gl_simple.append([name,obj,enum,value.value])
  p.callbacks[name]=query
 for name in ('glGetShaderInfoLog','glGetProgramInfoLog'):
  def info_log(name=name):
   obj,size,length,out=[p.x(i) for i in range(4)]
   if size>1048576: raise RuntimeError('info log length out of bounds')
   written=C.c_int(); buf=C.create_string_buffer(max(size,1))
   ip.cfunc(p.mesa.gl,name,None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.c_void_p])(obj,size,C.byref(written),buf)
   if length: p.put(length,'i',written.value)
   if out and size: p.u.mem_write(out,buf.raw[:min(size,written.value+1)])
   gl_simple.append([name,obj,buf.value.decode(errors='replace')])
  p.callbacks[name]=info_log
 def current_context():
  ctx=ip.cfunc(p.mesa.egl,'eglGetCurrentContext',C.c_void_p,[])()
  if not ctx or ctx!=p.mesa.context: raise RuntimeError('Mesa current EGL context mismatch')
  p.x(0,ctx); gl_simple.append(['eglGetCurrentContext','non-null'])
 p.callbacks['eglGetCurrentContext']=current_context
 import zlib
 def crc32_guest():
  seed,ptr,length=p.x(0),p.x(1),p.x(2)
  if length>16*1024*1024: raise RuntimeError('crc32 length out of bounds')
  data=bytes(p.u.mem_read(ptr,length)) if ptr and length else b''
  p.x(0,zlib.crc32(data,seed)&0xffffffff)
 p.callbacks['crc32']=crc32_guest
 renderer_initializers=[]
 init_section=by['.init_array']
 for offset in range(0,init_section[5],8):
  address=p.get(base+init_section[3]+offset,'Q')[0]
  if address:
   name='renderer_initializer_'+str(offset//8); p.syms[name]=address
   p.call(name,[]); renderer_initializers.append(hex(address))
 vertex_name='SPen::VertexDescriptor::addAttribute(SPen::VertexDescriptor::AttributeType, int, int, int)'
 vertex_entries=[]
 def observe_vertex(u,a,z,d):
  if a==p.syms[vertex_name]: vertex_entries.append({'pc':hex(a),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
 vertex_hook=p.u.hook_add(UC_HOOK_CODE,observe_vertex)
 init_diagnostic={'before200':hex(p.get(selected_rt+200,'Q')[0]),'vertex_symbol':hex(p.syms[vertex_name]),'renderer_initializers':renderer_initializers}
 try:
  p.call('SPen::Pencil2StrokeDrawableRTV4::Init()',[selected_rt])
  init_diagnostic.update({'outcome':'returned','after200':hex(p.get(selected_rt+200,'Q')[0])})
 except Exception as error:
  init_diagnostic.update({'outcome':'exception','error':repr(error),'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'registers':{f'x{i}':hex(p.x(i)) for i in range(9)}})
 p.u.hook_del(vertex_hook)
 init_diagnostic['vertex_entries']=vertex_entries
 init_diagnostic['gl_strings']=gl_strings
 init_diagnostic['gl_generated_buffers']=gl_generated
 init_diagnostic['gl_simple']=list(gl_simple)
 init_diagnostic['gl_error']=p.mesa.f['glGetError']()
 result['init_diagnostic']=init_diagnostic
 update_entries=[]
 def observe_update(u,a,z,d):
  if a==p.syms['SPen::Pencil2StrokeDrawableRTV4::Update(bool)']:
   receiver=u.reg_read(R.UC_ARM64_REG_X0); update_entries.append({'pc':hex(a),'receiver':hex(receiver),'graphics_ptr':hex(p.get(receiver+200,'Q')[0])})
  if a==0x5099648:
   update_entries.append({'pc':hex(a),'x8':hex(u.reg_read(R.UC_ARM64_REG_X8))})
 update_hook=p.u.hook_add(UC_HOOK_CODE,observe_update)
 diagnostic={'selected_rt':hex(selected_rt),'before200':hex(p.get(selected_rt+200,'Q')[0]),'before80':hex(p.get(selected_rt+80,'Q')[0]),'before88':hex(p.get(selected_rt+88,'Q')[0]),'before_count':p.get(selected_rt+72,'I')[0]}
 try:
  p.call('SPen::Pencil2StrokeDrawableRTV4::Update(bool)',[selected_rt,0])
  diagnostic.update({'outcome':'returned','after_count':p.get(selected_rt+72,'I')[0],'after88':hex(p.get(selected_rt+88,'Q')[0])})
 except Exception as error:
  diagnostic.update({'outcome':'exception','error':repr(error),'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 p.u.hook_del(update_hook)
 diagnostic['entries']=update_entries
 diagnostic['gl_simple_after_init']=gl_simple[len(init_diagnostic['gl_simple']):]
 diagnostic['gl_error']=p.mesa.f['glGetError']()
 diagnostic['entry_matches_selected_rt']=bool(update_entries and update_entries[0]['receiver']==hex(selected_rt) and update_entries[0]['graphics_ptr']==diagnostic['before200'])
 result['update_after_handoff']=diagnostic
 def fence_sync():
  value=ip.cfunc(p.mesa.gl,'glFenceSync',C.c_void_p,[C.c_uint,C.c_uint])(p.x(0),p.x(1));p.x(0,value or 0)
 p.callbacks['glFenceSync']=fence_sync
 p.callbacks['glFlush']=lambda:ip.cfunc(p.mesa.gl,'glFlush',None,[])()
 def gen_framebuffers():
  count,out=p.x(0),p.x(1); ids=(C.c_uint*count)();ip.cfunc(p.mesa.gl,'glGenFramebuffers',None,[C.c_int,C.POINTER(C.c_uint)])(count,ids);p.put(out,'I'*count,*ids);gl_simple.append(['glGenFramebuffers',*ids])
 p.callbacks['glGenFramebuffers']=gen_framebuffers
 scalar_gl('glViewport',None,[C.c_int,C.c_int,C.c_int,C.c_int])
 for name,count in [('glBlendEquationSeparate',2),('glBlendFuncSeparate',4),('glEnable',1),('glDisable',1),('glUseProgram',1),('glActiveTexture',1),('glBindSampler',2),('glBindVertexArray',1),('glDrawArraysInstanced',4),('glDrawArrays',3),('glColorMask',4),('glEnableVertexAttribArray',1),('glDisableVertexAttribArray',1),('glVertexAttribDivisor',2)]:scalar_gl(name,None,[C.c_uint]*count)
 for name,types in [('glBindFramebuffer',[C.c_uint,C.c_uint]),('glFramebufferTexture2D',[C.c_uint,C.c_uint,C.c_uint,C.c_uint,C.c_int]),('glCheckFramebufferStatus',[C.c_uint])]:
  scalar_gl(name,C.c_uint if name=='glCheckFramebufferStatus' else None,types)

 def uniform_matrix():
  loc,count,transpose,ptr=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,count*64));data=C.create_string_buffer(raw);ip.cfunc(p.mesa.gl,'glUniformMatrix4fv',None,[C.c_int,C.c_int,C.c_ubyte,C.c_void_p])(loc,count,transpose,data);gl_simple.append(['glUniformMatrix4fv',loc,count,transpose,list(struct.unpack('<%df'%(count*16),raw))])
 p.callbacks['glUniformMatrix4fv']=uniform_matrix
 def uniform2():
  loc,count,ptr=[p.x(i) for i in range(3)];raw=bytes(p.u.mem_read(ptr,count*8));data=C.create_string_buffer(raw);ip.cfunc(p.mesa.gl,'glUniform2fv',None,[C.c_int,C.c_int,C.c_void_p])(loc,count,data);gl_simple.append(['glUniform2fv',loc,count,list(struct.unpack('<%df'%(count*2),raw))])
 p.callbacks['glUniform2fv']=uniform2
 def uniform1f():
  loc,value=p.x(0),p.f(0);ip.cfunc(p.mesa.gl,'glUniform1f',None,[C.c_int,C.c_float])(loc,value);gl_simple.append(['glUniform1f',loc,value])
 p.callbacks['glUniform1f']=uniform1f
 scalar_gl('glUniform1i',None,[C.c_int,C.c_int])
 def attrib_pointer():
  vals=[p.x(i) for i in range(6)];ip.cfunc(p.mesa.gl,'glVertexAttribPointer',None,[C.c_uint,C.c_int,C.c_uint,C.c_ubyte,C.c_int,C.c_void_p])(*vals);gl_simple.append(['glVertexAttribPointer',*vals])
 p.callbacks['glVertexAttribPointer']=attrib_pointer
 def invalidate():
  target_enum,count,ptr=[p.x(i) for i in range(3)]; raw=bytes(p.u.mem_read(ptr,count*4)) if count else b''; data=C.create_string_buffer(raw) if raw else None;ip.cfunc(p.mesa.gl,'glInvalidateFramebuffer',None,[C.c_uint,C.c_int,C.c_void_p])(target_enum,count,data);gl_simple.append(['glInvalidateFramebuffer',target_enum,count])
 p.callbacks['glInvalidateFramebuffer']=invalidate
 target_gl_start=len(gl_simple)
 target={"constructor_args":{'width':192,'height':192,'rect':[0,0,192,192],'render_target':1,'layouts':[6,2]},"stage":"constructor","selected_rt":hex(selected_rt),"boundary":"native Graphics SPSubBitmapRT constructor with existing synthetic queue; no target fields or vtables fabricated"}
 try:
  constructor='SPen::SPSubBitmapRT::SPSubBitmapRT(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
  target['symbol_present']=constructor in p.syms
  rect=p.alloc(16);p.put(rect,'4i',0,0,192,192)
  bitmap=p.alloc(512); name=p.alloc(32);p.u.mem_write(name,b'pencil-target'+bytes([0]))
  target_args=[bitmap,queue,0,rect,192,192,name,0]
  for i,v in enumerate(target_args):p.x(i,v)
  sp=0x200f0000;p.u.mem_write(sp,struct.pack('<3Q',1,6,2))
  p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
  p.u.emu_start(p.syms[constructor],0x200f1000,count=5000000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError('constructor instruction limit')
  target.update({'constructor':'returned','bitmap':hex(bitmap),'vptr':hex(p.get(bitmap,'Q')[0]),'texture':hex(p.get(bitmap+80,'Q')[0]),'framebuffer_before':hex(p.get(bitmap+88,'Q')[0])})
  target['stage']='framebuffer'
  p.call('SPen::SPSubBitmapRT::CreateFrameBuffer()',[bitmap],limit=5000000)
  target.update({'framebuffer':'returned','framebuffer_after':hex(p.get(bitmap+88,'Q')[0])})
  target['stage']='activation'
  p.call('SPen::SPSubBitmapRT::ActivateFrameBufferRT(bool)',[bitmap,0],limit=5000000)
  f=p.mesa.f;f['glViewport'](0,0,192,192);f['glClearColor'](0,0,0,0);f['glClear'](0x4000)
  binding=C.c_int();ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0x8ca6,C.byref(binding));target['active_framebuffer']=binding.value
  pre=C.create_string_buffer(192*192*4);f['glReadPixels'](0,0,192,192,0x1908,0x1401,pre);target['prior_sha256']=hashlib.sha256(pre.raw).hexdigest();target['prior_nonzero_bytes']=sum(v!=0 for v in pre.raw)
  attachment=C.c_int();ip.cfunc(p.mesa.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])(0x8d40,0x8ce0,0x8cd1,C.byref(attachment));target['attachment_texture']=attachment.value
  target['stage']='draw'
  matrix=p.alloc(64);p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,32,32,0,1)
  p.call('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[selected_rt,bitmap,matrix],limit=8000000)
  pixels=C.create_string_buffer(192*192*4);f['glReadPixels'](0,0,192,192,0x1908,0x1401,pixels)
  ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0x8ca6,C.byref(binding));target['readback_framebuffer']=binding.value
  raw=pixels.raw;Path('/home/jding/RustroverProjects/sdocx/tools/pencil2-public-replay/drawing-native-target-translated.rgba.zlib').write_bytes(zlib.compress(raw,9));target['row_origin']='bottom';target.update({'draw':'returned','matrix':[1,0,0,0,0,1,0,0,0,0,1,0,32,32,0,1],'readback_sha256':hashlib.sha256(raw).hexdigest(),'nonzero_bytes':sum(v!=0 for v in raw),'gl_error':f['glGetError']()})

 except Exception as error:
  target.update({'outcome':'exception','error':repr(error),'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'registers':{f'x{i}':hex(p.x(i)) for i in range(9)}})
 target['gl_calls']=gl_simple[target_gl_start:]
 target['gl_error']=p.mesa.f['glGetError']()
 result['native_target_probe']=target
""" + needle)
    return code

m.adapt_source = adapt
sys.argv = [str(source), '--capture', '--case', 'handoff', '--width', '1920', '--height', '1080', '--profile-evidence', 'synthetic Drawing handoff diagnostic']
m.main()
