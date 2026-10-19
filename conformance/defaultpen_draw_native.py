"""Execute DefaultPen RTV1 setup/draw and Renderer blending with bounded host interfaces."""
import argparse, hashlib, json, os, re, struct, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'conformance'))
from defaultpen_saved_native import Oracle as SavedOracle, LIB
from unicorn import arm64_const as R

OB=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
RENDERER_HASH='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
DEFAULTPEN_HASH='1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f'
PENCOMMON_HASH='afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d'
BASE_HASH='e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'
FOUNTAIN_HELPER_HASH='1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'

class Oracle(SavedOracle):
 def __init__(self):
  super().__init__()
  self.renderer_gl=[]; self.descriptor_ptr=0; self.descriptor_bytes=b''; self.init_calls=[]
  self.fake={}; self.draw_calls=[]; self.draw_samples=[]
  self.u.hook_add(4,self.fake_hook) # UC_HOOK_CODE
  self.u.hook_add(4,self.renderer_trace_hook)
  self._map_renderer()
  self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=self.capture_descriptor
  for name,fn in [('glBlendEquationSeparate',self.blend_equation),('glBlendFuncSeparate',self.blend_func),('glEnable',lambda:self.renderer_gl.append(['glEnable',self.x(0)])),('glDisable',lambda:self.renderer_gl.append(['glDisable',self.x(0)])),('glColorMask',lambda:self.renderer_gl.append(['glColorMask',*[self.x(i) for i in range(4)]]))]:
   self.callbacks[name]=fn
 def _map_renderer(self):
  path=LIB/'libSPenRenderer.so'; b=path.read_bytes()
  if hashlib.sha256(b).hexdigest()!=RENDERER_HASH: raise ValueError('unverified Renderer DSO')
  base=0x4000000; off=struct.unpack_from('<Q',b,32)[0]; stride,n=struct.unpack_from('<HH',b,54)
  self.u.mem_map(base,0x200000)
  for j in range(n):
   kind,_,fo,va,_,fs,ms,_=struct.unpack_from('<II6Q',b,off+j*stride)
   if kind==1: self.u.mem_write(base+va,b[fo:fo+fs])
  for line in subprocess.check_output(['nm','-D','-C',str(path)],text=True).splitlines():
   cols=line.strip().split(' ',2)
   if len(cols)==3 and cols[1] in ('T','W'): self.syms[cols[2]]=base+int(cols[0],16)
  env=dict(os.environ); env['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
  dis=subprocess.check_output([str(OB),'-d','-C','-j','.plt',str(path)],env=env,text=True)
  for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M): self.plt[base+int(a,16)]=name
  # DefaultPen has these dynamic imports unresolved in the geometry-only loader.
  dso=LIB/'libSPenDefaultPen.so'
  for line in subprocess.check_output(['readelf','-rW',str(dso)],text=True).splitlines():
   cols=line.split()
   if len(cols)<5 or cols[2] not in ('R_AARCH64_GLOB_DAT','R_AARCH64_ABS64'): continue
   offset=int(cols[0],16); symval=int(cols[3],16); raw=cols[4]
   if symval: target=0x3000000+symval
   else:
    demangled=subprocess.check_output(['c++filt','-n',raw],text=True).strip(); target=self.syms.get(demangled,0)
    if len(cols)>6 and cols[5] in ('+','-'):
     add=int(cols[6],16); target += add if cols[5]=='+' else -add
   if target: self.put(0x3000000+offset,'Q',target)
 def _fake(self,name,fn=lambda:None):
  addr=0x200f2000+len(self.fake)*0x100; self.fake[addr]=(name,fn); return addr
 def fake_hook(self,u,a,size,data):
  if a not in self.fake: return
  name,fn=self.fake[a]; fn(); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 def renderer_trace_hook(self,u,a,size,data):
  names=('SPen::BlendStateDescriptor::BlendStateDescriptor(bool)','SPen::BlendStateDescriptor::SetColorBlendingMode(SPen::BlendingFunction, SPen::BlendingFactor, SPen::BlendingFactor)','SPen::BlendStateDescriptor::SetAlphaBlendingMode(SPen::BlendingFunction, SPen::BlendingFactor, SPen::BlendingFactor)')
  for name in names:
   if a==self.syms.get(name):
    count=1 if name.endswith('BlendStateDescriptor(bool)') else 3
    self.init_calls.append({'method':name,'args':[self.x(i) for i in range(1,count+1)]}); break
 def capture_descriptor(self):
  # Native RT::Init passes a descriptor in its stack frame. Preserve a copy in
  # oracle-owned memory because Renderer construction/activation follows Init.
  source=self.x(0); self.descriptor_bytes=bytes(self.u.mem_read(source,40))
  self.descriptor_ptr=self.alloc(40); self.u.mem_write(self.descriptor_ptr,self.descriptor_bytes)
  self.x(0,self.alloc(128))
 def blend_equation(self): self.renderer_gl.append(['glBlendEquationSeparate',self.x(0),self.x(1)])
 def blend_func(self): self.renderer_gl.append(['glBlendFuncSeparate',*[self.x(i) for i in range(4)]])
 def install_draw_interfaces(self,rt,triples):
  # Native RT::Draw executes; only SDK bitmap/program/graphics/GL vtables are bounded host facades.
  blend=self.alloc(8); bvt=self.alloc(40); self.put(blend,'Q',bvt); self.put(bvt+24,'Q',self._fake('blend.enable',lambda:self.draw_calls.append(['blend.enable'])))
  gfx=self.alloc(8); gvt=self.alloc(64); self.put(gfx,'Q',gvt)
  self.put(gvt+16,'Q',self._fake('graphics.update_buffer',lambda:self.draw_calls.append(['graphics.update_buffer',self.x(1),self.x(3)])))
  self.put(gvt+56,'Q',self._fake('graphics.draw',lambda:self.draw_calls.append(['graphics.draw',self.x(1),self.x(2),self.x(3)])))
  program=self.alloc(8); pvt=self.alloc(24); self.put(program,'Q',pvt); self.put(pvt+16,'Q',self._fake('shader.pre_draw',lambda:self.draw_calls.append(['shader.pre_draw'])))
  projection_binding=self.alloc(8); projection_vt=self.alloc(8); self.put(projection_binding,'Q',projection_vt)
  self.put(projection_vt,'Q',self._fake('shader.projection_uniform',lambda:self.draw_calls.append(['shader.projection_uniform'])))
  color_binding=self.alloc(8); color_vt=self.alloc(8); self.put(color_binding,'Q',color_vt)
  self.put(color_vt,'Q',self._fake('shader.color_uniform',lambda:self.draw_calls.append(['shader.color_uniform',list(self.get(self.x(1),'4f'))])))
  wrapper=self.alloc(24); self.put(wrapper,'QQQ',projection_binding,color_binding,program)
  # RT::Draw requires a nonempty vector record and Update derives instance count from floats/3.
  storage=self.alloc(len(triples)*12); flat=[v for t in triples for v in t]; self.put(storage,'f'*len(flat),*flat)
  vector=self.alloc(24); self.put(vector,'QQQ',storage,storage+len(flat)*4,storage+len(flat)*4)
  self.put(rt+88,'Q',vector); self.put(rt+200,'Q',gfx); self.put(rt+208,'Q',blend); self.put(rt+216,'Q',wrapper)
  return gfx
 def run_draw(self,rt,triples,bitmap,mat):
  self.install_draw_interfaces(rt,triples)
  self.draw_calls=[]
  self.call('SPen::DefaultPenStrokeDrawableRT::Update(bool)',x=(rt,1))
  self.call('SPen::DefaultPenStrokeDrawableRT::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',x=(rt,bitmap,mat))
  return {'instance_count':self.get(rt+72,'I')[0],'calls':self.draw_calls}

def embedded_shader():
 b=(LIB/'libSPenDefaultPen.so').read_bytes()
 # Translate ELF virtual addresses through the containing PT_LOAD segment.
 phoff=struct.unpack_from('<Q',b,32)[0]; phentsize,phnum=struct.unpack_from('<HH',b,54)
 def c_string(va):
  for i in range(phnum):
   p=phoff+i*phentsize; kind=struct.unpack_from('<I',b,p)[0]
   if kind!=1: continue
   fileoff,vaddr,filesz=struct.unpack_from('<QQQ',b,p+8)[0],struct.unpack_from('<Q',b,p+16)[0],struct.unpack_from('<Q',b,p+32)[0]
   if vaddr<=va<vaddr+filesz:
    start=fileoff+(va-vaddr); end=min(fileoff+filesz,start+0x1000)
    nul=b.find(b'\0',start,end)
    if nul<0: raise ValueError(f'unterminated shader string at ELF VA {va:#x}')
    return b[start:nul].decode()
  raise ValueError(f'ELF VA {va:#x} is not in a file-backed PT_LOAD segment')
 vertex=c_string(0xcd1e); fragment=c_string(0xce6f)
 return vertex,fragment

def execute():
 o=Oracle(); points=[[0.,0.],[10.,0.],[20.,0.],[30.,10.]]
 observed=o.run(points,[.25,.5,.75,1.],[10,20,30,40],[0.]*4,[0.]*4,tool=1,curve=True,width=10.)
 rt=o.rt
 # Preserve actual descriptor construction/setters while bypassing unrelated graphics/shader factories.
 o.put(rt+200,'Q',o.alloc(8)); o.put(rt+216,'Q',o.alloc(8)); o.put(rt+208,'Q',0)
 o.call('SPen::DefaultPenStrokeDrawableRT::Init()',x=(rt,))
 descriptor_hex=o.descriptor_bytes.hex(); desc=o.descriptor_ptr; blend_obj=o.alloc(128)
 o.call('SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)',x=(blend_obj,desc))
 o.call('SPen::GLES::BlendStateObject::Activate()',x=(blend_obj,))
 blend_gl=o.renderer_gl[:]
 # Use the actual RT::Update and RT::Draw against the exact saved-redraw vector twice,
 # changing only the native SetPenData ARGB alpha byte.
 mat=o.alloc(64); o.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
 bitmap=o.alloc(8); bvt=o.alloc(64); o.put(bitmap,'Q',bvt)
 o.put(bvt+40,'Q',o._fake('bitmap.activate'))
 def get_transform():
  ptr=o.x(8); o.put(ptr,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
 o.put(bvt+16,'Q',o._fake('bitmap.get_transform',get_transform)); o.put(bvt+48,'Q',o._fake('bitmap.deactivate'))
 triples=[p[:3] for p in observed['add_points']]
 alpha_runs=[]
 for color in (0x00336699,0x80336699):
  o.call('SPen::PenDrawableRT::SetPenData(float, int)',x=(rt,color),f=(10.75,))
  rgba=list(o.get(rt+44,'4f')); stored_size=o.get(rt+60,'I')[0]
  draw=o.run_draw(rt,triples,bitmap,mat)
  alpha_runs.append({'argb':color,'stored_rgba':rgba,'stored_size_u32':stored_size,'draw':draw})
 vertex,fragment=embedded_shader()
 files=('DefaultPen','PenCommon','Base','FountainPen')
 hashes={name:hashlib.sha256((LIB/f'libSPen{name}.so').read_bytes()).hexdigest() for name in files}
 hashes['Renderer']=hashlib.sha256((LIB/'libSPenRenderer.so').read_bytes()).hexdigest()
 expected={'DefaultPen':DEFAULTPEN_HASH,'PenCommon':PENCOMMON_HASH,'Base':BASE_HASH,'FountainPen':FOUNTAIN_HELPER_HASH,'Renderer':RENDERER_HASH}
 if hashes!=expected: raise ValueError(f'unverified native source DSOs: {hashes}')
 return {'schema':'sdocx-defaultpen-draw-native-v1','source_hashes':hashes,'input':{'points':points,'pressures':[.25,.5,.75,1.],'timestamps_ms':[10,20,30,40],'tool_type_raw':1,'saved_curve_field':True,'size_setting_f32':10.75},'native_saved_redraw':{'addpoint_triples':triples,'count':len(triples)},'native_rt_init':{'renderer_descriptor_hex_40':descriptor_hex,'descriptor_constructor_setter_calls':o.init_calls,'descriptor_capture_boundary':'RendererFactory::CreateBlendStateObject intercepted only to capture the descriptor emitted by native DefaultPen RT::Init and return a bounded object pointer; BlendStateDescriptor constructor and both setter methods executed from pinned Renderer.','renderer_blend_activate_gl_calls':blend_gl},'native_setdata_and_draw_by_alpha':alpha_runs,'embedded_shaders':{'symbol_addresses':{'vertex':'0xcd1e','fragment':'0xce6f'},'vertex_source':vertex,'fragment_source':fragment,'vertex_sha256':hashlib.sha256(vertex.encode()).hexdigest(),'fragment_sha256':hashlib.sha256(fragment.encode()).hexdigest()},'scope':'Pinned DefaultPen saved redraw/AddPoint, PenDrawableRT::SetPenData, DefaultPen RT::Init/Update/Draw, Renderer BlendStateDescriptor methods, GLES BlendStateObject constructor/Activate execute under Unicorn. RT::Init skips only graphics/shader cache factory branches using prefilled non-null object pointers; its actual blend descriptor path executes. RT::Draw is native, while bitmap, shader-binding, graphics-object, and GL entrypoints are explicit host facades recording arguments. Native embedded GLSL text is extracted from pinned DSO rodata but GPU shader compilation/rasterization and real device rendering are outside this oracle.'}

def main():
 ap=argparse.ArgumentParser(); g=ap.add_mutually_exclusive_group(required=True); g.add_argument('--write-fixture',action='store_true'); g.add_argument('--check-fixture',action='store_true'); args=ap.parse_args()
 actual=execute(); path=ROOT/'conformance/defaultpen-draw-native.json'; encoded=json.dumps(actual,indent=2,sort_keys=True)+'\n'
 if args.write_fixture: path.write_text(encoded); print('wrote',path); return
 expected=json.loads(path.read_text())
 if json.dumps(actual,sort_keys=True)!=json.dumps(expected,sort_keys=True): raise AssertionError('native DefaultPen draw contract differs from fixture')
 print('DefaultPen native draw oracle passed: native Init/SetPenData/Update/Draw and Renderer blend activation')
if __name__=='__main__': main()
