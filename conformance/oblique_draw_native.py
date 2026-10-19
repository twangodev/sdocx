"""Bounded native Oblique paint-input and Renderer blend conformance probe.

Run from the repository root with the research Python dependencies:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_draw_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/oblique_draw_native.py --check-fixture

ARGB and host renderer resources are explicit synthetic inputs. GLES calls are
intercepted; this does not execute a device renderer or claim pixel parity.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path
from unicorn import arm64_const as R
from oblique_redraw_native import RedrawOracle
from oblique_native import BASE, SENTINEL, LIBRARY, STACK, HEAP

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'conformance/oblique-draw-native.json'
PENCOMMON=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so'
PENBASE=0x60000000
RENDERER=ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so'
RENDERER_BASE=0x61000000
CTOR=0x445fc; INIT=0x4473c; SET_PEN=0x4a558; DRAW=0x4746c

class Probe(RedrawOracle):
 def __init__(self):
  case={'id':'explicit-argb-draw','points':[[0.,0.],[10.,5.],[20.,10.]],'size':9.75,'tool_type':0}
  super().__init__(LIBRARY,case)
  self.callbacks={}; self.next_stub=HEAP+0x18000
  self._map_pencommon()
  self.blend_descriptors=[]; self.blend_activations=[]; self.callback_log=[]
  self.renderer_gl_calls=[]
  self.native_color_binding=None
  self.probe_entries=[]
  self._map_renderer()
  self.native_import_symbols={**self.pen_symbols,**self.renderer_symbols}
  # Resolve PenDrawableRT::SetPenData and descriptor methods from its actual owning DSO.
  self._patch_pencommon_jumps()
  self.extra_imports={address: ('disable_depth' if 'DisableDepthStencil' in name else
                                'disable_blend' if 'DisableBlending' in name else
                                'rect_empty' if 'RectF::IsEmpty' in name else
                                'matrix_ctor' if 'Matrix4<float>::Matrix4' in name else 'other')
                      for address,name in self.stub_names.items()
                      if any(token in name for token in ('DisableDepthStencil','DisableBlending','RectF::IsEmpty','Matrix4<float>::Matrix4'))}
  for off in (288,296,320,328,336): self.u.mem_write(self.rt+off,struct.pack('<Q',HEAP+0x14000+off))
  # ctor owns fields and vtable; install our host resource field values after it.
  self._call(BASE+CTOR,[self.rt,self.manager])
  for off in (288,296,320,328,336): self.u.mem_write(self.rt+off,struct.pack('<Q',HEAP+0x14000+off))
  self._call(BASE+INIT,[self.rt])
  self._call(PENBASE+SET_PEN,[self.rt],s0=9.75,w1=0x80402010)
  self._init_draw_hosts()
 def _map_pencommon(self):
  self.pen_blob=PENCOMMON.read_bytes(); self.pen_sha=hashlib.sha256(self.pen_blob).hexdigest()
  self.u.mem_map(PENBASE,0x100000)
  phoff=struct.unpack_from('<Q',self.pen_blob,32)[0]; stride,num=struct.unpack_from('<HH',self.pen_blob,54)
  for i in range(num):
   typ,flags,off,va,pa,filesz,memsz,align=struct.unpack_from('<II6Q',self.pen_blob,phoff+i*stride)
   if typ==1 and memsz:
    self.u.mem_write(PENBASE+va,self.pen_blob[off:off+filesz])
    if memsz>filesz:self.u.mem_write(PENBASE+va+filesz,bytes(memsz-filesz))
  shoff=struct.unpack_from('<Q',self.pen_blob,40)[0]; es,n,si=struct.unpack_from('<HHH',self.pen_blob,58)
  secs=[struct.unpack_from('<IIQQQQIIQQ',self.pen_blob,shoff+i*es) for i in range(n)]
  names_sec=secs[si]; names=self.pen_blob[names_sec[4]:names_sec[4]+names_sec[5]]
  by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}
  ds,st=by['.dynsym'],by['.dynstr']; strings=self.pen_blob[st[4]:st[4]+st[5]]
  self.pen_symbols={}
  for off in range(ds[4],ds[4]+ds[5],ds[9]):
   no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',self.pen_blob,off)
   name=strings[no:strings.find(b'\0',no)].decode() if no else ''
   if val:self.pen_symbols[name]=PENBASE+val
  for secname in ('.rela.dyn','.rela.plt'):
   sec=by.get(secname)
   if not sec:continue
   for off in range(sec[4],sec[4]+sec[5],sec[9]):
    where,info,addend=struct.unpack_from('<QQq',self.pen_blob,off); typ,idx=info&0xffffffff,info>>32
    if typ==0x403:self.u.mem_write(PENBASE+where,struct.pack('<Q',PENBASE+addend))
    elif typ==0x402:
     no=struct.unpack_from('<IBBHQQ',self.pen_blob,ds[4]+idx*ds[9])[0]
     name=strings[no:strings.find(b'\0',no)].decode() if no else ''
     target=self.pen_symbols.get(name,self.symbols.get(name))
     if target is None:target=self._host_stub_addr(f'pen_import:{name}')
     self.u.mem_write(PENBASE+where,struct.pack('<Q',target))
 def _map_renderer(self):
  blob=RENDERER.read_bytes(); self.renderer_sha=hashlib.sha256(blob).hexdigest(); self.u.mem_map(RENDERER_BASE,0x100000)
  phoff=struct.unpack_from('<Q',blob,32)[0]; stride,num=struct.unpack_from('<HH',blob,54)
  for i in range(num):
   typ,flags,off,va,pa,filesz,memsz,align=struct.unpack_from('<II6Q',blob,phoff+i*stride)
   if typ==1 and memsz:
    self.u.mem_write(RENDERER_BASE+va,blob[off:off+filesz])
    if memsz>filesz:self.u.mem_write(RENDERER_BASE+va+filesz,bytes(memsz-filesz))
  shoff=struct.unpack_from('<Q',blob,40)[0]; es,n,si=struct.unpack_from('<HHH',blob,58); secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*es) for i in range(n)]
  ss=secs[si]; names=blob[ss[4]:ss[4]+ss[5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=blob[st[4]:st[4]+st[5]]; self.renderer_symbols={}
  for off in range(ds[4],ds[4]+ds[5],ds[9]):
   no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',blob,off); name=strings[no:strings.find(b'\0',no)].decode() if no else ''
   if val:self.renderer_symbols[name]=RENDERER_BASE+val
  plt=by['.plt'][3]; rel=by['.rela.plt']; ix=0
  for off in range(rel[4],rel[4]+rel[5],rel[9]):
   where,info,addend=struct.unpack_from('<QQq',blob,off); typ,si=info&0xffffffff,info>>32
   if typ==0x402:
    no=struct.unpack_from('<IBBHQQ',blob,ds[4]+si*ds[9])[0]; name=strings[no:strings.find(b'\0',no)].decode() if no else ''
    address=RENDERER_BASE+plt+0x20+ix*16
    stub=self._host_stub_addr(f'renderer_import:{name}')
    self.u.mem_write(RENDERER_BASE+where,struct.pack('<Q',stub)); ix+=1
  reladyn=by.get('.rela.dyn')
  if reladyn:
   for off in range(reladyn[4],reladyn[4]+reladyn[5],reladyn[9]):
    where,info,addend=struct.unpack_from('<QQq',blob,off)
    if (info&0xffffffff)==0x403:self.u.mem_write(RENDERER_BASE+where,struct.pack('<Q',RENDERER_BASE+addend))
 def _patch_pencommon_jumps(self):
  shoff=struct.unpack_from('<Q',self.blob,40)[0]; es,n,si=struct.unpack_from('<HHH',self.blob,58)
  secs=[struct.unpack_from('<IIQQQQIIQQ',self.blob,shoff+i*es) for i in range(n)]
  ss=secs[si]; names=self.blob[ss[4]:ss[4]+ss[5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}
  ds,st=by['.dynsym'],by['.dynstr']; strings=self.blob[st[4]:st[4]+st[5]]
  sym=[]
  for off in range(ds[4],ds[4]+ds[5],ds[9]):
   no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',self.blob,off); sym.append(strings[no:strings.find(b'\0',no)].decode() if no else '')
  for secname in ('.rela.dyn','.rela.plt'):
   sec=by.get(secname)
   if not sec:continue
   for off in range(sec[4],sec[4]+sec[5],sec[9]):
    where,info,addend=struct.unpack_from('<QQq',self.blob,off); typ,idx=info&0xffffffff,info>>32
    if typ==0x402 and sym[idx] in self.pen_symbols:
     self.u.mem_write(BASE+where,struct.pack('<Q',self.pen_symbols[sym[idx]]))
 def _alloc_bytes(self,raw): return self._alloc(raw)
 def _call(self,addr,args,s0=None,w1=None,limit=1000000):
  for i,a in enumerate(args):self.u.reg_write(getattr(R,f'UC_ARM64_REG_X{i}'),a)
  if s0 is not None:self.u.reg_write(R.UC_ARM64_REG_S0,struct.unpack('<I',struct.pack('<f',s0))[0])
  if w1 is not None:self.u.reg_write(R.UC_ARM64_REG_W1,w1)
  self.u.reg_write(R.UC_ARM64_REG_SP,STACK+0x8000); self.u.reg_write(R.UC_ARM64_REG_LR,SENTINEL)
  try:self.u.emu_start(addr,SENTINEL,count=limit)
  except Exception as e:raise RuntimeError(f'call fault addr={addr:#x} pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={self.u.reg_read(R.UC_ARM64_REG_LR):#x} import={self.stub_names.get(self.u.reg_read(R.UC_ARM64_REG_PC))} trace={[hex(a-BASE) for a in self.trace[-20:]]}') from e
  if self.u.reg_read(R.UC_ARM64_REG_PC)!=SENTINEL: raise RuntimeError(f'call did not return addr={addr:#x}, pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x}')
 def _init_draw_hosts(self):
  # Minimal native interface objects to drive the real Draw control flow.
  self.stub_base=HEAP+0x18000
  self.primary=HEAP+0x19000
  def obj(size=32):
   p=self._alloc(bytes(size)); return p
  def stub(name):
   p=self.next_stub; self.next_stub+=8; self.u.mem_write(p,struct.pack('<II',0xd503201f,0xd65f03c0)); self.callbacks[p]=name; return p
  def table(methods,size=256):
   t=self._alloc(bytes(size));
   for off,name in methods.items():self.u.mem_write(t+off,struct.pack('<Q',stub(name)))
   return t
  def setobj(p,v):self.u.mem_write(p,struct.pack('<Q',v))
  def setfield(off,p):self.u.mem_write(self.rt+off,struct.pack('<Q',p))
  # Ensure the vector guard passes and has a bounded nonempty Vector3 stream.
  vec=self._alloc(bytes(24)); data=self._alloc(struct.pack('<9f',0,0,0,1,0,0,0,1,0)); self.u.mem_write(vec,struct.pack('<3Q',data,data+36,data+36)); setfield(272,vec); self.u.mem_write(self.rt+193,b'\0')
  self.u.mem_write(self.rt+25,b'\1')
  # Populate BlendStateObject vtables returned by native Init factory shims.
  # Replace the blend factory PLT's host shim via the common hook path.
  for off in (288,296,320,328,336):
   p=obj(64); vt=table({16:'plain',24:'plain',40:'plain',48:'plain',64:'plain',72:'plain',168:'plain',192:'plain'},256); setobj(p,vt); bridge=obj(8); setobj(bridge,stub('shader_interface')); self.u.mem_write(vt,struct.pack('<Q',bridge)); setfield(off,p)
  # bitmap with required entries 40,72
  self.bitmap=obj(); bmpvt=table({40:'plain',48:'plain',72:'bitmap_dim'},128); setobj(self.bitmap,bmpvt)
  # Graphics endpoint used for two texture-/shader queries.
  self.gfx=obj(); gfxvt=table({192:'gfx_query'},256); setobj(self.gfx,gfxvt); setfield(256,self.gfx)
  self.resource=obj(); resvt=table({168:'resource_child',64:'shader_handle',72:'resource_count'},192); setobj(self.resource,resvt)
  self.child=obj(); childvt=table({64:'shader_handle',72:'resource_count'},128); setobj(self.child,childvt)
  self.shader_handle=obj(); shv=table({16:'matrix_out',40:'plain',48:'shader_draw'},64); setobj(self.shader_handle,shv)
  shader_aux=obj(32); self.u.mem_write(shader_aux+16,struct.pack('<Q',stub('plain'))); self.u.mem_write(self.shader_handle+8,struct.pack('<Q',shader_aux))
  # Per-field shader objects carry embedded interface objects at +8 or +16.
  for off in (320,328,336):
   p=struct.unpack('<Q',self.u.mem_read(self.rt+off,8))[0]; inner=obj(32); ivt=table({0:'shader_interface',16:'matrix_out'},64); setobj(inner,ivt); self.u.mem_write(p+8,struct.pack('<Q',inner)); self.u.mem_write(p+16,struct.pack('<Q',inner))
  # callback stubs on any vtables we created are dispatched by extra code hook.
  self.resource_count_calls=0
  def draw_hook(u,address,size,_):
   name=self.callbacks.get(address)
   entry={'entered':name,'address':hex(address),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),
          'args_x0_x3':[u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}')) for i in range(4)]}
   if name=='shader_interface':
    ptr=u.reg_read(R.UC_ARM64_REG_X1)
    if ptr:
     try:entry['arg1_bytes_hex']=bytes(u.mem_read(ptr,16)).hex()
     except Exception:pass
    if u.reg_read(R.UC_ARM64_REG_LR)==BASE+0x47b90:
     self.native_color_binding={'setter_return_pc':'0x47b90','receiver':hex(u.reg_read(R.UC_ARM64_REG_X0)),
                                'rgba_field_ptr':hex(ptr),'rgba_field_is_SetPenData_output':ptr==self.rt+44,
                                'rgba_bytes_hex':bytes(u.mem_read(ptr,16)).hex()}
   self.callback_log.append(entry)
   if not name:return
   x=lambda i:u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))
   if name=='bitmap_dim':u.reg_write(R.UC_ARM64_REG_W0,0)
   elif name=='gfx_query':u.reg_write(R.UC_ARM64_REG_X0,self.resource)
   elif name=='resource_child':u.reg_write(R.UC_ARM64_REG_X0,self.child)
   elif name=='resource_count':
    self.resource_count_calls+=1; value=1 if self.resource_count_calls==1 else 2; u.reg_write(R.UC_ARM64_REG_W0,value); self.callback_log.append({'name':name,'return':value})
   elif name=='shader_handle':u.reg_write(R.UC_ARM64_REG_X0,self.shader_handle)
   elif name=='matrix_out':
    out=u.reg_read(R.UC_ARM64_REG_X8); u.mem_write(out,struct.pack('<16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1))
   elif name=='shader_interface':u.reg_write(R.UC_ARM64_REG_X0,self.shader_handle)
   elif name=='shader_draw':self.callback_log.append({'name':name,'args':[x(i) for i in range(4)]})
   elif name=='blend_activate':self.blend_activations.append({'object':hex(x(0)),'args':[x(i) for i in range(1,4)]})
   elif name.startswith('renderer_import:'):
    glname=name.split(':',1)[1]
    counts={'glBlendEquationSeparate':2,'glBlendFuncSeparate':4,'glEnable':1,'glDisable':1,'glColorMask':4}
    if glname in counts:self.renderer_gl_calls.append({'call':glname,'args':[x(i) for i in range(counts[glname])]})
    else:raise RuntimeError(f'unexpected Renderer GLES import {glname}')
   elif name.startswith('pen_import:'):
    raise RuntimeError(f'unimplemented PenCommon import {name.split(":",1)[1]}')
   elif name=='plain':pass
   u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  self.draw_dispatch=draw_hook
 def _hook(self,uc,address,size,_data):
  if address in (BASE+CTOR,BASE+INIT,BASE+DRAW):self.probe_entries.append(address)
  if address==BASE+0x47530:
   self.callback_log.append({'name':'compare','w22':uc.reg_read(R.UC_ARM64_REG_W22),'w23':uc.reg_read(R.UC_ARM64_REG_W23)})
  if address in (BASE+0x477e4,BASE+0x475d8):
   x0=uc.reg_read(R.UC_ARM64_REG_X0); x8=uc.reg_read(R.UC_ARM64_REG_X8); self.callback_log.append({'name':'trace_vcall','pc':hex(address-BASE),'x0':hex(x0),'x8':hex(x8),'q0':bytes(uc.mem_read(x0,16)).hex() if x0 else None,'q8':bytes(uc.mem_read(x8,16)).hex() if x8 else None})
  if address in getattr(self,'callbacks',{}):
   if self.callbacks[address].startswith('pen_import:'):
    raise RuntimeError(f"unimplemented PenCommon import {self.callbacks[address].split(':',1)[1]}")
   if getattr(self,'draw_dispatch',None):self.draw_dispatch(uc,address,size,_data)
   else:self.u.reg_write(R.UC_ARM64_REG_PC,self.u.reg_read(R.UC_ARM64_REG_LR))
   return
  # Resolve all native PenCommon state routines and use a narrow factory boundary.
  if address in getattr(self,'stub_names',{}):
   name=self.stub_names[address]
   if 'CreateBlendStateObject' in name:
    ptr=self.u.reg_read(R.UC_ARM64_REG_X0); raw=bytes(self.u.mem_read(ptr,32)); self.blend_descriptors.append(raw.hex())
    obj=self._alloc(bytes(16)); vt=self._alloc(bytes(64)); self.u.mem_write(obj,struct.pack('<Q',vt)); self.u.mem_write(vt+24,struct.pack('<Q',self._host_stub_addr('blend_activate')))
    self.u.reg_write(R.UC_ARM64_REG_X0,obj); self._return(); return
   if 'ManagedObject' in name and 'Register' in name:self._return();return
   if name=='__android_log_print':self.u.reg_write(R.UC_ARM64_REG_W0,0);self._return();return
   if address in self.extra_imports:
    kind=self.extra_imports[address]
    if kind=='matrix_ctor':
     out=self.u.reg_read(R.UC_ARM64_REG_X0);self.u.mem_write(out,struct.pack('<16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1))
    elif kind=='rect_empty':self.u.reg_write(R.UC_ARM64_REG_W0,1)
    self._return();return
   if name in self.native_import_symbols:
    self.u.reg_write(R.UC_ARM64_REG_PC,self.native_import_symbols[name]); return
  if address in getattr(self,'extra_imports',{}):
   name=self.extra_imports[address]
   if name=='other':raise RuntimeError(f'unimplemented Oblique host import at {address:#x}')
   if name=='matrix_ctor':
    out=self.u.reg_read(R.UC_ARM64_REG_X0); self.u.mem_write(out,struct.pack('<16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)); self._return();return
   if name in ('disable_blend','disable_depth','rect_empty'):
    if name=='rect_empty':self.u.reg_write(R.UC_ARM64_REG_W0,1)
    self._return();return
  super()._hook(uc,address,size,_data)
 def _host_stub_addr(self,name):
  if not hasattr(self,'callbacks'):self.callbacks={}
  p=self.next_stub; self.next_stub+=8; self.u.mem_write(p,struct.pack('<II',0xd503201f,0xd65f03c0)); self.callbacks[p]=name; return p
 def _return(self):self.u.reg_write(R.UC_ARM64_REG_PC,self.u.reg_read(R.UC_ARM64_REG_LR))
def generate():
    p=Probe()
    matrix=p._alloc(struct.pack('<16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1))
    p._call(BASE+DRAW,[p.rt,p.bitmap,matrix],limit=1_000_000)
    draw_return_pc=p.u.reg_read(R.UC_ARM64_REG_PC)
    ctor=p.renderer_symbols['_ZN4SPen4GLES16BlendStateObjectC1ERKNS_20BlendStateDescriptorE']
    setter=p.renderer_symbols['_ZN4SPen4GLES16BlendStateObject13SetDescriptorERKNS_20BlendStateDescriptorE']
    activate=p.renderer_symbols['_ZN4SPen4GLES16BlendStateObject8ActivateEv']
    mapping=[]
    for raw in p.blend_descriptors:
        descriptor=p._alloc(bytes.fromhex(raw)); obj=p._alloc(bytes(96)); before=len(p.renderer_gl_calls)
        p._call(ctor,[obj,descriptor]); p._call(setter,[obj,descriptor]); p._call(activate,[obj])
        mapping.append({'descriptor_hex':raw,'gles_calls':p.renderer_gl_calls[before:]})
    expected=[
        [{'call':'glBlendEquationSeparate','args':[0x8006,0x8006]}, {'call':'glBlendFuncSeparate','args':[1,0x0303,1,0x0303]}, {'call':'glEnable','args':[0x0be2]}, {'call':'glColorMask','args':[1,1,1,1]}],
        [{'call':'glBlendEquationSeparate','args':[0x8008,0x8008]}, {'call':'glBlendFuncSeparate','args':[1,1,1,1]}, {'call':'glEnable','args':[0x0be2]}, {'call':'glColorMask','args':[1,1,1,1]}],
    ]
    if len(p.blend_descriptors)!=2 or [item['gles_calls'] for item in mapping]!=expected:
        raise AssertionError('actual Renderer blend mapping differed from expected four-call sequences')
    expected_rgba=struct.pack('<4f',64/255,32/255,16/255,128/255).hex()
    if not p.native_color_binding or not p.native_color_binding['rgba_field_is_SetPenData_output'] or p.native_color_binding['rgba_bytes_hex']!=expected_rgba:
        raise AssertionError(f'native SetPenData color did not reach Draw shader color argument: {p.native_color_binding}')
    rt_blends={304:struct.unpack('<Q',p.u.mem_read(p.rt+304,8))[0],312:struct.unpack('<Q',p.u.mem_read(p.rt+312,8))[0]}
    activation_map=[]
    for event in p.blend_activations:
        address=int(event['object'],16); offset=next((k for k,v in rt_blends.items() if v==address),None)
        activation_map.append({'rt_field_offset':offset,'descriptor_index':{304:0,312:1}.get(offset)})
    if [item['rt_field_offset'] for item in activation_map] != [312,304]:
        raise AssertionError(f'unexpected Draw blend activation order: {activation_map}')
    library_hashes={
        'libSPenObliquePen.so':hashlib.sha256(Path(LIBRARY).read_bytes()).hexdigest(),
        'libSPenPenCommon.so':p.pen_sha,
        'libSPenRenderer.so':p.renderer_sha,
    }
    return {
        'schema':'sdocx-oblique-draw-native-v1',
        'libraries':library_hashes,
        'entrypoints':{'ObliquePenStrokeDrawableRTV1::ObliquePenStrokeDrawableRTV1':'0x445fc','ObliquePenStrokeDrawableRTV1::Init':'0x4473c','PenDrawableRT::SetPenData':'libSPenPenCommon.so:0x4a558','ObliquePenStrokeDrawableRTV1::Draw':'0x4746c','GLES::BlendStateObject::BlendStateObject':'libSPenRenderer.so:resolved ELF symbol','GLES::BlendStateObject::SetDescriptor':'libSPenRenderer.so:resolved ELF symbol','GLES::BlendStateObject::Activate':'libSPenRenderer.so:resolved ELF symbol'},
        'explicit_input':{'kind':'synthetic, explicit host input; not captured device data','argb_u32':'0x80402010','set_pen_data_size_f32':9.75,'draw_matrix_f32':[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]},
        'native_init':{'constructor_entry_count':p.probe_entries.count(BASE+CTOR),'entry_count':p.probe_entries.count(BASE+INIT),'blend_descriptors_hex':p.blend_descriptors,'factory_boundary':'synthetic CreateBlendStateObject and ManagedObject::Register host adapters; Init and descriptor construction/mutation are native'},
        'native_set_pen_data':{'executed':True,'rgba_bytes_hex':expected_rgba,'rgba_f32':list(struct.unpack('<4f',p.u.mem_read(p.rt+44,16))),'stored_size_u32':struct.unpack('<I',p.u.mem_read(p.rt+60,4))[0]},
        'native_draw':{'entry_count':p.probe_entries.count(BASE+DRAW),'return_pc':hex(draw_return_pc),'color_binding':p.native_color_binding,'blend_objects_by_rt_field_offset':{str(k):hex(v) for k,v in rt_blends.items()},'blend_activation_order':activation_map,'host_adapters':['bitmap interface methods','graphics/resource handles','shader-interface methods','queue/context handles'],'scope':'actual RTV1 Draw control flow, matrix/color helper, and blend activation; interface objects are synthetic adapters'},
        'native_renderer_blend_mapping':mapping,
        'limitations':['ARGB and host resources are explicit synthetic inputs; no captured Samsung color or framebuffer','Renderer GLES entry points are intercepted after actual BlendStateObject methods; no GPU or pixels are executed'],
    }

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture',action='store_true')
    mode.add_argument('--check-fixture',action='store_true')
    args=parser.parse_args()
    result=generate()
    encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f'wrote {FIXTURE}')
    else:
        if not FIXTURE.exists() or json.loads(FIXTURE.read_text())!=result:
            raise SystemExit(f'native Oblique Draw differs from {FIXTURE}')
        print('verified native Oblique Init, SetPenData, Draw, and Renderer blend mappings')

if __name__=='__main__':
    main()
