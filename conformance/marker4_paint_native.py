"""Capture Marker4 RTV1 blend setup and saved ARGB forwarding from pinned APK code.

Native RT::Init, Renderer blend descriptor setters/activation, and inherited
PenDrawableRT::SetPenData execute. GL calls and renderer factory/bitmap interfaces
are explicit host facades. No GPU pixels are claimed by this oracle.
"""
import argparse, hashlib, json, os, re, struct, subprocess
from pathlib import Path
from fountain_native import NativeFountain, HASHES

ROOT=Path(__file__).resolve().parents[1]
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
OB=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
OB_ENV={**os.environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
HASHES['Marker4']='23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff'
HASHES['Renderer']='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
MARKER_SHA=HASHES['Marker4']; RENDERER_SHA=HASHES['Renderer']
BASES={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Marker4':0x3000000,'Renderer':0x4000000}

def elf_sections(blob):
 shoff=struct.unpack_from('<Q',blob,40)[0]; ent,n,si=struct.unpack_from('<HHH',blob,58)
 rows=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*ent) for i in range(n)]
 ss=rows[si]; names=blob[ss[4]:ss[4]+ss[5]]
 return {names[s[0]:names.find(b'\0',s[0])].decode():s for s in rows}

def load_elf(o,name):
 blob=(LIB/f'libSPen{name}.so').read_bytes()
 expected={'Marker4':MARKER_SHA,'Renderer':RENDERER_SHA}[name]
 if hashlib.sha256(blob).hexdigest()!=expected: raise ValueError(f'unpinned {name} DSO')
 base=BASES[name]; phoff=struct.unpack_from('<Q',blob,32)[0]; ent,n=struct.unpack_from('<HH',blob,54)
 o.u.mem_map(base,0x200000)
 for i in range(n):
  typ,flags,off,va,pa,filesz,memsz,align=struct.unpack_from('<II6Q',blob,phoff+i*ent)
  if typ==1:
   o.u.mem_write(base+va,blob[off:off+filesz])
   if memsz>filesz:o.u.mem_write(base+va+filesz,bytes(memsz-filesz))
 sec=elf_sections(blob); ds,st=sec['.dynsym'],sec['.dynstr']; strings=blob[st[4]:st[4]+st[5]]
 syms=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',blob,off)
  nm=strings[no:strings.find(b'\0',no)].decode() if no else ''; syms.append(nm)
  if val:
   dem=subprocess.check_output(['c++filt','-n',nm],text=True).strip() if nm.startswith('_Z') else nm
   o.syms[dem]=base+val
 # Map stubs by .rela.plt order, never by objdump's sometimes-wrong labels.
 rel=sec['.rela.plt']; plt=sec['.plt'][3]
 slot=0
 for off in range(rel[4],rel[4]+rel[5],rel[9]):
  where,info,add=struct.unpack_from('<QQq',blob,off); typ,ix=info&0xffffffff,info>>32
  if typ==0x402:
   raw=syms[ix]
   dem=subprocess.check_output(['c++filt','-n',raw],text=True).strip() if raw.startswith('_Z') else raw
   o.plt[base+plt+0x20+slot*16]=dem; slot+=1
 return blob,sec

def map_exact_plt(o,name):
 blob=(LIB/f'libSPen{name}.so').read_bytes(); sec=elf_sections(blob)
 ds,st=sec['.dynsym'],sec['.dynstr']; strings=blob[st[4]:st[4]+st[5]]
 syms=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no=struct.unpack_from('<I',blob,off)[0]; syms.append(strings[no:strings.find(b'\0',no)].decode() if no else '')
 rel=sec['.rela.plt']; plt=sec['.plt'][3]; slot=0; base=BASES[name]
 for off in range(rel[4],rel[4]+rel[5],rel[9]):
  where,info,add=struct.unpack_from('<QQq',blob,off); typ,ix=info&0xffffffff,info>>32
  if typ==0x402:
   raw=syms[ix]; dem=subprocess.check_output(['c++filt','-n',raw],text=True).strip() if raw.startswith('_Z') else raw
   o.plt[base+plt+0x20+slot*16]=dem;slot+=1
 # Verify every PLT stub's ADRP/LDR GOT reference against its exact RELA slot.
 dis=subprocess.check_output([str(OB),'-d','-j','.plt',LIB/f'libSPen{name}.so'],text=True,env=OB_ENV)
 rows={}
 for line in dis.splitlines():
  m=re.match(r'^\s*([0-9a-f]+):\s+[0-9a-f]+\s+(.+)$',line)
  if m:rows[int(m.group(1),16)]=m.group(2)
 got_by_stub={}
 for i in range(slot):
  stub=base+plt+0x20+i*16; a=stub-base
  adrp=rows.get(a,''); ldr=rows.get(a+4,''); add=rows.get(a+8,'')
  pm=re.search(r'adrp\s+x16,\s*([0-9a-f]+)',adrp)
  lm=re.search(r'ldr\s+x17,\s*\[x16(?:, #?(\d+))?\]',ldr)
  am=re.search(r'add\s+x16, x16, #0x([0-9a-f]+)',add)
  if not (pm and lm and am):raise RuntimeError(f'cannot decode {name} PLT stub {a:#x}: {adrp} / {ldr} / {add}')
  page=int(pm.group(1),16); ldr_got=page+int(lm.group(1) or 0); add_target=page+int(am.group(1),16)
  rela_off=rel[4]+i*rel[9]; expected_got=struct.unpack_from('<Q',blob,rela_off)[0]
  if ldr_got!=expected_got:
   raise RuntimeError(f'{name} PLT slot {i} GOT mismatch: ldr={ldr_got:#x} rela={expected_got:#x}')
  got_by_stub[stub]={'rela_got':expected_got,'resolver_add_target':add_target}
 targets={hex(k):{'rela_got':hex(v['rela_got']),'resolver_add_target':hex(v['resolver_add_target'])} for k,v in got_by_stub.items()}
 o.plt_audit[name]={'checked_slots':slot,'verified_adrp_ldr_got_against_rela':True,'stub_targets_sha256':hashlib.sha256(json.dumps(targets,sort_keys=True,separators=(',',':')).encode()).hexdigest()}

class Oracle(NativeFountain):
 def __init__(self):
  super().__init__(LIB,OB,('Marker4',))
  self.init_descriptors=[]; self.descriptor_ptrs=[]; self.gl=[]; self.current=None
  self.reached_imports=[]; self.plt_audit={}
  load_elf(self,'Renderer')
  map_exact_plt(self,'Renderer')
  # Replace Marker4 PLT labels with exact .rela.plt mappings.
  map_exact_plt(self,'Marker4')
  self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=self.capture_blend
  self.callbacks['SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)']=lambda:self.x(0,self.alloc(8))
  self.callbacks['glEnable']=lambda:self.gl.append(['enable',self.x(0)])
  self.callbacks['glDisable']=lambda:self.gl.append(['disable',self.x(0)])
  self.callbacks['glBlendEquationSeparate']=lambda:self.gl.append(['blend_equation',self.x(0),self.x(1)])
  self.callbacks['glBlendFuncSeparate']=lambda:self.gl.append(['blend_func',*[self.x(i) for i in range(4)]])
  self.callbacks['glColorMask']=lambda:self.gl.append(['color_mask',*[self.x(i) for i in range(4)]])
 def hook(self,u,a,size,data):
  if a in self.plt:self.reached_imports.append(self.plt[a])
  super().hook(u,a,size,data)
 def capture_blend(self):
  src=self.x(0); raw=bytes(self.u.mem_read(src,40)); self.init_descriptors.append(raw.hex())
  desc=self.alloc(40); self.u.mem_write(desc,raw); self.descriptor_ptrs.append(desc)
  self.x(0,self.alloc(128))

def execute():
 o=Oracle(); rt=o.alloc(512)
 # Init's graphics and shader cache are explicitly pre-populated. It executes
 # the actual two descriptor-construction branches, whose factories are captures.
 for off in (224,232,240,248):o.put(rt+off,'Q',o.alloc(8))
 o.call('SPen::Marker4StrokeDrawableRTV1::Init()',x=(rt,))
 if len(o.init_descriptors)!=2:raise AssertionError(f'expected two Marker4 blend descriptors, got {len(o.init_descriptors)}')
 expected_desc=['01010000000000000000000006000000070000000100000007000000000000000000000000000000','01010000040000000400000001000000010000000100000001000000000000000000000000000000']
 if o.init_descriptors!=expected_desc:raise AssertionError(f'unexpected native Marker4 blend descriptors {o.init_descriptors}')
 activations=[]
 for desc in o.descriptor_ptrs:
  blend=o.alloc(128); o.call('SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)',x=(blend,desc))
  begin=len(o.gl); o.call('SPen::GLES::BlendStateObject::Activate()',x=(blend,)); activations.append(o.gl[begin:])
 # Exercise inherited saved ARGB-to-RT conversion natively at alpha 0, 128, 255.
 data=[]
 for color in (0x00336699,0x80336699,0xff336699):
  o.call('SPen::PenDrawableRT::SetPenData(float, int)',x=(rt,color),f=(10.75,))
  data.append({'argb':color,'stored_rgba':list(o.get(rt+44,'4f')),'stored_size_u32':o.get(rt+60,'I')[0]})
 expected_gl=[[['blend_equation',32774,32774],['blend_func',770,771,1,771],['enable',3042],['color_mask',1,1,1,1]],[['blend_equation',32776,32776],['blend_func',1,1,1,1],['enable',3042],['color_mask',1,1,1,1]]]
 if activations!=expected_gl:raise AssertionError(f'unexpected Renderer blend activation calls {activations}')
 expected_rgba=[[.2,.4,.6,0.],[.2,.4,.6,128/255],[.2,.4,.6,1.]]
 for item,expected in zip(data,expected_rgba):
  if item['stored_size_u32']!=10 or any(abs(a-b)>1e-6 for a,b in zip(item['stored_rgba'],expected)):
   raise AssertionError(f'ARGB forwarding/size differs: {item}')
 hashes={name:hashlib.sha256((LIB/f'libSPen{name}.so').read_bytes()).hexdigest() for name in ('Marker4','Renderer','FountainPen','PenCommon','Base')}
 return {'schema':'sdocx-marker4-paint-native-v1','source_sha256':hashes,'native_rt_init':{'method':'Marker4StrokeDrawableRTV1::Init','actual_blend_descriptor_bytes_40':o.init_descriptors,'native_renderer_activate_gl_calls':activations,'reached_imports':sorted(set(o.reached_imports)),'plt_relocation_audit':o.plt_audit,'factory_boundary':'RendererFactory::CreateBlendStateObject is a host capture returning a bounded opaque handle; Marker4 Init and Renderer BlendStateDescriptor setters plus GLES BlendStateObject construction/Activate execute natively.'},'saved_argb_forwarding':{'method':'PenDrawableRT::SetPenData(float,int)','cases':data},'blend_semantics':{'first_descriptor_activation':'GL_FUNC_ADD with source-alpha / one-minus-source-alpha color factors and one / one-minus-source-alpha alpha factors. For opaque destination this produces sourceRGB*sourceA + destinationRGB*(1-sourceA).','second_descriptor_activation':'GL_MAX with ONE/ONE factors for color and alpha; min/max equations ignore factors, yielding componentwise max(source,destination) for submitted fragments. This is the native state used by a Marker4 pass, not a claim about its shader output or complete overlap pixels.','synthetic_overlap_example':{'dst_rgba':[0.2,0.3,0.4,0.5],'src_rgba':[0.6,0.1,0.8,0.25],'max_pass_result':[0.6,0.3,0.8,0.5]}},'host_boundaries':['RT graphics/shader cache handles are pre-populated to bypass unrelated factory/cache branches','CreateBlendStateObject capture','GL calls are recorded by explicit host callbacks','no RT Draw or GPU shader execution in this fixture'],'evidence_limit':'This fixture establishes Marker4 RTV1 native blend descriptor setup and activation commands plus inherited ARGB alpha forwarding. It does not execute RT Draw, submit actual overlapping fragments, evaluate Marker4 shader output, rasterize, or establish device GPU pixels.'}

def main():
 p=argparse.ArgumentParser(); g=p.add_mutually_exclusive_group(required=True); g.add_argument('--write-fixture',action='store_true'); g.add_argument('--check-fixture',action='store_true'); a=p.parse_args()
 out=execute(); path=ROOT/'conformance/marker4-paint-native.json'; enc=json.dumps(out,indent=2,sort_keys=True)+'\n'
 if a.write_fixture:path.write_text(enc); print('wrote',path); return
 old=json.loads(path.read_text())
 if out!=old:raise AssertionError('Marker4 paint native capture differs from fixture')
 print('Marker4 native Init and saved ARGB forwarding passed')
if __name__=='__main__':main()
