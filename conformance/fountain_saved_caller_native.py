#!/usr/bin/env python3
"""Replay the pinned FountainPen ObjectStroke -> Drawing caller.

Run with the project's pinned Unicorn and AArch64 toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/fountain_saved_caller_native.py

No arguments check the fixture; --write-fixture explicitly refreshes it.
V14 rows capture the native RTV4 AddPoint argument stream. V16 rows establish
selected-class and size handoff only; they do not claim V16 geometry capture.
"""
import os,sys,struct,json,hashlib,argparse
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R
ROOT=Path(__file__).resolve().parents[1]; os.chdir(ROOT)
FIXTURE=ROOT/'conformance/fountain-saved-caller-native.json'
MODE_PARSER=argparse.ArgumentParser(description='Hash-pinned FountainPen ObjectStroke -> Drawing saved-caller native oracle')
MODE=MODE_PARSER.add_mutually_exclusive_group()
MODE.add_argument('--write-fixture',action='store_true',help='explicitly write or refresh the native capture')
MODE.add_argument('--check-fixture',action='store_true',help='replay and compare the native capture')
MODE_ARGS=MODE_PARSER.parse_args()
HELPER_SHA256={'native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91','oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'}
LIBRARY_SHA256={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
for _name,_expected in HELPER_SHA256.items():
 _path=ROOT/'conformance'/_name
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned helper changed: {_name}')
for _name,_expected in LIBRARY_SHA256.items():
 _path=ROOT/'scratch/apk-analysis-native/arm64-v8a'/('libc++_shared.so' if _name=='libc++_shared' else f'libSPen{_name}.so')
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned library changed: {_name}')
sys.path[:0]=[str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
import native_drawing_runtime
rt=native_drawing_runtime.create_runtime();p=rt.p; p.dots=[]; LIBS=rt.LIBS; env=rt.env
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
queue=p.alloc(64);qvt=p.alloc(64);p.put(queue,'Q',qvt);p.put(qvt+16,'Q',0x20001000);p._runtime_queue=queue;p._task_returns=[]
p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001020,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6'))
p.put(p.context_vtable+16,'Q',0x20001040)
def taskqueue(u,a,z,d):
 if a==0x20001040:p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1);vt=p.get(task,'Q')[0];method=p.get(vt+16,'Q')[0]
  if not method:raise RuntimeError('null queued task')
  caller=u.reg_read(R.UC_ARM64_REG_LR);p._task_returns.append(caller);p.x(0,task);u.reg_write(R.UC_ARM64_REG_LR,0x20001020);u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,p._task_returns.pop())
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
# Module-loader host adapter resolves only the already-mapped, pinned FountainPen plugin.
def cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\0',1)[0].decode('ascii','replace')
loader=[]
FP_HANDLE=0x50000001; DP_HANDLE=0x90000001
def dlopen():
 name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if Path(name).name=='libSPenFountainPen.so':p.x(0,FP_HANDLE)
 elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)
 else:raise RuntimeError('unexpected DSO '+name)
def dlsym():
 handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if name!='createPenInst' or handle not in (FP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)
 plugin='libSPenFountainPen.so' if handle==FP_HANDLE else 'libSPenDefaultPen.so'
 raw=FP.read_bytes() if handle==FP_HANDLE else DP.read_bytes()
 nm=__import__('subprocess').check_output(['nm','-D',str(FP if handle==FP_HANDLE else DP)],text=True,env=env)
 offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]
 if not offsets:raise RuntimeError(plugin+' lacks createPenInst')
 p.x(0,(fpbase if handle==FP_HANDLE else dpbase)+offsets[0])
p.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)
# Native ObjectStroke and per-case actual Drawing caller probes.

def call(name,args=(),floats=(),limit=5_000_000):
 for i,value in enumerate(args):p.x(i,value)
 for i,value in enumerate(floats):p.f(i,value)
 stop=0x200f1000
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,stop)
 try:p.u.emu_start(p.syms[name],stop,count=limit)
 except Exception as e:raise RuntimeError(f'{name} failed PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x} X0-X5={[hex(p.x(i)) for i in range(6)]}') from e
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=stop:raise RuntimeError(f'{name} did not return at {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
 return p.x(0)

def string(value):
 o=p.alloc(32); call('SPen::String::String()',[o]); call('SPen::String::Construct()',[o]); b=p.alloc(len(value.encode())+1);p.u.mem_write(b,value.encode()+b'\0');call('SPen::String::Set(char const*)',[o,b]);return o

drawing=call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1_000_000)
if not drawing:raise RuntimeError('DrawingFactory::CreateDrawing returned null')
# Context returns its native profile object for device resolution.
records=[]
for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:
 loader.clear()
 drawing=call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1_000_000)
 obj=p.alloc(256);impl=p.alloc(512);call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)
 call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.FountainPen')])
 if mode=='set':call('SPen::ObjectStroke::SetPenSize(float)',[obj],[size])
 # Native selection contract V14 is exercised with the actual saved advanced setting.
 call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])
 call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]);call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1])
 pts=p.alloc(24);prs=p.alloc(12);ts=p.alloc(12);tilt=p.alloc(12);ori=p.alloc(12)
 p.put(pts,'6f',0,0,10,0,20,2);p.put(prs,'3f',.3,.6,1.);p.put(ts,'3i',100,110,120);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)
 call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])
 model_arrays={}
 for _field,_getter in [('points','SPen::ObjectStroke::GetPoint() const'),('pressure','SPen::ObjectStroke::GetPressure() const'),('timestamps','SPen::ObjectStroke::GetTimeStamp() const'),('tilt','SPen::ObjectStroke::GetTilt() const'),('orientation','SPen::ObjectStroke::GetOrientation() const')]:
  call(_getter,[obj]); model_arrays[_field]=p.x(0)
 call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)
 call('SPen::ObjectBase::SetVisibility(bool)',[obj,1])
 size_s0=None
 call('SPen::ObjectStroke::GetPenSize() const',[obj]);size_s0=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 pen_set_calls=[]; add_calls=[]; redraws=[]
 set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']
 redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']
 redraw_addrs={p.syms[name]:name for name in redraw_names}
 add_names=['SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)','SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)','SPen::FountainPenStrokeDrawableRTV6::AddPoint(SPen::Vector3<float>, float, SPen::Vector3<float>)'];add_addrs={p.syms[name]:name for name in add_names}
 def trace(u,a,z,d):
  if a==set_size_addr:pen_set_calls.append({'this':hex(p.x(0)),'input_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'return_w0':None})
  elif a==set_size_addr+0x2c and pen_set_calls:pen_set_calls[-1]['return_w0']=p.x(0)
  elif a==get_size_addr: pass
  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})
  elif a in add_addrs:
   _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(6)]
   add_calls.append({'entry':add_addrs[a],'receiver':hex(p.x(0)),'float_words_s0_s5':_words,'native_addpoint_channels':{'position_xyz_f32_bits':_words[0:3],'direction_xy_f32_bits':_words[3:5],'transparency_f32_bits':_words[5]}})
 trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)
 rc=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])
 # Capture returned PenData/current Pen and its actual generic size.
 manager=p.get(drawing+16,'Q')[0]
 # Find any native Pen::SetSize target from calls; caller object is recovered from logged receiver.
 selected_pen=int(pen_set_calls[-1]['this'],16) if pen_set_calls else None
 size_bits=None
 if selected_pen:
  call('SPen::Pen::GetSize()',[selected_pen]);size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()

 try: drawable=call('SPen::FountainPen::GetStrokeDrawableGL()',[selected_pen]) if selected_pen else 0
 except Exception as e: drawable_error=str(e); drawable=0
 else: drawable_error=None
 vptr=p.get(drawable,'Q')[0] if drawable else 0
 p.u.hook_del(trace_hook)
 records.append({'case':label,'advanced_setting':advanced_value,'style_pen_size_setter':mode,'source_model_get_pen_size_f32_bits':size_s0,'source_model_get_point_count':model_count,'drawing_return':rc,'native_pen_setsize_calls':pen_set_calls,'selected_pen':hex(selected_pen) if selected_pen else None,'native_pen_getsize_f32_bits':size_bits,'selected_drawable':hex(drawable) if drawable else None,'drawable_error':drawable_error,'drawable_vtable':hex(vptr) if vptr else None,'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,'loader_calls':list(loader),'drawing_manager':hex(manager) if manager else None,'source_arrays':{'points_f32_bits':[f'{v:08x}' for v in struct.unpack('<6I',bytes(p.u.mem_read(model_arrays['points'],24)))],'pressure_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['pressure'],12)))],'timestamps_i32':list(struct.unpack('<3i',bytes(p.u.mem_read(model_arrays['timestamps'],12)))),'tilt_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['tilt'],12)))],'orientation_f32_bits':[f'{v:08x}' for v in struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['orientation'],12))) ]}})
for _row in records:
 _versions=[f'GLV{v}' for v in (14,16) if any(f'GLV{v}::RedrawPen' in call['entry'] for call in _row['redraw_entries'])]
 if len(_versions)!=1:raise RuntimeError(f'expected one FountainPen drawable class for {_row["case"]}: {_versions}')
 _row['selected_drawable_class']=_versions[0]
 _expected_version='GLV14' if _row['advanced_setting']=='14;' else 'GLV16'
 if _versions[0]!=_expected_version:raise RuntimeError(f'wrong selected class for {_row["case"]}: {_versions[0]}')
 if _row['drawing_return']!=1:raise RuntimeError(f'Drawing rejected {_row["case"]}')
 _expected_size={'omitted':'00000000','zero':'00000000','small':'cdcccc3d','normal':'00002041'}[_row['case'].split('_',1)[1]]
 _expected_pen_size='cdcccc3e' if _expected_size in ('00000000','cdcccc3d') else '00002041'
 if _row['source_model_get_pen_size_f32_bits']!=_expected_size:raise RuntimeError(f'Model GetPenSize mismatch for {_row["case"]}')
 if _row['native_pen_getsize_f32_bits']!=_expected_pen_size:raise RuntimeError(f'Pen GetSize mismatch for {_row["case"]}')
 if len(_row['native_pen_setsize_calls'])!=2 or [call['return_w0'] for call in _row['native_pen_setsize_calls']]!=[1,1]:raise RuntimeError(f'Pen::SetSize call/return contract mismatch for {_row["case"]}')
 if _row['native_pen_setsize_calls'][0]['input_s0']!='d578693e' or _row['native_pen_setsize_calls'][1]['input_s0']!=_expected_size:raise RuntimeError(f'Pen::SetSize input mismatch for {_row["case"]}')
 if len(_row['redraw_entries'])!=1:raise RuntimeError(f'expected one selected-class RedrawPen for {_row["case"]}')
 if not any(item['fn']=='dlsym' and item['name']=='createPenInst' and item['handle']=='0x50000001' for item in _row['loader_calls']):raise RuntimeError(f'FountainPen plugin was not selected for {_row["case"]}')
 _f32word=lambda v:f'{struct.unpack("<I",struct.pack("<f",v))[0]:08x}'
 _expected_arrays={'points_f32_bits':[_f32word(v) for pair in ((0.,0.),(10.,0.),(20.,2.)) for v in pair],'pressure_f32_bits':[_f32word(v) for v in (.3,.6,1.)],'timestamps_i32':[100,110,120],'tilt_f32_bits':[_f32word(0.)]*3,'orientation_f32_bits':[_f32word(0.)]*3}
 if _row['source_model_get_point_count']!=3 or _row['source_arrays']!=_expected_arrays:raise RuntimeError(f'Model source getter arrays differ from configured inputs for {_row["case"]}')
 if _expected_version=='GLV14' and (len(_row['rtv_addpoint_entries'])!=26 or any(len(item['float_words_s0_s5'])!=6 for item in _row['rtv_addpoint_entries'])):raise RuntimeError('V14 must emit 26 complete six-float RTV4 AddPoint records')
 if _expected_version=='GLV16':
  _row['evidence_scope']='class-selection-and-size-handoff-only; V16 geometry not captured'
  if _row['rtv_addpoint_entries']:raise RuntimeError('V16 row unexpectedly contains V14 RTV4 AddPoint records')
# Source and library pins were checked before runtime setup; include the stable
# input vectors and the selected native class plus every AddPoint FP lane.
value={'schema':'sdocx-fountain-saved-caller-v1','provenance':{'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'helpers':HELPER_SHA256,'libraries':LIBRARY_SHA256,'runner_boundary':'Native Model ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, PenManager lookup, and selected FountainPen RedrawPen execute. V14 enters native RTV4 AddPoint; V16 evidence is limited to selected GLV16 class and size handoff, with no V16 geometry capture. IContext dimensions, queue dispatch, allocator, CanvasProbe drawing callbacks, and non-null synthetic dlopen/dlsym handles are host adapters; no raster output is claimed.'},'profile':[1920,1080],'source_input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen','points':[[0,0],[10,0],[20,2]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'tool_type':2,'stroke_type':0,'curve_enabled':True,'visible':True},'cases':records}
if MODE_ARGS.write_fixture:
 FIXTURE.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
 print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(records)} cases)')
else:
 if not FIXTURE.exists():raise SystemExit('fixture missing; use --write-fixture explicitly')
 if json.loads(FIXTURE.read_text())!=value:
  raise SystemExit('native Fountain Drawing fixture mismatch')
 print(f'verified {len(records)} Fountain Drawing cases')
