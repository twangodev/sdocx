#!/usr/bin/env python3
"""Capture expanded Montblanc RTV1 vectors from the saved-stroke Drawing route.

This candidate uses only the pinned helper copies beside this file. From the repository root, choose one control profile:
  PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/conformance \
  python3 tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py --case pressure --write-fixture

It records native routes and typed AddPoint arguments, not rendered pixels.
"""
import os,sys,struct,json,hashlib,argparse,subprocess
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R
ROOT=Path(__file__).resolve().parents[3]; os.chdir(ROOT)
HELPER_DIR=Path(__file__).resolve().parent/'conformance'
MODE_PARSER=argparse.ArgumentParser(description='Hash-pinned Montblanc source-control cases through Drawing')
MODE_PARSER.add_argument('--case',choices=('baseline','pressure','size','far_endpoint'),required=True)
MODE=MODE_PARSER.add_mutually_exclusive_group()
MODE.add_argument('--write-fixture',action='store_true',help='explicitly write or refresh the native capture')
MODE.add_argument('--check-fixture',action='store_true',help='replay and compare the native capture')
MODE_ARGS=MODE_PARSER.parse_args()
CASE=MODE_ARGS.case
CASE_INPUTS={
 'baseline':{'points':[[0.0,0.0],[10.0,0.0],[20.0,2.0]],'pressure':[0.3,0.6,1.0],'pen_size':2.0},
 'pressure':{'points':[[0.0,0.0],[10.0,0.0],[20.0,2.0]],'pressure':[0.3,0.85,0.9],'pen_size':2.0},
 'size':{'points':[[0.0,0.0],[10.0,0.0],[20.0,2.0]],'pressure':[0.3,0.6,1.0],'pen_size':1.5},
 'far_endpoint':{'points':[[0.0,0.0],[10.0,0.0],[30.0,2.0]],'pressure':[0.3,0.6,1.0],'pen_size':2.0},
}
CASE_INPUT=CASE_INPUTS[CASE]
FIXTURE=Path(__file__).resolve().parent/'captures'/f'{CASE}.json'
HELPER_SHA256={'native_drawing_runtime.py':'8cfdb8ca4b39518856460246ae829957b65f94afbbd002c63ec447b409a4d9f5','oblique_canvas_native.py':'30c96ccf3fbfc7a969b9b443fae2aae429af67c35ff81a15a22eeeabaa24094e','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b','airbrush_saved_native.py':'c6ec4e94b7a88d4186947f779ad78106ff382a8bde35d4d57e29f8f3dbbb10d4','airbrush_texture_native.py':'6b33ed37a316f76908821e9069a852b34e483ac0da18e6b04e3357a889e59ea3','inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae','marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8'}
LIBRARY_SHA256={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','MontblancFountainPen':'4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','AirBrushPen':'92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','Marker3':'d9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
for _name,_expected in HELPER_SHA256.items():
 _path=HELPER_DIR/_name
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned helper changed: {_name}')
for _name,_expected in LIBRARY_SHA256.items():
 _path=ROOT/'scratch/apk-analysis-native/arm64-v8a'/('libc++_shared.so' if _name=='libc++_shared' else f'libSPen{_name}.so')
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned library changed: {_name}')
RUNTIME_INPUTS=Path(__file__).resolve().parent/'runtime-inputs'
for _filename,_digest in {'libSPenBase.so':LIBRARY_SHA256['Base'],'libSPenPenCommon.so':LIBRARY_SHA256['PenCommon'],'libSPenFountainPen.so':LIBRARY_SHA256['FountainPen'],'libSPenAirBrushPen.so':LIBRARY_SHA256['AirBrushPen'],'libSPenGraphics.so':LIBRARY_SHA256['Graphics'],'libSPenRenderer.so':LIBRARY_SHA256['Renderer'],'libSPenMarker3.so':LIBRARY_SHA256['Marker3'],'libc++_shared.so':LIBRARY_SHA256['libc++_shared'],'libSPenCxx.so':LIBRARY_SHA256['libc++_shared']}.items():
 _path=RUNTIME_INPUTS/_filename
 if not _path.is_file() or hashlib.sha256(_path.read_bytes()).hexdigest()!=_digest: raise SystemExit(f'pinned isolated runtime input changed: {_filename}')
sys.path[:0]=[str(HELPER_DIR),str(ROOT/'scratch/apk-analysis-runtime/python')]
import native_drawing_runtime
import airbrush_texture_native as at, inkpen_paint_native as ip, fountain_native as fn
rt=native_drawing_runtime.create_runtime();p=rt.p; p.callbacks.pop('SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)', None); p.dots=[]; LIBS=rt.LIBS; env=rt.env
# Renderer-aware host adapter from the reviewed Marker3 paint harness. Reuse
# only its Mesa/GLES callbacks; map the pinned Renderer DSO into this existing
# Drawing runtime at a non-overlapping base and resolve imports against this
# runtime's already loaded DSOs.
import ast
_ns={'at':at,'ip':ip,'fn':fn,'ROOT':ROOT,'LIB':RUNTIME_INPUTS,'sys':sys,'os':os,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':__import__('re'),'zlib':__import__('zlib'),'C':__import__('ctypes'),'json':json,'base64':__import__('base64'),'argparse':__import__('argparse'),'Path':Path,'ARGS':__import__('argparse').Namespace(width=256,height=256,point_limit=0)}
_tree=ast.parse((HELPER_DIR/'marker3_paint_scale_native.py').read_text()); _pnode=next(x for x in _tree.body if isinstance(x,ast.ClassDef) and x.name=='P')
exec(compile(ast.Module(body=[_pnode],type_ignores=[]),'<extract Renderer GLES adapter>','exec'),_ns)
_adapter=_ns['P'](); _adapter.u=p.u; _adapter.syms.clear(); _adapter.syms.update(p.syms); _adapter.plt.clear(); _adapter.plt.update(p.plt)
_adapter.extra_bases={}; _renderer_base=0xa000000; _adapter.map_extra('Renderer',10)
def _resolve_renderer(name):
 if name in _adapter.syms:return _adapter.syms[name]
 dem=subprocess.check_output(['scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt','-n',name],text=True,env=env).strip()
 if dem in _adapter.syms:return _adapter.syms[dem]
 if name in rt.rawsyms:return rt.rawsyms[name]
 if dem in rt.rawsyms:return rt.rawsyms[dem]
 raise RuntimeError('unresolved Renderer import '+name+' / '+dem)
_adapter.resolve_symbol=_resolve_renderer
_adapter.load_extra_relocs()
p.syms.update({k:v for k,v in _adapter.syms.items() if _renderer_base<=v<_renderer_base+0x200000})
p.plt.update({k:v for k,v in _adapter.plt.items() if _renderer_base<=k<_renderer_base+0x200000})
for _key,_callback in _adapter.callbacks.items():
 if _key.startswith('gl') or _key.startswith('egl') or _key in ('strstr','strcmp','strncmp','memcmp','memchr','strlen','__strlen_chk','__strncpy_chk','__strncpy_chk2','crc32','vsnprintf','fopen','unlink','__android_log_print','__errno','getauxval','__system_property_get','SPen::RendererFactory::GetRendererDeviceCaps()','SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'):
  p.callbacks[_key]=_callback
def _strncpy():
 _dst,_src,_n=p.x(0),p.x(1),p.x(2); _raw=bytes(p.u.mem_read(_src,_n)).split(b'\0',1)[0]; p.u.mem_write(_dst,_raw+b'\0'*max(0,_n-len(_raw))); p.x(0,_dst)
p.callbacks['strncpy']=_strncpy
def _delete_textures():
 _count,_ptr=p.x(0),p.x(1); _ids=list(p.get(_ptr,f'{_count}i')) if 0<_count<256 else []
 if _ids:_adapter.gles.gl.glDeleteTextures(_count,(__import__('ctypes').c_uint*_count)(*_ids))
p.callbacks['glDeleteTextures']=_delete_textures
def _delete_samplers():
 _count,_ptr=p.x(0),p.x(1); _ids=list(p.get(_ptr,f'{_count}i')) if 0<_count<256 else []
 if _ids:_adapter.gles.gl.glDeleteSamplers(_count,(__import__('ctypes').c_uint*_count)(*_ids))
p.callbacks['glDeleteSamplers']=_delete_samplers
rt.run_init(LIBS/'libSPenRenderer.so',_renderer_base)
_vertex_attrs=[]
_add_attribute=p.syms['SPen::VertexDescriptor::addAttribute(SPen::VertexDescriptor::AttributeType, int, int, int)']
_add_attribute_return=_add_attribute+0xd8
def _trace_add_attribute(u,a,z,d):
 if a==_add_attribute:
  _vp=p.x(0); _vertex_attrs.append({'descriptor':hex(_vp),'enum':p.x(1),'size':p.x(2),'stride':p.x(3),'offset':p.x(4),'pre_count':p.get(_vp,'i')[0],'pre_bytes':bytes(u.mem_read(_vp,132)).hex()})
 elif a==_add_attribute_return and _vertex_attrs:
  _vp=int(_vertex_attrs[-1]['descriptor'],16); _vertex_attrs[-1]['post_count']=p.get(_vp,'i')[0]; _vertex_attrs[-1]['post_bytes']=bytes(u.mem_read(_vp,132)).hex()
p.u.hook_add(UC_HOOK_CODE,_trace_add_attribute)
_paint_entry_names=['SPen::FountainPenStrokeDrawableRTV5::Init()','SPen::FountainPenStrokeDrawableRTV5::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)','SPen::FountainPenStrokeDrawableRTV5::Update(bool)','SPen::FountainPenStrokeDrawableRTV5::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']
_paint_entry_addrs={p.syms[n]:n for n in _paint_entry_names}; _paint_entries=[]
def _trace_paint_entry(u,a,z,d):
 if a in _paint_entry_addrs:_paint_entries.append({'symbol':_paint_entry_addrs[a],'this':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2))})
p.u.hook_add(UC_HOOK_CODE,_trace_paint_entry)
# Explicitly initialize the mapped FountainPen plugin and its static factory data.
FP=LIBS/'libSPenFountainPen.so'; fpbase=rt.bases['FountainPen']; assert hashlib.sha256(FP.read_bytes()).hexdigest()=='1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
rt.run_init(FP,fpbase)
# Drawing's PenManager constructs its built-in DefaultPen before resolving the
# requested FountainPen. Map that pinned plugin into a free scratch region so
# the real selection path can continue.
DP=LIBS/'libSPenDefaultPen.so'; dpbase=0x9000000
assert hashlib.sha256(DP.read_bytes()).hexdigest()=='1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f'
b=DP.read_bytes(); phoff=struct.unpack_from('<Q',b,32)[0]; ent,num=struct.unpack_from('<HH',b,54); extent=0
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1:extent=max(extent,va+ms)
p.u.mem_map(dpbase,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(dpbase+va,b[off:off+fs])
  if ms>fs:p.u.mem_write(dpbase+va+fs,bytes(ms-fs))
for line in __import__('subprocess').check_output(['nm','-D','-C',str(DP)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=dpbase+int(parts[0],16)
for line in __import__('subprocess').check_output(['nm','-D',str(DP)],text=True,env=env).splitlines():
 parts=line.strip().split()
 if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(parts[2],dpbase+int(parts[0],16))
shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(dpbase+where,struct.pack('<Q',dpbase+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx]; target=dpbase+val if val else rt.rawsyms.get(name,0) or p.syms.get(name,0)
   if target:p.u.mem_write(dpbase+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
import elf_plt
for off,sym in elf_plt.resolve(DP).items():
 import subprocess
 pretty=subprocess.check_output(['c++filt','-n',sym],text=True).strip() if sym.startswith('_Z') else sym
 p.plt[dpbase+off]=pretty
rt.run_init(DP,dpbase)
# Map the distinct Montblanc plugin as its own DSO at a fresh address. Its
# factory symbol is intentionally resolved only for the Montblanc preload name.
MB=LIBS/'libSPenMontblancFountainPen.so'; mbbase=0xb000000
assert hashlib.sha256(MB.read_bytes()).hexdigest()=='4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350'
b=MB.read_bytes(); phoff=struct.unpack_from('<Q',b,32)[0]; ent,num=struct.unpack_from('<HH',b,54); extent=0
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1:extent=max(extent,va+ms)
p.u.mem_map(mbbase,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(mbbase+va,b[off:off+fs])
  if ms>fs:p.u.mem_write(mbbase+va+fs,bytes(ms-fs))
for line in subprocess.check_output(['nm','-D','-C',str(MB)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=mbbase+int(parts[0],16)
for line in subprocess.check_output(['nm','-D',str(MB)],text=True,env=env).splitlines():
 parts=line.strip().split()
 if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(parts[2],mbbase+int(parts[0],16))
shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(mbbase+where,struct.pack('<Q',mbbase+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx]; target=mbbase+val if val else rt.rawsyms.get(name,0) or p.syms.get(name,0)
   if target:p.u.mem_write(mbbase+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
for off,sym in elf_plt.resolve(MB).items():
 pretty=subprocess.check_output(['c++filt','-n',sym],text=True).strip() if sym.startswith('_Z') else sym
 p.plt[mbbase+off]=pretty
rt.run_init(MB,mbbase)
queue=p.alloc(64);qvt=p.alloc(64);p.queue_trace=[];p.put(queue,'Q',qvt);p.put(qvt+16,'Q',0x20001000);p._runtime_queue=queue;p._task_returns=[];p._task_log=[]
p.managed_queue=p.alloc(32);p.managed_queue_vtable=p.alloc(64);p.put(p.managed_queue,'Q',p.managed_queue_vtable);p.put(qvt-16,'qQ',0,p.syms.get('typeinfo for SPen::IGLManagedMsgQueue',0));p.put(qvt+56,'Q',0x20001060);p.put(p.managed_queue_vtable+16,'Q',0x20001080);p.put(p.managed_queue_vtable+24,'Q',0x200010a0)
p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001020,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001060,bytes.fromhex('1f2003d5c0035fd6'));p.u.mem_write(0x20001080,bytes.fromhex('1f2003d5c0035fd6'));p.u.mem_write(0x200010a0,bytes.fromhex('1f2003d5c0035fd6'))
p.put(p.context_vtable+16,'Q',0x20001040)
def taskqueue(u,a,z,d):
 if a==0x20001040:p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001060:p.x(0,p.managed_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a in (0x20001080,0x200010a0):p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1);vt=p.get(task,'Q')[0];method=p.get(vt+16,'Q')[0];p.queue_trace.append({'task':hex(task),'vtable':hex(vt),'run':hex(method),'task_prefix':bytes(p.u.mem_read(task,64)).hex()})
  if not method:raise RuntimeError('null queued task')
  caller=u.reg_read(R.UC_ARM64_REG_LR);p._task_returns.append(caller);p._task_log.append({'task':task,'vtable':vt,'run':method,'before':bytes(u.mem_read(task,128)).hex()});p.x(0,task);u.reg_write(R.UC_ARM64_REG_LR,0x20001020);u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:
  _row=p._task_log[-1];_row['after']=bytes(u.mem_read(_row['task'],128)).hex();p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,p._task_returns.pop())
p.u.hook_add(UC_HOOK_CODE,taskqueue)
# Match the existing native Drawing caller's explicit 1920x1080 IContext adapter.
ctx=p.alloc(32);vtable=p.alloc(256);p.put(ctx,'Q',vtable)
ctx_child=p.alloc(64);child_vt=p.alloc(128);p.put(ctx_child,'Q',child_vt)
for addr in (0x200f2300,0x200f2340,0x200f2380,0x200f23c0,0x200f23e0,0x200f2200,0x200f2400,0x200f2420,0x200f2440,0x200f2460):p.u.mem_write(addr,bytes.fromhex('c0035fd6'))
p.put(vtable+16,'Q',0x200f2300);p.put(vtable+24,'Q',0x200f23e0);p.put(vtable+80,'Q',0x200f2340);p.put(vtable+144,'Q',0x200f2200);p.put(vtable+152,'Q',0x200f2200);p.put(child_vt+32,'Q',0x200f2380);p.put(child_vt+40,'Q',0x200f23c0)
def ctxhooks(u,a,z,d):
 if a==0x200f2300:p.x(0,ctx_child)
 elif a==0x200f23e0:p.x(0,queue)
 elif a==0x200f2340:p.x(0,p.x(1))
 elif a==0x200f2380:p.x(0,1920)
 elif a==0x200f23c0:p.x(0,1080)
 elif a in (0x200f2400,0x200f2420):p.x(0,64)
 elif a==0x200f2440:p.x(0,0)
 elif a==0x200f2460:p.x(0,0)
 else:return
 u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,ctxhooks)
# Module-loader host adapter resolves only the already-mapped, pinned plugins.
def cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\0',1)[0].decode('ascii','replace')
loader=[]
FP_HANDLE=0x50000001; DP_HANDLE=0x90000001; MB_HANDLE=0xb0000001
def dlopen():
 name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if Path(name).name=='libSPenFountainPen.so':p.x(0,FP_HANDLE)
 elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)
 elif Path(name).name=='libSPenMontblancFountainPen.so':p.x(0,MB_HANDLE)
 else:raise RuntimeError('unexpected DSO '+name)
def dlsym():
 handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if name!='createPenInst' or handle not in (FP_HANDLE,DP_HANDLE,MB_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)
 plugin_path={FP_HANDLE:FP,DP_HANDLE:DP,MB_HANDLE:MB}[handle]
 plugin=plugin_path.name
 nm=__import__('subprocess').check_output(['nm','-D',str(plugin_path)],text=True,env=env)
 offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]
 if not offsets:raise RuntimeError(plugin+' lacks createPenInst')
 p.x(0,({FP_HANDLE:fpbase,DP_HANDLE:dpbase,MB_HANDLE:mbbase}[handle])+offsets[0])
p.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)
# One short, synthetic but source-backed Model/ObjectStroke -> Drawing case.
# All Montblanc RedrawPen overloads and all declared RTV1 point-entry methods
# are observed passively; the capture never calls a drawable method directly.

def call(name,args=(),floats=(),limit=5_000_000):
 for i,value in enumerate(args):p.x(i,value)
 for i,value in enumerate(floats):p.f(i,value)
 stop=0x200f1000
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,stop)
 try:p.u.emu_start(p.syms[name],stop,count=limit)
 except Exception as e:raise RuntimeError(f'{name} failed PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x} X0-X5={[hex(p.x(i)) for i in range(6)]} cause={e!r}') from e
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=stop:raise RuntimeError(f'{name} did not return at {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
 return p.x(0)

def string(value):
 o=p.alloc(32); call('SPen::String::String()',[o]); call('SPen::String::Construct()',[o]); b=p.alloc(len(value.encode())+1);p.u.mem_write(b,value.encode()+b'\0');call('SPen::String::Set(char const*)',[o,b]);return o

loader.clear()
drawing=call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1_000_000)
if not drawing:raise RuntimeError('DrawingFactory::CreateDrawing returned null')
obj=p.alloc(256);impl=p.alloc(512);call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)
pen_name='com.samsung.android.sdk.pen.pen.preload.MontblancFountainPen'
call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string(pen_name)])
advanced_value='1;'
call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])
call('SPen::ObjectStroke::SetPenSize(float)',[obj],[CASE_INPUT['pen_size']])
call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]);call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1]);call('SPen::ObjectBase::SetVisibility(bool)',[obj,1])
# Exactly three sample positions and aligned model channels.
pts=p.alloc(24);prs=p.alloc(12);ts=p.alloc(12);tilt=p.alloc(12);ori=p.alloc(12)
p.put(pts,'6f',*sum(CASE_INPUT['points'],[]));p.put(prs,'3f',*CASE_INPUT['pressure']);p.put(ts,'3i',100,110,120);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])
model_arrays={}
for field,getter in [('points','SPen::ObjectStroke::GetPoint() const'),('pressure','SPen::ObjectStroke::GetPressure() const'),('timestamps','SPen::ObjectStroke::GetTimeStamp() const'),('tilt','SPen::ObjectStroke::GetTilt() const'),('orientation','SPen::ObjectStroke::GetOrientation() const')]:
 call(getter,[obj]);model_arrays[field]=p.x(0)
call('SPen::ObjectStroke::GetPointCount() const',[obj]);model_count=p.x(0)
call('SPen::ObjectStroke::GetPenSize() const',[obj]);model_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
pen_set_calls=[];add_calls=[];redraws=[];route_events=[]
set_size_addr=p.syms['SPen::Pen::SetSize(float)']
redraw_names=[
 'SPen::MontblancFountainPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)',
 'SPen::MontblancFountainPenStrokeDrawableGL::RedrawPen(SPen::List const*, SPen::RectF*)',
]
redraw_addrs={p.syms[name]:name for name in redraw_names if name in p.syms}
add_names=[
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddStartPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
]
add_addrs={p.syms[name]:name for name in add_names}
def trace(u,a,z,d):
 if a==set_size_addr:pen_set_calls.append({'this':hex(p.x(0)),'input_s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()})
 elif a in redraw_addrs:
  receiver=p.x(0);redraws.append({'entry':redraw_addrs[a],'this':hex(receiver),'vptr':hex(p.get(receiver,'Q')[0]),'arg1':hex(p.x(1)),'arg2':hex(p.x(2))})
 elif a in add_addrs:
  words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(6)]
  recv=p.x(0)
  add_calls.append({'entry':add_addrs[a],'receiver':hex(recv),'receiver_vptr':hex(p.get(recv,'Q')[0]),
   'float_words_s0_s5':words,'typed_parameters':{'Vector3_position_xyz_f32_bits':words[0:3],
    'Vector2_direction_xy_f32_bits':words[3:5],'trailing_scalar_f32_bits':words[5]}})
watch_names=[
 'SPen::MontblancFountainPenStrokeDrawableRTV1::Update(bool)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
]
watch_addrs={p.syms[n]:n for n in watch_names if n in p.syms}
def trace_route(u,a,z,d):
 if a in watch_addrs:route_events.append({'symbol':watch_addrs[a],'this':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2))})
# Passive entry/return telemetry for source arithmetic and the exact SmPath
# values consumed by Montblanc. This observes the natural Drawing path only.
STATE_TRACE_SYMBOLS=[
 'SPen::MontblancFountainPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::endPen(SPen::MotionEvent const*, SPen::RectF*)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::redrawLine(float, float, float, float)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::drawLine(float, float, float, float)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::getCalculatedWidth(float, float, float, float)',
 'SPen::MontblancFountainPenStrokeDrawableGLV1::drawPoint(SPen::SmPoint const*, int, float)',
 'SPen::SmPath::moveTo(float, float)',
 'SPen::SmPath::quadTo(float, float, float, float)',
 'SPen::SmPath::resetPath(bool)',
 'SPen::SmPath::getLength()',
 'SPen::SmPath::getPosTan(float, SPen::SmPoint*, SPen::SmPoint*)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddStartPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
]
state_trace=[];state_pending={};state_missing=[]
state_addrs={}
for name in STATE_TRACE_SYMBOLS:
 addr=p.syms.get(name)
 # getRepeatEff is hidden in the pinned DSO's dynamic symbol table.
 if name=='SPen::MontblancFountainPenStrokeDrawableGLV1::getRepeatEff(float, float)':addr=mbbase+0x1f3ec
 if addr:state_addrs[addr]=name
 else:state_missing.append(name)
state_addrs[mbbase+0x1f3ec]='SPen::MontblancFountainPenStrokeDrawableGLV1::getRepeatEff(float, float) [pinned hidden RVA 0x1f3ec]'
state_name_by_addr=state_addrs.copy()
state_seq=0

# Passive entry/return observer for the real RTV1 body and its vector buffers.
# Only metadata is sampled per Add* call; raw vector contents are copied once,
# at the final AddEndPoint return (and once at Update if that route occurs).
RTV1_METHOD_NAMES=(
 'SPen::MontblancFountainPenStrokeDrawableRTV1::Init()',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddStartPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::Update(bool)',
 'SPen::MontblancFountainPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
)
rtv1_addrs={p.syms[n]:n for n in RTV1_METHOD_NAMES if n in p.syms}
rtv1_body_events=[];rtv1_pending_returns={};rtv1_body_hits={};rtv1_return_hits={}
rtv1_mesh_snapshots={'add_vectors':None,'upload_vectors':None}
MAX_RTV1_VECTOR_BYTES=1_048_576

def _rtv1_vector_state(receiver,field,include_data=False):
 row={'field_offset':field,'vector_object_ptr':None,'begin_ptr':None,'end_ptr':None,'capacity_ptr':None,
      'float_count':None,'capacity_floats':None,'stride5_vertex_count':None,'data_sha256':None,'f32_le_hex':None,'error':None}
 try:
  vector_obj=p.get(receiver+field,'Q')[0]
  row['vector_object_ptr']=hex(vector_obj) if vector_obj else None
  if not vector_obj:return row
  begin,end,cap=p.get(vector_obj,'3Q')
  row.update({'begin_ptr':hex(begin) if begin else None,'end_ptr':hex(end) if end else None,'capacity_ptr':hex(cap) if cap else None})
  if end<begin or cap<end or (end-begin)%4 or (cap-begin)%4:
   row['error']='invalid std::vector<float> begin/end/capacity layout';return row
  length=end-begin;capacity=cap-begin
  row['float_count']=length//4;row['capacity_floats']=capacity//4
  row['stride5_vertex_count']=(length//20) if length%20==0 else None
  if include_data and length:
   if length>MAX_RTV1_VECTOR_BYTES:row['error']='vector byte cap exceeded';return row
   raw=bytes(p.u.mem_read(begin,length))
   row['data_sha256']=hashlib.sha256(raw).hexdigest();row['f32_le_hex']=raw.hex()
  return row
 except Exception as e:
  row['error']=f'{type(e).__name__}: {e}'
  return row

def _rtv1_vector_set(receiver,fields,include_data=False):
 rows=[_rtv1_vector_state(receiver,field,include_data) for field in fields]
 total=sum((row.get('float_count') or 0)*4 for row in rows)
 if total>MAX_RTV1_VECTOR_BYTES:
  return {'fields':[{**row,'f32_le_hex':None,'error':row.get('error') or 'combined vector byte cap exceeded'} for row in rows],
          'combined_data_bytes':total,'combined_cap_exceeded':True}
 return {'fields':rows,'combined_data_bytes':total,'combined_cap_exceeded':False}

def trace_rtv1_body(u,a,z,d):
 # Observe a completed native method at its actual caller return PC.
 waiting=rtv1_pending_returns.get(a)
 if waiting:
  event_index=waiting.pop(0)
  if not waiting:rtv1_pending_returns.pop(a,None)
  event=rtv1_body_events[event_index]
  event['returned']=True;rtv1_return_hits[event['method']]=rtv1_return_hits.get(event['method'],0)+1
  receiver=int(event['this'],16)
  if event['method'].endswith('AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)') and rtv1_mesh_snapshots['add_vectors'] is None:
   rtv1_mesh_snapshots['add_vectors']={'captured_after':'final AddEndPoint body return','this':event['this'],
    'vectors':_rtv1_vector_set(receiver,(200,208,216),True)}
  if event['method'].endswith('Update(bool)'):
   rtv1_mesh_snapshots['upload_vectors']={'captured_at':'Update body return','this':event['this'],
    'vectors':_rtv1_vector_set(receiver,(224,232,240),True)}
 method=rtv1_addrs.get(a)
 if method is None:return
 rtv1_body_hits[method]=rtv1_body_hits.get(method,0)+1
 receiver=p.x(0);lr=u.reg_read(R.UC_ARM64_REG_LR)
 event={'method':method,'entry_pc':hex(a),'entry_word_le':bytes(u.mem_read(a,4)).hex(),
  'this':hex(receiver),'lr':hex(lr),'callback_name_registered':method in p.callbacks,
  'x0_x3':[hex(p.x(i)) for i in range(4)],
  'vector_fields_before':None,'returned':False}
 if method.endswith('AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)') or method.endswith('AddStartPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)') or method.endswith('AddEndPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)'):
  event['vector_fields_before']=_rtv1_vector_set(receiver,(200,208,216),False)
 elif method.endswith('Update(bool)'):
  event['vector_fields_before']=_rtv1_vector_set(receiver,(224,232,240),False)
 elif method.endswith('SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'):
  event['buffer_arguments']=[hex(p.x(i)) for i in (1,2,3)]
 elif method.endswith('SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'):
  event['buffer_arguments']=[hex(p.x(i)) for i in (1,2,3)]
 event_index=len(rtv1_body_events);rtv1_body_events.append(event)
 if lr:rtv1_pending_returns.setdefault(lr,[]).append(event_index)

def _safe_mem(addr,size):
 try:return bytes(p.u.mem_read(addr,size)).hex() if addr else None
 except Exception as e:return {'read_error':type(e).__name__,'address':hex(addr) if addr else None,'size':size}

def _fwords(u,count=6):
 return [struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(count)]

def trace_state(u,a,z,d):
 global state_seq
 # The caller PC is the return site. Log ABI registers before entering a
 # source method and its return values/memory after the method returns.
 waiting=state_pending.get(a)
 if waiting:
  frame=waiting.pop()
  if not waiting:state_pending.pop(a,None)
  row=state_trace[frame]
  row['return']={'pc':hex(a),'x0_x3':[hex(p.x(i)) for i in range(4)],'s0_s5_f32_bits':_fwords(u),
                 'point_out_bytes':_safe_mem(row.get('_point_out',0),8),
                 'tangent_out_bytes':_safe_mem(row.get('_tangent_out',0),8),
                 'path_after_bytes':_safe_mem(row.get('_path',0),160),
                 'receiver_after_bytes':_safe_mem(row.get('_receiver',0),320)}
  for key in ('_point_out','_tangent_out','_path','_receiver'):row.pop(key,None)
 if a not in state_name_by_addr:return
 name=state_name_by_addr[a];state_seq+=1
 row={'id':state_seq,'symbol':name,'entry_pc':hex(a),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),
      'x0_x3':[hex(p.x(i)) for i in range(4)],'s0_s5_f32_bits':_fwords(u),
      'x0_bytes':_safe_mem(p.x(0),160)}
 if 'getPosTan' in name:
  row.update({'distance_s0_f32_bits':_fwords(u,1)[0], 'point_out_ptr':hex(p.x(1)),
              'tangent_out_ptr':hex(p.x(2)),'_path':p.x(0),'_point_out':p.x(1),'_tangent_out':p.x(2)})
 elif 'getLength' in name or 'moveTo' in name or 'quadTo' in name or 'resetPath' in name:
  row['_path']=p.x(0)
 elif 'RedrawPen(SPen::MotionEvent' in name:
  row.update({'motion_event_ptr':hex(p.x(1)),'motion_event_bytes':_safe_mem(p.x(1),160)})
 elif 'drawPoint' in name:
  row.update({'point_ptr':hex(p.x(1)),'point_bytes':_safe_mem(p.x(1),16),'point_index_x2':p.x(2),
              'draw_point_state_bytes':_safe_mem(p.x(0),320),'_receiver':p.x(0)})
 elif 'redrawLine' in name or 'drawLine' in name or 'getCalculatedWidth' in name:
  row.update({'drawable_state_bytes':_safe_mem(p.x(0),320),'_receiver':p.x(0)})
 elif 'Add' in name:
  row.update({'rt_this':hex(p.x(0)),'rt_receiver_bytes':_safe_mem(p.x(0),64),'_receiver':p.x(0)})
 lr=u.reg_read(R.UC_ARM64_REG_LR)
 if lr:
  state_pending.setdefault(lr,[]).append(len(state_trace))
 state_trace.append(row)


trace_hook=p.u.hook_add(UC_HOOK_CODE,trace);route_hook=p.u.hook_add(UC_HOOK_CODE,trace_route);state_hook=p.u.hook_add(UC_HOOK_CODE,trace_state);rtv1_body_hook=p.u.hook_add(UC_HOOK_CODE,trace_rtv1_body)
# V3 passive source-state observer. All addresses below are RVAs in the exact
# pinned Montblanc DSO. It observes instructions during the existing Drawing
# call only; it does not alter registers or memory.
ALPHA_SOURCE_RVAS={
 0x1d278:'GLV1 constructor stores default member +96 (170)',
 0x1d76c:'startPen stores default member +92 (170)',
 0x1d7d4:'endPen entry',
 0x1e93c:'redrawLine entry',
 0x1ea64:'redrawLine prepares pressure half-scale',
 0x1ea68:'redrawLine loads member +96',
 0x1ea74:'redrawLine converts member +96 integer to float',
 0x1ea78:'redrawLine multiplies input pressure by half-scale',
 0x1ea7c:'redrawLine multiplies pressure term by member +96',
 0x1ea80:'redrawLine converts pressure term to integer',
 0x1ea84:'redrawLine adds pressure term to member value',
 0x1ea8c:'redrawLine converts updated integer to float',
 0x1ea88:'redrawLine loads drawable PenGLDataManager pointer',
 0x1ea90:'redrawLine loads PenGLData pointer from manager',
 0x1ea98:'redrawLine loads full width from PenGLData[0]',
 0x1eaa0:'redrawLine sets size-level marker',
 0x1eaa4:'redrawLine converts computed alpha accumulator to integer',
 0x1eaa8:'redrawLine stores PenGLData width at drawable +104',
 0x1eaac:'redrawLine stores computed alpha index at drawable +92',
 0x1dad4:'endPen first-point drawPoint call',
 0x1dadc:'endPen chooses path-generation branch',
 0x1dae0:'endPen rewinds SmPath',
 0x1daf0:'endPen chooses cubic or quadratic source path',
 0x1db2c:'endPen quadratic path branch',
 0x1db3c:'endPen appends quadratic path segment',
 0x1db40:'endPen resets path accumulation',
 0x1db4c:'endPen begins width/alpha calculation',
 0x1db60:'endPen loads pen data size',
 0x1db68:'endPen computes width base',
 0x1db74:'endPen computes pressure/width blend',
 0x1db84:'endPen adds width terms',
 0x1db88:'endPen loads prior width +104',
 0x1db8c:'endPen computes half-width candidate',
 0x1db90:'endPen compares prior and candidate width',
 0x1dba4:'endPen selects candidate width',
 0x1dbac:'endPen selects width relative to pressure target',
 0x1dbb4:'endPen applies five-unit width floor',
 0x1dbc4:'endPen reads source path length',
 0x1dbd0:'endPen computes native repeat count',
 0x1dbd4:'endPen loads member +96 alpha seed',
 0x1dbd8:'endPen computes source pressure term',
 0x1dbdc:'endPen loads member +92 alpha state',
 0x1dbe0:'endPen loads prior width +104',
 0x1dbe8:'endPen converts member +96 integer to float',
 0x1dbec:'endPen compares prior width to candidate',
 0x1dbf0:'endPen fused alpha seed/pressure update',
 0x1dbf4:'endPen computes alpha shaping term',
 0x1dbf8:'endPen truncates candidate alpha seed',
 0x1dbfc:'endPen computes alpha shaping factor',
 0x1dc00:'endPen converts truncated seed to float',
 0x1dc04:'endPen multiplies shaping factor and seed',
 0x1dc08:'endPen converts member +92 to float',
 0x1dc0c:'endPen converts candidate alpha to integer',
 0x1dc10:'endPen converts candidate alpha back to float',
 0x1dc14:'endPen compares candidate/current alpha distance',
 0x1dc18:'endPen converts alpha distance to integer',
 0x1dc20:'endPen compares alpha change to threshold',
 0x1dc28:'endPen chooses bounded alpha step direction',
 0x1dc3c:'endPen adds bounded alpha step to member +92',
 0x1dc40:'endPen compares alpha to 60 clamp',
 0x1dc4c:'endPen converts direct candidate alpha',
 0x1dc50:'endPen applies lower alpha clamp',
 0x1dc60:'endPen applies upper alpha clamp',
 0x1dc64:'endPen selects clamped alpha',
 0x1dc68:'endPen checks optional alpha shaping branch',
 0x1dc74:'endPen applies dynamic alpha shaping factor',
 0x1dc80:'endPen converts shaped alpha to integer',
 0x1dc88:'endPen stores updated alpha at member +92',
 0x1dc8c:'endPen compares repeat count',
 0x1dc98:'endPen converts repeat count for interpolation',
 0x1dc9c:'endPen computes alpha interval',
 0x1dca0:'endPen converts start alpha to float',
 0x1dcb4:'endPen computes width step',
 0x1dcbc:'endPen computes alpha step',
 0x1dcc0:'endPen computes path-distance step',
 0x1dcc8:'endPen sampler loop entry',
 0x1dcd0:'endPen marks final endpoint iteration',
 0x1dcd4:'endPen prepares sampled path distance',
 0x1dce4:'endPen evaluates SmPath position and tangent',
 0x1dcec:'endPen converts sample index to float',
 0x1dcf8:'endPen interpolates sample alpha/index',
 0x1dcfc:'endPen FCVTZS input for drawPoint W2',
 0x1dd00:'endPen has computed W2 and calculates half-width',
 0x1dd04:'endPen drawPoint call ABI',
 0x1dd08:'endPen increments width and path distance',
 0x1dd10:'endPen increments sample index',
}
REDRAW_LOOP_RVAS={0x1ec40,0x1ec64,0x1ec68,0x1ec6c,0x1ec70,0x1ec74,0x1ec78,0x1ec7c}
ENDPEN_LOOP_RVAS={0x1dcc8,0x1dcd0,0x1dcd4,0x1dce4,0x1dcec,0x1dcf8,0x1dcfc,0x1dd00,0x1dd04,0x1dd08,0x1dd10}
ALPHA_SOURCE_LOOP_RVAS=REDRAW_LOOP_RVAS|ENDPEN_LOOP_RVAS
alpha_source_addrs={mbbase+rva:(rva,label) for rva,label in ALPHA_SOURCE_RVAS.items()}
alpha_source_rows=[];alpha_source_hits={};ALPHA_SOURCE_ROW_LIMIT=192

def trace_alpha_source(u,a,z,d):
 item=alpha_source_addrs.get(a)
 if item is None:return
 rva,label=item
 hit=alpha_source_hits.get(rva,0);alpha_source_hits[rva]=hit+1
 if rva in REDRAW_LOOP_RVAS:
  iteration=p.x(23);repeat=p.x(20);loop_kind='redrawLine';loop_index_register='x23';loop_count_register='x20'
 elif rva in ENDPEN_LOOP_RVAS:
  iteration=p.x(26)
  raw_count=p.x(22)&0xffffffff
  signed_count=raw_count-(1<<32) if raw_count&(1<<31) else raw_count
  repeat=-signed_count
  loop_kind='endPen';loop_index_register='x26';loop_count_register='-x22(s32)'
 else:
  iteration=None;repeat=None;loop_kind=None;loop_index_register=None;loop_count_register=None
 if loop_kind:
  # Keep the first, second, and last native loop iterations. Hit counts retain
  # the complete visit count at each PC.
  keep=(iteration in (0,1) or (repeat>0 and iteration==repeat-1)) if loop_kind=='redrawLine' else (iteration in (1,2) or (repeat>0 and iteration in (repeat-1,repeat)))
 else:
  keep=hit<4
 if not keep:return
 if len(alpha_source_rows)>=ALPHA_SOURCE_ROW_LIMIT:
  raise RuntimeError('bounded alpha source observer row limit exceeded')
 if rva in (0x1e93c,0x1d7d4):receiver=p.x(0)
 elif rva==0x1d278:receiver=p.x(19)
 elif rva==0x1d76c:receiver=p.x(20)
 elif 0x1dad4<=rva<=0x1dde0:receiver=p.x(20)
 else:receiver=p.x(19)
 member_bytes=_safe_mem(receiver+80,48)
 xs={f'x{i}':hex(p.x(i)) for i in range(24)}
 ss={f's{i}_f32_bits':struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(16)}
 row={'rva':hex(rva),'symbol':label,'this':hex(receiver),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),
      'x_registers':xs,'s_registers':ss,'drawable_member_bytes_offset_80_len_48':member_bytes,
      'visit_index_for_pc':hit,'loop_kind':loop_kind,'loop_iteration':iteration,'loop_index_register':loop_index_register,'loop_repeat':repeat,'loop_count_register':loop_count_register}
 if rva in (0x1ea88,0x1ea90,0x1ea98,0x1eaa8,0x1eaac):
  pen_data=p.get(receiver+72,'Q')[0]
  gl_data=p.get(pen_data,'Q')[0] if pen_data else 0
  row['drawable_pen_data_width_source_chain']={
   'drawable_plus72_pen_data_ptr':hex(pen_data) if pen_data else None,
   'pen_data_first_gl_data_ptr':hex(gl_data) if gl_data else None,
   'gl_data_first_f32_bits':bytes(p.u.mem_read(gl_data,4)).hex() if gl_data else None,
   'gl_data_first_f32_value':struct.unpack('<f',bytes(p.u.mem_read(gl_data,4)))[0] if gl_data else None,
  }
 alpha_source_rows.append(row)

alpha_source_hook=p.u.hook_add(UC_HOOK_CODE,trace_alpha_source)
rc=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])
p.u.hook_del(alpha_source_hook)
manager=p.get(drawing+16,'Q')[0]
selected_pen=int(pen_set_calls[-1]['this'],16) if pen_set_calls else None
pen_size_bits=None
if selected_pen:
 call('SPen::Pen::GetSize()',[selected_pen]);pen_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
# Query selected pen getter only after the actual Drawing caller; do not force a
# Redraw overload. This extra getter gives a class/vtable cross-check.
drawable=0;drawable_error=None
if selected_pen:
 try:drawable=call('SPen::MontblancFountainPen::GetStrokeDrawableGL()',[selected_pen])
 except Exception as e:drawable_error=f'{type(e).__name__}: {e}'
primary_vptr=p.get(drawable,'Q')[0] if drawable else 0
secondary_vptr=p.get(drawable+8,'Q')[0] if drawable else 0
p.u.hook_del(trace_hook);p.u.hook_del(route_hook);p.u.hook_del(state_hook);p.u.hook_del(rtv1_body_hook)
source_arrays={'points_f32_bits':[f'{v:08x}' for v in struct.unpack('<6I',bytes(p.u.mem_read(model_arrays['points'],24)))],
 'pressure_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['pressure'],12)))],
 'timestamps_i32':list(struct.unpack('<3i',bytes(p.u.mem_read(model_arrays['timestamps'],12)))),
 'tilt_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['tilt'],12)))],
 'orientation_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['orientation'],12)))]}
mb_factory_events=[row for row in loader if row.get('fn')=='dlsym' and row.get('name')=='createPenInst' and row.get('handle')==hex(MB_HANDLE)]
if not mb_factory_events:raise RuntimeError('Drawing did not select the separately mapped Montblanc factory for the exact preload name')
expected_vptrs=[hex(mbbase+0x360d0),hex(mbbase+0x36180)]
observed_vptrs=[hex(primary_vptr),hex(secondary_vptr)] if drawable else None
records=[{'case':f'montblanc_preload_saved_stroke_{CASE}','control_case':CASE,'stored_pen_name':pen_name,'advanced_setting':'1;',
 'tool_type_enum':2,'stroke_type_enum':0,'curve_enabled':True,'visible':True,
 'synthetic_model_input':{'points':CASE_INPUT['points'],'pressure':CASE_INPUT['pressure'],'pen_size':CASE_INPUT['pen_size'],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0]},
 'source_model_get_point_count':model_count,'source_model_get_pen_size_f32_bits':model_size_bits,'source_model_getter_arrays':source_arrays,
 'drawing_return':rc,'drawing_manager':hex(manager) if manager else None,'native_pen_setsize_calls':pen_set_calls,
 'selected_pen':hex(selected_pen) if selected_pen else None,'native_pen_getsize_f32_bits':pen_size_bits,
 'actual_drawing_redraw_entries':redraws,'actual_montblanc_rtv1_addpoint_records':add_calls,'rtv1_native_body_events':rtv1_body_events,'rtv1_native_body_entry_counts':rtv1_body_hits,'rtv1_native_body_return_counts':rtv1_return_hits,'rtv1_final_vectors':rtv1_mesh_snapshots,'source_arithmetic_entry_return_trace':state_trace,'source_arithmetic_symbols_missing':state_missing,'source_alpha_member_register_pc_trace':alpha_source_rows,'source_alpha_observer_total_pc_hits':{hex(k):v for k,v in sorted(alpha_source_hits.items())},
 'actual_rtv_update_draw_setbuffer_entries':route_events,'post_drawing_selected_drawable_getter_error':drawable_error,
 'post_drawing_selected_drawable':hex(drawable) if drawable else None,
 'post_drawing_drawable_vptrs':observed_vptrs,'post_drawing_source_expected_vptrs':expected_vptrs,'post_drawing_vptr_pair_matches_pinned_GLV1':observed_vptrs==expected_vptrs if observed_vptrs else None,
 'loader_calls':list(loader),'boundary':'one synthetic three-point Model/ObjectStroke source-control case via Drawing; actual dynamic plugin and redraw/AddPoint routes are observed; no direct RedrawPen/AddPoint invocation and no pixels'}]
value={'schema':'sdocx-montblanc-rtv1-mesh-capture.v1','control_case':CASE,
 'provenance':{'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'helpers':HELPER_SHA256,'libraries':LIBRARY_SHA256,
  'runner_boundary':'Native ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, plugin lookup, selected class RedrawPen routes, Montblanc RTV1 Add* methods and passive pinned-instruction/vector probes execute. DSO/ABI hashes are pinned. IContext/queue/allocator/GLES and loader are host adapters. The selected source-control points/pressure/size are synthetic inputs; output is actual native-call evidence, not raster appearance.'},
 'profile':[1920,1080],'source_input':records[0]['synthetic_model_input'],'cases':records}
if MODE_ARGS.write_fixture:
 FIXTURE.parent.mkdir(parents=True,exist_ok=True);FIXTURE.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n');print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(records)} cases)')
else:
 if not FIXTURE.exists():raise SystemExit('fixture missing; use --write-fixture explicitly')
 if json.loads(FIXTURE.read_text())!=value:raise SystemExit('native Montblanc Drawing fixture mismatch')
 print(f'verified {len(records)} Montblanc Drawing case(s)')
