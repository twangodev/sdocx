import pathlib, struct, subprocess, os, hashlib, json, argparse, collections
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
parser=argparse.ArgumentParser(description='Pinned native ObjectDrawing -> Calligraphy saved-stroke caller capture')
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--print-capture',action='store_true')
parser.add_argument('--skip-batch',action='store_true',help='skip the native outer ObjectList batch capture')
cli_args=parser.parse_args()
if cli_args.write_fixture and cli_args.skip_batch:
 raise SystemExit('--write-fixture cannot be combined with --skip-batch')
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
ctx_queue=0x200f23e0
bitmap_width=0x200f2400;bitmap_height=0x200f2420
renderer_context_query=0x200f2440
renderer_clear_queue=0x200f2460
p.put(vtable+16,'Q',ctx_get);p.put(vtable+24,'Q',ctx_queue);p.put(vtable+80,'Q',ctx_color);p.put(ctx_child_vt+32,'Q',ctx_child32);p.put(ctx_child_vt+40,'Q',ctx_child40)
queue_vtable=p.alloc(64); p.put(queue,'Q',queue_vtable); p.put(queue_vtable+16,'Q',0x20001000)
p._task_returns=[]
p._task_invocations=[]
p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6'))
p.put(p.context_vtable+16,'Q',0x20001040)
def caller_context_hooks(u,a,size,data):
 if a==ctx_get: p.x(0,ctx_child)
 elif a==ctx_queue: p.x(0,queue)
 elif a==ctx_color: p.x(0,p.x(1))
 elif a==ctx_child32: p.x(0,1920)
 elif a==ctx_child40: p.x(0,1080)
 elif a==bitmap_width: p.x(0,1920)
 elif a==bitmap_height: p.x(0,1080)
 elif a==renderer_context_query: p.x(0,0)
 elif a==renderer_clear_queue: p._renderer_clear_calls.append({'receiver':hex(p.x(0)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 else:return
 u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,caller_context_hooks)
def taskqueue(u,a,z,d):
 if a==0x20001040:
  p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1);vt=struct.unpack('<Q',u.mem_read(task,8))[0];method=struct.unpack('<Q',u.mem_read(vt+16,8))[0]
  if not method:raise RuntimeError('queued native task has null Run method')
  p._task_invocations.append({'task':hex(task),'vtable':hex(vt),'run':hex(method),'bytes':bytes(u.mem_read(task,64)).hex()})
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
caller_records={'add_points':[],'sample_ends':[],'build_calls':[],'drawable':None,'event':None,'rtv1_lifecycle':[],'redraw_sessions':[]}
clock_calls=[]
def deterministic_clock_gettime():
 clock_id=p.x(0);out=p.x(1);tick=len(clock_calls);clock_calls.append(clock_id)
 p.put(out,'qq',1700000000,tick*1000000);p.x(0,0)
p.callbacks['clock_gettime']=deterministic_clock_gettime
screen_resolution_calls=[]
screen_resolution_addr=p.syms['SPen::Pen::SetScreenResolution(int, int)']
pen_data_addr=p.syms.get('SPen::PenManager::GetPenData(SPen::String*)',0);pending_pen_data={};pen_data_returns=[]
event_getters={n:p.syms[n] for n in ('SPen::MotionEvent::GetHistorySize() const','SPen::MotionEvent::GetPointerCount() const','SPen::MotionEvent::GetEventTime() const','SPen::MotionEvent::GetX(int) const','SPen::MotionEvent::GetY(int) const','SPen::MotionEvent::GetHistoricalX(int, int) const','SPen::MotionEvent::GetHistoricalY(int, int) const','SPen::MotionEvent::GetHistoricalPressure(int) const','SPen::MotionEvent::GetPressure(int) const') if n in p.syms};pending_event_getters={};event_getter_calls=[]
redraw_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)']
draw_stroke_addr=p.syms['SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)']
add_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::AddPoint(SPen::Vector4<float>, float)']
build_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::buildMesh(SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Rect const&, std::__ndk1::vector<SPen::Vector4<float>, std::__ndk1::allocator<SPen::Vector4<float> > >*)']
clear_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Clear()']
update_addr=p.syms['SPen::MontblancCalligraphyPenStrokeDrawableRTV1::Update(bool)']
pending_redraw_returns={}
native_batch_active=False
def tr(u,a,z,data):
 if a in pending_redraw_returns:
  session=pending_redraw_returns.pop(a)
  session['end_add_point_index']=len(caller_records['add_points'])
  session['end_build_call_index']=len(caller_records['build_calls'])
  session['terminal_start_add_point_index']=caller_records['terminal_start']
  session['glv1_state_after_hex']=bytes(u.mem_read(session['drawable_address'],512)).hex()
  if native_batch_active:
   session['rtv1_state_after']=snapshot_rtv1_state(session['drawable_address'])
   build_start=session['start_build_call_index'];build_end=session['end_build_call_index']
   session['rtv1_mesh_at_redraw_return']=capture_mesh(session['drawable_address'],caller_records['build_calls'][build_start:build_end])
  caller_records['redraw_sessions'].append(session)
 if a in pending_pen_data:
  rec=pending_pen_data.pop(a);rec['return']=hex(p.x(0));pen_data_returns.append(rec)
 if a in pending_event_getters:
  rec=pending_event_getters.pop(a);rec['result_x0']=hex(p.x(0));rec['result_s0']=struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex();rec['result_d0']=struct.pack('<Q',u.reg_read(R.UC_ARM64_REG_D0)).hex();event_getter_calls.append(rec)
 if a in event_getters.values():
  caller=u.reg_read(R.UC_ARM64_REG_LR);name=next(n for n,v in event_getters.items() if v==a);rec={'method':name,'args':[hex(p.x(i)) for i in range(3)],'entry_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()};pending_event_getters[caller]=rec
 if a in addrs:
  seen.append(addrs[a])
 if a==draw_stroke_addr:
  caller_records.setdefault('draw_stroke_calls',[]).append({'drawing':hex(p.x(0)),'canvas':hex(p.x(1)),'pen_canvas':hex(p.x(2)),'stroke':hex(p.x(3)),'flag':bool(p.x(4))})
 if a==redraw_addr:
  caller_records['rtv1_lifecycle'].append({'event':'GLV1::RedrawPen(MotionEvent)','drawable':hex(p.x(0)),'motion_event':hex(p.x(1))})
 if a==clear_addr:caller_records['rtv1_lifecycle'].append({'event':'RTV1::Clear','receiver':hex(p.x(0))})
 if a==update_addr:caller_records['rtv1_lifecycle'].append({'event':'RTV1::Update(bool)','receiver':hex(p.x(0)),'force':bool(p.x(1))})
 if a==screen_resolution_addr:screen_resolution_calls.append({'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)})
 if a==redraw_addr:
  caller_records['drawable']=p.x(0);caller_records['event']=p.x(1);caller_records['event_entry_bytes']=bytes(u.mem_read(p.x(1),192)).hex()
  ret=u.reg_read(R.UC_ARM64_REG_LR)
  session={'drawable':hex(p.x(0)),'drawable_address':p.x(0),'motion_event':hex(p.x(1)),
           'start_add_point_index':len(caller_records['add_points']),'start_build_call_index':len(caller_records['build_calls']),
           'glv1_state_before_hex':bytes(u.mem_read(p.x(0),512)).hex()}
  if native_batch_active:
   session['rtv1_state_before']=snapshot_rtv1_state(p.x(0))
  pending_redraw_returns[ret]=session
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
def snapshot_rtv1_state(drawable):
 rtv1=p.get(drawable+16,'Q')[0]
 return {'center_bits':[struct.pack('<f',p.get(rtv1+off,'f')[0]).hex() for off in (296,300)],
         'initial_flag':p.get(rtv1+304,'B')[0],
         'edge_state_bits':[p.get(rtv1+308+4*i,'I')[0].to_bytes(4,'little').hex() for i in range(12)]}
def draw_capture(drawing,stroke):
 add_start=len(caller_records['add_points']);build_start=len(caller_records['build_calls']);sample_start=len(caller_records['sample_ends']);lifecycle_start=len(caller_records['rtv1_lifecycle']);redraw_start=len(caller_records['redraw_sessions']);pen_data_start=len(pen_data_returns)
 caller_records['terminal_start']=None
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,stroke,1],limit=5000000)
 adds=caller_records['add_points'][add_start:];builds=caller_records['build_calls'][build_start:]
 if not caller_records['drawable'] or not builds:raise RuntimeError('saved stroke draw returned without native drawable/buildMesh evidence')
 return {'native_return':p.x(0),'drawable':caller_records['drawable'],
         'pen_data':pen_data_returns[-1]['return'] if len(pen_data_returns)>pen_data_start else None,
         'add_points':adds,'sample_boundaries':caller_records['sample_ends'][sample_start:],
         'rtv1_lifecycle':caller_records['rtv1_lifecycle'][lifecycle_start:],
         'redraw_sessions':caller_records['redraw_sessions'][redraw_start:],
         'terminal_start_add_point_index':caller_records['terminal_start'],
         'build_calls':builds,'rtv1_mesh':capture_mesh(caller_records['drawable'],builds),
         'post_draw_rtv1_state':snapshot_rtv1_state(caller_records['drawable'])}
try:
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[d,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 drawable=caller_records['drawable'];rtv1=p.get(drawable+16,'Q')[0]
 first_mesh=capture_mesh(drawable,caller_records['build_calls'])
 first_post_rtv1_state=snapshot_rtv1_state(drawable)
 first_drawable=caller_records['drawable'];first_pen_data=pen_data_returns[-1]['return'] if pen_data_returns else None;first_manager=pen_data_returns[-1]['manager'] if pen_data_returns else None
 first_add_count=len(caller_records['add_points']);first_build_count=len(caller_records['build_calls'])
 first_add_points=caller_records['add_points'][:first_add_count]
 first_build_calls=caller_records['build_calls'][:first_build_count]
 first_redraw_session=caller_records['redraw_sessions'][-1]
 repeated_capture=draw_capture(d,obj)
 repeat_summary={'same_drawable':first_drawable==repeated_capture['drawable'],'same_pen_data':first_pen_data==repeated_capture['pen_data'],'same_manager':first_manager==(pen_data_returns[-1]['manager'] if pen_data_returns else None),'new_add_points':len(repeated_capture['add_points']),'new_build_calls':len(repeated_capture['build_calls']),'native_return':repeated_capture['native_return']}
 # Second ObjectStroke, same active profile and Drawing: this probes whether
 # saved-call history reuses the cached PenData/drawable across stroke objects.
 obj2=p.alloc(256);impl2=p.alloc(512);p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]);p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]);p.put(obj2+32,'Q',impl2);p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj2,pname]);p.f(0,6.0);p.call('SPen::ObjectStroke::SetPenSize(float)',[obj2]);p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj2,2]);p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj2,0]);p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj2,1]);p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj2,0x80112233])
 pts2=p.alloc(24);prs2=p.alloc(12);ts2=p.alloc(12);tilt2=p.alloc(12);ori2=p.alloc(12);p.put(pts2,'6f',0,0,4,1,9,5);p.put(prs2,'3f',.3,.6,.8);p.put(ts2,'3i',100,115,130);p.put(tilt2,'3f',.1,.2,.3);p.put(ori2,'3f',0,.35,.7);p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj2,pts2,prs2,ts2,tilt2,ori2,3])
 obj3=p.alloc(256);impl3=p.alloc(512);p.call('SPen::ObjectStroke::ObjectStroke()',[obj3]);p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl3,obj3]);p.put(obj3+32,'Q',impl3);p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj3,pname]);p.f(0,4.25);p.call('SPen::ObjectStroke::SetPenSize(float)',[obj3]);p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj3,2]);p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj3,0]);p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj3,1]);p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj3,0x80112233])
 pts3=p.alloc(16);prs3=p.alloc(8);ts3=p.alloc(8);tilt3=p.alloc(8);ori3=p.alloc(8);p.put(pts3,'4f',30,2,31.25,2.5);p.put(prs3,'2f',.75,.4);p.put(ts3,'2i',200,215);p.put(tilt3,'2f',.05,.25);p.put(ori3,'2f',.9,1.1);p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj3,pts3,prs3,ts3,tilt3,ori3,2])
 second_model_channels={}
 for label,method,element_size in [('points','SPen::ObjectStroke::GetPoint() const',8),('pressure','SPen::ObjectStroke::GetPressure() const',4),('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),('tilt','SPen::ObjectStroke::GetTilt() const',4),('orientation','SPen::ObjectStroke::GetOrientation() const',4)]:
  p.call(method,[obj3]);data=p.x(0);length=2*element_size
  second_model_channels[label]={'getter':method,'count':2,'element_size':element_size,'bytes':bytes(p.u.mem_read(data,length)).hex()}
 for label,method in [('pen_size','SPen::ObjectStroke::GetPenSize() const'),('color','SPen::ObjectStroke::GetColor() const'),('tool_type','SPen::ObjectStroke::GetToolType() const'),('stroke_type','SPen::ObjectStroke::GetStrokeType() const'),('point_count','SPen::ObjectStroke::GetPointCount() const')]:
  p.call(method,[obj3]);second_model_channels[label]=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex() if label=='pen_size' else p.x(0)
 second_same_drawing=draw_capture(d,obj3);pdata2=len(pen_data_returns)
 old_drawing=d;p.call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1000000);d=p.x(0)
 sample2=len(caller_records['sample_ends'])
 fresh_capture=draw_capture(d,obj2)
 fresh_return=fresh_capture['native_return'];fresh_pen_data=pen_data_returns[-1] if len(pen_data_returns)>pdata2 else None
 fresh_add_points=fresh_capture['add_points'];fresh_build_calls=fresh_capture['build_calls']
 fresh_drawable=fresh_capture['drawable'];fresh_mesh=fresh_capture['rtv1_mesh']
 fresh_manager=hex(p.get(d+16,'Q')[0])
 original_fresh_drawing=d
 p.call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1000000);short_fresh_drawing=p.x(0)
 short_fresh_capture=draw_capture(short_fresh_drawing,obj3)
 short_fresh_manager=hex(p.get(short_fresh_drawing+16,'Q')[0])
 # Preserve the direct-caller fixture observations before the outer batch
 # appends its own MotionEvent, screen-resolution, and sample-boundary calls.
 direct_event_entry_bytes=caller_records['event_entry_bytes']
 direct_event_getter_calls=list(event_getter_calls)
 direct_screen_resolution_calls=list(screen_resolution_calls)
 direct_terminal_start=caller_records['terminal_start']
 if cli_args.skip_batch:
  batch_capture=None
 else:
  uuid_calls=[];uuid_context=p.alloc(80);uuid_sequence=[0]
  def host_uuid_create():
   pointer=p.x(0);p.put(pointer,'Q',uuid_context);uuid_calls.append({'method':'uuid_create','out_slot':hex(pointer),'uuid_context':hex(uuid_context)});p.x(0,0)
  def host_uuid_make():
   uuid_calls.append({'method':'uuid_make','args':[hex(p.x(i)) for i in range(4)]});p.x(0,0)
  def host_uuid_export():
   uuid_sequence[0]+=1;destination=p.get(p.x(2),'Q')[0];value=f'00000000-0000-4000-8000-{uuid_sequence[0]:012x}\0'.encode();p.u.mem_write(destination,value);uuid_calls.append({'method':'uuid_export','destination':hex(destination),'value':value[:-1].decode()});p.x(0,0)
  p.callbacks['uuid_create']=host_uuid_create;p.callbacks['uuid_make']=host_uuid_make;p.callbacks['uuid_export']=host_uuid_export;p.callbacks['uuid_destroy']=lambda:p.x(0,0)
  renderer_holder=p.alloc(32);renderer_holder_vtable=p.alloc(128);p.put(renderer_holder,'Q',renderer_holder_vtable);p.put(renderer_holder_vtable+32,'Q',renderer_context_query)
  p._renderer_clear_calls=[];p.put(renderer_holder_vtable+40,'Q',renderer_clear_queue)
  p.callbacks['SPen::RendererFactory::GetContextHolder()']=lambda:p.x(0,renderer_holder)
  # ObjectInstanceManager's native tree/map headers are Model DSO static
  # objects. Run only this mapped library's actual initializer array before
  # constructing list-owned ObjectBase state.
  runtime.run_init(runtime.paths['Model'],runtime.bases['Model'])
  # Exercise the native outer list dispatcher with two different strokes. The
  # list and visibility flags are created through shipped Model methods; the
  # bitmap/factory boundary remains the same explicit host canvas adapter.
  batch_drawing_result=p.call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1000000);batch_drawing=p.x(0)
  list_bitmap=p.alloc(160);list_bitmap_vtable=p.alloc(256);p.put(list_bitmap,'Q',list_bitmap_vtable);p.put(list_bitmap+8,'Q',ctx);p.put(list_bitmap_vtable+72,'Q',bitmap_width);p.put(list_bitmap_vtable+80,'Q',bitmap_height)
  object_list=p.alloc(32);p.call('SPen::ObjectList::ObjectList()',[object_list]);p.call('SPen::ObjectList::Construct()',[object_list])
  object_list_status=p.x(0)
  if not object_list_status:raise RuntimeError('native ObjectList::Construct failed')
  batch_strokes=[obj,obj3]
  batch_objects=[]
  for ordinal,stroke in enumerate(batch_strokes):
   # Reuse the already populated native ObjectStroke/ObjectStrokeImpl records,
   # then attach the native ObjectBaseImpl required by ObjectList traversal.
   p.call('SPen::ObjectBase::Construct(SPen::Object::Type)',[stroke,1])
   base_construct_return=p.x(0)
   if not base_construct_return:raise RuntimeError(f'native ObjectBase::Construct(Type=1) failed for list input {ordinal}')
   p.call('SPen::ObjectBase::SetVisibility(bool)',[stroke,1])
   p.f(0,1.0);p.call('SPen::ObjectBase::SetAlpha(float)',[stroke]);alpha_set_return=p.x(0)
   p.call('SPen::ObjectBase::GetType() const',[stroke]);object_type=p.x(0)
   p.call('SPen::ObjectBase::IsVisible() const',[stroke]);visible=p.x(0)
   p.call('SPen::ObjectBase::GetAlpha() const',[stroke]);alpha_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
   source_count=3 if ordinal==0 else 2
   source_channels={}
   for label,method,element_size in [('points','SPen::ObjectStroke::GetPoint() const',8),('pressure','SPen::ObjectStroke::GetPressure() const',4),('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),('tilt','SPen::ObjectStroke::GetTilt() const',4),('orientation','SPen::ObjectStroke::GetOrientation() const',4)]:
    p.call(method,[stroke]);data=p.x(0);source_channels[label]={'count':source_count,'element_size':element_size,'bytes':bytes(p.u.mem_read(data,source_count*element_size)).hex()}
   batch_objects.append({'address':hex(stroke),'source_role':'three-point-curve' if ordinal==0 else 'different-short-stroke','type':object_type,'visible':bool(visible),'alpha_f32_bits':alpha_bits,'base_construct_return':base_construct_return,'alpha_set_return':alpha_set_return,'source_channels':source_channels,'construct':'native ObjectBase::Construct(Type=1) attached to existing native ObjectStroke/ObjectStrokeImpl with actual saved channel setters'})
   p.call('SPen::ObjectList::Add(SPen::ObjectBase const*)',[object_list,stroke])
  p.call('SPen::ObjectList::GetCount() const',[object_list]);batch_list_count=p.x(0)
  batch_add_start=len(caller_records['add_points']);batch_build_start=len(caller_records['build_calls']);batch_draw_start=len(caller_records.get('draw_stroke_calls',[]));batch_redraw_start=len(caller_records['redraw_sessions']);batch_pen_start=len(pen_data_returns);batch_lifecycle_start=len(caller_records['rtv1_lifecycle'])
  native_batch_active=True
  try:batch_return=p.call('SPen::ObjectDrawing::DrawObjectList(SPen::ISPBitmap*, SPen::ObjectList const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool, std::__ndk1::function<bool (int, int)>*)',[batch_drawing,list_bitmap,object_list,0,0,0,0],limit=10000000)
  finally:native_batch_active=False
  batch_adds=caller_records['add_points'][batch_add_start:];batch_builds=caller_records['build_calls'][batch_build_start:]
  batch_draw_calls=caller_records.get('draw_stroke_calls',[])[batch_draw_start:];batch_sessions=caller_records['redraw_sessions'][batch_redraw_start:]
  batch_runs=[]
  for ordinal,(draw_call,session) in enumerate(zip(batch_draw_calls,batch_sessions,strict=True)):
   start_add=session['start_add_point_index'];end_add=session['end_add_point_index'];start_build=session['start_build_call_index'];end_build=session['end_build_call_index'];drawable=int(session['drawable'],16)
   batch_runs.append({'ordinal':ordinal,'stroke':draw_call['stroke'],'drawable':session['drawable'],'add_points':caller_records['add_points'][start_add:end_add],'build_calls':caller_records['build_calls'][start_build:end_build],'rtv1_mesh':session['rtv1_mesh_at_redraw_return'],'glv1_prestate_hex':session['glv1_state_before_hex'],'glv1_poststate_hex':session['glv1_state_after_hex'],'rtv1_state_before':session['rtv1_state_before'],'rtv1_state_after':session['rtv1_state_after']})
  if batch_list_count!=2 or len(batch_draw_calls)!=2 or len(batch_sessions)!=2 or len(batch_runs)!=2:
   raise RuntimeError(f'native DrawObjectList did not traverse both source strokes: list={batch_list_count}, draw={len(batch_draw_calls)}, redraw={len(batch_sessions)}, runs={len(batch_runs)}')
  if [int(call['stroke'],16) for call in batch_draw_calls]!=batch_strokes:
   raise RuntimeError('native DrawObjectList changed ObjectList source order')
  if batch_runs[0]['drawable']!=batch_runs[1]['drawable']:
   raise RuntimeError('native DrawObjectList did not reuse the cached profile drawable within one list batch')
  if batch_runs[0]['rtv1_state_after']!=batch_runs[1]['rtv1_state_before']:
   raise RuntimeError('second native saved stroke did not enter with first stroke RTV1 nib state intact')
  batch_sessions=[{key:value for key,value in session.items() if key!='rtv1_mesh_at_redraw_return'} for session in batch_sessions]
  batch_capture={'native_return':p.x(0),'object_list_construct_return':object_list_status,'list_count':batch_list_count,'objects':batch_objects,'drawing':hex(batch_drawing),'manager':hex(p.get(batch_drawing+16,'Q')[0]),'draw_object_stroke_calls':batch_draw_calls,'redraw_sessions':batch_sessions,'runs':batch_runs,'same_drawable_reused':batch_runs[0]['drawable']==batch_runs[1]['drawable'],'add_point_count':len(batch_adds),'build_call_count':len(batch_builds),'pen_data_manager_results':pen_data_returns[batch_pen_start:],'rtv1_lifecycle':caller_records['rtv1_lifecycle'][batch_lifecycle_start:],'renderer_context_clear_calls':p._renderer_clear_calls,'host_clock_calls':clock_calls,'host_uuid_calls':uuid_calls,'model_initializer_scope':['libSPenModel.so .init_array'],'source_inputs':['three-point-curve-fresh-drawing.input','reused_drawing_streams.different_short_stroke.input'],'host_boundaries':['Drawing::DrawObjectList and ObjectList traversal/add, ObjectBase state, ObjectDrawing::drawObjectStroke, PenManager lookup, GLV1 RedrawPen, RTV1 AddPoint and buildMesh, and Model static initializers execute from pinned native DSOs','ISPBitmap width/height and canvas/context factories are explicit host callbacks; RendererFactory context holder/queue clear, clock_gettime, UUID generation/identity, task queue registration/dispatch adapter remain host boundaries']}
 library_hashes={name:hashlib.sha256(libpath.read_bytes()).hexdigest() for name,libpath in runtime.paths.items()}
 library_hashes['libc++_shared.so']=hashlib.sha256((LIBS/'libc++_shared.so').read_bytes()).hexdigest()
 library_hashes['libSPenMontblancCalligraphyPen.so']=hashlib.sha256(path.read_bytes()).hexdigest()
 result={'schema':'sdocx-calligraphy-saved-caller-case-v1','case_id':'three-point-curve-fresh-drawing','input':{'kind':'synthetic native ObjectStroke setters; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.MontblancCalligraphyPen','points_f32_bits':['00000000','00000000','00008040','0000803f','00001041','0000a040'],'pressure_f32_bits':['9a99993e','9a99193f','cdcc4c3f'],'timestamps_i32':[100,115,130],'tilt_f32_bits':['cdcccc3d','cdcc4c3e','9a99993e'],'orientation_f32_bits':['00000000','3333b33e','3333333f'],'pen_size_f32_bits':'0000c040','argb_u32':0x80112233,'tool_type':2,'stroke_type':0,'curve_enabled_setter_arg':1},'model_getters':model_channels,'context':{'profile_width':1920,'profile_height':1080,'color_get_color_selector3':'identity','flags_u32':1,'version_f32_bits':'0000803f'},'initializer_scope':{'executed':['libSPenMontblancCalligraphyPen.so .init_array'],'not_executed':'Mapped support DSO init arrays are not run by this capture; helper exposes an opt-in runner but this fixture does not invoke it.'},'motion_event':{'constructor_and_getters_native':True,'entry_bytes':direct_event_entry_bytes,'native_redraw_getter_calls':direct_event_getter_calls,'getter_results_scope':'only getters intercepted during native RedrawPen; no post-destruction manual calls','note':'MotionEvent time getter reports relative saved timestamps [0,15,30] while downTime is 100; coordinate getter ABI was recorded with both historical argument orders.'},'cache_observation':{'first_drawing':hex(old_drawing),'first_manager':first_manager,'first_pen_data':first_pen_data,'first_drawable':hex(first_drawable),'same_drawing_repeat':repeat_summary,'fresh_drawing':hex(d),'fresh_manager':fresh_manager,'fresh_pen_data':fresh_pen_data['return'] if fresh_pen_data else None,'fresh_drawable':hex(fresh_drawable),'distinct_manager':fresh_manager!=first_manager,'distinct_pen_data':(fresh_pen_data['return'] if fresh_pen_data else None)!=first_pen_data},'native_capture':{'draw_object_stroke_return':fresh_return,'screen_resolution_calls':direct_screen_resolution_calls,'first_draw':{'add_points':first_add_points,'redraw_session':first_redraw_session,'build_calls':first_build_calls,'rtv1_mesh':first_mesh},'same_drawing_repeat':repeat_summary,'fresh_drawing':{'add_points':fresh_add_points,'observed_sample_boundary_hooks':fresh_capture['sample_boundaries'],'terminal_start_add_point_index':direct_terminal_start,'build_calls':fresh_build_calls,'rtv1_mesh':fresh_mesh}},'libraries':library_hashes,'host_boundaries':['explicit synthetic IContext display profile 1920x1080, identity color conversion, flags=1/version=1.0','Canvas/bitmap/context ownership and queued-task registration remain host adapters; the native task Run methods, ObjectDrawing::drawObjectStroke, PenManager lookup, MotionEvent constructor/getters, Calligraphy GLV1 redraw, RTV1 AddPoint and buildMesh execute','ManagedObject::OnRelease/OnRestore relocations target BRK traps to prove they are not needed on the capture path; __sF is zeroed host data; unresolved imports are left zero and recorded below','the outer Composer DrawObjectList bitmap traversal and DrawingFactory::ReleaseDrawing resource teardown are not exercised']}
 unresolved_counts=collections.Counter((lib,typ,name) for lib,where,name,typ in runtime.unresolved_imports)
 result['unresolved_imports']=[{'dso':lib,'relocation_type':f'R_AARCH64_{"ABS64" if typ==0x101 else "GLOB_DAT"}','symbol':name,'relocation_count':count} for (lib,typ,name),count in sorted(unresolved_counts.items())]
 emutls_counts=collections.Counter((row['size'],row['align'],row['existing_nonnull'],row['template_nonnull'],row['template_sha256'],row['template_nonzero'],row['template_bytes_hex']) for row in runtime.emutls_descriptors)
 result['emutls_descriptors']=[{'size':size,'align':align,'existing_nonnull':existing,'template_nonnull':template,'template_sha256':digest,'template_nonzero':nonzero,'template_bytes_hex':template_bytes,'observed_calls':count} for (size,align,existing,template,digest,nonzero,template_bytes),count in sorted(emutls_counts.items())]
 result['reused_drawing_streams']={
  'scope':'sequential direct ObjectDrawing::drawObjectStroke calls on one reused Drawing; outer DrawObjectList batching is not exercised',
  'lifecycle_observation':'Each saved stroke call entered GLV1::RedrawPen(MotionEvent); no RTV1::Clear or RTV1::Update(bool) entry was observed. Per-call GLV1 512-byte state snapshots and RTV1 AddPoint prestate/poststate are captured below.',
  'same_stroke_repeat':repeated_capture,
  'same_stroke_prior_post_state':first_post_rtv1_state,
  'original_three_point_fresh_drawing':{'drawing':hex(original_fresh_drawing),'capture':fresh_capture},
  'different_short_stroke':{'input':{'point_bytes_f32_le':bytes(p.u.mem_read(pts3,16)).hex(),'pressure_bytes_f32_le':bytes(p.u.mem_read(prs3,8)).hex(),'timestamps_i32':[200,215],'tilt_bytes_f32_le':bytes(p.u.mem_read(tilt3,8)).hex(),'orientation_bytes_f32_le':bytes(p.u.mem_read(ori3,8)).hex(),'pen_size_f32_bits':struct.pack('<f',4.25).hex(),'argb_u32':0x80112233,'stroke_type':0,'tool_type':2,'curve_enabled_setter_arg':1},'model_getters':second_model_channels,'same_drawing_capture':second_same_drawing,'fresh_drawing_capture':short_fresh_capture,'fresh_drawing':hex(short_fresh_drawing),'fresh_manager':short_fresh_manager}
 }
 if first_post_rtv1_state['edge_state_bits']!=repeated_capture['add_points'][0]['pre_state']['edge_state_bits']:
  raise RuntimeError('same-Drawing repeat did not begin with the prior completed native nib edge state')
 if repeated_capture['post_draw_rtv1_state']['edge_state_bits']!=second_same_drawing['add_points'][0]['pre_state']['edge_state_bits']:
  raise RuntimeError('different stroke did not begin with the prior completed native nib edge state')
 if any(bytes.fromhex(value)!=bytes(4) for value in fresh_capture['add_points'][0]['pre_state']['edge_state_bits']):
  raise RuntimeError('fresh Drawing did not begin with a zeroed nib edge state')
 if not short_fresh_capture['add_points'] or any(bytes.fromhex(value)!=bytes(4) for value in short_fresh_capture['add_points'][0]['pre_state']['edge_state_bits']):
  raise RuntimeError('fresh Drawing short stroke did not begin with a zeroed nib edge state')
 if short_fresh_capture['add_points'][0]['vector_bytes'] != second_same_drawing['add_points'][0]['vector_bytes']:
  raise RuntimeError('short stroke fresh reference did not consume the same first saved sample')
 for capture in (repeated_capture,second_same_drawing,fresh_capture,short_fresh_capture):
  if [item['event'] for item in capture['rtv1_lifecycle']]!=['GLV1::RedrawPen(MotionEvent)']:
   raise RuntimeError(f'unexpected Clear/Update lifecycle call in saved stroke draw: {capture["rtv1_lifecycle"]}')
 if len(fresh_add_points)!=13 or fresh_return!=1 or not fresh_mesh['buffers']:
  raise RuntimeError(f'unexpected fresh caller capture: AddPoint={len(fresh_add_points)} return={fresh_return}')
 if [item['sha256'] for item in first_mesh['buffers']] != [item['sha256'] for item in fresh_mesh['buffers']]:
  raise RuntimeError('same-input fresh Drawing generated different native RTV1 buffers')
 normalized_add_points=lambda rows:[{key:value for key,value in row.items() if key!='receiver'} for row in rows]
 if normalized_add_points(first_add_points)!=normalized_add_points(fresh_add_points):
  raise RuntimeError('same-input fresh Drawing changed AddPoint values or prestate')
 fixture_data={'schema':'sdocx-calligraphy-saved-caller-native-v1','cases':{result['case_id']:result}}
 if batch_capture is not None:
  fixture_data['cases']['two-stroke-object-list-batch']={'schema':'sdocx-calligraphy-object-list-batch-v1','case_id':'two-stroke-object-list-batch','input':{'kind':'native Model ObjectList of two existing synthetic ObjectStroke records','sources':[result['input'],result['reused_drawing_streams']['different_short_stroke']['input']]},'context':result['context'],'libraries':library_hashes,'scope':'Native ObjectList construction/Add and Drawing::DrawObjectList are executed. Model static initializers run before ObjectBase construction. This is an explicit synthetic host bitmap/context replay, not an Android export lifecycle capture.','capture':batch_capture}
 if cli_args.write_fixture:FIXTURE.write_text(json.dumps(fixture_data,indent=2,sort_keys=True)+'\n');print('wrote',FIXTURE.relative_to(ROOT))
 elif cli_args.check_fixture:
  if not FIXTURE.exists():raise SystemExit('saved caller fixture is missing')
  expected_fixture=json.loads(FIXTURE.read_text())
  if cli_args.skip_batch:expected_fixture['cases'].pop('two-stroke-object-list-batch',None)
  if expected_fixture!=fixture_data:raise SystemExit('saved caller fixture mismatch')
  print(f"verified {FIXTURE.relative_to(ROOT)}: {len(fresh_add_points)} fresh AddPoint calls, {len(fresh_mesh['buffers'])} native vectors" + (f"; batch {len(batch_capture['runs'])} strokes, {batch_capture['add_point_count']} AddPoint calls" if batch_capture is not None else "; batch skipped"))
 else:print(json.dumps(fixture_data,indent=2,sort_keys=True))
except:
 raise
