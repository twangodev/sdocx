#!/usr/bin/env python3
"""Bounded native RGBA bitmap readback and PNG conversion conformance capture.

From the repository root, no arguments checks the fixture; --write-fixture regenerates it.
"""
import argparse
import ctypes
import ctypes as C
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import zlib
from pathlib import Path
from unicorn import UC_HOOK_CODE

ROOT = Path('/home/jding/RustroverProjects/sdocx')
FIXTURE = ROOT / 'conformance/pdf-image-rgba-native.json'
LIBZ_PATH = Path('/usr/lib/x86_64-linux-gnu/libz.so.1')
LIBZ_SHA256 = '85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e'
PINNED_HOST_GL_DSOS = {
    "/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0": "f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6",
    "/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0": "a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812",
    "/usr/lib/x86_64-linux-gnu/libEGL_mesa.so.0.0.0": "af1229604d80d73422ff3767f572849dce074a0ecdba8254e93a06a88cfec2e6",
    "/usr/lib/x86_64-linux-gnu/libGLdispatch.so.0.0.0": "890baeef03fd73560a2fde85ebe9fb49ca3e90686923c5ee22101b28ec8ee7aa",
    "/usr/lib/x86_64-linux-gnu/libLLVM.so.19.1": "3523f50f635d2a1ea47518a392451f2854b92823ceaf1345fb806099dd1a3b8a",
}
for path, expected_hash in PINNED_HOST_GL_DSOS.items():
    actual_hash = hashlib.sha256(Path(path).read_bytes()).hexdigest()
    if actual_hash != expected_hash:
        raise RuntimeError(f"pinned Mesa host DSO hash mismatch: {path}: {actual_hash}")

LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
NM = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm'
OBJ = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
sys.path.insert(0, str(ROOT / 'conformance'))
import airbrush_saved_native as air
import inkpen_paint_native as ip
from unicorn import arm64_const as R
class P(air.Oracle):
 def __init__(self):
  super().__init__(); self.extra_bases={};self.actual_entries=[];self.pending_native_returns=[];self.return_tramp=self.alloc(8);self.u.mem_write(self.return_tramp,struct.pack('<II',0xd503201f,0xd65f03c0));self.dl_iterate_return=self.alloc(8);self.u.mem_write(self.dl_iterate_return,struct.pack('<II',0xd503201f,0xd65f03c0));self.dl_iterate_state=None;self.dl_iterate_rows=[];self.dl_iterate_events=[];self.host_rwlocks=set();self.host_rwlock_reads={};self.rwlock_events=[];self.map_extra('Graphics',4);self.map_extra('Renderer',5);self.renderer_base=0x5000000;self.extra_bases['Base']=0x02000000;self.load_extra_relocs();self.create_calls=[];self.texture_records=[];self.state_records=[];self.gl=[];self.allocs={};self.vertex_attributes=[];self.graphics_factory=[];self.blend_descriptors=[];self.renderer_factory_obj=self.alloc(64);self.gl_id=100;self.synthetic={};self.synthetic_calls=[];self.shader_values=[];self.blend_gl=[];self.host_create_graphics_calls=0;self.host_create_texture_calls=0;self.checked_strlen_calls=[];self.host_mutexes=set();self.host_mutex_attrs=set();self.host_locked_mutexes=set();self.host_static_mutex_ranges=[];self.static_mutex_adoptions=[]
  self.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=self.get_caps
  self.callbacks['strncpy']=self.strncpy
  self.callbacks['strstr']=self.strstr
  self.callbacks['operator new[](unsigned long)']=self.new_array
  self.callbacks['operator delete[](void*)']=lambda:None
  self.callbacks['malloc']=self.malloc_guest
  self.callbacks['realloc']=self.realloc_guest
  self.callbacks['clock_gettime']=self.clock_gettime
  self.callbacks['SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)']=self.create_graphics
  self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=self.create_blend
  self.callbacks['SPen::ManagedObject::Register()']=lambda:None
  self.callbacks['SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']=self.create_texture
  self.single_thread_mutex_calls=[]
  self.callbacks['pthread_mutex_lock']=lambda:self.single_thread_mutex('lock')
  self.callbacks['pthread_mutex_unlock']=lambda:self.single_thread_mutex('unlock')
  self.callbacks['pthread_mutex_init']=self.pthread_mutex_init
  self.callbacks['pthread_mutex_destroy']=lambda:self.pthread_mutex_destroy()
  self.callbacks['pthread_mutexattr_init']=self.pthread_mutexattr_init
  self.callbacks['pthread_mutexattr_settype']=self.pthread_mutexattr_settype
  self.callbacks['pthread_mutexattr_destroy']=self.pthread_mutexattr_destroy
  self.tls_keys={};self.tls_values={};self.next_tls_key=1;self.tls_calls=[]
  self.callbacks['pthread_key_create']=self.pthread_key_create
  self.callbacks['pthread_key_delete']=self.pthread_key_delete
  self.callbacks['pthread_getspecific']=self.pthread_getspecific
  self.callbacks['pthread_setspecific']=self.pthread_setspecific
  self.pthread_cond_calls=[]
  self.callbacks['pthread_cond_broadcast']=lambda:self.pthread_cond_broadcast()
  self.callbacks['pthread_cond_init']=lambda:self.pthread_cond_noop('init')
  self.callbacks['pthread_cond_destroy']=lambda:self.pthread_cond_noop('destroy')
  self.locale_object=self.alloc(64);self.locale_name=self.alloc(2);self.u.mem_write(self.locale_name,b'C\0');self.current_locale=self.alloc(64);self.u.mem_write(self.current_locale,b'C\0');self.locale_calls=[]
  self.callbacks['newlocale']=self.newlocale
  self.callbacks['uselocale']=self.uselocale
  self.callbacks['freelocale']=lambda:self.x(0,0)
  self.callbacks['setlocale']=lambda:self.x(0,self.locale_name)
  self.callbacks['mbtowc']=self.mbtowc
  self.egl_calls=[]
  self.callbacks['eglGetCurrentContext']=self.egl_get_current_context
  self.crc32_calls=[]
  self.callbacks['crc32']=self.crc32_native
  self.errno_cell=self.alloc(4);self.callbacks['__errno']=lambda:self.x(0,self.errno_cell)
  self.access_calls=[];self.callbacks['access']=self.access_host
  self.mkdir_calls=[];self.callbacks['mkdir']=self.mkdir_host
  self.vsnprintf_calls=[];self.callbacks['vsnprintf']=self.vsnprintf_host
  self.getauxval_calls=[]
  self.callbacks['getauxval']=self.getauxval
  self.system_property_calls=[]
  self.callbacks['__system_property_get']=self.system_property_get
  self.callbacks['strncmp']=self.strncmp_native
  self.callbacks['getcwd']=self.getcwd_host
  self.callbacks['getpid']=lambda:self.x(0,os.getpid())
  self.callbacks['syscall']=self.syscall_host
  self.callbacks['vfprintf']=self.vfprintf_host
  self.callbacks['fputc']=self.fputc_host
  self.callbacks['vasprintf']=self.vasprintf_host
  self.callbacks['android_set_abort_message']=self.android_abort_message
  self.callbacks['pthread_rwlock_rdlock']=lambda:self.pthread_rwlock('read-lock')
  self.callbacks['pthread_rwlock_wrlock']=lambda:self.pthread_rwlock('write-lock')
  self.callbacks['pthread_rwlock_unlock']=lambda:self.pthread_rwlock('unlock')
  self.callbacks['pthread_rwlock_init']=lambda:self.pthread_rwlock('init')
  self.callbacks['pthread_rwlock_destroy']=lambda:self.pthread_rwlock('destroy')
  self.put(self.renderer_base+0x73388,'II',4096,4096);self.u.mem_write(self.renderer_base+0x733c0,b'\x01')
  self.callbacks.update({'glBlendEquationSeparate':lambda:self.blend_gl.append({'name':'glBlendEquationSeparate','args':[self.x(0),self.x(1)]}),'glBlendFuncSeparate':lambda:self.blend_gl.append({'name':'glBlendFuncSeparate','args':[self.x(i) for i in range(4)]}),'glEnable':lambda:self.blend_gl.append({'name':'glEnable','args':[self.x(0)]}),'glDisable':lambda:self.blend_gl.append({'name':'glDisable','args':[self.x(0)]}),'glColorMask':lambda:self.blend_gl.append({'name':'glColorMask','args':[self.x(i) for i in range(4)]}),'glDrawArraysInstanced':lambda:self.gl.append({'name':'glDrawArraysInstanced','mode':self.x(0),'first':self.x(1),'count':self.x(2),'instances':self.x(3)}),'glGetString':self.get_string,'glGetIntegerv':self.get_integer,'glGetError':lambda:self.x(0,0),'glGenBuffers':self.gen_buffers,'glGenSamplers':self.gen_samplers,'glBindBuffer':lambda:self.gl.append({'name':'glBindBuffer','target':self.x(0),'id':self.x(1)}),'glBufferData':self.buffer_data,'glDisableVertexAttribArray':lambda:self.gl.append({'name':'glDisableVertexAttribArray','index':self.x(0)}),'glEnableVertexAttribArray':lambda:self.gl.append({'name':'glEnableVertexAttribArray','index':self.x(0)}),'glVertexAttribPointer':self.vertex_attrib_pointer,'glVertexAttribDivisor':lambda:self.gl.append({'name':'glVertexAttribDivisor','index':self.x(0),'divisor':self.x(1)}),'glBindVertexArray':lambda:self.gl.append({'name':'glBindVertexArray','id':self.x(0)}),'glGenVertexArrays':self.gen_buffers,'glBindTexture':self.bind_texture,'glPixelStorei':self.pixel_store,'glTexSubImage2D':self.tex_sub_image,'glTexParameteri':self.tex_parameter,'glSamplerParameteri':self.sampler_parameter,'glGenTextures':self.gen_textures,'glActiveTexture':self.active_texture,'glTexImage2D':lambda:self.gl.append({'name':'glTexImage2D','args':[self.x(i) for i in range(8)]}),'__strncpy_chk':self.strncpy_checked,'__strlen_chk':self.strlen_checked})
  self.u.mem_map(0x21000000,0x10000);self.state_stub=0x21000000;self.u.mem_write(self.state_stub,struct.pack('<II',0xd503201f,0xd65f03c0))
  self.texture_vtable=0x21000100
  for off in range(0,256,8):self.put(self.texture_vtable+off,'Q',self.state_stub)
  self.put(self.texture_vtable+32,'Q',self.syms['SPen::GLES::TextureObject::GetWidth() const']);self.put(self.texture_vtable+40,'Q',self.syms['SPen::GLES::TextureObject::GetHeight() const']);self.put(self.texture_vtable+64,'Q',self.syms['SPen::GLES::TextureObject::Update(void const*, int)']);self.put(self.texture_vtable+112,'Q',self.syms['SPen::GLES::TextureObject::SetParam(int, int)'])
  self.bitmap_update_method=self.syms['SPen::SPTextureBitmap::UpdateBitmap(void const*, int)']
  self.callbacks.pop('SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',None)
  self.active_native_import=None
 def single_thread_mutex(self,operation):
  pointer=self.x(0)
  if pointer and pointer not in self.host_mutexes and any(start <= pointer and pointer + 40 <= end for start,end in getattr(self,'host_static_mutex_ranges',[])) and bytes(self.u.mem_read(pointer,40))==bytes(40):
   # bionic's PTHREAD_MUTEX_INITIALIZER is the zero-filled static mutex form;
   # accept it only in the mapped libc++ .bss region and retain it as initialized.
   self.host_mutexes.add(pointer)
   self.static_mutex_adoptions.append(hex(pointer))
  if not pointer or pointer not in self.host_mutexes:
   caller=self.u.reg_read(R.UC_ARM64_REG_LR)
   state=bytes(self.u.mem_read(pointer,40)).hex() if pointer else None
   raise RuntimeError(f"pthread_mutex_{operation} received uninitialized mutex {pointer:#x} state={state} at caller {caller:#x}; args={[hex(self.x(i)) for i in range(8)]}; recent_pcs={[hex(a) for a in getattr(self,'trace',[])]}")
  if operation=='lock' and pointer in self.host_locked_mutexes or operation=='unlock' and pointer not in self.host_locked_mutexes:
   raise RuntimeError(f"invalid single-thread mutex {operation} transition for {pointer:#x}")
  if operation=='lock':self.host_locked_mutexes.add(pointer)
  else:self.host_locked_mutexes.remove(pointer)
  self.single_thread_mutex_calls.append({'operation':operation,'mutex':hex(pointer),'caller':hex(self.u.reg_read(R.UC_ARM64_REG_LR)),'result':0})
  self.x(0,0)
 def pthread_mutex_init(self):
  pointer,attrs=self.x(0),self.x(1)
  if not pointer or (attrs and attrs not in self.host_mutex_attrs):raise RuntimeError(f'unsupported pthread_mutex_init args mutex={pointer:#x} attrs={attrs:#x}')
  self.host_mutexes.add(pointer);self.x(0,0)
 def pthread_mutex_destroy(self):
  pointer=self.x(0)
  if pointer not in self.host_mutexes or pointer in self.host_locked_mutexes:raise RuntimeError(f'unsupported pthread_mutex_destroy {pointer:#x}')
  self.host_mutexes.remove(pointer);self.x(0,0)
 def pthread_mutexattr_init(self):
  pointer=self.x(0)
  if not pointer:raise RuntimeError('pthread_mutexattr_init received null')
  self.host_mutex_attrs.add(pointer);self.x(0,0)
 def pthread_mutexattr_settype(self):
  pointer,value=self.x(0),self.x(1)
  if pointer not in self.host_mutex_attrs or value not in (0,1,2):raise RuntimeError(f'unsupported pthread_mutexattr_settype {pointer:#x} {value}')
  self.x(0,0)
 def pthread_mutexattr_destroy(self):
  pointer=self.x(0)
  if pointer not in self.host_mutex_attrs:raise RuntimeError(f'unsupported pthread_mutexattr_destroy {pointer:#x}')
  self.host_mutex_attrs.remove(pointer);self.x(0,0)
 def pthread_cond_noop(self,operation):
  pointer=self.x(0)
  if not pointer:raise RuntimeError(f'pthread_cond_{operation} received null')
  self.pthread_cond_calls.append({'operation':operation,'condition':hex(pointer),'boundary':'single-thread condition state'})
  self.x(0,0)
 def pthread_cond_broadcast(self):
  pointer=self.x(0)
  if not pointer:raise RuntimeError('pthread_cond_broadcast received null')
  self.pthread_cond_calls.append({'operation':'broadcast','condition':hex(pointer),'boundary':'no other emulated thread can be waiting'})
  self.x(0,0)
 def pthread_key_create(self):
  output,destructor=self.x(0),self.x(1)
  if not output:raise RuntimeError('pthread_key_create received null output')
  key=self.next_tls_key;self.next_tls_key+=1
  self.tls_keys[key]=destructor;self.put(output,'I',key)
  self.tls_calls.append({'operation':'key-create','key':key,'destructor':hex(destructor) if destructor else None})
  self.x(0,0)
 def pthread_key_delete(self):
  key=self.x(0)
  if key not in self.tls_keys:raise RuntimeError(f'pthread_key_delete unknown key {key}')
  self.tls_keys.pop(key);self.tls_values.pop(key,None);self.tls_calls.append({'operation':'key-delete','key':key});self.x(0,0)
 def pthread_getspecific(self):
  key=self.x(0)
  if key not in self.tls_keys:raise RuntimeError(f'pthread_getspecific unknown key {key}')
  self.x(0,self.tls_values.get(key,0));self.tls_calls.append({'operation':'get-specific','key':key})
 def pthread_setspecific(self):
  key,value=self.x(0),self.x(1)
  if key not in self.tls_keys:raise RuntimeError(f'pthread_setspecific unknown key {key}')
  self.tls_values[key]=value;self.tls_calls.append({'operation':'set-specific','key':key,'value':hex(value) if value else None});self.x(0,0)
 def getauxval(self):
  key=self.x(0)
  if key not in (16,26):raise RuntimeError(f'unsupported getauxval key {key}')
  self.getauxval_calls.append({'key':key,'value':0,'meaning':'AT_HWCAP/AT_HWCAP2 optional feature bits disabled for conservative baseline path'})
  self.x(0,0)
 def system_property_get(self):
  key=self.cstr(self.x(0))
  if key!='ro.arch':raise RuntimeError(f'unsupported Android system property {key!r}')
  output=self.x(1);value=b'arm64-v8a'
  self.u.mem_write(output,value+b'\0')
  self.system_property_calls.append({'key':key,'value':value.decode(),'boundary':'host Android property callback'})
  self.x(0,len(value))
 def strncmp_native(self):
  left,right,count=self.x(0),self.x(1),self.x(2)
  if count>4096:raise RuntimeError(f'strncmp bound exceeded: {count}')
  a=bytes(self.u.mem_read(left,count));b=bytes(self.u.mem_read(right,count))
  for av,bv in zip(a,b):
   if av!=bv:self.x(0,av-bv);return
   if av==0:self.x(0,0);return
  self.x(0,0)
 def getcwd_host(self):
  pointer,size=self.x(0),self.x(1)
  value=os.getcwd().encode()+b'\0'
  if not pointer or size<len(value) or size>0x10000:raise RuntimeError(f'unsupported getcwd buffer={pointer:#x} size={size}')
  self.u.mem_write(pointer,value);self.x(0,pointer)
 def syscall_host(self):
  number=self.x(0)
  if number!=270:raise RuntimeError(f'unsupported native syscall number {number}')
  pid,local_iov,local_count,remote_iov,remote_count,flags=(self.x(i) for i in range(1,7))
  if pid!=os.getpid() or local_count!=1 or remote_count!=1 or flags!=0:
   raise RuntimeError(f'unsupported process_vm_readv pid={pid} local={local_iov:#x}/{local_count} remote={remote_iov:#x}/{remote_count} flags={flags}')
  local_base,local_len=self.get(local_iov,'QQ');remote_base,remote_len=self.get(remote_iov,'QQ')
  count=min(local_len,remote_len)
  if count>1<<20:raise RuntimeError(f'process_vm_readv transfer exceeds bound: {count}')
  payload=bytes(self.u.mem_read(remote_base,count))
  self.u.mem_write(local_base,payload)
  self.syscall_calls=getattr(self,'syscall_calls',[])
  self.syscall_calls.append({'number':number,'pid':pid,'local':[hex(local_base),local_len],'remote':[hex(remote_base),remote_len],'bytes':count})
  self.x(0,count)
 def vfprintf_host(self):
  stream,format_pointer,va=self.x(0),self.x(1),self.x(2)
  fmt=self.cstr(format_pointer)
  if stream!=0x20002188 or len(fmt)>4096:raise RuntimeError(f'unsupported vfprintf stream={stream:#x} format={fmt!r}')
  stack,gr_top,vr_top,gr_offs,vr_offs=self.get(va,'QQQii')
  def gp():
   nonlocal stack,gr_offs
   if gr_offs<=0:
    value=self.get(gr_top+gr_offs,'Q')[0];gr_offs+=8;self.put(va+24,'i',gr_offs);return value
   value=self.get(stack,'Q')[0];stack+=8;self.put(va,'Q',stack);return value
  def fp():
   nonlocal stack,vr_offs
   if vr_offs<=0:
    value=self.get(vr_top+vr_offs,'d')[0];vr_offs+=16;self.put(va+28,'i',vr_offs);return value
   value=self.get(stack,'d')[0];stack+=8;self.put(va,'Q',stack);return value
  pieces=[];cursor=0
  for match in re.finditer(r'%([-+ #0]*)(\d*)(?:\.(\d+))?(hh|h|ll|l|z|t|j|L)?([diuoxXpcsfeEgGaA%])',fmt):
   pieces.append(fmt[cursor:match.start()]);cursor=match.end()
   flags,width,precision,length,kind=match.groups();width=int(width or 0);precision=int(precision) if precision else None
   if kind=='%':pieces.append('%');continue
   if kind=='s':
    pointer=gp();raw=b'(null)' if not pointer else bytes(self.u.mem_read(pointer,min(4096,(precision if precision is not None else 4096))))
    raw=raw.split(b'\0',1)[0];value=raw.decode('utf-8','replace')
   elif kind=='p':
    pointer=gp();value='(nil)' if not pointer else f'0x{pointer:x}'
   elif kind in 'fFeEgGaA':
    number=fp();fmtchar='f' if kind in 'aA' else kind
    value=(f'{number:.{precision}f}' if fmtchar=='f' and precision is not None else f'{number:g}' if fmtchar in 'gG' else f'{number:.{precision or 6}e}')
    if kind.isupper():value=value.upper()
   else:
    number=gp();bits=32 if length in ('h','hh') else 64
    if kind in 'di':
     signed=number-(1<<bits) if number&(1<<(bits-1)) else number
     value=str(signed)
    elif kind=='c':value=chr(number&0xff)
    else:
     number &= (1<<bits)-1
     value=(format(number,'o') if kind=='o' else format(number,'x' if kind=='x' else 'X' if kind=='X' else 'd'))
   if width>len(value):
    pad='0' if '0' in flags else ' '
    value=value.rjust(width,pad) if '-' not in flags else value.ljust(width,' ')
   pieces.append(value)
  pieces.append(fmt[cursor:]);output=''.join(pieces).encode('utf-8','replace')
  if len(output)>1<<16:raise RuntimeError(f'vfprintf output too large: {len(output)}')
  host_stderr.extend(output)
  self.x(0,len(output))
  self.syscall_calls=getattr(self,'syscall_calls',[])
  self.syscall_calls.append({'event':'vfprintf','format':fmt,'rendered':output.decode('utf-8','replace')})
 def fputc_host(self):
  value,stream=self.x(0)&0xff,self.x(1)
  if stream!=0x20002188:raise RuntimeError(f'fputc unsupported stream {stream:#x}')
  host_stderr.append(value);self.x(0,value)
 def vasprintf_host(self):
  output_ptr,format_ptr,va=self.x(0),self.x(1),self.x(2)
  if not output_ptr:raise RuntimeError('vasprintf received null output pointer')
  start=len(host_stderr)
  self.x(0,0x20002188);self.x(1,format_ptr);self.x(2,va)
  self.vfprintf_host()
  rendered=bytes(host_stderr[start:]);del host_stderr[start:]
  pointer=self.alloc(len(rendered)+1);self.u.mem_write(pointer,rendered+b'\0');self.allocs[pointer]=len(rendered)+1
  self.put(output_ptr,'Q',pointer);self.x(0,len(rendered))
 def android_abort_message(self):
  pointer=self.x(0);message=self.cstr(pointer) if pointer else '<null>'
  raise RuntimeError(f'Android abort message: {message!r}; stderr={bytes(host_stderr).decode("utf-8","replace")!r}; gl={host_gl["gl"]}')
 def dl_iterate_phdr(self):
  callback,data=self.x(0),self.x(1)
  if not callback or not self.dl_iterate_rows:raise RuntimeError(f'dl_iterate_phdr lacks callback/ELF rows callback={callback:#x} rows={len(self.dl_iterate_rows)}')
  self.dl_iterate_state={'callback':callback,'data':data,'outer_lr':self.u.reg_read(R.UC_ARM64_REG_LR),'outer_sp':self.u.reg_read(R.UC_ARM64_REG_SP),'index':0}
  self.dl_iterate_next()
 def dl_iterate_next(self):
  state=self.dl_iterate_state
  if state is None:raise RuntimeError('dl_iterate_phdr return without active iterator')
  index=state['index']
  if index>=len(self.dl_iterate_rows):
   self.x(0,0);self.u.reg_write(R.UC_ARM64_REG_SP,state['outer_sp']);self.u.reg_write(R.UC_ARM64_REG_PC,state['outer_lr']);self.dl_iterate_state=None;return
  row=self.dl_iterate_rows[index]
  name=self.alloc(len(row['name'])+1);self.u.mem_write(name,row['name']+b'\0')
  phdr=self.alloc(len(row['phdrs']));self.u.mem_write(phdr,row['phdrs'])
  info=self.alloc(64)
  values=(row['base'],name,phdr,row['count'],1,0,row['tls_modid'],row['tls_data'])
  self.put(info,'8Q',*values)
  state.update(info=info,name=row['name'].decode(errors='replace'))
  self.x(0,info);self.x(1,64);self.x(2,state['data']);self.u.reg_write(R.UC_ARM64_REG_SP,(state['outer_sp']-16)&~15);self.u.reg_write(R.UC_ARM64_REG_LR,self.dl_iterate_return);self.u.reg_write(R.UC_ARM64_REG_PC,state['callback'])
 def dl_iterate_callback_return(self):
  state=self.dl_iterate_state
  if state is None:raise RuntimeError('stray dl_iterate_phdr callback return')
  result=self.x(0);self.dl_iterate_events.append({'name':state['name'],'result':result,'callback':hex(state['callback'])})
  if result:
   self.x(0,result);self.u.reg_write(R.UC_ARM64_REG_SP,state['outer_sp']);self.u.reg_write(R.UC_ARM64_REG_PC,state['outer_lr']);self.dl_iterate_state=None;return
  state['index']+=1;self.dl_iterate_next()
 def pthread_rwlock(self,operation):
  pointer=self.x(0)
  if not pointer:raise RuntimeError(f'pthread_rwlock_{operation} received null')
  if operation=='init':
   self.host_rwlocks.add(pointer);self.host_rwlock_reads[pointer]=0;self.x(0,0);return
  if operation in ('read-lock','write-lock') and pointer not in self.host_rwlocks:
   in_static=any(start<=pointer and pointer+56<=end for start,end in getattr(self,'host_static_rwlock_ranges',[]))
   if in_static and bytes(self.u.mem_read(pointer,56))==bytes(56):
    self.host_rwlocks.add(pointer);self.host_rwlock_reads[pointer]=0
  if pointer not in self.host_rwlocks:raise RuntimeError(f'pthread_rwlock_{operation} on unknown/uninitialized lock {pointer:#x} bytes={bytes(self.u.mem_read(pointer,56)).hex()}')
  if operation in ('read-lock','write-lock'):
   if operation=='write-lock' and self.host_rwlock_reads[pointer]:raise RuntimeError(f'write lock while readers active at {pointer:#x}')
   self.host_rwlock_reads[pointer]+=1;self.rwlock_events.append({'operation':operation,'lock':hex(pointer),'readers':self.host_rwlock_reads[pointer],'boundary':'single-threaded POSIX synchronization'})
  elif operation=='unlock':
   if self.host_rwlock_reads[pointer]<=0:raise RuntimeError(f'unbalanced rwlock unlock {pointer:#x}')
   self.host_rwlock_reads[pointer]-=1;self.rwlock_events.append({'operation':operation,'lock':hex(pointer),'readers':self.host_rwlock_reads[pointer],'boundary':'single-threaded POSIX synchronization'})
  elif operation=='destroy':
   if self.host_rwlock_reads[pointer]:raise RuntimeError(f'destroying held rwlock {pointer:#x}')
   self.host_rwlocks.remove(pointer);self.host_rwlock_reads.pop(pointer,None)
  self.x(0,0)
 def newlocale(self):
  mask,name,base=self.x(0),self.cstr(self.x(1)),self.x(2)
  if mask!=0x1fbf or name not in ('C','POSIX') or base:raise RuntimeError(f'unsupported newlocale request mask={mask:#x} name={name!r} base={base:#x}')
  self.locale_calls.append({'operation':'newlocale','mask':hex(mask),'name':name,'base':base,'result':hex(self.locale_object)})
  self.x(0,self.locale_object)
 def uselocale(self):
  current=self.x(0);old=self.current_locale
  if current:self.current_locale=current
  self.locale_calls.append({'operation':'uselocale','requested':hex(current),'previous':hex(old)})
  self.x(0,old)
 def mbtowc(self):
  output,source,count=self.x(0),self.x(1),self.x(2)
  if not source or not count:self.x(0,0xffffffffffffffff);return
  first=bytes(self.u.mem_read(source,1))[0]
  if first>=0x80:self.x(0,0xffffffffffffffff);return
  if output:self.put(output,'I',first)
  self.x(0,0 if first==0 else 1)
 def egl_get_current_context(self):
  token=0x75000000
  self.egl_calls.append({'operation':'eglGetCurrentContext','host_simulated_context_token':hex(token)})
  self.x(0,token)
 def crc32_native(self):
  checksum,pointer,length=self.x(0),self.x(1),self.x(2)
  if length>0x1000000 or (length and not pointer):raise RuntimeError(f'unsupported crc32 input pointer={pointer:#x} length={length}')
  payload=bytes(self.u.mem_read(pointer,length)) if length else b''
  value=zlib.crc32(payload,checksum)&0xffffffff
  self.crc32_calls.append({'length':length,'seed':checksum,'result':value})
  self.x(0,value)
 def access_host(self):
  path,mode=self.cstr(self.x(0)),self.x(1)
  if mode not in (0,1,2,4,6):raise RuntimeError(f'unsupported access mode {mode}')
  allowed=path.startswith('/tmp/') or path=='/tmp'
  if not allowed:raise RuntimeError(f'host access path outside probe cache: {path!r}')
  result=0 if os.access(path,mode) else -1
  self.put(self.errno_cell,'i',0 if result==0 else 2)
  self.access_calls.append({'path':path,'mode':mode,'result':result,'boundary':'host filesystem read-only access check'})
  self.x(0,result)
 def mkdir_host(self):
  path,mode=self.cstr(self.x(0)),self.x(1)&0o777
  if path!='/tmp/RendererCache':raise RuntimeError(f'host mkdir path outside native cache probe boundary: {path!r}')
  try:
   os.mkdir(path,mode)
   result,err=0,0
  except FileExistsError:
   result,err=-1,17
  self.put(self.errno_cell,'i',err)
  self.mkdir_calls.append({'path':path,'mode':mode,'result':result,'errno':err,'boundary':'host filesystem mkdir for native RendererCache'})
  self.x(0,result)
 def vsnprintf_host(self):
  destination,size,format_pointer,va=self.x(0),self.x(1),self.x(2),self.x(3)
  fmt=self.cstr(format_pointer)
  if not destination or not 0<size<=4096 or fmt not in ('/sh%0lu.dat','%lu','%p'):raise RuntimeError(f'unsupported native vsnprintf request dest={destination:#x} size={size} format={fmt!r}')
  stack,gr_top,vr_top,gr_offs,vr_offs=self.get(va,'QQQii')
  if gr_offs<=0:
   value_pointer=gr_top+gr_offs
   next_gr_offs=gr_offs+8
   self.put(va+24,'i',next_gr_offs)
  else:
   value_pointer=stack
   stack+=8
   self.put(va,'Q',stack)
  value=self.get(value_pointer,'Q')[0]
  output=(f'/sh{value}.dat' if fmt=='/sh%0lu.dat' else f'0x{value:x}' if fmt=='%p' else str(value)).encode()
  written=output[:size-1]
  self.u.mem_write(destination,written+b'\0')
  self.vsnprintf_calls.append({'format':fmt,'value':value,'result':output.decode(),'va_gr_offs_before':gr_offs,'va_gr_offs_after':gr_offs+8 if gr_offs<=0 else gr_offs,'result_length':len(output),'buffer_size':size})
  self.x(0,len(output))
 def map_extra(self,name,index):
  path=LIB/f'libSPen{name}.so'; b=path.read_bytes(); base=index*0x1000000; self.extra_bases[name]=base; self.u.mem_map(base,0x200000)
  phoff=struct.unpack_from('<Q',b,32)[0]; ent,n=struct.unpack_from('<HH',b,54)
  for i in range(n):
   p=struct.unpack_from('<II6Q',b,phoff+i*ent)
   if p[0]==1 and p[5]: self.u.mem_write(base+p[3],b[p[2]:p[2]+p[5]])
  env={**os.environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
  nm=subprocess.run([str(NM),'-D','-C',str(path)],capture_output=True,text=True,check=True,env=env)
  for line in nm.stdout.splitlines():
   z=line.strip().split(' ',2)
   if len(z)==3 and z[1] in ('T','W','V','D','B','R'): self.syms[z[2]]=base+int(z[0],16)
  dis=subprocess.run([str(OBJ),'-d','-C','-j','.plt',str(path)],capture_output=True,text=True,check=True,env=env)
  for a,s in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis.stdout,re.M):self.plt[base+int(a,16)]=s
 def load_extra_relocs(self):
  # Reuse the repo's narrow RELA relocation resolver; unresolved imports remain traps.
  for name,base in self.extra_bases.items():
   b=(LIB/f'libSPen{name}.so').read_bytes(); shoff=struct.unpack_from('<Q',b,40)[0]; es,n,si=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*es) for i in range(n)]; st=secs[si]; names=b[st[4]:st[4]+st[5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; sy=by['.dynsym']; ds=by['.dynstr']; strings=b[ds[4]:ds[4]+ds[5]]; syml=[]
   for off in range(sy[4],sy[4]+sy[5],sy[9]):
    no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',b,off); end=strings.find(b'\0',no);syml.append((strings[no:end].decode() if no else '',shndx,val))
   sec=by.get('.rela.dyn')
   if not sec:continue
   for off in range(sec[4],sec[4]+sec[5],sec[9]):
    where,info,add=struct.unpack_from('<QQq',b,off);typ,idx=info&0xffffffff,info>>32
    if typ==0x403:val=base+add
    elif typ==0x401 and syml[idx][1]:val=base+syml[idx][2]
    elif typ==0x101:val=self.resolve_symbol(syml[idx][0])+add
    elif typ==0x401:val=self.resolve_symbol(syml[idx][0])
    else:continue
    self.put(base+where,'Q',val)
 def hook(self,u,address,size,data):
  if address==self.dl_iterate_return:
   self.dl_iterate_callback_return();return
  if address in self.plt and self.plt[address]=='dl_iterate_phdr':
   self.dl_iterate_phdr();return
  if address==getattr(self,'pthread_once_thunk',None):
   self.pthread_once_calls.append({'once':hex(self.x(0)),'callback':hex(self.x(1)),'lr':hex(self.u.reg_read(R.UC_ARM64_REG_LR)),'sp':hex(self.u.reg_read(R.UC_ARM64_REG_SP))})
  if address in self.synthetic:
   name=self.synthetic[address];rec={'name':name,'args':[self.x(i) for i in range(6)]};self.synthetic_calls.append(rec)
   if name=='bitmap.get_projection':u.mem_write(self.x(8),struct.pack('<16f',1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.))
   elif name=='bitmap.get_readable':self.x(0,self.readable)
   elif name=='shader.ProjectionMatrix':self.shader_values.append({'name':name,'matrix':list(struct.unpack('<16f',bytes(u.mem_read(self.x(1),64))))})
   elif name=='shader.inputColor':self.shader_values.append({'name':name,'color':list(struct.unpack('<4f',bytes(u.mem_read(self.x(1),16))))})
   elif name=='shader.sampler0':self.shader_values.append({'name':name,'unit':self.get(self.x(1),'I')[0]})
   u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  if address in self.plt and self.plt[address].startswith('SPen::RendererFactory::CreateTextureObject('):
   args=[self.x(i) for i in range(6)];self.texture_records.append({'desc':list(self.get(args[0],'8I')),'name':self.cstr(args[1]),'buffer_info':list(self.get(args[5],'4I')) if args[5] else None,'pairs':list(self.get(args[3],'I'*(args[4]*2))) if args[4] else [],'pair_count':args[4]})
   self.pending_native_returns.append(u.reg_read(R.UC_ARM64_REG_LR));u.reg_write(R.UC_ARM64_REG_LR,self.return_tramp);u.reg_write(R.UC_ARM64_REG_PC,self.syms['SPen::GLES::RendererFactoryImpl::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']);return
  if address in self.plt and self.plt[address]=='SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)':
   self.create_blend();return
  if address==self.syms.get('SPen::VertexDescriptor::addAttribute(SPen::VertexDescriptor::AttributeType, int, int, int)'):
   self.vertex_attributes.append({'descriptor':self.x(0),'attribute_type':self.x(1),'components':self.x(2),'stride':self.x(3),'divisor':self.x(4)})
  if address in self.plt and self.plt[address]=='SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)':
   self.create_graphics();u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  if address==self.return_tramp:
   if not self.pending_native_returns: raise RuntimeError('unexpected return trampoline')
   u.reg_write(R.UC_ARM64_REG_PC,self.pending_native_returns.pop());return
  # Native factory runs in place of its imported call; all its imports still dispatch normally.
  if address in self.plt and self.plt[address].startswith('SPen::SPGraphicsFactory::CreateBitmap('):
   self.create_calls.append({'x':[self.x(i) for i in range(8)],'stack':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_SP),16)).hex(),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
   self.pending_native_returns.append(u.reg_read(R.UC_ARM64_REG_LR));u.reg_write(R.UC_ARM64_REG_LR,self.return_tramp);u.reg_write(R.UC_ARM64_REG_PC,self.syms['SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)']);return
  if address==self.syms.get('SPen::GLES::TextureObject::SetParam(int, int)'):
   self.actual_entries.append({'name':'SPen::GLES::TextureObject::SetParam(int, int)','args':[self.x(i) for i in range(3)]})
  for name in ('SPen::AirBrushPenStrokeDrawableRTV1::Init()','SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)','SPen::AirBrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)','SPen::GLES::GraphicsObject::UpdateBuffer(unsigned int, void const*, unsigned int)','SPen::GLES::GraphicsObject::DrawInstanced(unsigned int, int, int) const','SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)','SPen::GLES::BlendStateObject::Activate()'):
   if address==self.syms.get(name):self.actual_entries.append({'name':name,'args':[self.x(i) for i in range(4)]})
  for name in ('SPen::GLES::RendererFactoryImpl::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)','SPen::GLES::TextureObject::TextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int)','SPen::GLES::TextureObject::CreateTexture(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)','SPen::GLES::TextureObject::createTextureInternalsSampler(SPen::TextureDescriptor const&, char const*, void const*, std::__ndk1::map<int, int, std::__ndk1::less<int>, std::__ndk1::allocator<std::__ndk1::pair<int const, int> > > const&, SPen::TextureBufferInfo const*)'):
   if address==self.syms.get(name):self.actual_entries.append({'name':name,'args':[self.x(i) for i in range(7)]})
  if address in (self.syms.get('SPen::SPTextureBitmap::UpdateBitmap(void const*, int)'),self.syms.get('non-virtual thunk to SPen::SPSubBitmapRT::UpdateBitmap(void const*, int)'),self.syms.get('SPen::SPTextureBitmap::SetTextureFilter(SPen::TextureParamFiltering, SPen::TextureParamFilteringValue)'),self.syms.get('SPen::SPTextureBitmap::SetTextureWrap(SPen::TextureParamWrapping, SPen::TextureParamWrappingValue)')):
   nm=next(k for k,v in self.syms.items() if v==address);self.actual_entries.append({'name':nm,'args':[self.x(i) for i in range(4)]})
  if address==self.state_stub:
   self.state_records.append({'object':self.x(0),'enum':self.x(1),'value':self.x(2)});u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  if address==getattr(self,"queue_wait",None):
   self.queue_events=getattr(self,"queue_events",[])
   self.queue_events.append({"operation":"completion-wait","receiver":hex(self.x(0)),"lr":hex(self.u.reg_read(R.UC_ARM64_REG_LR)),"boundary":"host synchronous completion after native task dispatch"})
   u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  if address==getattr(self,"queue_post",None):
   task=self.x(1);vptr=self.get(task,"Q")[0];method=self.get(vptr+16,"Q")[0]
   names={value:name for name,value in self.syms.items()}
   self.queue_events=getattr(self,"queue_events",[])
   self.queue_events.append({"operation":"post-and-run","receiver":hex(self.x(0)),"task":hex(task),"task_vptr":hex(vptr),"run_method":hex(method),"run_symbol":names.get(method),"caller":hex(self.u.reg_read(R.UC_ARM64_REG_LR)),"boundary":"host synchronous scheduler invokes actual native task vtable slot +16"})
  super().hook(u,address,size,data)
 def strstr(self):
  hay=self.cstr(self.x(0)).encode();needle=self.cstr(self.x(1)).encode();i=hay.find(needle);self.x(0,self.x(0)+i if i>=0 else 0)
 def strncpy(self):
  n=self.x(2);raw=bytes(self.u.mem_read(self.x(1),n)).split(b'\0',1)[0];self.u.mem_write(self.x(0),raw+b'\0'*max(0,n-len(raw)));self.x(0,self.x(0))
 def new_array(self):
  n=self.x(0)
  if n>0x1000000:raise RuntimeError(f'array size {n}')
  p=self.alloc(max(n,1));self.u.mem_write(p,b'\xcd'*max(n,1));self.allocs[p]=n;self.x(0,p)
 def posix_memalign(self):
  output,alignment,size=self.x(0),self.x(1),self.x(2)
  if not output or alignment<8 or alignment&(alignment-1) or alignment%8 or size>0x1000000:
   self.x(0,22);return
  self.heap=(self.heap+alignment-1)&-alignment
  pointer=self.alloc(max(size,1));self.put(output,'Q',pointer)
  self.allocs[pointer]=size;self.x(0,0)
 def malloc_guest(self):
  size=self.x(0)
  if size>0x1000000:raise RuntimeError(f'malloc size exceeds bound {size}')
  pointer=self.alloc(max(size,1));self.allocs[pointer]=size;self.x(0,pointer)
 def realloc_guest(self):
  old,size=self.x(0),self.x(1)
  if size>0x1000000:raise RuntimeError(f'realloc size exceeds bound {size}')
  if not old:
   self.malloc_guest();return
  if old not in self.allocs:raise RuntimeError(f'realloc on unknown guest allocation {old:#x} size={size}')
  old_size=self.allocs.pop(old)
  if size==0:self.x(0,0);return
  new=self.alloc(size);self.u.mem_write(new,bytes(self.u.mem_read(old,min(old_size,size))));self.allocs[new]=size;self.x(0,new)
 def clock_gettime(self):
  self.put(self.x(1),'qq',0,0);self.x(0,0)
 def strncpy_checked(self):
  n,dest=self.x(2),self.x(3)
  if n>4096 or n>dest:raise RuntimeError(f'__strncpy_chk bounds n={n} dest={dest}')
  raw=bytes(self.u.mem_read(self.x(1),n)).split(b'\0',1)[0]
  self.u.mem_write(self.x(0),raw+b'\0'*max(0,n-len(raw)));self.x(0,self.x(0))
 def strlen_checked(self):
  pointer,bound=self.x(0),self.x(1)
  scan_limit=4096 if bound==(1<<64)-1 else bound
  if not 0<scan_limit<=4096:raise RuntimeError(f'__strlen_chk bound outside probe limit: {bound}')
  for length in range(scan_limit):
   if self.u.mem_read(pointer+length,1)==b'\0':
    self.checked_strlen_calls.append({'bound':bound,'scan_limit':scan_limit,'length':length});self.x(0,length);return
  raise RuntimeError(f'__strlen_chk found no terminator within scan limit {scan_limit} (bound {bound})')
 def cstr(self,p):
  b=bytearray()
  for i in range(4096):
   q=bytes(self.u.mem_read(p+i,1))
   if q==b'\0':return b.decode(errors='replace')
   b+=q
  raise RuntimeError('string bound')
 def method(self,name):
  p=self.alloc(16);self.synthetic[p]=name;return p
 def get_caps(self):
  c=self.alloc(16);vt=self.alloc(32);self.put(c,'Q',vt);fn=self.alloc(8);self.u.mem_write(fn,struct.pack('<II',0x52820000,0xd65f03c0));self.put(vt+16,'Q',fn);self.x(0,c)
 def create_graphics(self):
  self.host_create_graphics_calls += 1
  args=[self.x(i) for i in range(5)];desc=args[2];raw=bytes(self.u.mem_read(desc,0x130));obj=self.alloc(0xa40);self.put(obj,'Q',self.renderer_base+0x6a220);self.u.mem_write(obj+0x68,raw);self.put(obj+36,'II',41,42);self.put(obj+2540,'I',2);self.put(obj+2612,'I',5);self.graphics_factory.append({'args':args,'descriptor_ptr':desc,'descriptor_bytes':len(raw),'descriptor_sha256':hashlib.sha256(raw).hexdigest(),'descriptor_hex':raw.hex(),'attributes':list(self.vertex_attributes),'return_object_synthetic':True,'object_size':0xa40});self.x(0,obj)
 def create_blend(self):
  desc=self.x(0);raw=bytes(self.u.mem_read(desc,32));self.blend_descriptors.append(raw.hex());obj=self.alloc(48);self.pending_native_returns.append(self.u.reg_read(R.UC_ARM64_REG_LR));self.u.reg_write(R.UC_ARM64_REG_LR,self.return_tramp);self.x(0,obj);self.x(1,desc);self.u.reg_write(R.UC_ARM64_REG_PC,self.syms['SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)'])
 def create_texture(self):
  self.host_create_texture_calls += 1
  d=self.x(0);label=self.x(1);info=self.x(5);desc=list(self.get(d,'8I'));info_words=list(self.get(info,'4I'));name=self.cstr(label);t=self.alloc(320);self.put(t,'Q',self.texture_vtable);self.put(t+40,'II',0x0de1,1);self.put(t+48,'I',7);self.put(t+64,'II',desc[0],desc[1]);self.put(t+76,'II',desc[3],desc[4]);self.put(t+232,'Q',t+240);self.put(t+268,'I',0);pair_count=self.x(4);pair_ptr=self.x(3);pairs=list(self.get(pair_ptr,'I'*(pair_count*2))) if pair_count else [];self.texture_records.append({'name':name,'desc':desc,'info':info_words,'pairs_u32':pairs,'pair_count':pair_count,'ptr':t});self.x(0,t)
 def get_string(self):
  e=self.x(0);self.gl.append({'name':'glGetString','enum':e});p=self.alloc(32);self.u.mem_write(p,b'OpenGL ES 3.2\0');self.x(0,p)
 def get_integer(self):
  enum=self.x(0);ptr=self.x(1);value={0x8869:16,0x8B4D:16,0x8A2F:256}.get(enum,0);self.put(ptr,'I',value);self.gl.append({'name':'glGetIntegerv','enum':enum,'value':value})
 def gen_buffers(self):
  n=self.x(0);ptr=self.x(1);ids=list(range(self.gl_id,self.gl_id+n));self.gl_id+=n;self.put(ptr,'I'*n,*ids);self.gl.append({'name':'glGenBuffers','count':n,'ids':ids})
 def buffer_data(self):
  target,size,ptr,usage=[self.x(i) for i in range(4)];raw=bytes(self.u.mem_read(ptr,min(size,0x100000))) if ptr and size else b'';self.gl.append({'name':'glBufferData','target':target,'bytes':size,'data_sha256':hashlib.sha256(raw).hexdigest() if raw else None,'data_hex':raw.hex() if size<=64 else None,'usage':usage})
 def vertex_attrib_pointer(self):
  sp=self.u.reg_read(R.UC_ARM64_REG_SP);self.gl.append({'name':'glVertexAttribPointer','index':self.x(0),'size':self.x(1),'type':self.x(2),'normalized':self.x(3),'stride':self.x(4),'pointer':self.x(5)})
 def gen_textures(self):
  n=self.x(0);p=self.x(1); ids=list(range(self.gl_id,self.gl_id+n));self.gl_id+=n;self.put(p,'I'*n,*ids);self.gl.append({'name':'glGenTextures','n':n,'ids':ids})
 def gen_samplers(self):
  n=self.x(0);p=self.x(1); ids=list(range(self.gl_id,self.gl_id+n));self.gl_id+=n;self.put(p,'I'*n,*ids);self.gl.append({'name':'glGenSamplers','n':n,'ids':ids})
 def bind_texture(self):self.gl.append({'name':'glBindTexture','target':self.x(0),'id':self.x(1)})
 def active_texture(self):self.gl.append({'name':'glActiveTexture','texture':self.x(0)})
 def pixel_store(self):self.gl.append({'name':'glPixelStorei','pname':self.x(0),'value':self.x(1)})
 def sampler_parameter(self):self.gl.append({'name':'glSamplerParameteri','sampler':self.x(0),'pname':self.x(1),'value':self.x(2)})
 def tex_parameter(self):self.gl.append({'name':'glTexParameteri','target':self.x(0),'pname':self.x(1),'value':self.x(2)})
 def tex_sub_image(self):
  sp=self.u.reg_read(R.UC_ARM64_REG_SP); pixels=self.get(sp,'Q')[0]; w,h,fmt,typ=[self.x(i) for i in range(4,8)]; n=w*h*4
  if n>0x1000000:raise RuntimeError('upload size bound')
  raw=bytes(self.u.mem_read(pixels,n));self.upload_bytes=raw;self.gl.append({'name':'glTexSubImage2D','target':self.x(0),'wh':[w,h],'format':fmt,'type':typ,'sha256':hashlib.sha256(raw).hexdigest(),'bytes':n})
 def run(self):
  tramp=struct.pack('<II',0xd503201f,0xd65f03c0);assert bytes(self.u.mem_read(self.return_tramp,8))==tramp;alloc_start=self.heap;self.renderer_factory_obj=self.alloc(64);obj=self.alloc(512);data=self.alloc(64);settings=self.alloc(64);mgr=self.alloc(64);canvas=self.alloc(64);queue=self.alloc(32);queuevt=self.alloc(64);self.queue_post=self.alloc(16);self.queue_task_return=self.alloc(16);assert self.return_tramp+8<=alloc_start and not (obj<=self.return_tramp<obj+512);self.put(queue,'Q',queuevt);self.put(queuevt+16,'Q',self.queue_post);self.queue=queue;self.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:self.x(0,self.queue)
  self.call('SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)',[obj,data,mgr])
  rt=self.get(obj+16,'Q')[0];self.put(obj+24,'Q',mgr);self.put(obj+32,'Q',canvas);self.put(obj+72,'Q',data);self.put(data,'Q',settings);self.put(settings,'fI',14.,0xff336699)
  rt=self.get(obj+16,'Q')[0];self.put(obj+24,'Q',mgr);self.put(obj+32,'Q',canvas);self.put(obj+72,'Q',data);self.put(data,'Q',settings);self.put(settings,'fI',14.,0xff336699)
  self.put(rt+136,'Q',mgr);self.put(rt+200,'Q',0);self.put(rt+208,'Q',self.alloc(32));self.put(rt+216,'Q',0)
  try:self.call('SPen::AirBrushPenStrokeDrawableRTV1::Init()',[rt],instruction_limit=5000000)
  except Exception as e:
   print('INITFAIL',repr(e),'PC',hex(self.u.reg_read(R.UC_ARM64_REG_PC)),'LR',hex(self.u.reg_read(R.UC_ARM64_REG_LR)),'X0-8',[hex(self.x(i)) for i in range(9)],'GL',self.gl,'SAMP',self.state_records,'ACTUAL',self.actual_entries,'FACTORY',self.texture_records,file=__import__('sys').stderr);raise
  assert bytes(self.u.mem_read(self.return_tramp,8))==tramp,'Init overwrote return trampoline'
  graphics=self.get(rt+200,'Q')[0];self.call('SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)',[rt],[10.0]);fx=json.loads((ROOT/'conformance/airbrush-saved-native.json').read_text());rows=fx['cases'][0]['native']['stamps'];vals=[struct.unpack('<f',struct.pack('<I',int(w,16)))[0] for row in rows for w in row];raw=b''.join(struct.pack('<f',v) for v in vals);ptr=self.alloc(len(raw));self.u.mem_write(ptr,raw);self.call('SPen::GLES::GraphicsObject::UpdateBuffer(unsigned int, void const*, unsigned int)',[graphics,1,ptr,len(rows)],instruction_limit=2000000);self.put(rt+72,'I',len(rows));vec=self.alloc(24);self.put(vec,'QQQ',ptr,ptr+len(raw),ptr+len(raw));self.put(rt+88,'Q',vec)
  # Synthetic render-input bitmap and shader parameter sinks; pattern bitmap construction/upload remains above.
  bitmap=self.alloc(224);bvt=self.alloc(256);self.put(bitmap,'Q',bvt);self.put(bvt+16,'Q',self.method('bitmap.get_projection'));self.put(bvt+40,'Q',self.method('bitmap.activate'));self.put(bvt+48,'Q',self.method('bitmap.deactivate'));self.put(bitmap,'Q',bvt)
  pattern=self.alloc(224);pvt=self.alloc(256);self.put(pattern,'Q',pvt);self.readable=self.alloc(16);rv=self.alloc(64);self.put(self.readable,'Q',rv);self.put(rv+16,'Q',self.method('pattern.activate'));self.put(pvt+144,'Q',self.method('bitmap.get_readable'));self.put(rt+224,'Q',pattern)
  sw=self.alloc(64);shader=self.alloc(32);sv=self.alloc(128);self.put(shader,'Q',sv);self.put(sv+16,'Q',self.method('shader.activate'));self.put(sw+24,'Q',shader);self.put(rt+208,'Q',sw)
  for off,label in ((0,'shader.ProjectionMatrix'),(8,'shader.sampler0'),(16,'shader.inputColor')):
   ob=self.alloc(16);vt=self.alloc(32);self.put(ob,'Q',vt);self.put(vt,'Q',self.method(label));self.put(sw+off,'Q',ob)
  self.put(rt+44,'4f',.25,.5,.75,.8);caller=self.alloc(64);self.put(caller,'16f',1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.)
  try:self.call('SPen::AirBrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,caller],instruction_limit=2000000)
  except Exception as e:print('NATIVEDRAWFAIL',repr(e),'PC',hex(self.u.reg_read(R.UC_ARM64_REG_PC)),'LR',hex(self.u.reg_read(R.UC_ARM64_REG_LR)),'X0-6',[hex(self.x(i)) for i in range(7)],'TR',[hex(a) for a in getattr(self,'trace',[])],'GL',self.gl,'RT',hex(rt),'BLEND',hex(self.get(rt+216,'Q')[0]),'VP',hex(self.get(self.get(rt+216,'Q')[0],'Q')[0]),'SLOT',hex(self.get(self.get(self.get(rt+216,'Q')[0],'Q')[0]+24,'Q')[0]),'syms',hex(self.syms.get('SPen::GLES::BlendStateObject::Activate()',0)),file=__import__('sys').stderr);raise
  return {'rt':rt,'bitmap_pattern':self.get(rt+224,'Q')[0],'graphics':self.get(rt+200,'Q')[0],'shader':self.get(rt+208,'Q')[0],'blend':self.get(rt+216,'Q')[0],'saved_stamp_count':len(rows),'saved_stamp_bytes':len(raw),'saved_stamp_sha256':hashlib.sha256(raw).hexdigest()}

PINNED = {
    "libSPenComposer.so": "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f",
    "libSPenGraphics.so": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "libSPenRenderer.so": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
    "libSPenBase.so": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "libSPenLibpng.so": "898abcc6ddbe3a4ad7e076484df666b732913db95a7abc7d6e99bfd835f555eb",
    "libSPenZlib.so": "7796339a9c41c3218bef47c17bc1573c544bc431a8d312da390dce40c64adcd7",
    "libSPenPdf.so": "cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04",
    "libc++_shared.so": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
    "libSPenFountainPen.so": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "libSPenPenCommon.so": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "libSPenAirBrushPen.so": "92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423",
}
PINNED_HELPERS = {
    "conformance/airbrush_saved_native.py": "cad2bd9ec88a393f0b0b4a7dbe58c7b469f9b00b283d4c544f491a8ab190999a",
    "conformance/inkpen_paint_native.py": "a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae",
    "conformance/fountain_native.py": "40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
}
for relative, expected_hash in PINNED_HELPERS.items():
    actual_hash = hashlib.sha256((ROOT / relative).read_bytes()).hexdigest()
    if actual_hash != expected_hash:
        raise RuntimeError(f"pinned helper hash mismatch: {relative}: {actual_hash}")

for filename, expected_hash in PINNED.items():
    actual_hash = hashlib.sha256((ROOT / "scratch/apk-analysis-native/arm64-v8a" / filename).read_bytes()).hexdigest()
    if actual_hash != expected_hash:
        raise RuntimeError(f"pinned DSO hash mismatch: {filename}: {actual_hash}")
if hashlib.sha256(LIBZ_PATH.read_bytes()).hexdigest() != LIBZ_SHA256:
    raise RuntimeError("host zlib pin mismatch")

parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group()
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
mode.add_argument("--capture-case", choices=("rgba8_clamp", "rgba8_premul"))
args = parser.parse_args()
if args.capture_case is None:
    captured_cases = []
    for case_name in ("rgba8_clamp", "rgba8_premul"):
        command = [sys.executable, str(Path(__file__).resolve()), "--capture-case", case_name]
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=True)
        try:
            captured_cases.append(json.loads(result.stdout))
        except json.JSONDecodeError as error:
            raise RuntimeError(f"capture stdout was not exactly JSON for {case_name}: {result.stdout[:300]!r}; stderr={result.stderr[-1000:]!r}") from error
    capture_all = {"schema": "sdocx.pdf-image-rgba-native.v1", "cases": captured_cases}
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(capture_all, indent=2, sort_keys=True) + "\n")
        print(json.dumps({"schema": capture_all["schema"], "wrote_fixture": str(FIXTURE.relative_to(ROOT)), "cases": [case["case"] for case in captured_cases]}, sort_keys=True))
    else:
        expected = json.loads(FIXTURE.read_text())
        if capture_all != expected:
            raise SystemExit("native RGBA bitmap capture differs from fixture")
        print(json.dumps({"schema": capture_all["schema"], "matches_fixture": True, "cases": [case["case"] for case in captured_cases]}, sort_keys=True))
    raise SystemExit(0)

CASE = args.capture_case
if CASE == "rgba8_clamp":
    # Nonzero RGB under alpha 0 and values above alpha exercise zeroing and saturation.
    CASE_INPUT = bytes((17, 83, 149, 127, 231, 45, 91, 0, 201, 33, 7, 64, 12, 240, 81, 255))
elif CASE == "rgba8_premul":
    # Every RGB channel is <= alpha; no channel needs saturation during unpremultiplication.
    CASE_INPUT = bytes((16, 32, 64, 128, 0, 17, 34, 68, 1, 2, 3, 4, 100, 150, 200, 255))
else:
    raise RuntimeError(f"unsupported RGBA case {CASE!r}")
CASE_LAYOUT, CASE_TARGET, CASE_ROW_LENGTH = 6, 2, 2
CASE_INTERNAL, CASE_FORMAT = 0x8058, 0x1908
CASE_CREATE_FLAGS = (1, 8, 1)

if hashlib.sha256(LIBZ_PATH.read_bytes()).hexdigest() != LIBZ_SHA256:
    raise RuntimeError("host zlib pin mismatch")
CACHE_DIR = Path(f"/tmp/sdocx-pdf-image-rgba-native/{CASE}/temp_export_pdf")
CACHE_DIR.mkdir(parents=True, exist_ok=True)
for cached in CACHE_DIR.glob("*.png"):
    cached.unlink()
machine = P()
native_trace = []
bitmap_trace_names = (
    "SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool)",
    "SPen::SPSubBitmap::SPSubBitmap(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPSubBitmap::SPSubBitmap(SPen::IGLMsgQueue*, int, SPen::Rect const&, SPen::ITextureObject*, char const*)",
    "SPen::SPSubBitmap::SPSubBitmap(SPen::IGLMsgQueue*, SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool)",
    "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::UpdateBitmap(void const*, int)",
    "SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)",
    "SPen::SPCanvasImpl::GetBitmap()",
    "SPen::SPTextureBitmap::GetSubBitmap(int)",
    "SPen::SPSubBitmapRT::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::SPSubBitmap::GetBitmapData(SPen::Rect const&, void*)",
)
bitmap_trace_addresses = {machine.syms[name]: name for name in bitmap_trace_names if name in machine.syms}

def trace_bitmap_entry(uc, address, _size, _data):
    if address in bitmap_trace_addresses:
        native_trace.append({"name": bitmap_trace_addresses[address], "address": hex(address), "args": [uc.reg_read(R.UC_ARM64_REG_X0 + index) for index in range(5)], "lr": hex(uc.reg_read(R.UC_ARM64_REG_LR))})

machine.u.hook_add(UC_HOOK_CODE, trace_bitmap_entry)
constructor_events = []
lifecycle_snapshots = []
tracked_objects = {}
constructor_map = {address: name for address, name in (
    (0x04000000 + 0x8F140, "SPTextureBitmap ctor(layout)"),
    (0x04000000 + 0x8F5DC, "SPTextureBitmap ctor(Rect)"),
    (0x04000000 + 0x8FE5C, "SPTextureBitmap ctor(two queues)"),
    (0x04000000 + 0x90118, "SPTextureBitmap ctor(copy)"),
    (0x04000000 + 0x8A8D4, "SPSubBitmap ctor(layout)"),
    (0x04000000 + 0x8ACA0, "SPSubBitmap ctor(texture)"),
    (0x04000000 + 0x8B1D8, "SPSubBitmap ctor(two queues)"),
)}
def trace_ctor_early(uc, address, _size, _data):
    if address in constructor_map:
        event = {"name": constructor_map[address], "address": hex(address), "args": [uc.reg_read(R.UC_ARM64_REG_X0 + i) for i in range(8)], "lr": hex(uc.reg_read(R.UC_ARM64_REG_LR))}
        constructor_events.append(event)
        if "SPSubBitmap" in event["name"]: tracked_objects["sub"] = event["args"][0]
        if "SPTextureBitmap" in event["name"]: tracked_objects["factory"] = event["args"][0]
machine.u.hook_add(UC_HOOK_CODE, trace_ctor_early)
def trace_lifecycle(uc, address, _size, _data):
    labels = {0x04000000 + 0x8F33C: "subbitmap-ctor-return", 0x04000000 + 0xC0558: "factory-ctor-return", 0x04000000 + 0xC056C: "factory-create-return"}
    if address in labels:
        for key, ptr in tracked_objects.items():
            vptr = struct.unpack("<Q", bytes(uc.mem_read(ptr + 24, 8)))[0] if key == "sub" else 0
            vtable = [hex(struct.unpack("<Q", bytes(uc.mem_read(vptr + off, 8)))[0]) for off in range(0,96,8)] if vptr else []
            lifecycle_snapshots.append({"stage": labels[address], "object": key, "ptr": hex(ptr), "bytes": bytes(uc.mem_read(ptr,96)).hex(), "interface_vptr_plus24": hex(vptr) if vptr else None, "interface_vtable": vtable})
machine.u.hook_add(UC_HOOK_CODE, trace_lifecycle)

def call_native_with_stack_args(name, register_args, stack_args, instruction_limit=5_000_000):
    for index, value in enumerate(register_args):
        machine.x(index, value)
    stack = 0x200F0000
    machine.u.reg_write(R.UC_ARM64_REG_SP, stack)
    machine.u.reg_write(R.UC_ARM64_REG_LR, 0x200F1000)
    for index, value in enumerate(stack_args):
        machine.put(stack + index * 8, "Q", value)
    machine.u.emu_start(machine.syms[name], 0x200F1000, count=instruction_limit)
    if machine.u.reg_read(R.UC_ARM64_REG_PC) != 0x200F1000:
        raise RuntimeError(f"native instruction limit in {name}")

host_gl = {"next_id": 900, "next_sync": 0x74000000, "syncs": set(), "texture": None, "texture_format": None, "readbacks": [], "gl": [], "fbo": 0, "unpack_row_length": 0, "unpack_row_length_history": [], "shaders": {}, "programs": {}}
host_files = {}
host_stderr = bytearray()
next_host_file = 0x70000000
host_file_events = []
host_zstreams = {}
next_zstream_token = 0x73000000
host_zlib_events = []
libz = ctypes.CDLL(str(LIBZ_PATH))
libc = ctypes.CDLL(None)
libc.malloc.argtypes = [ctypes.c_size_t]
libc.malloc.restype = ctypes.c_void_p
libc.free.argtypes = [ctypes.c_void_p]
libc.free.restype = None
host_zallocations = set()
allocator_errors = []

@ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint, ctypes.c_uint)
def bridge_zalloc(_opaque, items, size):
    total = items * size
    if total > 0x10000000 or (items and total // items != size):
        allocator_errors.append(f"zalloc size overflow: {items} * {size}")
        return None
    pointer = libc.malloc(total)
    if pointer:
        host_zallocations.add(pointer)
    return pointer

@ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_void_p)
def bridge_zfree(_opaque, pointer):
    if pointer:
        if pointer not in host_zallocations:
            allocator_errors.append(f"zfree unknown pointer {pointer:#x}")
            return
        host_zallocations.remove(pointer)
        libc.free(pointer)

class HostZStream(ctypes.Structure):
    _fields_ = [
        ("next_in", ctypes.c_void_p), ("avail_in", ctypes.c_uint), ("total_in", ctypes.c_ulong),
        ("next_out", ctypes.c_void_p), ("avail_out", ctypes.c_uint), ("total_out", ctypes.c_ulong),
        ("msg", ctypes.c_void_p), ("state", ctypes.c_void_p), ("zalloc", ctypes.c_void_p),
        ("zfree", ctypes.c_void_p), ("opaque", ctypes.c_void_p), ("data_type", ctypes.c_int),
        ("adler", ctypes.c_ulong), ("reserved", ctypes.c_ulong),
    ]

libz.zlibVersion.restype = ctypes.c_char_p
libz.deflateInit2_.argtypes = [ctypes.POINTER(HostZStream), ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
libz.deflateInit2_.restype = ctypes.c_int
libz.deflate.argtypes = [ctypes.POINTER(HostZStream), ctypes.c_int]
libz.deflate.restype = ctypes.c_int
libz.deflateEnd.argtypes = [ctypes.POINTER(HostZStream)]
libz.deflateEnd.restype = ctypes.c_int
strtok_state = {"cursor": 0}

def gen_ids():
    count, out = machine.x(0), machine.x(1)
    if count > 16:
        raise RuntimeError(f"host GL object count exceeds bound: {count}")
    ids = list(range(host_gl["next_id"], host_gl["next_id"] + count))
    host_gl["next_id"] += count
    machine.put(out, "I" * count, *ids)
    host_gl["gl"].append({"call": "gen-object", "count": count, "ids": ids})

def bind_object():
    host_gl["fbo"] = machine.x(1)
    host_gl["gl"].append({"call": "bind-object", "args": [machine.x(0), machine.x(1)]})

def attach_texture():
    host_gl["gl"].append({"call": "framebuffer-texture", "args": [machine.x(i) for i in range(5)]})

def framebuffer_status():
    if machine.x(0) != 0x8D40:
        raise RuntimeError(f"unsupported framebuffer target {machine.x(0):#x}")
    raise RuntimeError("non-Mesa framebuffer-status fallback reached; RGBA capture requires the Mesa bridge")

def read_pixels():
    args = [machine.x(i) for i in range(7)]
    x, y, width, height, fmt, typ, destination = args
    if x or y or width != 2 or height != 2 or fmt != 0x1908 or typ != 0x1401:
        raise RuntimeError(f"unsupported native glReadPixels request {args}")
    source = host_gl["texture"]
    raise RuntimeError("non-Mesa glReadPixels fallback reached; RGBA capture requires the Mesa bridge")

def fence_sync():
    condition, flags = machine.x(0), machine.x(1)
    if condition != 0x9117 or flags != 0:
        raise RuntimeError(f"unsupported native glFenceSync request condition={condition:#x} flags={flags:#x}")
    token = host_gl["next_sync"]
    host_gl["next_sync"] += 8
    host_gl["syncs"].add(token)
    host_gl["gl"].append({"call": "fence-sync", "condition": condition, "flags": flags, "sync": hex(token)})
    machine.x(0, token)

def client_wait_sync():
    sync, flags, timeout = machine.x(0), machine.x(1), machine.x(2)
    if sync not in host_gl["syncs"] or flags not in (0, 1) or timeout > 0xFFFFFFFFFFFFFFFF:
        raise RuntimeError(f"unsupported native glClientWaitSync request sync={sync:#x} flags={flags} timeout={timeout}")
    host_gl["gl"].append({"call": "client-wait-sync", "sync": hex(sync), "flags": flags, "timeout": timeout, "result": 0x911A})
    machine.x(0, 0x911A)  # GL_ALREADY_SIGNALED: this host task queue is synchronous.

def delete_sync():
    sync = machine.x(0)
    if sync not in host_gl["syncs"]:
        raise RuntimeError(f"native glDeleteSync on unknown token {sync:#x}")
    host_gl["syncs"].remove(sync)
    host_gl["gl"].append({"call": "delete-sync", "sync": hex(sync)})
    machine.x(0, 0)

def tex_image():
    args = [machine.x(i) for i in range(8)]
    stack = machine.u.reg_read(R.UC_ARM64_REG_SP)
    pixels = machine.get(stack, "Q")[0]
    target, level, internal, width, height, border, fmt, typ = args
    if args != [0x0DE1, 0, CASE_INTERNAL, 2, 2, 0, CASE_FORMAT, 0x1401]:
        raise RuntimeError(f"unsupported native glTexImage2D request {args}")
    channels = 4 if fmt == 0x1908 else 3 if fmt == 0x1907 else 1
    size = width * height * channels
    data = bytes(machine.u.mem_read(pixels, size)) if pixels and size <= 0x1000000 else None
    host_gl["texture"] = data
    host_gl["texture_format"] = internal
    host_gl["gl"].append({"call": "tex-image-2d", "args": args, "pixels_pointer": hex(pixels), "data_hex": data.hex() if data else None})

def tex_sub_image():
    args = [machine.x(i) for i in range(8)]
    stack = machine.u.reg_read(R.UC_ARM64_REG_SP)
    pixels = machine.get(stack, "Q")[0]
    target, level, x, y, width, height, fmt, typ = args
    channels = 4 if fmt == 0x1908 else 3 if fmt == 0x1907 else 1
    size = width * height * channels
    if not pixels or size > 0x1000000:
        raise RuntimeError(f"invalid native glTexSubImage2D pixels={pixels:#x} size={size}")
    data = bytes(machine.u.mem_read(pixels, size))
    if x or y or width != 2 or height != 2 or fmt != CASE_FORMAT or typ != 0x1401:
        raise RuntimeError(f"unsupported native glTexSubImage2D args={args}")
    if CASE.startswith("rgba8_") and host_gl["unpack_row_length"] != 2:
        raise RuntimeError(f"RGBA8 update must request two pixels per unpack row, got {host_gl['unpack_row_length']}")
    host_gl["texture"] = data
    host_gl["gl"].append({"call": "tex-sub-image-2d", "args": args, "pixels_pointer": hex(pixels), "data_hex": data.hex()})

original_pixel_store = machine.callbacks.get("glPixelStorei")
def capture_pixel_store():
    parameter, value = machine.x(0), machine.x(1)
    if parameter == 0x0CF2:  # GL_UNPACK_ROW_LENGTH
        host_gl["unpack_row_length"] = value
        host_gl["unpack_row_length_history"].append(value)
    if original_pixel_store:
        original_pixel_store()
machine.callbacks["glPixelStorei"] = capture_pixel_store

def no_op():
    host_gl["gl"].append({"call": "gl-boundary", "name": machine.plt.get(machine.u.reg_read(R.UC_ARM64_REG_PC), "unknown"), "args": [machine.x(i) for i in range(4)]})

machine.callbacks.update({
    "glGenFramebuffers": gen_ids,
    "glDeleteFramebuffers": no_op,
    "glBindFramebuffer": bind_object,
    "glFramebufferTexture2D": attach_texture,
    "glCheckFramebufferStatus": framebuffer_status,
    "glGenRenderbuffers": gen_ids,
    "glBindRenderbuffer": bind_object,
    "glRenderbufferStorage": no_op,
    "glFramebufferRenderbuffer": no_op,
    "glViewport": no_op,
    "glFlush": no_op,
    "glFinish": no_op,
    "glInvalidateFramebuffer": no_op,
    "glReadPixels": read_pixels,
    "glFenceSync": fence_sync,
    "glClientWaitSync": client_wait_sync,
    "glDeleteSync": delete_sync,
    "glTexImage2D": tex_image,
    "glTexSubImage2D": tex_sub_image,
})
mesa = ip.GLES() if CASE.startswith("rgba8_") else None
mesa_functions = dict(mesa.f) if mesa else {}
if mesa:
    mesa_functions["glFlush"] = ip.cfunc(mesa.gl, "glFlush", None, [])
    mesa_functions["glFinish"] = ip.cfunc(mesa.gl, "glFinish", None, [])
    mesa_functions["glTexSubImage2D"] = ip.cfunc(mesa.gl, "glTexSubImage2D", None,
        [C.c_uint, C.c_int, C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p])
    def early_gen_textures():
        count, output = machine.x(0), machine.x(1)
        if not 0 < count <= 16: raise RuntimeError(f"Mesa texture count outside bound: {count}")
        ids=(C.c_uint*count)();mesa_functions["glGenTextures"](count,ids);machine.put(output,"I"*count,*ids)
        host_gl["gl"].append({"call":"glGenTextures","ids":list(ids),"backend":"Mesa GLES"})
    def early_tex_image():
        args=[machine.x(i) for i in range(8)];pixels=machine.get(machine.u.reg_read(R.UC_ARM64_REG_SP),"Q")[0]
        channels={0x1903:1,0x1906:1,0x1908:4,0x80E1:4}.get(args[6],4);size=args[3]*args[4]*channels
        if size>1<<24:raise RuntimeError(f"texture storage exceeds bound: {size}")
        raw=bytes(machine.u.mem_read(pixels,size)) if pixels and size else b"";owner=C.create_string_buffer(raw) if raw else None
        mesa_functions["glTexImage2D"](*args,C.cast(owner,C.c_void_p) if owner else None)
        host_gl["gl"].append({"call":"glTexImage2D","args":args,"input_sha256":hashlib.sha256(raw).hexdigest() if raw else None,"backend":"Mesa GLES"})
    def early_tex_sub_image():
        args=[machine.x(i) for i in range(8)];pixels=machine.get(machine.u.reg_read(R.UC_ARM64_REG_SP),"Q")[0]
        channels={0x1903:1,0x1906:1,0x1908:4,0x80E1:4}.get(args[6],4);size=args[4]*args[5]*channels
        if not pixels or size>1<<24:raise RuntimeError(f"invalid Mesa texture update {pixels:#x}/{size}")
        raw=bytes(machine.u.mem_read(pixels,size));owner=C.create_string_buffer(raw)
        mesa_functions["glTexSubImage2D"](*args,C.cast(owner,C.c_void_p))
        host_gl["texture"]=raw;host_gl["gl"].append({"call":"glTexSubImage2D","args":args,"input_sha256":hashlib.sha256(raw).hexdigest(),"backend":"Mesa GLES"})
    def early_sync(name, restype, argtypes):
        address=mesa.egl.eglGetProcAddress(name.encode())
        if not address:raise RuntimeError(f"Mesa GLES lacks {name}")
        return C.CFUNCTYPE(restype,*argtypes)(address)
    early_syncs={}
    def early_fence():
        token=machine.alloc(8);sync=early_sync("glFenceSync",C.c_void_p,[C.c_uint,C.c_uint])(machine.x(0),machine.x(1))
        if not sync:raise RuntimeError("Mesa glFenceSync failed")
        early_syncs[token]=sync;machine.x(0,token)
    def early_wait():
        token=machine.x(0)
        if token not in early_syncs:raise RuntimeError(f"unknown Mesa sync {token:#x}")
        fn=early_sync("glClientWaitSync",C.c_uint,[C.c_void_p,C.c_uint,C.c_uint64]);machine.x(0,fn(early_syncs[token],machine.x(1),machine.x(2)))
    def early_delete_sync():
        token=machine.x(0)
        if token not in early_syncs:raise RuntimeError(f"unknown Mesa sync {token:#x}")
        early_sync("glDeleteSync",None,[C.c_void_p])(early_syncs.pop(token));machine.x(0,0)
    machine.callbacks.update({
        "glGenTextures":early_gen_textures,
        "glBindTexture":lambda:mesa_functions["glBindTexture"](machine.x(0),machine.x(1)),
        "glTexImage2D":early_tex_image,
        "glTexSubImage2D":early_tex_sub_image,
        "glTexParameteri":lambda:mesa_functions["glTexParameteri"](machine.x(0),machine.x(1),machine.x(2)),
        "glActiveTexture":lambda:mesa_functions["glActiveTexture"](machine.x(0)),
        "glPixelStorei":lambda:mesa_functions["glPixelStorei"](machine.x(0),machine.x(1)),
        "glFenceSync":early_fence,"glClientWaitSync":early_wait,"glDeleteSync":early_delete_sync,
        "glFlush":lambda:mesa_functions["glFlush"](),"glFinish":lambda:mesa_functions["glFinish"](),
        "glGetError":lambda:machine.x(0,mesa_functions["glGetError"]()),
    })

# The render-thread queue executes each queued native task synchronously. This
# is an explicit scheduler boundary; actual factory/bitmap methods still run.
queue, queue_vtable = machine.alloc(32), machine.alloc(64)
machine.queue = queue
machine.queue_post = machine.alloc(16)
machine.queue_task_return = machine.alloc(16)
machine.queue_wait = machine.alloc(16)
machine.pending_queue_return = None
machine.put(queue, "Q", queue_vtable)
machine.put(queue_vtable + 16, "Q", machine.queue_post)
machine.put(queue_vtable + 24, "Q", machine.queue_wait)

cache_root = f"/tmp/sdocx-pdf-image-rgba-native/{CASE}".encode() + b"\0"
cache_path = machine.alloc(len(cache_root))
machine.u.mem_write(cache_path, cache_root)
machine.call("SPen::System::SetSDKCacheDirectoryPath(char const*)", [cache_path])

pixels = CASE_INPUT
pixel_ptr = machine.alloc(len(pixels))
machine.u.mem_write(pixel_ptr, pixels)
bitmap_name = machine.alloc(32)
machine.u.mem_write(bitmap_name, b"pdf-image-probe\0")
factory_name = "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)"

try:
 call_native_with_stack_args(factory_name, [queue, 2, 2, bitmap_name, pixel_ptr, *CASE_CREATE_FLAGS], [CASE_LAYOUT, CASE_TARGET])
except Exception as e:
 print("QUEUE_FAIL",repr(e),"PC",hex(machine.u.reg_read(R.UC_ARM64_REG_PC)),"LR",hex(machine.u.reg_read(R.UC_ARM64_REG_LR)),"SP",hex(machine.u.reg_read(R.UC_ARM64_REG_SP)),"X",[hex(machine.x(i)) for i in range(8)],"TRACE",[hex(a) for a in machine.trace[-30:]],"QPOST",hex(machine.queue_post),file=sys.stderr);raise
factory_bitmap = machine.x(0)
machine.call("SPen::SPTextureBitmap::UpdateBitmap(void const*, int)", [factory_bitmap, pixel_ptr, CASE_ROW_LENGTH], instruction_limit=5_000_000)
bitmap = factory_bitmap
rect = machine.alloc(16)
machine.put(rect, "4i", 0, 0, 2, 2)
readback = machine.alloc(16)
machine.u.mem_write(readback, b"\xcc" * 16)
rgba = None  # populated from the actual Mesa glReadPixels destination after native SetImage

# Extend the same pinned guest with the actual Composer consumer, the APK's
# actual Base/libpng/zlib dependencies, and the SDK cache ABI already set above.
composer_path = ROOT / "scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so"
composer_blob = composer_path.read_bytes()
composer_base = 0x06000000
composer_phoff = struct.unpack_from("<Q", composer_blob, 32)[0]
composer_phent, composer_phnum = struct.unpack_from("<HH", composer_blob, 54)
composer_end = max(
    struct.unpack_from("<II6Q", composer_blob, composer_phoff + index * composer_phent)[3]
    + struct.unpack_from("<II6Q", composer_blob, composer_phoff + index * composer_phent)[6]
    for index in range(composer_phnum)
    if struct.unpack_from("<II6Q", composer_blob, composer_phoff + index * composer_phent)[0] == 1
)
machine.u.mem_map(composer_base, (composer_end + 0xFFF) & ~0xFFF)
for index in range(composer_phnum):
    header = struct.unpack_from("<II6Q", composer_blob, composer_phoff + index * composer_phent)
    if header[0] == 1 and header[5]:
        machine.u.mem_write(composer_base + header[3], composer_blob[header[2]:header[2] + header[5]])
nm = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm"
objdump = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump"
runtime_lib = ROOT / "scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
env = {**os.environ, "LD_LIBRARY_PATH": str(runtime_lib)}
for line in subprocess.run([str(nm), "-D", "-C", str(composer_path)], check=True, capture_output=True, text=True, env=env).stdout.splitlines():
    columns = line.strip().split(" ", 2)
    if len(columns) == 3 and columns[1] in ("T", "W", "V", "D", "B", "R"):
        machine.syms[columns[2]] = composer_base + int(columns[0], 16)
for address, symbol in re.findall(
    r"^([0-9a-f]+) <(.+)@plt>:",
    subprocess.run([str(objdump), "-d", "-C", "-j", ".plt", str(composer_path)], check=True, capture_output=True, text=True, env=env).stdout,
    re.M,
):
    machine.plt[composer_base + int(address, 16)] = symbol
machine.map_extra("Libpng", 7)
machine.map_extra("Zlib", 8)
machine.map_extra("Pdf", 9)
libcpp_path = ROOT / "scratch/apk-analysis-native/arm64-v8a/libc++_shared.so"
libcpp_blob = libcpp_path.read_bytes()
libcpp_base = 0x0A000000
if __import__("hashlib").sha256(libcpp_blob).hexdigest() != "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4":
    raise RuntimeError("pinned libc++_shared.so hash mismatch")
libcpp_phoff = struct.unpack_from("<Q", libcpp_blob, 32)[0]
libcpp_phent, libcpp_phnum = struct.unpack_from("<HH", libcpp_blob, 54)
libcpp_end = max(
    struct.unpack_from("<II6Q", libcpp_blob, libcpp_phoff + index * libcpp_phent)[3]
    + struct.unpack_from("<II6Q", libcpp_blob, libcpp_phoff + index * libcpp_phent)[6]
    for index in range(libcpp_phnum)
    if struct.unpack_from("<II6Q", libcpp_blob, libcpp_phoff + index * libcpp_phent)[0] == 1
)
machine.u.mem_map(libcpp_base, (libcpp_end + 0xFFF) & ~0xFFF)
for index in range(libcpp_phnum):
    header = struct.unpack_from("<II6Q", libcpp_blob, libcpp_phoff + index * libcpp_phent)
    if header[0] == 1 and header[5]:
        machine.u.mem_write(libcpp_base + header[3], libcpp_blob[header[2]:header[2] + header[5]])
for line in subprocess.run([str(nm), "-D", "-C", str(libcpp_path)], check=True, capture_output=True, text=True, env=env).stdout.splitlines():
    columns = line.strip().split(" ", 2)
    if len(columns) == 3 and columns[1] in ("T", "W", "V", "D", "B", "R"):
        machine.syms[columns[2]] = libcpp_base + int(columns[0], 16)
for address, symbol in re.findall(
    r"^([0-9a-f]+) <(.+)@plt>:",
    subprocess.run([str(objdump), "-d", "-C", "-j", ".plt", str(libcpp_path)], check=True, capture_output=True, text=True, env=env).stdout,
    re.M,
):
    machine.plt[libcpp_base + int(address, 16)] = symbol
machine.load_extra_relocs()

# Apply the pinned libc++ .rela.dyn data relocations needed by its real methods.
libcpp_shoff = struct.unpack_from("<Q", libcpp_blob, 40)[0]
libcpp_shent, libcpp_shnum, libcpp_shstrndx = struct.unpack_from("<HHH", libcpp_blob, 58)
libcpp_sections = [struct.unpack_from("<IIQQQQIIQQ", libcpp_blob, libcpp_shoff + i * libcpp_shent) for i in range(libcpp_shnum)]
libcpp_shstr = libcpp_sections[libcpp_shstrndx]
libcpp_names = libcpp_blob[libcpp_shstr[4]:libcpp_shstr[4] + libcpp_shstr[5]]
libcpp_by_name = {libcpp_names[section[0]:libcpp_names.find(b"\0", section[0])].decode(): section for section in libcpp_sections}
libcpp_bss = libcpp_by_name[".bss"]
machine.host_static_mutex_ranges.append((libcpp_base + libcpp_bss[3], libcpp_base + libcpp_bss[3] + libcpp_bss[5]))
libcpp_symtab = libcpp_by_name[".dynsym"]
libcpp_dynstr = libcpp_by_name[".dynstr"]
libcpp_strings = libcpp_blob[libcpp_dynstr[4]:libcpp_dynstr[4] + libcpp_dynstr[5]]
libcpp_symbols = []
libcpp_defined_symbols = {}
for offset in range(libcpp_symtab[4], libcpp_symtab[4] + libcpp_symtab[5], libcpp_symtab[9]):
    name_offset, info, other, section_index, value, size = struct.unpack_from("<IBBHQQ", libcpp_blob, offset)
    end = libcpp_strings.find(b"\0", name_offset)
    name = libcpp_strings[name_offset:end].decode() if name_offset else ""
    libcpp_symbols.append(name)
    if name and section_index:
        libcpp_defined_symbols[name] = libcpp_base + value

libcpp_plt = libcpp_by_name[".plt"]
libcpp_rela_plt = libcpp_by_name[".rela.plt"]
for index, offset in enumerate(range(libcpp_rela_plt[4], libcpp_rela_plt[4] + libcpp_rela_plt[5], libcpp_rela_plt[9])):
    _where, info, _addend = struct.unpack_from("<QQq", libcpp_blob, offset)
    symbol_index = info >> 32
    import_name = libcpp_symbols[symbol_index].split("@", 1)[0]
    # Android's AArch64 PLT has a 32-byte resolver header and 24-byte slots.
    machine.plt[libcpp_base + libcpp_plt[3] + 32 + index * 24] = import_name
    if import_name in libcpp_defined_symbols:
        machine.syms[import_name] = libcpp_defined_symbols[import_name]
libcpp_rela = libcpp_by_name.get(".rela.dyn")
if libcpp_rela:
    for offset in range(libcpp_rela[4], libcpp_rela[4] + libcpp_rela[5], libcpp_rela[9]):
        where, info, addend = struct.unpack_from("<QQq", libcpp_blob, offset)
        relocation_type, symbol_index = info & 0xFFFFFFFF, info >> 32
        if relocation_type == 0x403:
            value = libcpp_base + addend
        elif relocation_type == 0x401 and libcpp_symbols[symbol_index]:
            value = machine.resolve_symbol(libcpp_symbols[symbol_index])
        elif relocation_type == 0x101:
            value = machine.resolve_symbol(libcpp_symbols[symbol_index]) + addend
        else:
            continue
        machine.put(libcpp_base + where, "Q", value)

# libc++ uses pthread_once during its real static initialization. This tiny
# guest thunk executes the supplied guest initializer exactly once in the
# single-threaded probe process.
pthread_once_thunk = 0x200F2080
machine.pthread_once_thunk=pthread_once_thunk
machine.pthread_once_calls=[]
machine.u.mem_write(pthread_once_thunk, bytes.fromhex("080040f9080100b5fd7bbfa9fd030091280080d2080000f920003fd6fd7bc1a8000080d2c0035fd6"))
machine.syms["pthread_once"] = pthread_once_thunk

def run_native_init_array(path, base):
    blob = path.read_bytes()
    shoff = struct.unpack_from("<Q", blob, 40)[0]
    entry_size, count, string_index = struct.unpack_from("<HHH", blob, 58)
    sections = [struct.unpack_from("<IIQQQQIIQQ", blob, shoff + index * entry_size) for index in range(count)]
    strings_section = sections[string_index]
    strings = blob[strings_section[4]:strings_section[4] + strings_section[5]]
    init_section = next((section for section in sections if strings[section[0]:strings.find(b"\0", section[0])] == b".init_array"), None)
    if init_section is None:
        return 0
    called = 0
    for offset in range(0, init_section[5], 8):
        target = machine.get(base + init_section[3] + offset, "Q")[0]
        if not target:
            continue
        for register in range(4):
            machine.x(register, 0)
        machine.u.reg_write(R.UC_ARM64_REG_SP, 0x200EF000)
        machine.u.reg_write(R.UC_ARM64_REG_LR, 0x200F1000)
        try:
            machine.u.emu_start(target, 0x200F1000, count=3_000_000)
        except Exception as error:
            raise RuntimeError(f"{path.name} init-array entry {target:#x} faulted at pc={machine.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={machine.u.reg_read(R.UC_ARM64_REG_LR):#x}; trace={[hex(a) for a in getattr(machine,'trace',[])]}: {error}") from error
        if machine.u.reg_read(R.UC_ARM64_REG_PC) != 0x200F1000:
            raise RuntimeError(f"{path.name} init-array entry {target:#x} failed at {machine.u.reg_read(R.UC_ARM64_REG_PC):#x}")
        called += 1
    return called

trace_names = (
    "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::UpdateBitmap(void const*, int)",
    "SPen::SPTextureBitmap::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::System::SetSDKCacheDirectoryPath(char const*)",
    "SPen::PdfImageAdapter::PdfImageAdapter(std::__ndk1::shared_ptr<SPen::PdfExporter>, SPen::IContext const*)",
    "SPen::PdfImageAdapter::SetImage(SPen::ISPBitmap*)",
    "SPen::PdfImageAdapter::saveBitmap(SPen::ISPBitmap*, SPen::Rect, SPen::String&)",
    "SPen::SPSubBitmapRT::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::SPSubBitmap::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::Bitmap::Construct(unsigned char*, int, int, int, SPen::Bitmap::Format, bool, bool)",
    "SPen::BitmapFactory::SaveBitmap(SPen::Bitmap const*, SPen::String const*, int)",
    "SPen::System::GetSDKCacheDirectoryPath()",
    "SPen::PDFUtil::String2string(SPen::String const*)",
    "png_create_write_struct",
    "png_set_write_fn",
    "png_write_info",
    "png_write_image",
    "png_write_end",
    "png_write_png",
    "png_destroy_write_struct",
)
trace_addresses = {machine.syms[name]: name for name in trace_names if name in machine.syms}

def trace_native_entry(uc, address, _size, _data):
    if address in machine.plt and ("SPTextureBitmap" in machine.plt[address] or "SPSubBitmap" in machine.plt[address]):
        constructor_events.append({"plt": hex(address), "name": machine.plt[address], "args": [uc.reg_read(R.UC_ARM64_REG_X0 + i) for i in range(6)], "lr": hex(uc.reg_read(R.UC_ARM64_REG_LR))})
    if address in trace_addresses:
        native_trace.append({"name": trace_addresses[address], "address": hex(address), "args": [uc.reg_read(R.UC_ARM64_REG_X0 + index) for index in range(5)], "lr": hex(uc.reg_read(R.UC_ARM64_REG_LR))})

machine.u.hook_add(UC_HOOK_CODE, trace_native_entry)
machine.call("SPen::System::SetSDKCacheDirectoryPath(char const*)", [cache_path])

def guest_string(pointer, limit=4096):
    value = bytearray()
    for offset in range(limit):
        byte = bytes(machine.u.mem_read(pointer + offset, 1))
        if byte == b"\0":
            return value.decode("utf-8", errors="replace")
        value.extend(byte)
    raise RuntimeError("guest string exceeded bound")

def host_mkdir():
    path = guest_string(machine.x(0))
    os.makedirs(path, mode=machine.x(1) & 0o777, exist_ok=True)
    machine.x(0, 0)

def host_stat():
    path, output = guest_string(machine.x(0)), machine.x(1)
    try:
        result = os.stat(path)
    except FileNotFoundError:
        machine.u.mem_write(output, bytes(128))
        machine.x(0, (-1) & ((1 << 64) - 1))
        return
    value = bytearray(128)
    struct.pack_into("<Q", value, 0, result.st_dev)
    struct.pack_into("<Q", value, 8, result.st_ino)
    struct.pack_into("<I", value, 16, result.st_mode)
    struct.pack_into("<I", value, 20, result.st_nlink)
    struct.pack_into("<I", value, 24, result.st_uid)
    struct.pack_into("<I", value, 28, result.st_gid)
    struct.pack_into("<Q", value, 32, result.st_rdev)
    struct.pack_into("<q", value, 48, result.st_size)
    struct.pack_into("<I", value, 56, result.st_blksize)
    struct.pack_into("<Q", value, 64, result.st_blocks)
    struct.pack_into("<qq", value, 72, int(result.st_atime), result.st_atime_ns % 1_000_000_000)
    struct.pack_into("<qq", value, 88, int(result.st_mtime), result.st_mtime_ns % 1_000_000_000)
    struct.pack_into("<qq", value, 104, int(result.st_ctime), result.st_ctime_ns % 1_000_000_000)
    machine.u.mem_write(output, bytes(value))
    machine.x(0, 0)

def host_strtok():
    start = machine.x(0) or strtok_state["cursor"]
    delimiters = set(guest_string(machine.x(1)))
    if not start:
        machine.x(0, 0)
        return
    for offset in range(4096):
        char = bytes(machine.u.mem_read(start + offset, 1))[0]
        if char == 0:
            strtok_state["cursor"] = 0
            machine.x(0, 0)
            return
        if chr(char) not in delimiters:
            start += offset
            break
    else:
        raise RuntimeError("strtok scan bound")
    end = start
    for _ in range(4096):
        char = bytes(machine.u.mem_read(end, 1))[0]
        if char == 0:
            strtok_state["cursor"] = 0
            machine.x(0, start)
            return
        if chr(char) in delimiters:
            machine.u.mem_write(end, b"\0")
            strtok_state["cursor"] = end + 1
            machine.x(0, start)
            return
        end += 1
    raise RuntimeError("strtok token bound")

def host_strcmp():
    left = guest_string(machine.x(0)).encode()
    right = guest_string(machine.x(1)).encode()
    for first, second in zip(left + b"\0", right + b"\0"):
        if first != second:
            machine.x(0, (first - second) & ((1 << 64) - 1))
            return
        if first == 0:
            machine.x(0, 0)
            return

def host_strncmp():
    count = min(machine.x(2), 4096)
    left = bytes(machine.u.mem_read(machine.x(0), count))
    right = bytes(machine.u.mem_read(machine.x(1), count))
    for first, second in zip(left, right):
        if first != second:
            machine.x(0, (first - second) & ((1 << 64) - 1))
            return
        if first == 0:
            machine.x(0, 0)
            return
    machine.x(0, 0)

def host_strchr():
    pointer, character = machine.x(0), machine.x(1) & 0xFF
    for offset in range(4096):
        value = bytes(machine.u.mem_read(pointer + offset, 1))[0]
        if value == character:
            machine.x(0, pointer + offset)
            return
        if value == 0:
            machine.x(0, 0)
            return
    raise RuntimeError("strchr scan bound")

def host_strncpy_chk2():
    destination, source, count, destination_size, source_size = (machine.x(i) for i in range(5))
    if count > 1 << 20 or count > destination_size or source_size > 1 << 20:
        raise RuntimeError(f"__strncpy_chk2 bounds count={count} dst={destination_size} src={source_size}")
    raw = bytes(machine.u.mem_read(source, source_size))
    nul = raw.find(b"\0")
    if nul < 0 and count > source_size:
        raise RuntimeError(f"__strncpy_chk2 source lacks terminator within {source_size} bytes")
    prefix = raw[:nul] if nul >= 0 else raw
    copied = prefix[:count]
    output = copied + bytes(max(0, count - len(copied)))
    machine.u.mem_write(destination, output)
    machine.x(0, destination)

def host_fopen():
    global next_host_file
    path, mode = guest_string(machine.x(0)), guest_string(machine.x(1))
    # The SDK's cached program blobs are Android/vendor-driver artifacts and
    # cannot be handed to the separate Mesa GLES implementation used here.
    # Model a normal cache miss for that namespace; all shader compilation and
    # binary production below then runs against Mesa itself.
    if path.startswith("/tmp/RendererCache/") and "r" in mode:
        host_file_events.append({"event": "fopen", "path": path, "mode": mode, "result": "host-driver cache miss"})
        machine.x(0, 0)
        return
    try:
        handle = open(path, mode)
    except OSError:
        machine.x(0, 0)
        return
    token = next_host_file
    next_host_file += 0x10
    host_files[token] = handle
    host_file_events.append({"event": "fopen", "path": path, "mode": mode})
    machine.x(0, token)

def host_fwrite():
    source, size, count, token = [machine.x(index) for index in range(4)]
    if token == 0x20002188 and size * count <= 4096:
        data=bytes(machine.u.mem_read(source,size*count))
        host_stderr.extend(data)
        host_file_events.append({"event":"stderr-fwrite","bytes":len(data),"data":data.decode("utf-8","replace")})
        machine.x(0,count)
        return
    if token not in host_files or size * count > 0x10000000:
        raw=bytes(machine.u.mem_read(source,min(size*count,4096))) if source and size*count else b''
        raise RuntimeError(f"invalid native fwrite stream={token:#x} size={size} count={count} data={raw!r} pc={machine.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={machine.u.reg_read(R.UC_ARM64_REG_LR):#x}")
    data = bytes(machine.u.mem_read(source, size * count))
    written = host_files[token].write(data)
    host_file_events.append({"event": "fwrite", "bytes": written})
    machine.x(0, written // size if size else 0)

def host_fread():
    destination, size, count, token = (machine.x(index) for index in range(4))
    if token not in host_files or size * count > 0x10000000:
        raise RuntimeError(f"invalid native fread stream={token:#x} size={size} count={count}")
    data = host_files[token].read(size * count)
    if data and destination:
        machine.u.mem_write(destination, data)
    elements = len(data) // size if size else 0
    host_file_events.append({"event": "fread", "bytes": len(data), "elements": elements})
    machine.x(0, elements)

def host_fclose():
    token = machine.x(0)
    if token not in host_files:
        raise RuntimeError(f"invalid native fclose stream={token:#x}")
    host_files.pop(token).close()
    host_file_events.append({"event": "fclose"})
    machine.x(0, 0)

def host_fflush():
    token = machine.x(0)
    if token in host_files:
        host_files[token].flush()
    machine.x(0, 0)

def host_fsync():
    result = os.fsync(machine.x(0))
    machine.x(0, result if result is not None else 0)

def host_setbuf():
    token = machine.x(0)
    if token not in host_files:
        raise RuntimeError(f"invalid native setbuf stream={token:#x}")
    # Python's host file object already owns its buffering strategy; libpng
    # only requests a caller buffer here, so leaving buffering unchanged is a
    # valid host stdio behavior for this probe.
    machine.x(0, 0)

def host_fileno():
    token = machine.x(0)
    if token not in host_files:
        raise RuntimeError(f"invalid native fileno stream={token:#x}")
    machine.x(0, host_files[token].fileno())

def host_setjmp():
    # Successful PNG encoding follows the initial setjmp return. A later
    # longjmp is an unsupported error path and remains fail-closed.
    host_gl["gl"].append({"call": "setjmp-initial", "buffer": hex(machine.x(0))})
    machine.x(0, 0)

def host_longjmp():
    raise RuntimeError(f"native longjmp error path reached env={machine.x(0):#x} value={machine.x(1)}")

def host_crc32():
    checksum, pointer, length = machine.x(0), machine.x(1), machine.x(2)
    if length > 0x10000000 or (length and not pointer):
        raise RuntimeError(f"invalid crc32 input pointer={pointer:#x} length={length}")
    payload = bytes(machine.u.mem_read(pointer, length)) if length else b""
    machine.x(0, zlib.crc32(payload, checksum) & 0xFFFFFFFF)

def host_adler32():
    checksum, pointer, length = machine.x(0), machine.x(1), machine.x(2)
    if length > 0x10000000 or (length and not pointer):
        raise RuntimeError(f"invalid adler32 input pointer={pointer:#x} length={length}")
    payload = bytes(machine.u.mem_read(pointer, length)) if length else b""
    machine.x(0, zlib.adler32(payload, checksum) & 0xFFFFFFFF)

def host_deflate_init2():
    global next_zstream_token
    guest_stream = machine.x(0)
    level, method, window, mem_level, strategy = [machine.x(index) for index in range(1, 6)]
    version = guest_string(machine.x(6)).encode()
    size = machine.x(7)
    if size != ctypes.sizeof(HostZStream):
        raise RuntimeError(f"z_stream ABI size mismatch guest={size} host={ctypes.sizeof(HostZStream)}")
    zalloc = machine.get(guest_stream + 64, "Q")[0]
    zfree = machine.get(guest_stream + 72, "Q")[0]
    opaque = machine.get(guest_stream + 80, "Q")[0]
    stream = HostZStream()
    stream.zalloc = ctypes.cast(bridge_zalloc, ctypes.c_void_p).value
    stream.zfree = ctypes.cast(bridge_zfree, ctypes.c_void_p).value
    stream.opaque = None
    token = next_zstream_token
    next_zstream_token += 0x10
    host_zstreams[guest_stream] = (stream, token, {"guest_zalloc": zalloc, "guest_zfree": zfree, "guest_opaque": opaque})
    result = libz.deflateInit2_(ctypes.byref(stream), level, method, window, mem_level, strategy, version, size)
    host_zlib_events.append({"event": "deflateInit2_", "guest_stream": hex(guest_stream), "level": level, "method": method, "window_bits": window, "mem_level": mem_level, "strategy": strategy, "version": version.decode(), "stream_size": size, "guest_allocator_pointers": [hex(value) if value else None for value in (zalloc, zfree, opaque)], "host_result": result})
    machine.put(guest_stream + 48, "Q", 0)  # msg
    machine.put(guest_stream + 56, "Q", token)  # state handle; guest does not dereference it
    # Preserve the guest png allocator pointers in guest memory. The actual
    # host zlib receives C ABI bridges, never the AArch64 code addresses.
    machine.put(guest_stream + 88, "I", stream.data_type)
    machine.put(guest_stream + 96, "Q", stream.adler)
    machine.put(guest_stream + 104, "Q", stream.reserved)
    machine.x(0, result)

def host_deflate():
    guest_stream, flush = machine.x(0), machine.x(1)
    if guest_stream not in host_zstreams:
        raise RuntimeError(f"deflate on uninitialized guest stream {guest_stream:#x}")
    stream, _token, _allocators = host_zstreams[guest_stream]
    next_in = machine.get(guest_stream, "Q")[0]
    avail_in = machine.get(guest_stream + 8, "I")[0]
    next_out = machine.get(guest_stream + 24, "Q")[0]
    avail_out = machine.get(guest_stream + 32, "I")[0]
    if avail_in > 0x10000000 or avail_out > 0x10000000 or (avail_in and not next_in) or (avail_out and not next_out):
        raise RuntimeError(f"invalid deflate buffers in={next_in:#x}/{avail_in} out={next_out:#x}/{avail_out}")
    input_bytes = bytes(machine.u.mem_read(next_in, avail_in)) if avail_in else b""
    input_storage = ctypes.create_string_buffer(input_bytes) if input_bytes else None
    output_storage = ctypes.create_string_buffer(avail_out) if avail_out else None
    stream.next_in = ctypes.cast(input_storage, ctypes.c_void_p).value if input_storage else None
    stream.avail_in = avail_in
    stream.next_out = ctypes.cast(output_storage, ctypes.c_void_p).value if output_storage else None
    stream.avail_out = avail_out
    before_in, before_out = stream.total_in, stream.total_out
    result = libz.deflate(ctypes.byref(stream), flush)
    consumed, produced = stream.total_in - before_in, stream.total_out - before_out
    host_zlib_events.append({"event": "deflate", "flush": flush, "consumed": consumed, "produced": produced, "host_result": result})
    if produced:
        machine.u.mem_write(next_out, ctypes.string_at(output_storage, produced))
    machine.put(guest_stream, "Q", next_in + consumed)
    machine.put(guest_stream + 8, "I", stream.avail_in)
    machine.put(guest_stream + 16, "Q", stream.total_in)
    machine.put(guest_stream + 24, "Q", next_out + produced)
    machine.put(guest_stream + 32, "I", stream.avail_out)
    machine.put(guest_stream + 40, "Q", stream.total_out)
    machine.put(guest_stream + 88, "I", stream.data_type)
    machine.put(guest_stream + 96, "Q", stream.adler)
    machine.x(0, result)

def host_deflate_end():
    guest_stream = machine.x(0)
    if guest_stream not in host_zstreams:
        raise RuntimeError(f"deflateEnd on uninitialized guest stream {guest_stream:#x}")
    stream, _token, _allocators = host_zstreams.pop(guest_stream)
    result = libz.deflateEnd(ctypes.byref(stream))
    host_zlib_events.append({"event": "deflateEnd", "host_result": result})
    machine.x(0, result)

def host_zlib_version():
    return_pointer = machine.alloc(16)
    machine.u.mem_write(return_pointer, libz.zlibVersion() + b"\0")
    machine.x(0, return_pointer)

def gl_create_shader():
    shader_type=machine.x(0);token=host_gl["next_id"];host_gl["next_id"]+=1
    if shader_type not in (0x8B30,0x8B31):raise RuntimeError(f'unsupported GL shader type {shader_type:#x}')
    host_gl["shaders"][token]={"type":shader_type,"source_bytes":0,"compile_requested":False}
    host_gl["gl"].append({"call":"create-shader","type":shader_type,"token":token,"host_simulated":True})
    machine.x(0,token)

def gl_shader_source():
    shader,count,strings,lengths=(machine.x(i) for i in range(4))
    if shader not in host_gl["shaders"] or not 0<count<=16:raise RuntimeError(f'unsupported glShaderSource shader={shader} count={count}')
    total=0
    for index in range(count):
        pointer=machine.get(strings+8*index,"Q")[0]
        length=machine.get(lengths+4*index,"i")[0] if lengths else -1
        if length<0:length=len(guest_string(pointer,0x100000).encode())
        if length>0x100000:raise RuntimeError(f'GL shader source length exceeded: {length}')
        total+=length
    host_gl["shaders"][shader]["source_bytes"]=total
    host_gl["gl"].append({"call":"shader-source","shader":shader,"strings":count,"bytes":total,"host_simulated":True})

def gl_compile_shader():
    shader=machine.x(0)
    if shader not in host_gl["shaders"]:raise RuntimeError(f'compile unknown shader {shader}')
    host_gl["shaders"][shader]["compile_requested"]=True
    host_gl["gl"].append({"call":"compile-shader-request","shader":shader,"host_status_stub":True})

def gl_shader_parameter():
    shader,pname,out=machine.x(0),machine.x(1),machine.x(2)
    if shader not in host_gl["shaders"] or pname not in (0x8B81,0x8B84):raise RuntimeError(f'unsupported glGetShaderiv shader={shader} pname={pname:#x}')
    value=1 if pname==0x8B81 else 1
    machine.put(out,"i",value);host_gl["gl"].append({"call":"get-shader-parameter","shader":shader,"pname":pname,"result":value,"host_status_stub":True})

def gl_create_program():
    token=host_gl["next_id"];host_gl["next_id"]+=1
    host_gl["programs"][token]={"shaders":[],"link_requested":False}
    host_gl["gl"].append({"call":"create-program","token":token,"host_simulated":True});machine.x(0,token)

def gl_attach_shader():
    program,shader=machine.x(0),machine.x(1)
    if program not in host_gl["programs"] or shader not in host_gl["shaders"]:raise RuntimeError('glAttachShader unknown handle')
    host_gl["programs"][program]["shaders"].append(shader);host_gl["gl"].append({"call":"attach-shader","program":program,"shader":shader})

def gl_link_program():
    program=machine.x(0)
    if program not in host_gl["programs"]:raise RuntimeError(f'link unknown program {program}')
    host_gl["programs"][program]["link_requested"]=True;host_gl["gl"].append({"call":"link-program-request","program":program,"host_status_stub":True})

def gl_program_parameter():
    program,pname,out=machine.x(0),machine.x(1),machine.x(2)
    if program not in host_gl["programs"] or pname not in (0x8B82,0x8B84):raise RuntimeError(f'unsupported glGetProgramiv program={program} pname={pname:#x}')
    value=1
    machine.put(out,"i",value);host_gl["gl"].append({"call":"get-program-parameter","program":program,"pname":pname,"result":value,"host_status_stub":True})

def gl_info_log():
    obj,max_length,written,buffer=(machine.x(i) for i in range(4))
    if max_length<0 or max_length>4096:raise RuntimeError(f'unsupported shader/program info log size {max_length}')
    if buffer and max_length:machine.u.mem_write(buffer,b'\0')
    if written:machine.put(written,'i',0)
    host_gl["gl"].append({"call":"info-log-empty","object":obj,"capacity":max_length})

def gl_delete_shader():
    shader=machine.x(0);host_gl["shaders"].pop(shader,None);host_gl["gl"].append({"call":"delete-shader","shader":shader})

def gl_delete_program():
    program=machine.x(0);host_gl["programs"].pop(program,None);host_gl["gl"].append({"call":"delete-program","program":program})

mesa_functions = dict(mesa_functions)
if mesa:
    specs = {
        "glGenFramebuffers": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glDeleteFramebuffers": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glBindFramebuffer": (None, [C.c_uint, C.c_uint]),
        "glFramebufferTexture2D": (None, [C.c_uint, C.c_uint, C.c_uint, C.c_uint, C.c_int]),
        "glCheckFramebufferStatus": (C.c_uint, [C.c_uint]),
        "glGenRenderbuffers": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glDeleteRenderbuffers": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glBindRenderbuffer": (None, [C.c_uint, C.c_uint]),
        "glRenderbufferStorage": (None, [C.c_uint, C.c_uint, C.c_int, C.c_int]),
        "glFramebufferRenderbuffer": (None, [C.c_uint, C.c_uint, C.c_uint, C.c_uint]),
        "glViewport": (None, [C.c_int, C.c_int, C.c_int, C.c_int]),
        "glFlush": (None, []),
        "glFinish": (None, []),
        "glInvalidateFramebuffer": (None, [C.c_uint, C.c_int, C.POINTER(C.c_uint)]),
        "glReadPixels": (None, [C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p]),
        "glGetIntegerv": (None, [C.c_uint, C.POINTER(C.c_int)]),
        "glGetError": (C.c_uint, []),
        "glGenTextures": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glDeleteTextures": (None, [C.c_int, C.POINTER(C.c_uint)]),
        "glBindTexture": (None, [C.c_uint, C.c_uint]),
        "glTexImage2D": (None, [C.c_uint, C.c_int, C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p]),
        "glTexSubImage2D": (None, [C.c_uint, C.c_int, C.c_int, C.c_int, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_void_p]),
        "glTexParameteri": (None, [C.c_uint, C.c_uint, C.c_int]),
        "glPixelStorei": (None, [C.c_uint, C.c_int]),
        "glActiveTexture": (None, [C.c_uint]),
        "glGetString": (C.c_char_p, [C.c_uint]),
        "glDetachShader": (None, [C.c_uint, C.c_uint]),
        "glDeleteShader": (None, [C.c_uint]),
        "glDeleteProgram": (None, [C.c_uint]),
        "glUniform1i": (None, [C.c_int, C.c_int]),
        "glUniform1f": (None, [C.c_int, C.c_float]),
        "glUniform2f": (None, [C.c_int, C.c_float, C.c_float]),
        "glUniform4f": (None, [C.c_int, C.c_float, C.c_float, C.c_float, C.c_float]),
        "glBindAttribLocation": (None, [C.c_uint, C.c_uint, C.c_char_p]),
        "glGetAttribLocation": (C.c_int, [C.c_uint, C.c_char_p]),
        "glGetUniformLocation": (C.c_int, [C.c_uint, C.c_char_p]),
        "glGetActiveUniform": (None, [C.c_uint, C.c_uint, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_uint), C.c_char_p]),
        "glGetActiveAttrib": (None, [C.c_uint, C.c_uint, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_uint), C.c_char_p]),
        "glUniformMatrix4fv": (None, [C.c_int, C.c_int, C.c_ubyte, C.POINTER(C.c_float)]),
        "glUniform1f": (None, [C.c_int, C.c_float]),
        "glUniform2f": (None, [C.c_int, C.c_float, C.c_float]),
        "glUniform4f": (None, [C.c_int, C.c_float, C.c_float, C.c_float, C.c_float]),
        "glProgramBinary": (None, [C.c_uint, C.c_uint, C.c_void_p, C.c_int]),
        "glGetProgramBinary": (None, [C.c_uint, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_uint), C.c_void_p]),
    }
    for name, (result, args) in specs.items():
        mesa_functions[name] = ip.cfunc(mesa.gl, name, result, args)
    # GLES.py binds the rest of the shader/program functions used below.
    mesa_functions.update(mesa.f)
    host_gl["mesa"] = mesa.identity

def mesa_text(pointer, limit=1 << 20):
    raw = bytearray()
    while len(raw) < limit:
        chunk = bytes(machine.u.mem_read(pointer + len(raw), min(4096, limit - len(raw))))
        end = chunk.find(b"\0")
        if end >= 0:
            return bytes(raw) + chunk[:end]
        raw.extend(chunk)
    raise RuntimeError("guest GL string exceeded bound")

def mesa_shader_source():
    shader, count, strings, lengths = (machine.x(i) for i in range(4))
    if not 0 < count <= 16:
        raise RuntimeError(f"GL shader source count outside bound: {count}")
    owners, pointers, sizes, decoded = [], [], [], []
    for i in range(count):
        pointer = machine.get(strings + i * 8, "Q")[0]
        length = machine.get(lengths + i * 4, "i")[0] if lengths else -1
        source = bytes(machine.u.mem_read(pointer, length)) if 0 <= length <= 1 << 20 else mesa_text(pointer)
        owner = C.create_string_buffer(source)
        owners.append(owner)
        pointers.append(C.cast(owner, C.c_char_p))
        sizes.append(len(source))
        decoded.append(source)
    c_strings, c_sizes = (C.c_char_p * count)(*pointers), (C.c_int * count)(*sizes)
    mesa_functions["glShaderSource"](shader, count, c_strings, c_sizes)
    host_gl["shaders"].setdefault(shader, {"type": None})["sources"] = [x.decode("utf-8", "replace") for x in decoded]
    host_gl["gl"].append({"call": "shader-source", "shader": shader, "bytes": sum(sizes), "backend": "Mesa GLES"})

def mesa_shader_status():
    shader, pname, output = machine.x(0), machine.x(1), machine.x(2)
    if pname not in (0x8B81, 0x8B84):
        raise RuntimeError(f"unsupported glGetShaderiv pname={pname:#x}")
    value = C.c_int()
    mesa_functions["glGetShaderiv"](shader, pname, C.byref(value))
    machine.put(output, "i", value.value)
    host_gl["gl"].append({"call": "get-shader-parameter", "shader": shader, "pname": pname, "result": value.value, "backend": "Mesa GLES"})

def mesa_program_status():
    program, pname, output = machine.x(0), machine.x(1), machine.x(2)
    if pname not in (0x8B82, 0x8B84, 0x8741, 0x8B86, 0x8B89):
        raise RuntimeError(f"unsupported glGetProgramiv pname={pname:#x}")
    value = C.c_int()
    mesa_functions["glGetProgramiv"](program, pname, C.byref(value))
    machine.put(output, "i", value.value)
    if pname == 0x8B82:
        host_gl["programs"].setdefault(program, {"shaders": []})["linked"] = bool(value.value)
    host_gl["gl"].append({"call": "get-program-parameter", "program": program, "pname": pname, "result": value.value, "backend": "Mesa GLES"})

def mesa_info_log(which):
    function = mesa_functions[which]
    def call():
        obj, capacity, output_length, output = (machine.x(i) for i in range(4))
        if not 0 <= capacity <= 4096:
            raise RuntimeError(f"shader/program log capacity outside bound: {capacity}")
        buffer = C.create_string_buffer(max(capacity, 1))
        actual = C.c_int()
        function(obj, capacity, C.byref(actual), buffer)
        if output_length:
            machine.put(output_length, "i", actual.value)
        if output and capacity:
            machine.u.mem_write(output, buffer.raw[:capacity])
        host_gl["gl"].append({"call": which, "object": obj, "written": actual.value, "backend": "Mesa GLES"})
    return call

def mesa_active(which):
    function = mesa_functions[which]
    def call():
        program, index, capacity, out_length, out_size, out_type, out_name = (machine.x(i) for i in range(7))
        if not 0 <= capacity <= 4096:
            raise RuntimeError(f"active shader name capacity outside bound: {capacity}")
        length, size, kind = C.c_int(), C.c_int(), C.c_uint()
        name = C.create_string_buffer(max(capacity, 1))
        function(program, index, capacity, C.byref(length) if out_length else None,
                 C.byref(size) if out_size else None, C.byref(kind) if out_type else None, name)
        if out_length: machine.put(out_length, "i", length.value)
        if out_size: machine.put(out_size, "i", size.value)
        if out_type: machine.put(out_type, "I", kind.value)
        if out_name and capacity: machine.u.mem_write(out_name, name.raw[:capacity])
    return call

def mesa_binary_miss():
    program, capacity, out_length, out_format, output = (machine.x(i) for i in range(5))
    if not 0 <= capacity <= 1 << 20 or (capacity and not output):
        raise RuntimeError(f"invalid glGetProgramBinary capacity={capacity} output={output:#x}")
    length, fmt = C.c_int(), C.c_uint()
    buffer = C.create_string_buffer(max(capacity, 1))
    mesa_functions["glGetProgramBinary"](program, capacity, C.byref(length), C.byref(fmt), buffer)
    if length.value < 0 or length.value > capacity:
        raise RuntimeError(f"Mesa returned invalid program binary length {length.value}/{capacity}")
    if out_length: machine.put(out_length, "i", length.value)
    if out_format: machine.put(out_format, "I", fmt.value)
    if output and length.value: machine.u.mem_write(output, buffer.raw[:length.value])
    host_gl["gl"].append({"call": "get-program-binary", "program": program, "length": length.value, "format": fmt.value, "backend": "Mesa GLES"})

def mesa_unexpected_binary():
    program, fmt, pointer, length = (machine.x(i) for i in range(4))
    if not 0 <= length <= 1 << 20 or (length and not pointer):
        raise RuntimeError(f"invalid glProgramBinary format={fmt:#x} ptr={pointer:#x} length={length}")
    owner = C.create_string_buffer(bytes(machine.u.mem_read(pointer, length))) if length else None
    mesa_functions["glProgramBinary"](program, fmt, C.cast(owner, C.c_void_p) if owner else None, length)
    error = mesa_functions["glGetError"]()
    host_gl["gl"].append({"call": "program-binary", "program": program, "format": fmt, "length": length, "mesa_error": error, "backend": "Mesa GLES"})
    if error:
        raise RuntimeError(f"Mesa rejected native program binary format={fmt:#x} length={length} GL error={error:#x}")

def mesa_uniform_matrix():
    location, count, transpose, pointer = (machine.x(i) for i in range(4))
    if not 0 < count <= 16: raise RuntimeError(f"uniform matrix count outside bound: {count}")
    values = machine.get(pointer, "f" * (16 * count))
    data = (C.c_float * len(values))(*values)
    mesa_functions["glUniformMatrix4fv"](location, count, transpose, data)

def mesa_uniform(name, kind):
    def call():
        location = machine.x(0)
        args = [machine.f(i) if kind[i] == "f" else machine.x(i) for i in range(1, len(kind) + 1)]
        mesa_functions[name](location, *args)
    return call

machine.callbacks.update({
    "mkdir": host_mkdir,
    "stat": host_stat,
    "strtok": host_strtok,
    "strcmp": host_strcmp,
    "strncmp": host_strncmp,
    "strchr": host_strchr,
    "__strncpy_chk2": host_strncpy_chk2,
    "fopen": host_fopen,
    "fopen64": host_fopen,
    "fwrite": host_fwrite,
    "fread": host_fread,
    "posix_memalign": machine.posix_memalign,
    "fclose": host_fclose,
    "fflush": host_fflush,
    "fsync": host_fsync,
    "setbuf": host_setbuf,
    "fileno": host_fileno,
    "setjmp": host_setjmp,
    "longjmp": host_longjmp,
    "crc32": host_crc32,
    "adler32": host_adler32,
    "deflateInit2_": host_deflate_init2,
    "deflate": host_deflate,
    "deflateEnd": host_deflate_end,
    "zlibVersion": host_zlib_version,
    "glCreateShader": gl_create_shader,
    "glShaderSource": gl_shader_source,
    "glCompileShader": gl_compile_shader,
    "glGetShaderiv": gl_shader_parameter,
    "glGetShaderInfoLog": gl_info_log,
    "glCreateProgram": gl_create_program,
    "glAttachShader": gl_attach_shader,
    "glLinkProgram": gl_link_program,
    "glGetProgramiv": gl_program_parameter,
    "glGetProgramInfoLog": gl_info_log,
    "glDeleteShader": gl_delete_shader,
    "glDeleteProgram": gl_delete_program,
})
if mesa:
    def mesa_create_shader():
        kind = machine.x(0)
        shader = mesa_functions["glCreateShader"](kind)
        if not shader: raise RuntimeError(f"Mesa glCreateShader failed for {kind:#x}")
        host_gl["shaders"][shader] = {"type": kind, "sources": []}
        host_gl["gl"].append({"call": "create-shader", "type": kind, "token": shader, "backend": "Mesa GLES"})
        machine.x(0, shader)
    def mesa_create_program():
        program = mesa_functions["glCreateProgram"]()
        if not program: raise RuntimeError("Mesa glCreateProgram failed")
        host_gl["programs"][program] = {"shaders": [], "linked": None}
        host_gl["gl"].append({"call": "create-program", "token": program, "backend": "Mesa GLES"})
        machine.x(0, program)
    def mesa_attach_shader():
        program, shader = machine.x(0), machine.x(1)
        mesa_functions["glAttachShader"](program, shader)
        host_gl["programs"].setdefault(program, {"shaders": []})["shaders"].append(shader)
    def mesa_link_program():
        program = machine.x(0)
        mesa_functions["glLinkProgram"](program)
        host_gl["gl"].append({"call": "link-program", "program": program, "backend": "Mesa GLES"})
    def mesa_bind_attrib():
        program, index, name = machine.x(0), machine.x(1), mesa_text(machine.x(2))
        mesa_functions["glBindAttribLocation"](program, index, name)
    def mesa_get_location(which):
        def call():
            program, name = machine.x(0), mesa_text(machine.x(1))
            machine.x(0, mesa_functions[which](program, name))
        return call
    def mesa_use_program():
        program = machine.x(0)
        mesa_functions["glUseProgram"](program)
    def mesa_delete(which):
        def call(): mesa_functions[which](machine.x(0))
        return call
    machine.callbacks.update({
        "glCreateShader": mesa_create_shader,
        "glShaderSource": mesa_shader_source,
        "glCompileShader": lambda: mesa_functions["glCompileShader"](machine.x(0)),
        "glGetShaderiv": mesa_shader_status,
        "glGetShaderInfoLog": mesa_info_log("glGetShaderInfoLog"),
        "glCreateProgram": mesa_create_program,
        "glAttachShader": mesa_attach_shader,
        "glDetachShader": lambda: mesa_functions["glDetachShader"](machine.x(0), machine.x(1)),
        "glLinkProgram": mesa_link_program,
        "glGetProgramiv": mesa_program_status,
        "glGetProgramInfoLog": mesa_info_log("glGetProgramInfoLog"),
        "glDeleteShader": mesa_delete("glDeleteShader"),
        "glDeleteProgram": mesa_delete("glDeleteProgram"),
        "glBindAttribLocation": mesa_bind_attrib,
        "glGetAttribLocation": mesa_get_location("glGetAttribLocation"),
        "glGetUniformLocation": mesa_get_location("glGetUniformLocation"),
        "glGetActiveUniform": mesa_active("glGetActiveUniform"),
        "glGetActiveAttrib": mesa_active("glGetActiveAttrib"),
        "glGetProgramBinary": mesa_binary_miss,
        "glProgramBinary": mesa_unexpected_binary,
        "glUseProgram": mesa_use_program,
        "glUniformMatrix4fv": mesa_uniform_matrix,
        "glUniform1f": mesa_uniform("glUniform1f", "f"),
        "glUniform2f": mesa_uniform("glUniform2f", "ff"),
        "glUniform4f": mesa_uniform("glUniform4f", "ffff"),
        "glUniform1i": mesa_uniform("glUniform1i", "i"),
    })
    def mesa_gen_objects(kind):
        def call():
            count, output = machine.x(0), machine.x(1)
            if not 0 <= count <= 16 or (count and not output): raise RuntimeError(f"{kind} count/output invalid")
            values = (C.c_uint * max(count, 1))()
            mesa_functions[kind](count, values)
            if count: machine.put(output, "I" * count, *values[:count])
            host_gl["gl"].append({"call": kind, "ids": list(values[:count]), "backend": "Mesa GLES"})
        return call
    def mesa_delete_objects(kind):
        def call():
            count, pointer = machine.x(0), machine.x(1)
            if not 0 <= count <= 16 or (count and not pointer): raise RuntimeError(f"{kind} count/pointer invalid")
            values = machine.get(pointer, "I" * count) if count else ()
            array = (C.c_uint * max(count, 1))(*values) if count else (C.c_uint * 1)()
            mesa_functions[kind](count, array)
            host_gl["gl"].append({"call": kind, "ids": list(values), "backend": "Mesa GLES"})
        return call
    def mesa_bind_framebuffer():
        target, obj = machine.x(0), machine.x(1)
        mesa_functions["glBindFramebuffer"](target, obj)
        host_gl["fbo"] = obj
        host_gl["gl"].append({"call": "glBindFramebuffer", "target": target, "id": obj, "backend": "Mesa GLES"})
    def mesa_attach_texture():
        args = [machine.x(i) for i in range(5)]
        mesa_functions["glFramebufferTexture2D"](*args)
        host_gl["gl"].append({"call": "glFramebufferTexture2D", "args": args, "backend": "Mesa GLES"})
    def mesa_framebuffer_status():
        target = machine.x(0)
        status = mesa_functions["glCheckFramebufferStatus"](target)
        host_gl["gl"].append({"call": "glCheckFramebufferStatus", "target": target, "result": status, "backend": "Mesa GLES"})
        machine.x(0, status)
    def mesa_tex_image():
        args = [machine.x(i) for i in range(8)]
        pixels = machine.get(machine.u.reg_read(R.UC_ARM64_REG_SP), "Q")[0]
        channels = {0x1903: 1, 0x1906: 1, 0x1908: 4, 0x80E1: 4}.get(args[6], 4)
        size = args[3] * args[4] * channels
        if size > 1 << 24: raise RuntimeError(f"GL texture allocation exceeds bound: {size}")
        raw = bytes(machine.u.mem_read(pixels, size)) if pixels and size else b""
        owner = C.create_string_buffer(raw) if raw else None
        mesa_functions["glTexImage2D"](*args, C.cast(owner, C.c_void_p) if owner else None)
        host_gl["texture"] = raw or None
        host_gl["texture_format"] = args[2]
        host_gl["gl"].append({"call": "glTexImage2D", "args": args, "input_bytes": size, "input_sha256": hashlib.sha256(raw).hexdigest() if raw else None, "backend": "Mesa GLES"})
    def mesa_tex_sub_image():
        args = [machine.x(i) for i in range(8)]
        pixels = machine.get(machine.u.reg_read(R.UC_ARM64_REG_SP), "Q")[0]
        channels = {0x1903: 1, 0x1906: 1, 0x1908: 4, 0x80E1: 4}.get(args[6], 4)
        size = args[4] * args[5] * channels
        if not pixels or size > 1 << 24: raise RuntimeError(f"invalid GL subimage buffer {pixels:#x}/{size}")
        raw = bytes(machine.u.mem_read(pixels, size)); owner = C.create_string_buffer(raw)
        mesa_functions["glTexSubImage2D"](*args, C.cast(owner, C.c_void_p))
        host_gl["texture"] = raw; host_gl["gl"].append({"call": "glTexSubImage2D", "args": args, "input_sha256": hashlib.sha256(raw).hexdigest(), "backend": "Mesa GLES"})
    def mesa_read_pixels():
        x, y, width, height, fmt, typ, destination = (machine.x(i) for i in range(7))
        if not 0 <= width <= 2048 or not 0 <= height <= 2048: raise RuntimeError(f"glReadPixels dimensions outside bound: {width}x{height}")
        channels = {0x1903: 1, 0x1908: 4}.get(fmt)
        if channels is None or typ != 0x1401: raise RuntimeError(f"unsupported Mesa readback format/type {fmt:#x}/{typ:#x}")
        size = width * height * channels
        output = C.create_string_buffer(size)
        mesa_functions["glReadPixels"](x, y, width, height, fmt, typ, output)
        machine.u.mem_write(destination, output.raw)
        host_gl["readbacks"].append({"destination": hex(destination), "rect": [x, y, width, height], "format": fmt, "type": typ, "bytes_hex": output.raw.hex()})
        host_gl["gl"].append({"call": "glReadPixels", "rect": [x, y, width, height], "format": fmt, "bytes": size, "sha256": hashlib.sha256(output.raw).hexdigest(), "backend": "Mesa GLES"})
    def mesa_integer():
        enum, pointer = machine.x(0), machine.x(1); value = C.c_int()
        mesa_functions["glGetIntegerv"](enum, C.byref(value)); machine.put(pointer, "I", value.value)
    def mesa_sync(name, restype, argtypes):
        address = mesa.egl.eglGetProcAddress(name.encode())
        if not address: raise RuntimeError(f"Mesa GLES lacks {name}")
        return C.CFUNCTYPE(restype, *argtypes)(address)
    syncs = {}
    def mesa_fence_sync():
        token = machine.alloc(8)
        sync = mesa_sync("glFenceSync", C.c_void_p, [C.c_uint, C.c_uint])(machine.x(0), machine.x(1))
        if not sync: raise RuntimeError("Mesa glFenceSync failed")
        syncs[token] = sync; machine.x(0, token)
    def mesa_wait_sync():
        token = machine.x(0)
        if token not in syncs: raise RuntimeError(f"unknown Mesa sync token {token:#x}")
        wait = mesa_sync("glClientWaitSync", C.c_uint, [C.c_void_p, C.c_uint, C.c_uint64])
        machine.x(0, wait(syncs[token], machine.x(1), machine.x(2)))
    def mesa_delete_sync():
        token = machine.x(0)
        if token not in syncs: raise RuntimeError(f"unknown Mesa sync token {token:#x}")
        mesa_sync("glDeleteSync", None, [C.c_void_p])(syncs.pop(token)); machine.x(0, 0)
    def mesa_pixel_store():
        pname, value = machine.x(0), machine.x(1)
        mesa_functions["glPixelStorei"](pname, value)
        if pname == 0x0CF2:
            host_gl["unpack_row_length"] = value; host_gl["unpack_row_length_history"].append(value)
    def mesa_get_string():
        enum = machine.x(0); value = mesa_functions["glGetString"](enum)
        data = value or b""; pointer = machine.alloc(len(data) + 1); machine.u.mem_write(pointer, data + b"\0"); machine.x(0, pointer)
    def mesa_toggle(name):
        def call(): mesa_functions[name](machine.x(0))
        return call
    machine.callbacks.update({
        "glGenTextures": mesa_gen_objects("glGenTextures"),
        "glDeleteTextures": mesa_delete_objects("glDeleteTextures"),
        "glBindTexture": lambda: mesa_functions["glBindTexture"](machine.x(0), machine.x(1)),
        "glTexImage2D": mesa_tex_image,
        "glTexSubImage2D": mesa_tex_sub_image,
        "glTexParameteri": lambda: mesa_functions["glTexParameteri"](machine.x(0), machine.x(1), machine.x(2)),
        "glActiveTexture": lambda: mesa_functions["glActiveTexture"](machine.x(0)),
        "glPixelStorei": mesa_pixel_store,
        "glGenFramebuffers": mesa_gen_objects("glGenFramebuffers"),
        "glDeleteFramebuffers": mesa_delete_objects("glDeleteFramebuffers"),
        "glBindFramebuffer": mesa_bind_framebuffer,
        "glFramebufferTexture2D": mesa_attach_texture,
        "glCheckFramebufferStatus": mesa_framebuffer_status,
        "glGenRenderbuffers": mesa_gen_objects("glGenRenderbuffers"),
        "glDeleteRenderbuffers": mesa_delete_objects("glDeleteRenderbuffers"),
        "glBindRenderbuffer": lambda: mesa_functions["glBindRenderbuffer"](machine.x(0), machine.x(1)),
        "glRenderbufferStorage": lambda: mesa_functions["glRenderbufferStorage"](machine.x(0), machine.x(1), machine.x(2), machine.x(3)),
        "glFramebufferRenderbuffer": lambda: mesa_functions["glFramebufferRenderbuffer"](machine.x(0), machine.x(1), machine.x(2), machine.x(3)),
        "glViewport": lambda: mesa_functions["glViewport"](*[machine.x(i) for i in range(4)]),
        "glReadPixels": mesa_read_pixels,
        "glGetIntegerv": mesa_integer,
        "glGetString": mesa_get_string,
        "glGetError": lambda: machine.x(0, mesa_functions["glGetError"]()),
        "glFenceSync": mesa_fence_sync,
        "glClientWaitSync": mesa_wait_sync,
        "glDeleteSync": mesa_delete_sync,
        "glFlush": lambda: mesa_functions["glFlush"](),
        "glFinish": lambda: mesa_functions["glFinish"](),
        "glEnable": mesa_toggle("glEnable"),
        "glDisable": mesa_toggle("glDisable"),
    })

# Execute the mapped providers' actual initialization entries now that the
# narrow runtime bridges are installed. Mutex calls require a native-created
# mutex or an allowed zero-filled static libc++ initializer.
dl_libraries = [
    ("libSPenFountainPen.so", LIB / "libSPenFountainPen.so", 0x00000000),
    ("libSPenPenCommon.so", LIB / "libSPenPenCommon.so", 0x01000000),
    ("libSPenBase.so", LIB / "libSPenBase.so", 0x02000000),
    ("libSPenAirBrushPen.so", LIB / "libSPenAirBrushPen.so", 0x03000000),
    ("libSPenGraphics.so", LIB / "libSPenGraphics.so", 0x04000000),
    ("libSPenRenderer.so", LIB / "libSPenRenderer.so", 0x05000000),
    ("libSPenComposer.so", composer_path, composer_base),
    ("libSPenLibpng.so", LIB / "libSPenLibpng.so", 0x07000000),
    ("libSPenZlib.so", LIB / "libSPenZlib.so", 0x08000000),
    ("libSPenPdf.so", LIB / "libSPenPdf.so", 0x09000000),
    ("libc++_shared.so", libcpp_path, libcpp_base),
]
machine.dl_iterate_rows = []
machine.host_static_rwlock_ranges = []
next_tls_module_id = 1
for soname, path, base in dl_libraries:
    blob = path.read_bytes()
    phoff = struct.unpack_from("<Q", blob, 32)[0]
    phent, phnum = struct.unpack_from("<HH", blob, 54)
    headers = [struct.unpack_from("<II6Q", blob, phoff + i * phent) for i in range(phnum)]
    for header in headers:
        if header[0] == 1 and header[6] > header[5]:
            machine.host_static_rwlock_ranges.append((base + header[3] + header[5], base + header[3] + header[6]))
    tls = next((header for header in headers if header[0] == 7), None)
    tls_data = 0
    tls_modid = 0
    if tls:
        tls_modid = next_tls_module_id; next_tls_module_id += 1
        tls_data = machine.alloc(tls[6]) if tls[6] else 0
        if tls_data and tls[5]: machine.u.mem_write(tls_data, blob[tls[2]:tls[2] + tls[5]])
    machine.dl_iterate_rows.append({
        "name": soname.encode(), "base": base,
        "phdrs": b"".join(blob[phoff + i * phent:phoff + (i + 1) * phent] for i in range(phnum)),
        "count": phnum, "tls_modid": tls_modid, "tls_data": tls_data,
    })
native_init_array_counts = {"libc++_shared": run_native_init_array(libcpp_path, libcpp_base)}
for init_name, init_path, init_base in (
    ("Base", LIB / "libSPenBase.so", 0x02000000),
    ("Renderer", LIB / "libSPenRenderer.so", 0x05000000),
    ("Graphics", LIB / "libSPenGraphics.so", 0x04000000),
):
    native_init_array_counts[init_name] = run_native_init_array(init_path, init_base)

if CASE.startswith("rgba8_"):
    canvas = machine.alloc(512)
    try:
        machine.call("SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)", [canvas, queue, factory_bitmap, 1], instruction_limit=5_000_000)
    except Exception as error:
        raise RuntimeError(f"CanvasImpl constructor failed pc={machine.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={machine.u.reg_read(R.UC_ARM64_REG_LR):#x} once={getattr(machine,'pthread_once_calls',[])} trace={[hex(address) for address in getattr(machine,'trace',[])]}: {error}") from error
    machine.call("SPen::SPCanvasImpl::GetBitmap()", [canvas], instruction_limit=5_000_000)
    canvas_bitmap = machine.x(0)
    if not canvas_bitmap:
        raise RuntimeError("native CanvasImpl::GetBitmap returned null")
    machine.call("SPen::SPTextureBitmap::GetSubBitmap(int)", [canvas_bitmap, 0], instruction_limit=5_000_000)
    bitmap = machine.x(0)
    if not bitmap:
        raise RuntimeError("native GetSubBitmap(0) returned null")
    bitmap = factory_bitmap

adapter = machine.alloc(256)
shared_ptr = machine.alloc(16)
machine.call("SPen::PdfImageAdapter::PdfImageAdapter(std::__ndk1::shared_ptr<SPen::PdfExporter>, SPen::IContext const*)", [adapter, shared_ptr, 0], instruction_limit=5_000_000)
info = machine.get(adapter + 32, "Q")[0]
if not info:
    raise RuntimeError("native PdfImageAdapter constructor returned null PDFExportInfo")
info_before_set_image = bytes(machine.u.mem_read(info, 128))
set_image_name = "SPen::PdfImageAdapter::SetImage(SPen::ISPBitmap*)"
try:
    machine.call(set_image_name, [adapter, bitmap], instruction_limit=20_000_000)
except Exception as error:
    print("FAULT", hex(machine.u.reg_read(R.UC_ARM64_REG_PC)), hex(machine.u.reg_read(R.UC_ARM64_REG_LR)), [hex(x) for x in machine.trace], [hex(machine.x(i)) for i in range(8)], file=sys.stderr)
    raise
set_image_result = machine.x(0)
info_after_set_image = bytes(machine.u.mem_read(info, 128))
info_words = list(machine.get(info, "16Q"))
native_path_length = machine.get(info + 88, "Q")[0]
native_path_pointer = machine.get(info + 96, "Q")[0]
if native_path_length > 4096 or (native_path_length and not native_path_pointer):
    raise RuntimeError(f"invalid native std::string path fields pointer={native_path_pointer:#x} length={native_path_length}")
output_path = bytes(machine.u.mem_read(native_path_pointer, native_path_length)).decode("utf-8") if native_path_length else None
if set_image_result != 1:
    sub_vptr=machine.get(bitmap,'Q')[0] if bitmap else 0
    symbols_by_addr={value:name for name,value in machine.syms.items()}
    sub_vtable=[machine.get(sub_vptr+8*i,'Q')[0] for i in range(48)] if sub_vptr else []
    sub_slots=[{'slot':i,'ptr':hex(value),'symbol':symbols_by_addr.get(value)} for i,value in enumerate(sub_vtable)]
    raise RuntimeError(f"native PdfImageAdapter::SetImage(ISPBitmap*) returned {set_image_result}; mesa={host_gl.get('mesa')}; textures={machine.texture_records}; actual_entries={machine.actual_entries}; bitmap={hex(bitmap)} fields={bytes(machine.u.mem_read(bitmap,128)).hex()} vtable={hex(sub_vptr)} slots={sub_slots}; files={host_file_events}; trace={native_trace}")
if not output_path or not Path(output_path).is_file():
    raise RuntimeError(f"native SetImage did not create its cached PNG at {output_path!r}")
png_bytes = Path(output_path).read_bytes()
if CASE.startswith("rgba8_"):
    if len(host_gl["readbacks"]) != 1:
        raise RuntimeError(f"expected one Mesa glReadPixels callback, got {host_gl["readbacks"]}")
    rgba = bytes.fromhex(host_gl["readbacks"][0]["bytes_hex"])
if not png_bytes.startswith(b"\x89PNG\r\n\x1a\n") or png_bytes[12:16] != b"IHDR":
    raise RuntimeError("native cached output is not a PNG with an IHDR")
if struct.unpack_from(">II", png_bytes, 16) != (2, 2):
    raise RuntimeError("native cached PNG dimensions differ from the native 2x2 bitmap")
def decode_rgba_png(data):
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("bad PNG signature")
    offset, idat, header = 8, bytearray(), None
    png_chunk_records = []
    while offset < len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + length]
        expected_crc = struct.unpack_from(">I", data, offset + 8 + length)[0]
        actual_crc = zlib.crc32(kind + payload) & 0xFFFFFFFF
        if actual_crc != expected_crc:
            raise RuntimeError(f"invalid PNG chunk CRC for {kind!r}")
        png_chunk_records.append({"type": kind.decode("ascii"), "length": length, "crc32": f"{expected_crc:08x}", "valid": True})
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", payload)
        elif kind == b"IDAT":
            idat.extend(payload)
        offset += length + 12
    width, height, bit_depth, color_type, compression, filtering, interlace = header
    if (width, height, bit_depth, color_type, compression, filtering, interlace) != (2, 2, 8, 6, 0, 0, 0):
        raise RuntimeError(f"unsupported native PNG header {header}")
    raw = zlib.decompress(idat)
    stride, bpp = width * 4, 4
    rows, previous = [], bytearray(stride)
    cursor = 0
    for _ in range(height):
        filter_type = raw[cursor]
        filtered = raw[cursor + 1:cursor + 1 + stride]
        cursor += stride + 1
        row = bytearray(stride)
        for index, value in enumerate(filtered):
            left = row[index - bpp] if index >= bpp else 0
            up = previous[index]
            upper_left = previous[index - bpp] if index >= bpp else 0
            if filter_type == 0:
                predictor = 0
            elif filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = up
            elif filter_type == 3:
                predictor = (left + up) // 2
            elif filter_type == 4:
                base = left + up - upper_left
                distances = (abs(base - left), abs(base - up), abs(base - upper_left))
                predictor = (left, up, upper_left)[distances.index(min(distances))]
            else:
                raise RuntimeError(f"unknown PNG row filter {filter_type}")
            row[index] = (value + predictor) & 0xFF
        rows.append(bytes(row))
        previous = row
    return b"".join(rows), png_chunk_records

decoded_pixels, png_chunk_records = decode_rgba_png(png_bytes)
if CASE.startswith("rgba8_"):
    expected_png = bytearray(rgba)
    for offset in range(0, len(expected_png), 4):
        alpha = expected_png[offset + 3]
        for channel in range(3):
            value = expected_png[offset + channel]
            expected_png[offset + channel] = 0 if alpha == 0 else min(255, (value * 255 + alpha // 2) // alpha)
    if decoded_pixels != bytes(expected_png):
        raise RuntimeError(f"native PNG did not match rounded straight-alpha conversion: png={decoded_pixels.hex()} Mesa={rgba.hex()} expected={bytes(expected_png).hex()} trace={native_trace}")
    if rgba != pixels:
        raise RuntimeError(f"Mesa RGBA8 readback differs from uploaded source: {rgba.hex()} != {pixels.hex()}")
elif decoded_pixels != rgba:
    raise RuntimeError(f"native PNG decoded bytes {decoded_pixels.hex()} differ from readback {rgba.hex()}")
if host_zstreams or host_zallocations or allocator_errors or host_files:
    raise RuntimeError(f"native PNG resource cleanup failed: streams={len(host_zstreams)} allocs={len(host_zallocations)} errors={allocator_errors} files={list(host_files)}")
# diagnostic capture: synthetic graphics factory use is reported as a proof boundary below
set_image_trace = next(item for item in native_trace if item["name"].endswith("SetImage(SPen::ISPBitmap*)"))
if set_image_trace["args"][0] != adapter or set_image_trace["args"][1] != bitmap:
    raise RuntimeError("native SetImage did not receive the adapter and actual SPTextureBitmap pointers")
if machine.get(adapter + 32, "Q")[0] != info:
    raise RuntimeError("adapter mInfo changed from native constructor-owned pointer")
if info_after_set_image[104] != 1:
    raise RuntimeError(f"native SetImage did not mark the cached path owned: byte={info_after_set_image[104]}")
trace_counts = {name: sum(item["name"] == name for item in native_trace) for name in {item["name"] for item in native_trace}}
subbitmap_pointer = tracked_objects.get("sub")
if not subbitmap_pointer or not any(item["object"] == "sub" and item["interface_vptr_plus24"] for item in lifecycle_snapshots):
    raise RuntimeError("native SPSubBitmap constructor did not establish its secondary interface vptr")
get_bitmap_data_calls = [item for item in native_trace if item["name"].endswith("GetBitmapData(SPen::Rect const&, void*)")]
if not any(item["name"].startswith("SPen::SPTextureBitmap::") and item["args"][0] == factory_bitmap for item in get_bitmap_data_calls):
    raise RuntimeError("native SPTextureBitmap::GetBitmapData receiver differs from CreateBitmap result")
if not any(item["name"].startswith("SPen::SPSubBitmap::") and item["args"][0] == subbitmap_pointer for item in get_bitmap_data_calls):
    raise RuntimeError("native SPSubBitmap::GetBitmapData receiver differs from actual constructor result")
if not machine.queue_events or any(int(item["receiver"], 16) != queue for item in machine.queue_events):
    raise RuntimeError(f"task/completion queue receiver differs from CreateBitmap queue: {machine.queue_events}")
if not any(item["operation"] == "post-and-run" for item in machine.queue_events) or not any(item["operation"] == "completion-wait" for item in machine.queue_events):
    raise RuntimeError("expected actual task dispatch and separate synchronous completion bridge")
if len(host_gl["readbacks"]) != 1 or bytes.fromhex(host_gl["readbacks"][0]["bytes_hex"]) != rgba:
    raise RuntimeError(f"Mesa readback destination mismatch: {host_gl['readbacks']}")
mesa_calls = host_gl["gl"]
status_rows = [item for item in mesa_calls if item.get("call") == "glCheckFramebufferStatus"]
if not status_rows or any(item["result"] != 0x8CD5 for item in status_rows):
    raise RuntimeError(f"Mesa framebuffer status was not complete: {status_rows}")
if not any(item.get("call") == "glReadPixels" and item.get("backend") == "Mesa GLES" for item in mesa_calls):
    raise RuntimeError("actual Mesa glReadPixels was not observed")
if CASE == "rgba8_premul" and any(channel > alpha for pixel in range(4) for channel, alpha in zip(pixels[pixel*4:pixel*4+3], [pixels[pixel*4+3]]*3)):
    raise RuntimeError("premultiplied control source has an RGB channel above alpha")
for required_name in (
    "SPen::System::SetSDKCacheDirectoryPath(char const*)",
    "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::UpdateBitmap(void const*, int)",
    "SPen::SPTextureBitmap::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::PdfImageAdapter::PdfImageAdapter(std::__ndk1::shared_ptr<SPen::PdfExporter>, SPen::IContext const*)",
    "SPen::PdfImageAdapter::SetImage(SPen::ISPBitmap*)",
    "SPen::PdfImageAdapter::saveBitmap(SPen::ISPBitmap*, SPen::Rect, SPen::String&)",
    "SPen::BitmapFactory::SaveBitmap(SPen::Bitmap const*, SPen::String const*, int)",
    "png_create_write_struct", "png_set_write_fn", "png_write_info", "png_write_image", "png_write_end", "png_destroy_write_struct",
):
    if trace_counts.get(required_name) != 1:
        raise RuntimeError(f"expected exactly one observed call to {required_name}, got {trace_counts.get(required_name, 0)}")

capture = {
    "case": CASE,
    "source_format": "RGBA8",
    "source_dimensions": [2, 2],
    "source_row_length_pixels": CASE_ROW_LENGTH,
    "texture_memory_layout": CASE_LAYOUT,
    "texture_target_layout": CASE_TARGET,
    "native_factory": "SPGraphicsFactory::CreateBitmap(queue,2,2,...)",
    "native_update": "SPTextureBitmap::UpdateBitmap(void const*,int)",
    "native_get_bitmap_data": "SPTextureBitmap and SPSubBitmap GetBitmapData executed; their queued native worker task called the explicit Mesa GLES glReadPixels host ABI bridge",
    "case_contract": "clamp_zero_alpha_control" if CASE == "rgba8_clamp" else "valid_premultiplied_control",
    "source_is_premultiplied": CASE == "rgba8_premul",
    "source_sha256": hashlib.sha256(pixels).hexdigest(),
    "bitmap_pointer": hex(bitmap),
    "native_bitmap_objects": {
        "factory_bitmap_pointer": hex(factory_bitmap),
        "factory_bitmap_ctor_event": next(item for item in constructor_events if item["name"] == "SPTextureBitmap ctor(layout)")["args"][0],
        "subbitmap_pointer": hex(subbitmap_pointer),
        "subbitmap_constructor_event": next(item for item in constructor_events if item["name"] == "SPSubBitmap ctor(layout)")["args"][0],
        "subbitmap_interface_vptr_at_plus24": next(item["interface_vptr_plus24"] for item in lifecycle_snapshots if item["object"] == "sub"),
        "set_image_bitmap_pointer": hex(set_image_trace["args"][1]),
        "texture_get_bitmap_data_receivers": [hex(item["args"][0]) for item in get_bitmap_data_calls if "SPTextureBitmap::" in item["name"]],
        "subbitmap_get_bitmap_data_receivers": [hex(item["args"][0]) for item in get_bitmap_data_calls if "SPSubBitmap::" in item["name"]],
    },
    "synthetic_source_pixels_hex": pixels.hex(),
    "source_alpha_bytes": list(pixels[3::4]),
    "mesa_get_bitmap_data_rgba_hex": rgba.hex(),
    "mesa_uploaded_source_readback_equality": rgba == pixels if CASE.startswith("rgba8_") else None,
    "alpha_channel_boundary": "Mesa glReadPixels returns uploaded RGBA bytes unchanged. Native Bitmap::Construct/BitmapFactory::SaveBitmap plus pinned APK libpng emits unchanged alpha and rounded RGB unpremultiplication min(255, round(channel*255/alpha)); alpha-zero RGB becomes zero. This is host Mesa plus native conversion, not Samsung-device GPU parity.",
    "native_set_image_bitmap_executed": True,
    "native_set_image_bitmap_pointer_identity": {"adapter_pointer": hex(adapter), "native_bitmap_pointer": hex(bitmap), "set_image_adapter_argument": hex(set_image_trace["args"][0]), "set_image_bitmap_argument": hex(set_image_trace["args"][1]), "matches": True},
    "native_info_pointer": hex(info),
    "native_info_constructor_snapshot_128_hex": info_before_set_image.hex(),
    "native_info_after_set_image_snapshot_128_hex": info_after_set_image.hex(),
    "native_info_path_ownership_byte": info_after_set_image[104],
    "native_set_image_bitmap_result": set_image_result,
    "native_call_counts": trace_counts,
    "native_cached_png_path": output_path,
    "native_cached_png_exists": bool(output_path and os.path.isfile(output_path)),
    "native_cached_png_sha256": hashlib.sha256(png_bytes).hexdigest(),
    "native_cached_png_bytes": len(png_bytes),
    "native_cached_png_full_hex": png_bytes.hex(),
    "decoded_png_rgba_hex": decoded_pixels.hex(),
    "png_chunks": png_chunk_records,
    "png_crc_validation": "all chunks CRC-checked; PNG row filters decoded before pixel comparison",
    "native_cached_png_dimensions": list(struct.unpack_from(">II", png_bytes, 16)),
    "native_cached_png_rgba": decoded_pixels.hex(),
    "native_cached_png_alpha": list(decoded_pixels[3::4]),
    "native_std_string_layout": {"field_offset": 80, "length": native_path_length, "data_pointer": hex(native_path_pointer)},
    "native_zlib_guest_allocator_pointers": "recorded in the guest z_stream at offsets 64/72; bridged for host zlib and never called as host pointers",
    "sdk_cache_root_input": f"/tmp/sdocx-pdf-image-rgba-native/{CASE}",
    "native_call_trace": native_trace,
    "host_file_events": host_file_events,
    "host_zlib_events": host_zlib_events,
    "host_zlib_sha256": LIBZ_SHA256,
    "pinned_helper_sha256": PINNED_HELPERS,
    "pinned_host_gl_dso_sha256": PINNED_HOST_GL_DSOS,
    "pinned_dso_sha256": {
        **PINNED,
    },
    "queue_contract": {
        "queue_pointer": hex(queue),
        "native_interface_vtable_slots": {"plus16": "enQueueMsg(ITask*)", "plus24": "waitForCompleting()"},
        "source_evidence": {"renderer_get_msg_queue_va": "0x589cc", "renderer_ctor_va": "0x5870c", "secondary_vtable_address": "0x6a980", "en_queue_thunk_va": "0x589ec", "wait_thunk_va": "0x58a20"},
        "host_adapter": "slot +16 synchronously invokes each actual native task vtable +16; slot +24 returns after that task",
        "observed_events": getattr(machine, "queue_events", []),
    },
    "mesa_readback": host_gl["readbacks"],
    "mesa_gles_host_backend": {
        "identity": mesa.identity,
        "calls": host_gl["gl"],
        "texture_format": host_gl["texture_format"],
        "unpack_row_length_history": host_gl["unpack_row_length_history"],
        "texture_ids": [identifier for call in host_gl["gl"] if call.get("call") == "glGenTextures" for identifier in call["ids"]],
        "framebuffer_ids": [identifier for call in host_gl["gl"] if call.get("call") == "glGenFramebuffers" for identifier in call["ids"]],
        "framebuffer_complete_checks": [call["result"] for call in status_rows],
        "texture_attachments": [call["args"] for call in host_gl["gl"] if call.get("call") == "glFramebufferTexture2D"],
        "storage": "Reached GLES calls use host Mesa llvmpipe EGL context with actual Mesa texture/FBO IDs, actual completeness query, and actual glReadPixels output; no device GPU",
        "gpu_rendering_claimed": False,
    },
    "renderer_texture_records": machine.texture_records,
    "native_entries": machine.actual_entries,
    "host_renderer_factory_callbacks": {"CreateGraphicsObject": machine.host_create_graphics_calls, "CreateTextureObject": machine.host_create_texture_calls},
    "native_subbitmap_constructor_snapshots": [item for item in lifecycle_snapshots if item["object"] == "sub"],
    "host_boundaries": {"queue": "synthetic IGLMsgQueue adapter executes the actual task vtable +16 synchronously; +24 completion returns after that task", "gl_gles": "actual reached GLES APIs run in Mesa llvmpipe; host ABI bridge only forwards arguments and copies glReadPixels output; no device GPU", "graphics_factory": "CreateGraphicsObject callback returns a synthetic object once; no GraphicsObject draw method or GL draw call was observed", "zlib": "pinned host libz 1.2.8 through z_stream and guest allocator bridges; APK libSPenLibpng is native", "pixels": "synthetic 2x2 source bytes, not DrawingFactory output", "filesystem": "bounded host stdio/path callbacks", "excluded": "DrawingFactory/ObjectStroke raster paint, PdfiumImageHandler::AddImage, and PDF export are not executed"},
}
print(json.dumps(capture, sort_keys=True))
