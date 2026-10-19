import pathlib, struct, subprocess, os, hashlib, json, argparse, collections
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
parser=argparse.ArgumentParser(description='Pinned native ObjectDrawing -> Calligraphy saved-stroke caller capture')
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--print-capture',action='store_true')
cli_args=parser.parse_args()
FIXTURE=ROOT/'conformance/calligraphy-saved-caller-native.json'
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; env=dict(os.environ); env['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
import native_drawing_runtime
runtime=native_drawing_runtime.create_runtime(); p=runtime.p; LIBS=runtime.LIBS; env=runtime.env
queue=p.alloc(64); p._runtime_queue=queue
# These CanvasProbe shortcuts are replaced by real Model methods once the
# Model DSO is mapped below. A caller capture must not inherit false results.
p.callbacks.pop('SPen::ObjectBase::GetType() const',None)
p.callbacks.pop('SPen::ObjectBase::IsVisible() const',None)
for callback_name in tuple(p.callbacks):
 if callback_name.startswith('SPen::MotionEvent::'):
  p.callbacks.pop(callback_name)
if any(name.startswith('SPen::MotionEvent::') for name in p.callbacks):
 raise RuntimeError('caller setup retained a synthetic MotionEvent callback')
path=LIBS/'libSPenMontblancCalligraphyPen.so'; base=0x9000000; raw=path.read_bytes(); assert hashlib.sha256(raw).hexdigest()=='75aa5312fba02e21ff01d713187d2fd8d015ac4781e2d58da85ae700114e6f42'
phoff=struct.unpack_from('<Q',raw,32)[0]; ent,num=struct.unpack_from('<HH',raw,54); extent=max(va+ms for typ,fl,off,va,pa,fs,ms,al in [struct.unpack_from('<II6Q',raw,phoff+i*ent) for i in range(num)] if typ==1); p.u.mem_map(base,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(base+va,raw[off:off+fs]);
  if ms>fs:p.u.mem_write(base+va+fs,bytes(ms-fs))
rawnames=dict(runtime.rawsyms)
for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
 parts=line.strip().split()
 if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rawnames[parts[2]]=base+int(parts[0],16)
for line in subprocess.check_output(['nm','-D','-C',str(path)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=base+int(parts[0],16)
from conformance.elf_plt import resolve as resolve_plt
for off,symbol in resolve_plt(path).items():
 pretty=subprocess.check_output(['c++filt','-n',symbol],text=True).strip();p.plt[base+off]=pretty
shoff=struct.unpack_from('<Q',raw,40)[0]; se,sn,ss=struct.unpack_from('<HHH',raw,58); secs=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*se) for i in range(sn)]; names=raw[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',raw,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for snm in ('.rela.dyn','.rela.plt'):
 sec=by.get(snm)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',raw,off);typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx];target=base+val if val else rawnames.get(name,0)
   if not target and typ!=0x402:raise RuntimeError(f'unresolved direct Calligraphy relocation {where:#x}: {name} type {typ:#x}')
   if not target:continue  # JUMP_SLOT is intercepted at its byte-decoded PLT veneer.
   p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
  else: raise RuntimeError(f'unsupported Calligraphy relocation {where:#x}, type {typ:#x}')
# init arrays for native plugin
sec=by['.init_array']
for i in range(0,sec[5],8):
 target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+i,8))[0]
 if target:
  for j in range(4):p.x(j,0)
  p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000); p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000);p.u.emu_start(target,0x200f1000,count=3000000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError(f'Calligraphy init failed at {target:#x}: PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
def lse(u,a,size,data):
 if a in (0x104caf0,0x104c400):
  address=u.reg_read(R.UC_ARM64_REG_X1); old=struct.unpack('<Q',u.mem_read(address,8))[0]; value=u.reg_read(R.UC_ARM64_REG_X0);u.mem_write(address,struct.pack('<Q',(old+value)&((1<<64)-1)));u.reg_write(R.UC_ARM64_REG_X0,old);u.reg_write(R.UC_ARM64_REG_PC,a+4)
p.u.hook_add(UC_HOOK_CODE,lse)
# Context host with empty display; ObjectDrawing ctor calls virtual display getters.
ctx=p.alloc(32); vtable=p.alloc(256); p.put(ctx,'Q',vtable)
p.u.mem_write(0x200f2200,bytes.fromhex('c0035fd6'))
p.put(vtable+144,'Q',0x200f2200); p.put(vtable+152,'Q',0x200f2200)
ctx_child,ctx_child_vt=p.alloc(64),p.alloc(128);p.put(ctx_child,'Q',ctx_child_vt)
ctx_get=0x200f2300;ctx_color=0x200f2340;ctx_child32=0x200f2380;ctx_child40=0x200f23c0
p.put(vtable+16,'Q',ctx_get);p.put(vtable+80,'Q',ctx_color);p.put(ctx_child_vt+32,'Q',ctx_child32);p.put(ctx_child_vt+40,'Q',ctx_child40)
queue_vtable=p.alloc(64); p.put(queue,'Q',queue_vtable); p.put(queue_vtable+16,'Q',0x20001000)
p._task_returns=[]
p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6'))
p.put(p.context_vtable+16,'Q',0x20001040)
def caller_context_hooks(u,a,size,data):
 if a==ctx_get: p.x(0,ctx_child)
 elif a==ctx_color: p.x(0,p.x(1))
 elif a==ctx_child32: p.x(0,1920)
 elif a==ctx_child40: p.x(0,1080)
 else:return
 u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,caller_context_hooks)
def taskqueue(u,a,z,d):
 if a==0x20001040:
  p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1);vt=struct.unpack('<Q',u.mem_read(task,8))[0];method=struct.unpack('<Q',u.mem_read(vt+16,8))[0]
  if not method:raise RuntimeError('queued native task has null Run method')
  caller=u.reg_read(R.UC_ARM64_REG_LR);p._task_returns.append(caller);p.x(0,task);u.reg_write(R.UC_ARM64_REG_LR,0x20001020);u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:
  p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,p._task_returns.pop())
p.u.hook_add(UC_HOOK_CODE,taskqueue)
try:
 p.call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1000000);d=p.x(0)
 obj=p.alloc(256); impl=p.alloc(512);p.call('SPen::ObjectStroke::ObjectStroke()',[obj]);p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)
 pname=p.alloc(32);p.call('SPen::String::String()',[pname]);name=p.alloc(80);rawname=b'com.samsung.android.sdk.pen.pen.preload.MontblancCalligraphyPen\0';p.u.mem_write(name,rawname);p.call('SPen::String::Construct(char const*)',[pname,name]);p.call('SPen::String::Set(char const*)',[pname,name]);p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);p.call('SPen::ObjectStroke::GetPenName() const',[obj])
 p.f(0,6.0);p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]);p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]);p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0]);p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,1]);p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,0x80112233])
 pts=p.alloc(24);prs=p.alloc(12);ts=p.alloc(12);tilt=p.alloc(12);ori=p.alloc(12);p.put(pts,'6f',0,0,4,1,9,5);p.put(prs,'3f',.3,.6,.8);p.put(ts,'3i',100,115,130);p.put(tilt,'3f',.1,.2,.3);p.put(ori,'3f',0,.35,.7);p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])
 model_channels={}
 for label,method,element_size in [('points','SPen::ObjectStroke::GetPoint() const',8),('pressure','SPen::ObjectStroke::GetPressure() const',4),('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),('tilt','SPen::ObjectStroke::GetTilt() const',4),('orientation','SPen::ObjectStroke::GetOrientation() const',4)]:
  p.call(method,[obj]);data=p.x(0);length=3*element_size
  model_channels[label]={'getter':method,'data_pointer':hex(data),'count':3,'element_size':element_size,'bytes':bytes(p.u.mem_read(data,length)).hex()}
 for label,method in [('pen_size','SPen::ObjectStroke::GetPenSize() const'),('color','SPen::ObjectStroke::GetColor() const'),('tool_type','SPen::ObjectStroke::GetToolType() const'),('stroke_type','SPen::ObjectStroke::GetStrokeType() const'),('point_count','SPen::ObjectStroke::GetPointCount() const')]:
  p.call(method,[obj]);model_channels[label]=p.x(0) if label not in ('pen_size',) else struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj])
 # dlopen/dlsym are host module-loader calls: the matching pinned DSO is already mapped and hash-verified.
 def dlopen_host():
  pathptr=p.x(0); pathstr=bytes(p.u.mem_read(pathptr,128)).split(b"\0",1)[0].decode(errors="replace")
  if pathlib.Path(pathstr).name!='libSPenMontblancCalligraphyPen.so':raise RuntimeError(f'unexpected native plugin request: {pathstr!r}')
  p.x(0,base)
 def dlsym_host():
  symptr=p.x(1); sym=bytes(p.u.mem_read(symptr,128)).split(b"\0",1)[0].decode(errors="replace")
  if sym!='createPenInst':raise RuntimeError(f'unexpected native plugin symbol request: {sym!r}')
  p.x(0,p.syms[sym])
 p.callbacks["dlopen"]=dlopen_host;p.callbacks["dlsym"]=dlsym_host
 p.callbacks['malloc']=lambda:p.x(0,p.alloc(max(1,p.x(0))))
 p.callbacks['free']=lambda:None

except:
 raise
from unicorn import UC_HOOK_CODE
trace_names=['SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)','SPen::PenManager::GetPenData(SPen::String*)','SPen::PenManager::GetPenData(SPen::String const*)','SPen::Pen::SetScreenResolution(int, int)','SPen::MontblancCalligraphyPen::GetStrokeDrawableGL()','SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)','SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Update(bool)','SPen::ObjectStroke::GetPenName() const','SPen::ObjectStroke::GetPointCount() const']
addrs={p.syms[nm]:nm for nm in trace_names if nm in p.syms}
seen=[]
caller_records={'add_points':[],'sample_ends':[],'build_calls':[],'drawable':None,'event':None}
screen_resolution_calls=[]
screen_resolution_addr=p.syms['SPen::Pen::SetScreenResolution(int, int)']
pen_data_addr=p.syms.get('SPen::PenManager::GetPenData(SPen::String*)',0);pending_pen_data={};pen_data_returns=[]
event_getters={n:p.syms[n] for n in ('SPen::MotionEvent::GetHistorySize() const','SPen::MotionEvent::GetPointerCount() const','SPen::MotionEvent::GetEventTime() const','SPen::MotionEvent::GetX(int) const','SPen::MotionEvent::GetY(int) const','SPen::MotionEvent::GetHistoricalX(int, int) const','SPen::MotionEvent::GetHistoricalY(int, int) const','SPen::MotionEvent::GetHistoricalPressure(int) const','SPen::MotionEvent::GetPressure(int) const') if n in p.syms};pending_event_getters={};event_getter_calls=[]
redraw_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)']
add_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::AddPoint(SPen::Vector4<float>, float)']
build_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::buildMesh(SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Rect const&, std::__ndk1::vector<SPen::Vector4<float>, std::__ndk1::allocator<SPen::Vector4<float> > >*)']
def tr(u,a,z,data):
 if a in pending_pen_data:
  rec=pending_pen_data.pop(a);rec['return']=hex(p.x(0));pen_data_returns.append(rec)
 if a in pending_event_getters:
  rec=pending_event_getters.pop(a);rec['result_x0']=hex(p.x(0));rec['result_s0']=struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex();rec['result_d0']=struct.pack('<Q',u.reg_read(R.UC_ARM64_REG_D0)).hex();event_getter_calls.append(rec)
 if a in event_getters.values():
  caller=u.reg_read(R.UC_ARM64_REG_LR);name=next(n for n,v in event_getters.items() if v==a);rec={'method':name,'args':[hex(p.x(i)) for i in range(3)],'entry_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()};pending_event_getters[caller]=rec
 if a in addrs:
  seen.append(addrs[a])
 if a==screen_resolution_addr:screen_resolution_calls.append({'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)})
 if a==redraw_addr:caller_records['drawable']=p.x(0);caller_records['event']=p.x(1);caller_records['event_entry_bytes']=bytes(u.mem_read(p.x(1),192)).hex()
 if a==redraw_addr+0x1cc:caller_records['sample_ends'].append({'history_index':u.reg_read(R.UC_ARM64_REG_W23),'add_count':len(caller_records['add_points'])})
 if a==redraw_addr+0x300:caller_records['terminal_start']=len(caller_records['add_points'])
 if a==add_addr:
  vec=p.x(1);this=p.x(0)
  pre={'center_bits':[struct.pack('<f',p.get(this+off,'f')[0]).hex() for off in (296,300)],'initial_flag':p.get(this+304,'B')[0], 'edge_state_bits':[p.get(this+308+4*i,'I')[0].to_bytes(4,'little').hex() for i in range(12)]}
  caller_records['add_points'].append({'receiver':hex(this),'vector_bytes':bytes(u.mem_read(vec,16)).hex(),'extent_bits':struct.pack('<f',p.f(0)).hex(),'pre_state':pre})
 if a==build_addr:caller_records['build_calls'].append({'rect':bytes(u.mem_read(p.x(1),32)).hex(),'output':hex(p.x(2))})
 if a==pen_data_addr and a:
  caller=p.u.reg_read(R.UC_ARM64_REG_LR);nameobj=p.x(1);storage=p.get(nameobj+8,'Q')[0];rec={'manager':hex(p.x(0)),'name_object':hex(nameobj),'name_storage':hex(storage),'arg_string_bytes':bytes(p.u.mem_read(storage,80)).hex()};pending_pen_data[caller]=rec
p.u.hook_add(UC_HOOK_CODE,tr)
def capture_mesh(drawable,build_calls):
 rtv1=p.get(drawable+16,'Q')[0]; rows=[]
 outputs=[]
 for call in build_calls:
  output=int(call['output'],16)
  if output not in outputs:outputs.append(output)
 if len(outputs)!=3:raise RuntimeError(f'expected three native RTV1 build vector objects, got {len(outputs)}')
 for slot,vector in enumerate(outputs):
  begin,end,capacity=p.get(vector,'QQQ'); size=end-begin
  if size<0 or size>16_000_000 or size%16:raise RuntimeError(f'invalid native Vector4 buffer object {vector:#x}: {size}')
  payload=bytes(p.u.mem_read(begin,size)) if size else b''
  rows.append({'build_output_slot':slot,'vector_object':hex(vector),'float_count':size//4,'vertex_count':size//16,'bytes_hex':payload.hex(),'sha256':hashlib.sha256(payload).hexdigest()})
 return {'rtv1':hex(rtv1),'buffers':rows}
try:
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[d,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 drawable=caller_records['drawable'];rtv1=p.get(drawable+16,'Q')[0]
 first_mesh=capture_mesh(drawable,caller_records['build_calls'])
 first_drawable=caller_records['drawable'];first_pen_data=pen_data_returns[-1]['return'] if pen_data_returns else None;first_manager=pen_data_returns[-1]['manager'] if pen_data_returns else None
 first_add_count=len(caller_records['add_points']);first_build_count=len(caller_records['build_calls'])
 first_add_points=caller_records['add_points'][:first_add_count]
 first_build_calls=caller_records['build_calls'][:first_build_count]
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[d,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 repeat_summary={'same_drawable':first_drawable==caller_records['drawable'],'same_pen_data':first_pen_data==(pen_data_returns[-1]['return'] if pen_data_returns else None),'same_manager':first_manager==(pen_data_returns[-1]['manager'] if pen_data_returns else None),'new_add_points':len(caller_records['add_points'])-first_add_count,'new_build_calls':len(caller_records['build_calls'])-first_build_count,'native_return':p.x(0)}
 # Second ObjectStroke, same active profile and Drawing: this probes whether
 # saved-call history reuses the cached PenData/drawable across stroke objects.
 obj2=p.alloc(256);impl2=p.alloc(512);p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]);p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]);p.put(obj2+32,'Q',impl2);p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname]);p.f(0,6.0);p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2]);p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,2]);p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,0]);p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,1]);p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,0x80112233])
 pts2=p.alloc(24);prs2=p.alloc(12);ts2=p.alloc(12);tilt2=p.alloc(12);ori2=p.alloc(12);p.put(pts2,'6f',0,0,4,1,9,5);p.put(prs2,'3f',.3,.6,.8);p.put(ts2,'3i',100,115,130);p.put(tilt2,'3f',.1,.2,.3);p.put(ori2,'3f',0,.35,.7);p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,pts2,prs2,ts2,tilt2,ori2,3]);add2=len(caller_records['add_points']);build2=len(caller_records['build_calls']);sample2=len(caller_records['sample_ends']);pdata2=len(pen_data_returns);pen_data_second=None
 old_drawing=d;p.call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1000000);d=p.x(0)
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[d,p.draw_canvas,p.pen_canvas,obj2,1],limit=5000000)
 fresh_return=p.x(0);fresh_pen_data=pen_data_returns[-1] if len(pen_data_returns)>pdata2 else None
 fresh_add_points=caller_records['add_points'][add2:];fresh_build_calls=caller_records['build_calls'][build2:]
 fresh_drawable=caller_records['drawable'];fresh_mesh=capture_mesh(fresh_drawable,fresh_build_calls)
 fresh_manager=hex(p.get(d+16,'Q')[0])
 library_hashes={name:hashlib.sha256(libpath.read_bytes()).hexdigest() for name,libpath in runtime.paths.items()}
 library_hashes['libc++_shared.so']=hashlib.sha256((LIBS/'libc++_shared.so').read_bytes()).hexdigest()
 library_hashes['libSPenMontblancCalligraphyPen.so']=hashlib.sha256(path.read_bytes()).hexdigest()
 result={'schema':'sdocx-calligraphy-saved-caller-case-v1','case_id':'three-point-curve-fresh-drawing','input':{'kind':'synthetic native ObjectStroke setters; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.MontblancCalligraphyPen','points_f32_bits':['00000000','00000000','00008040','0000803f','00001041','0000a040'],'pressure_f32_bits':['9a99993e','9a99193f','cdcc4c3f'],'timestamps_i32':[100,115,130],'tilt_f32_bits':['cdcccc3d','cdcc4c3e','9a99993e'],'orientation_f32_bits':['00000000','3333b33e','3333333f'],'pen_size_f32_bits':'0000c040','argb_u32':0x80112233,'tool_type':2,'stroke_type':0,'curve_enabled_setter_arg':1},'model_getters':model_channels,'context':{'profile_width':1920,'profile_height':1080,'color_get_color_selector3':'identity','flags_u32':1,'version_f32_bits':'0000803f'},'initializer_scope':{'executed':['libSPenMontblancCalligraphyPen.so .init_array'],'not_executed':'Mapped support DSO init arrays are not run by this capture; helper exposes an opt-in runner but this fixture does not invoke it.'},'motion_event':{'constructor_and_getters_native':True,'entry_bytes':caller_records['event_entry_bytes'],'native_redraw_getter_calls':event_getter_calls,'getter_results_scope':'only getters intercepted during native RedrawPen; no post-destruction manual calls','note':'MotionEvent time getter reports relative saved timestamps [0,15,30] while downTime is 100; coordinate getter ABI was recorded with both historical argument orders.'},'cache_observation':{'first_drawing':hex(old_drawing),'first_manager':first_manager,'first_pen_data':first_pen_data,'first_drawable':hex(first_drawable),'same_drawing_repeat':repeat_summary,'fresh_drawing':hex(d),'fresh_manager':fresh_manager,'fresh_pen_data':fresh_pen_data['return'] if fresh_pen_data else None,'fresh_drawable':hex(fresh_drawable),'distinct_manager':fresh_manager!=first_manager,'distinct_pen_data':(fresh_pen_data['return'] if fresh_pen_data else None)!=first_pen_data},'native_capture':{'draw_object_stroke_return':fresh_return,'screen_resolution_calls':screen_resolution_calls,'first_draw':{'add_points':first_add_points,'build_calls':first_build_calls,'rtv1_mesh':first_mesh},'same_drawing_repeat':repeat_summary,'fresh_drawing':{'add_points':fresh_add_points,'observed_sample_boundary_hooks':caller_records['sample_ends'][sample2:],'terminal_start_add_point_index':caller_records['terminal_start'],'build_calls':fresh_build_calls,'rtv1_mesh':fresh_mesh}},'libraries':library_hashes,'host_boundaries':['explicit synthetic IContext display profile 1920x1080, identity color conversion, flags=1/version=1.0','Canvas/bitmap/context ownership and queued-task registration remain host adapters; the native task Run methods, ObjectDrawing::drawObjectStroke, PenManager lookup, MotionEvent constructor/getters, Calligraphy GLV1 redraw, RTV1 AddPoint and buildMesh execute','ManagedObject::OnRelease/OnRestore relocations target BRK traps to prove they are not needed on the capture path; __sF is zeroed host data; unresolved imports are left zero and recorded below','the outer Composer DrawObjectList bitmap traversal and DrawingFactory::ReleaseDrawing resource teardown are not exercised']}
 unresolved_counts=collections.Counter((lib,typ,name) for lib,where,name,typ in runtime.unresolved_imports)
 result['unresolved_imports']=[{'dso':lib,'relocation_type':f'R_AARCH64_{"ABS64" if typ==0x101 else "GLOB_DAT"}','symbol':name,'relocation_count':count} for (lib,typ,name),count in sorted(unresolved_counts.items())]
 emutls_counts=collections.Counter((row['size'],row['align'],row['existing_nonnull'],row['template_nonnull'],row['template_sha256'],row['template_nonzero'],row['template_bytes_hex']) for row in runtime.emutls_descriptors)
 result['emutls_descriptors']=[{'size':size,'align':align,'existing_nonnull':existing,'template_nonnull':template,'template_sha256':digest,'template_nonzero':nonzero,'template_bytes_hex':template_bytes,'observed_calls':count} for (size,align,existing,template,digest,nonzero,template_bytes),count in sorted(emutls_counts.items())]
 if len(fresh_add_points)!=13 or fresh_return!=1 or not fresh_mesh['buffers']:
  raise RuntimeError(f'unexpected fresh caller capture: AddPoint={len(fresh_add_points)} return={fresh_return}')
 if [item['sha256'] for item in first_mesh['buffers']] != [item['sha256'] for item in fresh_mesh['buffers']]:
  raise RuntimeError('same-input fresh Drawing generated different native RTV1 buffers')
 normalized_add_points=lambda rows:[{key:value for key,value in row.items() if key!='receiver'} for row in rows]
 if normalized_add_points(first_add_points)!=normalized_add_points(fresh_add_points):
  raise RuntimeError('same-input fresh Drawing changed AddPoint values or prestate')
 fixture_data={'schema':'sdocx-calligraphy-saved-caller-native-v1','cases':{result['case_id']:result}}
 if cli_args.write_fixture:FIXTURE.write_text(json.dumps(fixture_data,indent=2,sort_keys=True)+'\n');print('wrote',FIXTURE.relative_to(ROOT))
 elif cli_args.check_fixture:
  if not FIXTURE.exists() or json.loads(FIXTURE.read_text())!=fixture_data:raise SystemExit('saved caller fixture mismatch')
  print(f"verified {FIXTURE.relative_to(ROOT)}: {len(fresh_add_points)} AddPoint calls, {len(fresh_mesh['buffers'])} native vectors")
 else:print(json.dumps(fixture_data,indent=2,sort_keys=True))
except:
 raise
