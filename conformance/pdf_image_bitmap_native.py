#!/usr/bin/env python3
"""Pinned native bitmap overload -> PNG cache conformance capture.

Run from the repository root with the extracted APK and bundled Unicorn:
  PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python conformance/pdf_image_bitmap_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python conformance/pdf_image_bitmap_native.py --check-fixture
"""
import argparse
import ctypes
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

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/pdf-image-bitmap-native.json'
LIBZ_PATH = Path('/usr/lib/x86_64-linux-gnu/libz.so.1')
LIBZ_SHA256 = '85590dd58edf5445e18bc7193e5ebc01ac5841f1ae187e97705a662e90c6421e'

LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
NM = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm'
OBJ = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
sys.path.insert(0, str(ROOT / 'conformance'))
import airbrush_saved_native as air
from unicorn import arm64_const as R
class P(air.Oracle):
 def __init__(self):
  super().__init__(); self.extra_bases={};self.actual_entries=[];self.pending_native_returns=[];self.return_tramp=self.alloc(8);self.u.mem_write(self.return_tramp,struct.pack('<II',0xd503201f,0xd65f03c0));self.map_extra('Graphics',4);self.map_extra('Renderer',5);self.renderer_base=0x5000000;self.load_extra_relocs();self.create_calls=[];self.texture_records=[];self.state_records=[];self.gl=[];self.allocs={};self.vertex_attributes=[];self.graphics_factory=[];self.blend_descriptors=[];self.renderer_factory_obj=self.alloc(64);self.gl_id=100;self.synthetic={};self.synthetic_calls=[];self.shader_values=[];self.blend_gl=[];self.host_create_graphics_calls=0;self.host_create_texture_calls=0
  self.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=self.get_caps
  self.callbacks['strncpy']=self.strncpy
  self.callbacks['strstr']=self.strstr
  self.callbacks['operator new[](unsigned long)']=self.new_array
  self.callbacks['operator delete[](void*)']=lambda:None
  self.callbacks['clock_gettime']=self.clock_gettime
  self.callbacks['SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)']=self.create_graphics
  self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=self.create_blend
  self.callbacks['SPen::ManagedObject::Register()']=lambda:None
  self.callbacks['SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']=self.create_texture
  self.put(self.renderer_base+0x73388,'II',4096,4096);self.u.mem_write(self.renderer_base+0x733c0,b'\x01')
  self.callbacks.update({'glBlendEquationSeparate':lambda:self.blend_gl.append({'name':'glBlendEquationSeparate','args':[self.x(0),self.x(1)]}),'glBlendFuncSeparate':lambda:self.blend_gl.append({'name':'glBlendFuncSeparate','args':[self.x(i) for i in range(4)]}),'glEnable':lambda:self.blend_gl.append({'name':'glEnable','args':[self.x(0)]}),'glDisable':lambda:self.blend_gl.append({'name':'glDisable','args':[self.x(0)]}),'glColorMask':lambda:self.blend_gl.append({'name':'glColorMask','args':[self.x(i) for i in range(4)]}),'glDrawArraysInstanced':lambda:self.gl.append({'name':'glDrawArraysInstanced','mode':self.x(0),'first':self.x(1),'count':self.x(2),'instances':self.x(3)}),'glGetString':self.get_string,'glGetIntegerv':self.get_integer,'glGetError':lambda:self.x(0,0),'glGenBuffers':self.gen_buffers,'glGenSamplers':self.gen_samplers,'glBindBuffer':lambda:self.gl.append({'name':'glBindBuffer','target':self.x(0),'id':self.x(1)}),'glBufferData':self.buffer_data,'glDisableVertexAttribArray':lambda:self.gl.append({'name':'glDisableVertexAttribArray','index':self.x(0)}),'glEnableVertexAttribArray':lambda:self.gl.append({'name':'glEnableVertexAttribArray','index':self.x(0)}),'glVertexAttribPointer':self.vertex_attrib_pointer,'glVertexAttribDivisor':lambda:self.gl.append({'name':'glVertexAttribDivisor','index':self.x(0),'divisor':self.x(1)}),'glBindVertexArray':lambda:self.gl.append({'name':'glBindVertexArray','id':self.x(0)}),'glGenVertexArrays':self.gen_buffers,'glBindTexture':self.bind_texture,'glPixelStorei':self.pixel_store,'glTexSubImage2D':self.tex_sub_image,'glTexParameteri':self.tex_parameter,'glSamplerParameteri':self.sampler_parameter,'glGenTextures':self.gen_textures,'glActiveTexture':self.active_texture,'glTexImage2D':lambda:self.gl.append({'name':'glTexImage2D','args':[self.x(i) for i in range(8)]}),'__strncpy_chk':self.strncpy_checked})
  self.u.mem_map(0x21000000,0x10000);self.state_stub=0x21000000;self.u.mem_write(self.state_stub,struct.pack('<II',0xd503201f,0xd65f03c0))
  self.texture_vtable=0x21000100
  for off in range(0,256,8):self.put(self.texture_vtable+off,'Q',self.state_stub)
  self.put(self.texture_vtable+32,'Q',self.syms['SPen::GLES::TextureObject::GetWidth() const']);self.put(self.texture_vtable+40,'Q',self.syms['SPen::GLES::TextureObject::GetHeight() const']);self.put(self.texture_vtable+64,'Q',self.syms['SPen::GLES::TextureObject::Update(void const*, int)']);self.put(self.texture_vtable+112,'Q',self.syms['SPen::GLES::TextureObject::SetParam(int, int)'])
  self.bitmap_update_method=self.syms['SPen::SPTextureBitmap::UpdateBitmap(void const*, int)']
  self.callbacks.pop('SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',None)
  self.active_native_import=None
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
  super().hook(u,address,size,data)
 def strstr(self):
  hay=self.cstr(self.x(0)).encode();needle=self.cstr(self.x(1)).encode();i=hay.find(needle);self.x(0,self.x(0)+i if i>=0 else 0)
 def strncpy(self):
  n=self.x(2);raw=bytes(self.u.mem_read(self.x(1),n)).split(b'\0',1)[0];self.u.mem_write(self.x(0),raw+b'\0'*max(0,n-len(raw)));self.x(0,self.x(0))
 def new_array(self):
  n=self.x(0)
  if not 0<n<=0x1000000:raise RuntimeError(f'array size {n}')
  p=self.alloc(n);self.u.mem_write(p,b'\xcd'*n);self.allocs[p]=n;self.x(0,p)
 def clock_gettime(self):
  self.put(self.x(1),'qq',0,0);self.x(0,0)
 def strncpy_checked(self):
  n,dest=self.x(2),self.x(3)
  if n>4096 or n>dest:raise RuntimeError(f'__strncpy_chk bounds n={n} dest={dest}')
  raw=bytes(self.u.mem_read(self.x(1),n)).split(b'\0',1)[0]
  self.u.mem_write(self.x(0),raw+b'\0'*max(0,n-len(raw)));self.x(0,self.x(0))
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
for filename, expected_hash in PINNED.items():
    actual_hash = hashlib.sha256((ROOT / "scratch/apk-analysis-native/arm64-v8a" / filename).read_bytes()).hexdigest()
    if actual_hash != expected_hash:
        raise RuntimeError(f"pinned DSO hash mismatch: {filename}: {actual_hash}")
if hashlib.sha256(LIBZ_PATH.read_bytes()).hexdigest() != LIBZ_SHA256:
    raise RuntimeError("host zlib pin mismatch")

machine = P()
native_trace = []
bitmap_trace_names = (
    "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::UpdateBitmap(void const*, int)",
)
bitmap_trace_addresses = {machine.syms[name]: name for name in bitmap_trace_names if name in machine.syms}

def trace_bitmap_entry(uc, address, _size, _data):
    if address in bitmap_trace_addresses:
        native_trace.append({"name": bitmap_trace_addresses[address], "address": hex(address), "args": [uc.reg_read(R.UC_ARM64_REG_X0 + index) for index in range(5)], "lr": hex(uc.reg_read(R.UC_ARM64_REG_LR))})

machine.u.hook_add(UC_HOOK_CODE, trace_bitmap_entry)

host_gl = {"next_id": 900, "texture": None, "gl": [], "fbo": 0}
host_files = {}
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
    machine.x(0, 0x8CD5)  # GL_FRAMEBUFFER_COMPLETE
    host_gl["gl"].append({"call": "framebuffer-status", "result": 0x8CD5})

def read_pixels():
    args = [machine.x(i) for i in range(7)]
    x, y, width, height, fmt, typ, destination = args
    if x or y or width != 2 or height != 2 or fmt != 0x1908 or typ != 0x1401:
        raise RuntimeError(f"unsupported native glReadPixels request {args}")
    source = host_gl["texture"]
    if source is None or len(source) != width * height:
        raise RuntimeError(f"expected one-channel 2x2 source texture, got {None if source is None else len(source)} bytes")
    # The uploaded native texture is GL_R8. GLES readback to RGBA expands R and
    # supplies zero G/B and opaque alpha, as the pixel-transfer API specifies.
    rgba = b"".join(bytes((value, 0, 0, 255)) for value in source)
    machine.u.mem_write(destination, rgba)
    host_gl["gl"].append({"call": "read-pixels", "args": args[:6], "rgba_hex": rgba.hex()})

def tex_image():
    args = [machine.x(i) for i in range(8)]
    stack = machine.u.reg_read(R.UC_ARM64_REG_SP)
    pixels = machine.get(stack, "Q")[0]
    target, level, internal, width, height, border, fmt, typ = args
    if args != [0x0DE1, 0, 0x8229, 2, 2, 0, 0x1903, 0x1401]:
        raise RuntimeError(f"unsupported native glTexImage2D request {args}")
    channels = 4 if fmt == 0x1908 else 3 if fmt == 0x1907 else 1
    size = width * height * channels
    data = bytes(machine.u.mem_read(pixels, size)) if pixels and size <= 0x1000000 else None
    host_gl["texture"] = data
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
    if x or y or width != 2 or height != 2 or fmt != 0x1903 or typ != 0x1401:
        raise RuntimeError(f"unsupported native glTexSubImage2D args={args}")
    host_gl["texture"] = data
    host_gl["gl"].append({"call": "tex-sub-image-2d", "args": args, "pixels_pointer": hex(pixels), "data_hex": data.hex()})

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
    "glFinish": no_op,
    "glInvalidateFramebuffer": no_op,
    "glReadPixels": read_pixels,
    "glTexImage2D": tex_image,
    "glTexSubImage2D": tex_sub_image,
})

# The render-thread queue executes each queued native task synchronously. This
# is an explicit scheduler boundary; actual factory/bitmap methods still run.
queue, queue_vtable = machine.alloc(32), machine.alloc(64)
machine.queue = queue
machine.queue_post = machine.alloc(16)
machine.queue_task_return = machine.alloc(16)
machine.pending_queue_return = None
machine.put(queue, "Q", queue_vtable)
machine.put(queue_vtable + 16, "Q", machine.queue_post)

cache_root = b"/tmp/sdocx-native-bitmap-cache\0"
cache_path = machine.alloc(len(cache_root))
machine.u.mem_write(cache_path, cache_root)
machine.call("SPen::System::SetSDKCacheDirectoryPath(char const*)", [cache_path])

pixels = bytes((17, 83, 149, 231))
pixel_ptr = machine.alloc(len(pixels))
machine.u.mem_write(pixel_ptr, pixels)
bitmap_name = machine.alloc(32)
machine.u.mem_write(bitmap_name, b"pdf-image-probe\0")
factory_name = "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)"
machine.call(factory_name, [queue, 2, 2, bitmap_name, pixel_ptr, 0, 0, 0, 0, 0], instruction_limit=5_000_000)
bitmap = machine.x(0)
machine.call("SPen::SPTextureBitmap::UpdateBitmap(void const*, int)", [bitmap, pixel_ptr, 1], instruction_limit=5_000_000)
rect = machine.alloc(16)
machine.put(rect, "4i", 0, 0, 2, 2)
readback = machine.alloc(16)
machine.u.mem_write(readback, b"\xcc" * 16)
machine.call("SPen::SPTextureBitmap::GetBitmapData(SPen::Rect const&, void*)", [bitmap, rect, readback], instruction_limit=5_000_000)
rgba = bytes(machine.u.mem_read(readback, 16))

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
libcpp_symtab = libcpp_by_name[".dynsym"]
libcpp_dynstr = libcpp_by_name[".dynstr"]
libcpp_strings = libcpp_blob[libcpp_dynstr[4]:libcpp_dynstr[4] + libcpp_dynstr[5]]
libcpp_symbols = []
for offset in range(libcpp_symtab[4], libcpp_symtab[4] + libcpp_symtab[5], libcpp_symtab[9]):
    name_offset, info, other, section_index, value, size = struct.unpack_from("<IBBHQQ", libcpp_blob, offset)
    end = libcpp_strings.find(b"\0", name_offset)
    libcpp_symbols.append(libcpp_strings[name_offset:end].decode() if name_offset else "")
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

trace_names = (
    "SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)",
    "SPen::SPTextureBitmap::UpdateBitmap(void const*, int)",
    "SPen::SPTextureBitmap::GetBitmapData(SPen::Rect const&, void*)",
    "SPen::System::SetSDKCacheDirectoryPath(char const*)",
    "SPen::PdfImageAdapter::PdfImageAdapter(std::__ndk1::shared_ptr<SPen::PdfExporter>, SPen::IContext const*)",
    "SPen::PdfImageAdapter::SetImage(SPen::ISPBitmap*)",
    "SPen::PdfImageAdapter::saveBitmap(SPen::ISPBitmap*, SPen::Rect, SPen::String&)",
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

def host_fopen():
    global next_host_file
    path, mode = guest_string(machine.x(0)), guest_string(machine.x(1))
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
    if token not in host_files or size * count > 0x10000000:
        raise RuntimeError(f"invalid native fwrite stream={token:#x} size={size} count={count}")
    data = bytes(machine.u.mem_read(source, size * count))
    written = host_files[token].write(data)
    host_file_events.append({"event": "fwrite", "bytes": written})
    machine.x(0, written // size if size else 0)

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

machine.callbacks.update({
    "mkdir": host_mkdir,
    "stat": host_stat,
    "strtok": host_strtok,
    "strcmp": host_strcmp,
    "strncmp": host_strncmp,
    "strchr": host_strchr,
    "fopen": host_fopen,
    "fopen64": host_fopen,
    "fwrite": host_fwrite,
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
})

adapter = machine.alloc(256)
shared_ptr = machine.alloc(16)
machine.call("SPen::PdfImageAdapter::PdfImageAdapter(std::__ndk1::shared_ptr<SPen::PdfExporter>, SPen::IContext const*)", [adapter, shared_ptr, 0], instruction_limit=5_000_000)
info = machine.get(adapter + 32, "Q")[0]
if not info:
    raise RuntimeError("native PdfImageAdapter constructor returned null PDFExportInfo")
info_before_set_image = bytes(machine.u.mem_read(info, 128))
set_image_name = "SPen::PdfImageAdapter::SetImage(SPen::ISPBitmap*)"
machine.call(set_image_name, [adapter, bitmap], instruction_limit=20_000_000)
set_image_result = machine.x(0)
info_after_set_image = bytes(machine.u.mem_read(info, 128))
info_words = list(machine.get(info, "16Q"))
native_path_length = machine.get(info + 88, "Q")[0]
native_path_pointer = machine.get(info + 96, "Q")[0]
if native_path_length > 4096 or (native_path_length and not native_path_pointer):
    raise RuntimeError(f"invalid native std::string path fields pointer={native_path_pointer:#x} length={native_path_length}")
output_path = bytes(machine.u.mem_read(native_path_pointer, native_path_length)).decode("utf-8") if native_path_length else None
if set_image_result != 1:
    raise RuntimeError(f"native PdfImageAdapter::SetImage(ISPBitmap*) returned {set_image_result}")
if not output_path or not Path(output_path).is_file():
    raise RuntimeError(f"native SetImage did not create its cached PNG at {output_path!r}")
png_bytes = Path(output_path).read_bytes()
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
if decoded_pixels != rgba:
    raise RuntimeError(f"native PNG decoded bytes {decoded_pixels.hex()} differ from GetBitmapData bytes via host GL simulation {rgba.hex()}")
if host_zstreams or host_zallocations or allocator_errors or host_files:
    raise RuntimeError(f"native PNG resource cleanup failed: streams={len(host_zstreams)} allocs={len(host_zallocations)} errors={allocator_errors} files={list(host_files)}")
if machine.host_create_graphics_calls or machine.host_create_texture_calls:
    raise RuntimeError(f"unexpected synthetic factory callback use: graphics={machine.host_create_graphics_calls}, texture={machine.host_create_texture_calls}")
set_image_trace = next(item for item in native_trace if item["name"].endswith("SetImage(SPen::ISPBitmap*)"))
if set_image_trace["args"][0] != adapter or set_image_trace["args"][1] != bitmap:
    raise RuntimeError("native SetImage did not receive the adapter and actual SPTextureBitmap pointers")
if machine.get(adapter + 32, "Q")[0] != info:
    raise RuntimeError("adapter mInfo changed from native constructor-owned pointer")
if info_after_set_image[104] != 1:
    raise RuntimeError(f"native SetImage did not mark the cached path owned: byte={info_after_set_image[104]}")
trace_counts = {name: sum(item["name"] == name for item in native_trace) for name in {item["name"] for item in native_trace}}
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
    "schema": "sdocx.pdf-image-bitmap-native.v1",
    "native_factory": "SPGraphicsFactory::CreateBitmap(queue,2,2,...)",
    "native_update": "SPTextureBitmap::UpdateBitmap(void const*,int)",
    "native_get_bitmap_data": "SPTextureBitmap::GetBitmapData(Rect const&,void*) called; returned bytes were supplied by explicit host GL/GLES readpixels boundary",
    "bitmap_pointer": hex(bitmap),
    "synthetic_source_r8_hex": pixels.hex(),
    "host_gl_simulated_get_bitmap_data_rgba_hex": rgba.hex(),
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
    "native_std_string_layout": {"field_offset": 80, "length": native_path_length, "data_pointer": hex(native_path_pointer)},
    "native_zlib_guest_allocator_pointers": "recorded in the guest z_stream at offsets 64/72; bridged for host zlib and never called as host pointers",
    "sdk_cache_root_input": "/tmp/sdocx-native-bitmap-cache",
    "native_call_trace": native_trace,
    "host_file_events": host_file_events,
    "host_zlib_events": host_zlib_events,
    "host_zlib_sha256": LIBZ_SHA256,
    "pinned_dso_sha256": {
        **PINNED,
    },
    "host_gl_simulation": {"calls": host_gl["gl"], "storage": "host bytearray models the native texture upload; framebuffer status is stubbed complete; readpixels expands R8 to RGBA", "gpu_rendering_claimed": False},
    "renderer_texture_records": machine.texture_records,
    "native_entries": machine.actual_entries,
    "host_renderer_factory_callbacks": {"CreateGraphicsObject": machine.host_create_graphics_calls, "CreateTextureObject": machine.host_create_texture_calls},
    "host_boundaries": {"queue": "host synchronous task scheduler dispatches the native queued task", "gl_gles": "host simulated GL/GLES objects and framebuffer/readpixels; no GPU implementation", "zlib": "pinned host libz 1.2.8 through z_stream and guest allocator bridges; APK libSPenLibpng is native", "pixels": "synthetic 2x2 R8 source bytes, not DrawingFactory output", "filesystem": "bounded host stdio/path callbacks", "excluded": "DrawingFactory/ObjectStroke raster paint, PdfiumImageHandler::AddImage, and PDF export are not executed"},
}
parser = argparse.ArgumentParser(description=__doc__)
mode = parser.add_mutually_exclusive_group(required=True)
mode.add_argument("--write-fixture", action="store_true")
mode.add_argument("--check-fixture", action="store_true")
args = parser.parse_args()
if args.write_fixture:
    FIXTURE.write_text(json.dumps(capture, indent=2, sort_keys=True) + "\n")
    print(f"wrote {FIXTURE.relative_to(ROOT)}")
else:
    expected = json.loads(FIXTURE.read_text())
    if capture != expected:
        raise SystemExit("native bitmap overload capture differs from fixture")
    print("PDF image bitmap native fixture matches")
