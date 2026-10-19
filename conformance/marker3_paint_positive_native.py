"""Capture synthetic Marker3 RTV1 paint through native factories and host Mesa.

Replay with PYTHONPATH=scratch/apk-analysis-runtime/python:conformance.
Points enter at RTV1 AddPoint, not saved ObjectStroke. Queue registration and
synchronous task execution, libc services, and GL forwarding are host boundaries;
this does not establish Android threading, destruction, or device pixel parity.
"""
import argparse, hashlib, sys, zlib, ctypes as C, struct, subprocess, re, json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'conformance'))
import airbrush_texture_native as at
import inkpen_paint_native as ip
import fountain_native as fn
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
sha='d9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5'
assert hashlib.sha256((LIB/'libSPenMarker3.so').read_bytes()).hexdigest()==sha
SOURCE_HASHES={
 'base_sha256':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'pencommon_sha256':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'fountainpen_sha256':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'airbrushpen_sha256':'92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423',
 'graphics_sha256':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'renderer_sha256':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'marker3_sha256':sha,
 'libcxx_shared_sha256':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
for key,value in SOURCE_HASHES.items():
 name=key.removesuffix('_sha256')
 filenames={'base':'libSPenBase.so','pencommon':'libSPenPenCommon.so','fountainpen':'libSPenFountainPen.so','airbrushpen':'libSPenAirBrushPen.so','graphics':'libSPenGraphics.so','renderer':'libSPenRenderer.so','marker3':'libSPenMarker3.so','libcxx_shared':'libc++_shared.so'}
 filename=filenames[name]
 actual=hashlib.sha256((LIB/filename).read_bytes()).hexdigest()
 if actual!=value: raise RuntimeError(f'unpinned {filename}: {actual}')
parser=argparse.ArgumentParser(description='Execute Marker3 V5 RTV1 Draw and host GLES contract capture')
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
parser.add_argument('--fixture',type=Path)
ARGS=parser.parse_args()
if ARGS.fixture is None:
 ARGS.fixture=ROOT/'conformance/marker3-paint-positive-native.json'
class P(at.TextureOracle):
 def __init__(self):
  original_uc=fn.Uc
  fn.Uc=lambda *args: self._max_cpu(original_uc,*args)
  super().__init__()
  self.gles=ip.GLES()
  self.gl_calls=[]
  self.shader_sources={}
  self.shader_source_calls=[]
  self.map_extra('Marker3',6)
  alias=LIB/'libSPenCxx.so'
  if not alias.exists(): alias.symlink_to(LIB/'libc++_shared.so')
  self.map_extra('Cxx',7)
  self.load_extra_relocs()
  self.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',None)
  self.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)',None)
  self.callbacks['operator new(unsigned long)']=lambda:self.x(0,self.alloc(self.x(0)))
  self.callbacks['posix_memalign']=self.posix_memalign
  self.callbacks['__cxa_throw']=self.capture_throw
  self.callbacks['malloc']=self.malloc_log
  self.callbacks['fopen']=self.fopen_miss
  self.callbacks['unlink']=self.unlink_miss
  self.callbacks['__strlen_chk']=self.strlen_checked
  self.callbacks['__strncpy_chk2']=self.strncpy_checked2
  self.callbacks['strcmp']=self.strcmp_native
  self.callbacks['strncmp']=self.strncmp_native
  self.callbacks['memcmp']=self.memcmp_native
  self.callbacks['memchr']=self.memchr_native
  self.callbacks['operator delete(void*)']=lambda:None
  no_op=['pthread_mutex_lock','pthread_mutex_unlock','pthread_mutex_init','pthread_mutex_destroy','pthread_mutexattr_init','pthread_mutexattr_destroy','pthread_mutexattr_settype','pthread_once','pthread_cond_broadcast','pthread_cond_signal','pthread_cond_init','pthread_cond_destroy']
  self.callbacks.update({name:(lambda:self.x(0,0)) for name in no_op})
  self.callbacks.update({'eglGetCurrentContext':lambda:self.x(0,0xabc), 'crc32':lambda:self.x(0,zlib.crc32(bytes(self.u.mem_read(self.x(1),self.x(2))),self.x(0))), 'vsnprintf':self.vsnprintf, 'glCreateShader':self.gl_create_shader, 'glCreateProgram':self.gl_create_program, 'glShaderSource':self.gl_shader_source, 'glCompileShader':lambda:self.gles.f['glCompileShader'](self.x(0)), 'glGetShaderiv':self.gl_get_shader_iv, 'glGetShaderInfoLog':self.gl_get_shader_log, 'glDeleteShader':lambda:self.gles.gl.glDeleteShader(self.x(0)), 'glAttachShader':lambda:self.gles.f['glAttachShader'](self.x(0),self.x(1)), 'glDetachShader':lambda:self.gles.gl.glDetachShader(self.x(0),self.x(1)), 'glBindAttribLocation':self.gl_bind_attrib, 'glLinkProgram':lambda:self.gles.f['glLinkProgram'](self.x(0)), 'glGetProgramiv':self.gl_get_program_iv, 'glGetProgramInfoLog':self.gl_get_program_log, 'glGetProgramBinary':self.gl_get_program_binary, 'glGetActiveUniform':self.gl_get_active, 'glGetActiveAttrib':self.gl_get_active, 'glGetAttribLocation':self.gl_get_attrib, 'glUseProgram':self.gl_use_program, 'glGetUniformLocation':self.gl_get_uniform, 'glUniformMatrix4fv':self.gl_uniform_matrix, 'glUniform4f':self.gl_uniform4f, 'glUniform4fv':self.gl_uniform4fv, 'glUniform1i':self.gl_uniform1i, 'glUniform1f':self.gl_uniform1f, 'glGenBuffers':self.gl_gen_buffers, 'glBindBuffer':self.gl_bind_buffer, 'glBufferData':self.gl_buffer_data, 'glEnableVertexAttribArray':lambda:self.gles.f['glEnableVertexAttribArray'](self.x(0)), 'glDisableVertexAttribArray':lambda:self.gles.gl.glDisableVertexAttribArray(self.x(0)), 'glVertexAttribPointer':self.gl_vertex_attrib_pointer, 'glVertexAttribDivisor':lambda:self.gles.f['glVertexAttribDivisor'](self.x(0),self.x(1)), 'glGenVertexArrays':self.gl_gen_vaos, 'glBindVertexArray':self.gl_bind_vertex_array, 'glGenFramebuffers':self.gl_gen_framebuffers, 'glDeleteFramebuffers':self.gl_delete_framebuffers, 'glBindFramebuffer':self.gl_bind_framebuffer, 'glFramebufferTexture2D':self.gl_framebuffer_texture, 'glCheckFramebufferStatus':self.gl_check_framebuffer, 'glGenTextures':self.gl_gen_textures, 'glBindTexture':self.gl_bind_texture, 'glBindSampler':self.gl_bind_sampler, 'glTexParameteri':lambda:self.gles.f['glTexParameteri'](self.x(0),self.x(1),self.x(2)), 'glActiveTexture':self.gl_active_texture, 'glTexImage2D':self.gl_tex_image_2d, 'glViewport':self.gl_viewport, 'glDisable':self.gl_disable, 'glEnable':self.gl_enable, 'glBlendEquationSeparate':self.gl_blend_equation, 'glBlendFuncSeparate':self.gl_blend_func, 'glColorMask':self.gl_color_mask, 'glClearColor':lambda:self.gles.f['glClearColor'](*[self.f(i) for i in range(4)]), 'glClear':lambda:self.gles.f['glClear'](self.x(0)), 'glGetBooleanv':self.gl_get_boolean, 'glGetIntegerv':self.gl_get_integer, 'glInvalidateFramebuffer':self.gl_invalidate_framebuffer, 'glDrawArrays':self.gl_draw_arrays, 'glDrawArraysInstanced':self.gl_draw_arrays_instanced})
  self.entries=[]
  self.native_samplers={}
  self.native_syncs={}
  self.native_logs=[]
  self.callbacks['glGenSamplers']=self.gl_gen_samplers
  self.callbacks['glSamplerParameteri']=self.gl_sampler_parameter
  self.callbacks['glFenceSync']=self.gl_fence_sync
  self.callbacks['glClientWaitSync']=self.gl_client_wait_sync
  self.callbacks['glWaitSync']=self.gl_wait_sync
  self.callbacks['glDeleteSync']=self.gl_delete_sync
  self.callbacks['glFlush']=lambda:self.gles.gl.glFlush()
  self.callbacks['glFinish']=lambda:self.gles.gl.glFinish()
  self.callbacks['__android_log_print']=self.log_print
  self.errno_ptr=self.alloc(4)
  self.callbacks['__errno']=lambda:self.x(0,self.errno_ptr)
  self.callbacks['getauxval']=self.getauxval
  self.callbacks['__system_property_get']=self.property_get
  self.locale_ptr=self.alloc(64); self.locale_name=self.alloc(2); self.u.mem_write(self.locale_name,b'C\0'); self.current_locale=0x7009200
  self.callbacks['newlocale']=self.newlocale
  self.callbacks['uselocale']=self.uselocale
  self.callbacks['freelocale']=lambda:self.x(0,0)
  self.callbacks['setlocale']=self.setlocale
  self.callbacks['mbtowc']=self.mbtowc
  self.patch_defined_abs64()
  self.run_initializers()
 def patch_defined_abs64(self):
  # Demangled destructor thunk names collide; apply defined ABS64 relocations
  # from exact dynsym st_value + load base + addend as the ELF loader does.
  for name,base in self.extra_bases.items():
   blob=(LIB/f'libSPen{name}.so').read_bytes(); shoff=struct.unpack_from('<Q',blob,40)[0]; es,count,stridx=struct.unpack_from('<HHH',blob,58)
   sections=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*es) for i in range(count)]; shstr=sections[stridx]; names=blob[shstr[4]:shstr[4]+shstr[5]]
   sec={names[s[0]:names.find(b'\0',s[0])].decode():s for s in sections}; dynsym=sec['.dynsym']; rel=sec.get('.rela.dyn')
   if not rel: continue
   syms=[struct.unpack_from('<IBBHQQ',blob,off) for off in range(dynsym[4],dynsym[4]+dynsym[5],dynsym[9])]
   for off in range(rel[4],rel[4]+rel[5],rel[9]):
    where,info,addend=struct.unpack_from('<QQq',blob,off); kind,index=info&0xffffffff,info>>32
    sym=syms[index]
    if kind==0x101 and sym[3]!=0: self.put(base+where,'Q',base+sym[4]+addend)
 @staticmethod
 def _max_cpu(factory,*args):
  u=factory(*args); u.ctl_set_cpu_model(__import__('unicorn').arm64_const.UC_CPU_ARM64_MAX); return u
 def map_extra(self,name,index):
  super().map_extra(name,index)
  base=self.extra_bases[name]
  path=LIB/f'libSPen{name}.so'; blob=path.read_bytes()
  shoff=struct.unpack_from('<Q',blob,40)[0]; es,count,stridx=struct.unpack_from('<HHH',blob,58)
  sections=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*es) for i in range(count)]
  shstr=sections[stridx]; names=blob[shstr[4]:shstr[4]+shstr[5]]
  by_name={names[s[0]:names.find(b'\0',s[0])].decode():s for s in sections}
  dynsym,dynstr,relsec=by_name['.dynsym'],by_name['.dynstr'],by_name['.rela.plt']
  strings=blob[dynstr[4]:dynstr[4]+dynstr[5]]; syms=[]
  for off in range(dynsym[4],dynsym[4]+dynsym[5],dynsym[9]):
   no,*_=struct.unpack_from('<IBBHQQ',blob,off); end=strings.find(b'\0',no); syms.append(strings[no:end].decode() if no else '')
  relnames={}
  for off in range(relsec[4],relsec[4]+relsec[5],relsec[9]):
   where,info,_add=struct.unpack_from('<QQq',blob,off)
   if info&0xffffffff==0x402: relnames[where]=syms[info>>32]
  env={**__import__('os').environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
  dis=subprocess.run([str(at.OBJ),'-d','-j','.plt',str(path)],capture_output=True,text=True,check=True,env=env).stdout
  # Remove objdump's ordinal labels; this Android PLT's labels do not match the GOT slots.
  for a in list(self.plt):
   if base+0x132000<=a<base+0x140000: self.plt.pop(a)
  pending=page=add=None
  for line in dis.splitlines():
   m=re.match(r'^\s*([0-9a-f]+):\s+[0-9a-f]+\s+adrp\s+x16,\s*([0-9a-f]+)',line)
   if m: pending,page,add=int(m.group(1),16),int(m.group(2),16),None; continue
   if pending is None: continue
   m=re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+add\s+x16, x16, #0x([0-9a-f]+)',line)
   if m: add=int(m.group(1),16); continue
   if re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+br\s+x17',line):
    raw=relnames.get(page+add) if add is not None else None
    if raw:
     dem=subprocess.run([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt'),'-n',raw],capture_output=True,text=True,check=True,env=env).stdout.strip()
     self.plt[base+pending]=dem
    pending=page=add=None
   elif re.match(r'^\s*[0-9a-f]+:',line) and not re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+(nop|ldr)\b',line): pending=page=add=None
 def run_initializers(self):
  bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'AirBrushPen':0x3000000,'Graphics':0x4000000,'Renderer':0x5000000,'Marker3':0x6000000,'Cxx':0x7000000}
  for name in ('Cxx','Base','PenCommon','FountainPen','AirBrushPen','Graphics','Renderer','Marker3'):
   path=LIB/('libc++_shared.so' if name=='Cxx' else f'libSPen{name}.so')
   blob=path.read_bytes(); shoff=struct.unpack_from('<Q',blob,40)[0]; es,count,stridx=struct.unpack_from('<HHH',blob,58)
   sections=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*es) for i in range(count)]
   shstr=sections[stridx]; names=blob[shstr[4]:shstr[4]+shstr[5]]
   init=next((s for s in sections if names[s[0]:names.find(b'\0',s[0])]==b'.init_array'),None)
   if init is None: continue
   called=0; start=bases[name]+init[3]
   for off in range(0,init[5],8):
    ptr=self.get(start+off,'Q')[0]
    if not ptr: continue
    self.x(0,0); self.x(1,0); self.x(2,0); self.x(3,0)
    self.u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_SP,0x200f0000)
    self.u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_LR,0x200f1000)
    try: self.u.emu_start(ptr,0x200f1000,count=3000000)
    except Exception as e: raise RuntimeError(f'{name} initializer {ptr:#x} failed at pc={self.u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_PC):#x}: {e}') from e
    if self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_PC)!=0x200f1000:
     raise RuntimeError(f'{name} init_array budget/return failure at {ptr:#x}, pc={self.u.reg_read(__import__("unicorn").arm64_const.UC_ARM64_REG_PC):#x}')
    called+=1
 def gl_create_shader(self): self.x(0,self.gles.f['glCreateShader'](self.x(0)))
 def posix_memalign(self):
  out,alignment,size=self.x(0),self.x(1),self.x(2)
  ptr=self.alloc(size+alignment); ptr=(ptr+alignment-1)&~(alignment-1)
  self.put(out,'Q',ptr); self.x(0,0)
 def capture_throw(self):
  obj,ti,dtor=self.x(0),self.x(1),self.x(2)
  nameptr=self.get(ti+8,'Q')[0] if ti else 0
  name=self.cstr(nameptr,256).decode(errors='replace') if nameptr else '<null>'
  raise RuntimeError(f'native __cxa_throw object={obj:#x} typeinfo={ti:#x} name={name!r} dtor={dtor:#x} logs={self.native_logs!r}')
 def log_print(self):
  vals=[self.x(i) for i in range(8)]; sp=self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_SP)
  self.native_logs.append({'args':[hex(v) for v in vals],'format':self.cstr(vals[2],512).decode(errors='replace'),'stack':[hex(v) for v in self.get(sp,'4Q')]})
  self.x(0,0)
 def call_stack_args(self,name,x,stack_args,instruction_limit=5000000):
  for i,v in enumerate(x): self.x(i,v)
  sp=0x200f0000; self.u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_SP,sp)
  self.u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_LR,0x200f1000)
  for i,v in enumerate(stack_args): self.put(sp+i*8,'Q',v)
  self.u.emu_start(self.syms[name],0x200f1000,count=instruction_limit)
  if self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_PC)!=0x200f1000: raise RuntimeError(f'instruction limit {name}')
 def malloc_log(self):
  n=self.x(0); self.x(0,self.alloc(n))
 def getauxval(self):
  key=self.x(0); value={6:4096,16:256,17:100,23:0,26:0,51:0}.get(key,0)
  if key not in (6,16,17,23,26,51): self.put(self.errno_ptr,'i',2)
  self.x(0,value)
 def property_get(self):
  key=self.cstr(self.x(0),128); value=b'arm64-v8a' if key==b'ro.arch' else b''
  self.u.mem_write(self.x(1),value+b'\0'); self.x(0,len(value))
 def newlocale(self): self.x(0,self.locale_ptr)
 def uselocale(self):
  old=self.current_locale
  if self.x(0): self.current_locale=self.x(0)
  self.x(0,old)
 def setlocale(self): self.x(0,self.locale_name)
 def mbtowc(self):
  out,src,n=self.x(0),self.x(1),self.x(2)
  if not src or not n: self.x(0,(1<<64)-1); return
  b=self.u.mem_read(src,1)[0]
  if b>=0x80: self.x(0,(1<<64)-1); return
  if out: self.put(out,'I',b)
  self.x(0,0 if b==0 else 1)
 def fopen_miss(self):
  self.put(self.errno_ptr,'i',2)
  self.x(0,0)
 def unlink_miss(self):
  self.put(self.errno_ptr,'i',2); self.x(0,-1)
 def strlen_checked(self): self.x(0,len(self.cstr(self.x(0),self.x(1))))
 def strncpy_checked2(self):
  dst,src,n,dst_size,src_size=[self.x(i) for i in range(5)]
  if n>dst_size or n>src_size: raise RuntimeError(f'fortified strncpy bounds: n={n} dst={dst_size} src={src_size}')
  raw=bytes(self.u.mem_read(src,n)); nul=raw.find(b'\0')
  if nul>=0: raw=raw[:nul]+bytes(n-nul)
  self.u.mem_write(dst,raw); self.x(0,dst)
 def strcmp_native(self):
  a,b=self.cstr(self.x(0)),self.cstr(self.x(1)); self.x(0,(a>b)-(a<b))
 def strncmp_native(self):
  n=self.x(2); a=bytes(self.u.mem_read(self.x(0),n)); b=bytes(self.u.mem_read(self.x(1),n)); self.x(0,(a>b)-(a<b))
 def memcmp_native(self):
  n=self.x(2); a=bytes(self.u.mem_read(self.x(0),n)); b=bytes(self.u.mem_read(self.x(1),n)); self.x(0,(a>b)-(a<b))
 def memchr_native(self):
  ptr,value,n=self.x(0),self.x(1)&255,self.x(2); raw=bytes(self.u.mem_read(ptr,n)); i=raw.find(bytes([value])); self.x(0,ptr+i if i>=0 else 0)
 def gl_create_program(self): self.x(0,self.gles.f['glCreateProgram']())
 def cstr(self,p,n=4096):
  raw=bytearray()
  while len(raw)<n:
   b=bytes(self.u.mem_read(p+len(raw),1))
   if not b or b==b'\0': break
   raw+=b
  if len(raw)==n: raise RuntimeError('C string bound exceeded')
  return bytes(raw)
 def gl_shader_source(self):
  shader,count,arr,lens=[self.x(i) for i in range(4)]
  raws=[]; keep=[]; ptrs=[]; lengths=[]
  for i in range(count):
   p=struct.unpack('<Q',bytes(self.u.mem_read(arr+8*i,8)))[0]
   size=struct.unpack('<i',bytes(self.u.mem_read(lens+4*i,4)))[0] if lens else -1
   raw=self.cstr(p) if size < 0 else bytes(self.u.mem_read(p,size))
   buf=C.create_string_buffer(raw); keep.append(buf); ptrs.append(C.cast(buf,C.c_char_p)); lengths.append(len(raw)); raws.append(raw)
  self.shader_sources[str(shader)]=[raw.decode('utf-8') for raw in raws]
  self.shader_source_calls.append({'shader':shader,'source':[raw.decode('utf-8') for raw in raws]})
  arrc=(C.c_char_p*len(ptrs))(*ptrs); lensc=(C.c_int*len(lengths))(*lengths)
  self.gles.f['glShaderSource'](shader,count,arrc,lensc)
 def _get_iv(self,fn,obj,pname,outptr):
  out=C.c_int(); fn(obj,pname,C.byref(out)); self.put(outptr,'i',out.value)
 def gl_get_shader_iv(self): self._get_iv(self.gles.f['glGetShaderiv'],self.x(0),self.x(1),self.x(2))
 def gl_get_program_iv(self): self._get_iv(self.gles.f['glGetProgramiv'],self.x(0),self.x(1),self.x(2))
 def _get_log(self,fn,obj,size,lenptr,bufptr):
  buf=C.create_string_buffer(size); out=C.c_int(); fn(obj,size,C.byref(out),buf)
  self.put(lenptr,'i',out.value); self.u.mem_write(bufptr,buf.raw)
 def gl_get_shader_log(self): self._get_log(self.gles.f['glGetShaderInfoLog'],self.x(0),self.x(1),self.x(2),self.x(3))
 def gl_get_program_log(self): self._get_log(self.gles.f['glGetProgramInfoLog'],self.x(0),self.x(1),self.x(2),self.x(3))
 def gl_bind_attrib(self): self.gles.f['glBindAttribLocation'](self.x(0),self.x(1),self.cstr(self.x(2)))
 def gl_get_uniform(self): self.x(0,self.gles.f['glGetUniformLocation'](self.x(0),self.cstr(self.x(1))))
 def gl_get_active(self):
  fn=getattr(self.gles.gl,'glGetActiveAttrib' if self.current_plt_symbol=='glGetActiveAttrib' else 'glGetActiveUniform')
  fn.argtypes=[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p]
  length,size,typ=C.c_int(),C.c_int(),C.c_uint(); buf=C.create_string_buffer(self.x(2))
  fn(self.x(0),self.x(1),self.x(2),C.byref(length),C.byref(size),C.byref(typ),buf)
  self.put(self.x(3),'i',length.value); self.put(self.x(4),'i',size.value); self.put(self.x(5),'I',typ.value); self.u.mem_write(self.x(6),buf.raw)
 def gl_get_boolean(self):
  fn=self.gles.gl.glGetBooleanv; fn.argtypes=[C.c_uint,C.POINTER(C.c_ubyte)]; out=C.c_ubyte(); fn(self.x(0),C.byref(out)); self.u.mem_write(self.x(1),bytes([out.value]))
 def gl_get_integer(self):
  pname=self.x(0); count=4 if pname in (0x0BA2,0x0C10) else 1
  fn=self.gles.gl.glGetIntegerv; fn.argtypes=[C.c_uint,C.POINTER(C.c_int)]; out=(C.c_int*count)(); fn(pname,out); self.u.mem_write(self.x(1),bytes(out))
 def gl_get_attrib(self): self.x(0,self.gles.gl.glGetAttribLocation(self.x(0),self.cstr(self.x(1))))
 def gl_get_program_binary(self):
  addr=self.gles.egl.eglGetProcAddress(b'glGetProgramBinary')
  if not addr: raise RuntimeError('host GLES lacks glGetProgramBinary')
  fn=C.CFUNCTYPE(None,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p)(addr)
  program,size,lp,fp,bp=[self.x(i) for i in range(5)]; buf=C.create_string_buffer(size); length=C.c_int(); fmt=C.c_uint()
  fn(program,size,C.byref(length),C.byref(fmt),buf)
  if lp: self.put(lp,'i',length.value)
  if fp: self.put(fp,'I',fmt.value)
  if bp and length.value: self.u.mem_write(bp,buf.raw[:length.value])
 def vsnprintf(self):
  dst,size=self.x(0),self.x(1)
  if size: self.u.mem_write(dst,b'\0')
  self.x(0,0)
 def gl_gen_buffers(self):
  n,out=self.x(0),self.x(1); vals=(C.c_uint*n)(); self.gles.f['glGenBuffers'](n,vals)
  for i,v in enumerate(vals): self.put(out+i*4,'I',v)
 def _gl_gen(self,key,count,out):
  vals=(C.c_uint*count)(); self.gles.f[key](count,vals)
  for i,v in enumerate(vals): self.put(out+i*4,'I',v)
 def gl_gen_framebuffers(self): self._gl_gen('glGenFramebuffers',self.x(0),self.x(1))
 def gl_delete_framebuffers(self):
  n,ptr=self.x(0),self.x(1); ids=list(self.get(ptr,f'{n}I')); delete=ip.cfunc(self.gles.gl,'glDeleteFramebuffers',None,[C.c_int,C.POINTER(C.c_uint)]); array=(C.c_uint*n)(*ids); delete(n,array); self.gl_calls.append({'call':'glDeleteFramebuffers','ids':ids})
 def gl_gen_textures(self): self._gl_gen('glGenTextures',self.x(0),self.x(1))
 def gl_tex_image_2d(self):
  target,level,internal,w,h,border,fmt,typ=[self.x(i) for i in range(8)]
  sp=self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_SP)
  ptr=self.get(sp,'Q')[0]
  channels={0x1903:1,0x1906:1,0x1908:4,0x80e1:4}.get(fmt,4); raw=bytes(self.u.mem_read(ptr,w*h*channels)) if ptr else b''
  buf=C.create_string_buffer(raw) if raw else None
  self.gl_calls.append({'call':'glTexImage2D','target':target,'internalFormat':internal,'width':w,'height':h,'format':fmt,'type':typ,'hasPixels':bool(ptr),'pixelBytes':len(raw),'pixelsSha256':hashlib.sha256(raw).hexdigest() if raw else None})
  self.gles.f['glTexImage2D'](target,level,internal,w,h,border,fmt,typ,C.cast(buf,C.c_void_p) if buf else None)
 def gl_invalidate_framebuffer(self):
  target,n,ptr=self.x(0),self.x(1),self.x(2); vals=(C.c_uint*n)(*self.get(ptr,'I'*n))
  fn=self.gles.gl.glInvalidateFramebuffer; fn.argtypes=[C.c_uint,C.c_int,C.POINTER(C.c_uint)]; fn(target,n,vals)
 def gl_gen_vaos(self):
  n,out=self.x(0),self.x(1); vals=(C.c_uint*n)(); self.gles.f['glGenVertexArrays'](n,vals)
  for i,v in enumerate(vals): self.put(out+i*4,'I',v)
 def gl_buffer_data(self):
  target,size,ptr,usage=[self.x(i) for i in range(4)]; raw=bytes(self.u.mem_read(ptr,size)) if ptr and size else b''
  buf=C.create_string_buffer(raw) if raw else None
  self.gl_calls.append({'call':'glBufferData','target':target,'size':size,'usage':usage,'dataSha256':hashlib.sha256(raw).hexdigest() if raw else None})
  self.gles.f['glBufferData'](target,size,C.cast(buf,C.c_void_p) if buf else None,usage)
 def gl_use_program(self): self.gl_calls.append({'call':'glUseProgram','program':self.x(0)}); self.gles.f['glUseProgram'](self.x(0))
 def gl_bind_buffer(self): self.gl_calls.append({'call':'glBindBuffer','target':self.x(0),'buffer':self.x(1)}); self.gles.f['glBindBuffer'](self.x(0),self.x(1))
 def gl_uniform_matrix(self):
  loc,n,transpose,ptr=[self.x(i) for i in range(4)]; vals=self.get(ptr,'f'*(16*n)); self.gl_calls.append({'call':'glUniformMatrix4fv','location':loc,'values':vals})
  arr=(C.c_float*len(vals))(*vals); self.gles.f['glUniformMatrix4fv'](loc,n,transpose,C.cast(arr,C.POINTER(C.c_float)))
 def gl_uniform4f(self):
  vals=[self.f(i) for i in range(4)]; self.gl_calls.append({'call':'glUniform4f','location':self.x(0),'values':vals}); self.gles.f['glUniform4f'](self.x(0),*vals)
 def gl_uniform4fv(self):
  loc,n,ptr=self.x(0),self.x(1),self.x(2); vals=self.get(ptr,'f'*(n*4)); self.gl_calls.append({'call':'glUniform4fv','location':loc,'values':vals})
  fn=self.gles.gl.glUniform4fv; fn.argtypes=[C.c_int,C.c_int,C.POINTER(C.c_float)]; arr=(C.c_float*len(vals))(*vals); fn(loc,n,arr)
 def gl_uniform1i(self): self.gl_calls.append({'call':'glUniform1i','location':self.x(0),'value':self.x(1)}); self.gles.f['glUniform1i'](self.x(0),self.x(1))
 def gl_uniform1f(self):
  fn=self.gles.gl.glUniform1f; fn.argtypes=[C.c_int,C.c_float]; loc,value=self.x(0),self.f(0); self.gl_calls.append({'call':'glUniform1f','location':loc,'value':value}); fn(loc,value)
 def gl_viewport(self):
  vals=[self.x(i) for i in range(4)]; self.gl_calls.append({'call':'glViewport','values':vals}); self.gles.f['glViewport'](*vals)
 def gl_disable(self): self.gl_calls.append({'call':'glDisable','cap':self.x(0)}); self.gles.f['glDisable'](self.x(0))
 def gl_enable(self): self.gl_calls.append({'call':'glEnable','cap':self.x(0)}); self.gles.f['glEnable'](self.x(0))
 def gl_blend_equation(self):
  vals=[self.x(i) for i in range(2)]; self.gl_calls.append({'call':'glBlendEquationSeparate','values':vals}); self.gles.f['glBlendEquationSeparate'](*vals)
 def gl_blend_func(self):
  vals=[self.x(i) for i in range(4)]; self.gl_calls.append({'call':'glBlendFuncSeparate','values':vals}); self.gles.f['glBlendFuncSeparate'](*vals)
 def gl_color_mask(self):
  vals=[self.x(i) for i in range(4)]; self.gl_calls.append({'call':'glColorMask','values':vals}); self.gles.gl.glColorMask(*[bool(v) for v in vals])
 def gl_draw_arrays(self):
  vals=[self.x(i) for i in range(3)]; self.gl_calls.append({'call':'glDrawArrays','values':vals,'state':self.gl_state()}); self.gles.f['glDrawArrays'](*vals)
 def gl_draw_arrays_instanced(self):
  vals=[self.x(i) for i in range(4)]; self.gl_calls.append({'call':'glDrawArraysInstanced','values':vals,'state':self.gl_state()}); self.gles.f['glDrawArraysInstanced'](*vals); red=C.create_string_buffer(128*32); read=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(self.gles.gl.glReadPixels); read(0,0,128,32,0x1903,0x1401,red); self.gl_calls.append({'call':'maskReadback','nonzero':sum(1 for b in red.raw if b)})
 def gl_bind_vertex_array(self):
  self.gl_calls.append({'call':'glBindVertexArray','id':self.x(0)}); self.gles.f['glBindVertexArray'](self.x(0))
 def gl_bind_framebuffer(self):
  self.gl_calls.append({'call':'glBindFramebuffer','target':self.x(0),'id':self.x(1)}); self.gles.f['glBindFramebuffer'](self.x(0),self.x(1))
 def gl_framebuffer_texture(self):
  values=[self.x(i) for i in range(5)]; self.gl_calls.append({'call':'glFramebufferTexture2D','framebuffer':self.gl_state()['framebuffer'],'args':values}); self.gles.f['glFramebufferTexture2D'](*values)
 def gl_check_framebuffer(self):
  target=self.x(0); status=self.gles.f['glCheckFramebufferStatus'](target); self.gl_calls.append({'call':'glCheckFramebufferStatus','target':target,'framebuffer':self.gl_state()['framebuffer'],'status':status}); self.x(0,status)
 def gl_bind_texture(self):
  self.gl_calls.append({'call':'glBindTexture','target':self.x(0),'id':self.x(1)}); self.gles.f['glBindTexture'](self.x(0),self.x(1))
 def gl_active_texture(self):
  self.gl_calls.append({'call':'glActiveTexture','unit':self.x(0)}); self.gles.f['glActiveTexture'](self.x(0))
 def gl_state(self):
  out={}; fn=self.gles.gl.glGetIntegerv; fn.argtypes=[C.c_uint,C.POINTER(C.c_int)]
  for label,pname in [('program',0x8B8D),('vao',0x85B5),('framebuffer',0x8CA6),('activeTexture',0x84E0),('texture2d',0x8069)]:
   v=C.c_int(); fn(pname,C.byref(v)); out[label]=v.value
  return out
 def gl_vertex_attrib_pointer(self):
  f=self.gles.f['glVertexAttribPointer']; f(self.x(0),self.x(1),self.x(2),self.x(3),self.x(4),C.c_void_p(self.x(5)))
 def gl_bind_sampler(self):
  fn=self.gles.egl.eglGetProcAddress(b'glBindSampler')
  if not fn: raise RuntimeError('host GLES lacks glBindSampler')
  unit,sampler=self.x(0),self.x(1)
  if sampler not in self.native_samplers and sampler != 0: raise RuntimeError(f'GL sampler {sampler} was bound before native glGenSamplers')
  C.CFUNCTYPE(None,C.c_uint,C.c_uint)(fn)(unit,self.native_samplers.get(sampler,0)); self.gl_calls.append({'call':'glBindSampler','unit':unit,'nativeSampler':sampler,'mesaSampler':self.native_samplers.get(sampler,0)})
 def gl_gen_samplers(self):
  count,out=self.x(0),self.x(1)
  if not 0<count<=8: raise RuntimeError(f'bounded native glGenSamplers count={count}')
  vals=(C.c_uint*count)(); fn=self.gles.egl.eglGetProcAddress(b'glGenSamplers')
  if not fn: raise RuntimeError('host GLES lacks glGenSamplers')
  C.CFUNCTYPE(None,C.c_int,C.POINTER(C.c_uint))(fn)(count,vals)
  native=list(range(self.next_gl_id,self.next_gl_id+count)); self.next_gl_id+=count
  for i,(nid,mid) in enumerate(zip(native,vals)):
   self.native_samplers[nid]=mid; self.put(out+4*i,'I',nid)
  self.gl_calls.append({'call':'glGenSamplers','native':native,'mesa':list(vals)})
 def gl_sampler_parameter(self):
  native,pname,value=self.x(0),self.x(1),self.x(2)
  if native not in self.native_samplers: raise RuntimeError(f'glSamplerParameteri unknown native sampler {native}')
  fn=self.gles.egl.eglGetProcAddress(b'glSamplerParameteri')
  if not fn: raise RuntimeError('host GLES lacks glSamplerParameteri')
  C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.c_int)(fn)(self.native_samplers[native],pname,value)
  self.gl_calls.append({'call':'glSamplerParameteri','nativeSampler':native,'mesaSampler':self.native_samplers[native],'pname':pname,'value':value})
 def _sync_proc(self,name,restype,args):
  addr=self.gles.egl.eglGetProcAddress(name.encode())
  if not addr: raise RuntimeError(f'host GLES lacks {name}')
  return C.CFUNCTYPE(restype,*args)(addr)
 def gl_fence_sync(self):
  condition,flags=self.x(0),self.x(1); host=self._sync_proc('glFenceSync',C.c_void_p,[C.c_uint,C.c_uint])(condition,flags)
  if not host: raise RuntimeError(f'host glFenceSync failed: {self.gles.f["glGetError"]():#x}')
  token=self.alloc(8); self.native_syncs[token]=host; self.x(0,token); self.gl_calls.append({'call':'glFenceSync','condition':condition,'flags':flags,'nativeToken':token})
 def gl_client_wait_sync(self):
  token,flags,timeout=self.x(0),self.x(1),self.x(2)
  if token not in self.native_syncs: raise RuntimeError(f'unknown native sync token {token:#x}')
  result=self._sync_proc('glClientWaitSync',C.c_uint,[C.c_void_p,C.c_uint,C.c_uint64])(self.native_syncs[token],flags,timeout)
  self.x(0,result); self.gl_calls.append({'call':'glClientWaitSync','token':token,'result':result})
 def gl_wait_sync(self):
  token,flags,timeout=self.x(0),self.x(1),self.x(2)
  if token not in self.native_syncs: raise RuntimeError(f'unknown native sync token {token:#x}')
  self._sync_proc('glWaitSync',None,[C.c_void_p,C.c_uint,C.c_uint64])(self.native_syncs[token],flags,timeout)
  self.gl_calls.append({'call':'glWaitSync','token':token})
 def gl_delete_sync(self):
  token=self.x(0)
  if token not in self.native_syncs: raise RuntimeError(f'unknown native sync token {token:#x}')
  self._sync_proc('glDeleteSync',None,[C.c_void_p])(self.native_syncs.pop(token)); self.x(0,0); self.gl_calls.append({'call':'glDeleteSync','token':token})
 def hook(self,u,a,size,data):
  if a==getattr(self,'queue_managed',None):
   self.x(0,self.managed_queue); u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if a==getattr(self,'managed_register',None):
   manager,obj=self.x(0),self.x(1); token=self.alloc(8); self.put(token,'Q',obj); self.managed_tokens[token]=obj
   self.managed_events.append({'method':'Register','manager':hex(manager),'object':hex(obj),'token':hex(token)})
   self.x(0,token); u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  if a==getattr(self,'managed_unregister',None):
   manager,token=self.x(0),self.x(1); obj=self.managed_tokens.pop(token,None)
   self.managed_events.append({'method':'Unregister','manager':hex(manager),'object':None if obj is None else hex(obj),'token':hex(token)})
   self.x(0,0); u.reg_write(__import__('unicorn').arm64_const.UC_ARM64_REG_PC,u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_LR)); return
  self.current_plt_symbol=self.plt.get(a,'')
  if self.current_plt_symbol.startswith('gl') and self.current_plt_symbol!='glGetError':
   prior=self.gles.f['glGetError']()
   if prior: self.gl_calls.append({'call':'pendingGLErrorBefore','next':self.current_plt_symbol,'error':prior})
  for n in ('SPen::Marker3::GetStrokeDrawableGL()','SPen::Marker3StrokeDrawableGLV5::Marker3StrokeDrawableGLV5(SPen::Marker3Data*, SPen::PenGLDataManager*)','SPen::Marker3StrokeDrawableRTV1::Marker3StrokeDrawableRTV1(SPen::PenGLDataManager*)'):
   if a==self.syms.get(n): self.entries.append(n)
  super().hook(u,a,size,data)
  if self.current_plt_symbol.startswith('gl') and self.current_plt_symbol!='glGetError':
   error=self.gles.f['glGetError']()
   self.gl_calls.append({'call':'errorAfter:'+self.current_plt_symbol,'error':error})
p=P()
queue=p.alloc(32); queue_vtable=p.alloc(64); p.queue_post=p.alloc(16); p.queue_task_return=p.alloc(16); p.pending_queue_return=None
p.queue_managed=p.alloc(16); p.managed_queue=p.alloc(32); p.managed_queue_vtable=p.alloc(64); p.managed_register=p.alloc(16); p.managed_unregister=p.alloc(16); p.managed_tokens={}; p.managed_events=[]
typeinfo=p.syms.get('typeinfo for SPen::IGLManagedMsgQueue')
if not typeinfo: raise RuntimeError('pinned Renderer lacks IGLManagedMsgQueue typeinfo')
p.put(queue,'Q',queue_vtable); p.put(queue_vtable-16,'qQ',0,typeinfo); p.put(queue_vtable+16,'Q',p.queue_post); p.put(queue_vtable+56,'Q',p.queue_managed)
p.put(p.managed_queue,'Q',p.managed_queue_vtable); p.put(p.managed_queue_vtable+16,'Q',p.managed_register); p.put(p.managed_queue_vtable+24,'Q',p.managed_unregister); p.queue=queue
obj=p.alloc(512)
manager=p.alloc(256)
p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager])
p.call('SPen::Marker3::Marker3()', [obj])
# Native Marker3 construction supplies the real V5 pen-data backing object.
p.put(obj+8,'Q',manager)
# GL factory reads cached drawable at +112 and manager at +8; null manager is allowed by ctor.
p.put(obj+112,'Q',0)
try:
 p.call('SPen::Marker3::GetStrokeDrawableGL()', [obj], instruction_limit=1000000)
 rt=p.get(p.x(0)+16,'Q')[0]
 shader=p.alloc(64)
 p.call('SPen::Marker3MaskShader::Marker3MaskShader()', [shader], instruction_limit=3000000)
 p.call('SPen::Marker3StrokeDrawableRTV1::Init()', [rt], instruction_limit=5000000)
 p.call('SPen::Marker3StrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)', [rt,128,32,p.queue], instruction_limit=5000000)
 bmp=p.get(rt+200,'Q')[0]; p.call('SPen::SPPenCanvasRT::GetBitmap()',[bmp]); subbmp=p.x(0)
 p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[subbmp,0]); subbitmap=p.x(0)
 vector=p.alloc(24); points=p.alloc(4096); p.put(vector,'3Q',points,points,points+4096)
 p.call('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,vector,0])
 for x,y in ((24.0,16.0),(64.0,16.0),(104.0,16.0)):
  p.call('SPen::Marker3StrokeDrawableRTV1::AddPoint(SPen::Vector2<float>)',[rt],[x,y])
 p.call('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',[rt,vector,0])
 p.call('SPen::PenDrawableRT::SetPenData(float, int)',[rt,0xff336699],[12.0])
 p.call('SPen::Marker3StrokeDrawableRTV1::Update(bool)',[rt,0],instruction_limit=5000000)
 matrix=p.alloc(64); p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
 # Native general RGBA bitmap plus actual CanvasImpl construction creates a
 # separate color-render target and attaches it to the bitmap.
 fmt=p.alloc(8); p.u.mem_write(fmt,b'RGBA\0')
 initial=p.alloc(128*32*4); p.u.mem_write(initial,bytes(128*32*4))
 factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
 # TextureMemoryLayout 6 selects RGBA8 and TextureTargetLayout 2 selects
 # RGBA upload format. Both enums are stack-passed by the factory ABI.
 p.call_stack_args(factory,[p.queue,128,32,fmt,initial,1,512,1],[6,2])
 output_bitmap=p.x(0)
 if not output_bitmap: raise RuntimeError('actual general RGBA CreateBitmap returned null')
 canvas=p.alloc(512)
 p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,p.queue,output_bitmap,1],instruction_limit=5000000)
 p.call('SPen::SPCanvasImpl::GetBitmap()',[canvas])
 bitmap=p.x(0)
 p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0])
 bitmap=p.x(0)
 p.call('SPen::Marker3StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,matrix],instruction_limit=5000000)
 rgba=C.create_string_buffer(128*32*4); read=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)
 read(0,0,128,32,0x1908,0x1401,rgba)
 pixels=bytes(rgba)
 gl_error=p.gles.f['glGetError']()
 samples=[{'xy':[x,y],'rgba':list(pixels[(y*128+x)*4:(y*128+x+1)*4])} for x,y in ((24,16),(29,16),(30,16),(31,16),(64,16),(104,16),(0,0))]
 if gl_error: raise RuntimeError(f'Mesa GL error after native Draw/readback: {gl_error:#x}')
 if not any(c['call']=='glDrawArraysInstanced' for c in p.gl_calls) or not any(c['call']=='glDrawArrays' for c in p.gl_calls): raise RuntimeError('native Marker3 did not execute both mask and composite draws')
 errors=[c for c in p.gl_calls if c['call'].startswith('errorAfter:') and c['error']!=0]
 pending=[c for c in p.gl_calls if c['call']=='pendingGLErrorBefore']
 if errors or pending: raise RuntimeError(f'GL errors across native calls: after={errors!r} pending={pending!r}')
 allocations=[c for c in p.gl_calls if c['call']=='glTexImage2D']
 attachments=[c for c in p.gl_calls if c['call']=='glFramebufferTexture2D']
 mask_draw=next(c for c in p.gl_calls if c['call']=='glDrawArraysInstanced')
 composite_draw=next(c for c in p.gl_calls if c['call']=='glDrawArrays')
 mask_attachment=next(c for c in attachments if c['framebuffer']==mask_draw['state']['framebuffer'])
 color_attachment=next(c for c in attachments if c['framebuffer']==composite_draw['state']['framebuffer'])
 if len(allocations)!=2 or allocations[1]['internalFormat']!=0x8058 or allocations[1]['format']!=0x1908:
  raise RuntimeError(f'external output target is not native RGBA8/RGBA: {allocations!r}')
 if mask_attachment['args'][3]==color_attachment['args'][3] or mask_draw['state']['framebuffer']==composite_draw['state']['framebuffer']:
  raise RuntimeError('mask and output target alias')
 if composite_draw['state']['texture2d']!=mask_attachment['args'][3]:
  raise RuntimeError('composite sampler does not reference the rendered Marker3 mask')
 complete={c['framebuffer'] for c in p.gl_calls if c['call']=='glCheckFramebufferStatus' and c['status']==0x8cd5}
 if mask_attachment['framebuffer'] not in complete or color_attachment['framebuffer'] not in complete:
  raise RuntimeError('native mask or external color target FBO is not complete')
 gl_calls=[]
 for call in p.gl_calls:
  item=dict(call)
  if item['call']=='glFenceSync': item.pop('nativeToken',None)
  gl_calls.append(item)
 fixture={'schema':'sdocx-marker3-v5-rtv1-paint-positive-native-v1','source':SOURCE_HASHES,
  'evidence_scope':'Pinned Marker3 V5 factory selected RTV1. Native RTV1 Init, CreatePenCanvas, actual SPTextureBitmap::GetSubBitmap(0), SetBuffer, three native AddPoint calls, SendDataToGPU, Update(false), and Draw execute. Embedded mask/composite shaders and actual Renderer GL state/draw calls execute in host Mesa GLES; native handles are mapped to host Mesa objects. Synthetic direct point stream is injected at RT AddPoint, rather than produced by saved ObjectStroke replay. Native PenDrawableRT::SetPenData(float,int) on the selected RTV1 is executed and its emitted color uniforms are captured. The native factory uses TextureMemoryLayout 6 and TextureTargetLayout 2 to allocate the independent RGBA8 output target. Runtime checks prove mask and output FBO/texture IDs are distinct, the composite samples the mask texture, both FBOs are complete, and each wrapped GL call ends without an error. Full readback bytes are summarized by byte length and SHA-256; sample values are observational host Mesa output, not Samsung-device parity.',
  'host_gles':p.gles.identity,'input':{'canvas':[128,32],'points':[[24.0,16.0],[64.0,16.0],[104.0,16.0]],'rt_size':12,'matrix':'identity','point_source':'direct synthetic calls to Marker3StrokeDrawableRTV1::AddPoint','saved_object_stroke_replay':False,'color_state':{'mode':'native PenDrawableRT::SetPenData(float,int)','argb':'0xff336699','size':12.0},'output_texture_memory_layout':6,'output_texture_target_layout':2},
  'topology':{'mask_attachment':{'framebuffer':mask_attachment['framebuffer'],'texture':mask_attachment['args'][3]},'color_attachment':{'framebuffer':color_attachment['framebuffer'],'texture':color_attachment['args'][3]},'mask_draw_state':mask_draw['state'],'composite_draw_state':composite_draw['state'],'output_format':{'internal':allocations[1]['internalFormat'],'format':allocations[1]['format'],'width':allocations[1]['width'],'height':allocations[1]['height']}},
  'native_vector_bytes':p.get(vector+8,'Q')[0]-p.get(vector,'Q')[0],
  'host_boundaries':['PenGLDataManager is assigned to the native-created Pen by the harness','Queue tasks run synchronously; managed registration uses host token bookkeeping','libc/locale/thread primitives are bounded host services','Native GLES calls are forwarded to Mesa; no Android device or destruction lifecycle is captured'],
  'native_color_rgba':list(p.get(rt+44,'4f')),
  'runtime_shader_source_calls':p.shader_source_calls,'gl_calls':gl_calls,'readback':{'bytes':len(pixels),'sha256':hashlib.sha256(pixels).hexdigest(),'samples':samples},'gl_error':gl_error}
 encoded=json.dumps(fixture,indent=2)+'\n'
 if ARGS.write_fixture:
  ARGS.fixture.write_text(encoded)
 elif ARGS.fixture.read_text()!=encoded:
  raise SystemExit('Marker3 paint fixture differs; rerun with --write-fixture to replace it')
except Exception as e:
 R=__import__('unicorn').arm64_const
 print(type(e).__name__,e,'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)), 'x0-x7',[hex(p.x(i)) for i in range(8)])
 raise
