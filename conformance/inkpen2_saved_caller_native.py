"""Native Drawing caller captures for InkPen2 null/empty/explicit V9 settings.

Run with the project's pinned Unicorn and AArch64 toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/inkpen2_saved_caller_native.py --check-fixture
"""
import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0,str(ROOT/'conformance'))
import oblique_canvas_native as oc
oc.native.HASHES['InkPen2']='61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c'
_original_native_init=oc.native.NativeFountain.__init__
def _inkpen2_native_init(self, library_dir, objdump, additional_libraries=()):
 _original_native_init(self, library_dir, objdump, tuple('InkPen2' if item=='ObliquePen' else item for item in additional_libraries))
oc.native.NativeFountain.__init__=_inkpen2_native_init
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
FIXTURE=ROOT/'conformance/inkpen2-saved-caller-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--print-capture',action='store_true')

parser.add_argument('--case',choices=('route_control','line0_curve_short_history','line0_curve_short_history_color'),default='route_control')
parser.add_argument('--advanced-setting',choices=('null','empty','9'),default='null',help='apply saved ObjectStroke advanced setting before Drawing selects the drawable')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')

args=parser.parse_args()
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
if args.case == 'route_control':
 INPUT_CASES={'route_control': {'points':[[0.0,0.0],[2.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]}}
else:
 INPUT_CASES={args.case: {'points':[[0.0,0.0],[8.0,0.0],[16.0,3.0],[24.0,3.0]],'size':4.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[0.2,0.4,0.6,0.8],'timestamps':[0,10,20,30],'tilt':[0.0,0.0,0.0,0.0],'orientation':[0.0,0.0,0.0,0.0]}}
input_case=INPUT_CASES[args.case]
if args.case in ('line0_curve_short_history','line0_curve_short_history_color') and args.advanced_setting != '9':
 raise SystemExit('the numeric V9 comparison case requires the explicit 9; saved setting')
if not (0 < args.width < 65536 and 0 < args.height < 65536): raise SystemExit('profile dimensions must be positive u16 values')
def mov_w0_ret(value):
 if not 0 <= value <= 0xffff: raise ValueError('screen dimension exceeds MOVZ immediate')
 return struct.pack('<II',0x52800000|(value<<5),0xd65f03c0)
def call_address(address, args, limit=5_000_000):
 for index,value in enumerate(args):p.x(index,value)
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200EF000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
 p.u.emu_start(address,oc.STOP,count=limit)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'address call {address:#x} did not return')
p=oc.CanvasProbe()
# The shared canvas probe installs unsupported/invisible ObjectBase defaults
# to suppress paint in its matrix-only test. This caller must use Model's
# pinned native type/visibility methods: ObjectStroke is explicitly visible.
for inherited in ('SPen::ObjectBase::GetType() const','SPen::ObjectBase::IsVisible() const'):
 p.callbacks.pop(inherited,None)
def trace(u,a,z,d):
 if a==0x2000000+0xc3d88: print('STACKFAIL guard',hex(u.reg_read(R.UC_ARM64_REG_TPIDR_EL0)),u.mem_read(0x20000000+40,8).hex(),'x20',hex(u.reg_read(R.UC_ARM64_REG_X20)),'x8',u.mem_read(u.reg_read(R.UC_ARM64_REG_X29)-8,8).hex())
p.u.hook_add(UC_HOOK_CODE,trace)
raw=MODEL.read_bytes(); digest=hashlib.sha256(raw).hexdigest(); assert digest=='4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'
phoff=struct.unpack_from('<Q',raw,32)[0]; ent,num=struct.unpack_from('<HH',raw,54); extent=0
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
 if typ==1:extent=max(extent,va+ms)
p.u.mem_map(BASE,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(BASE+va,raw[off:off+fs])
  if ms>fs:p.u.mem_write(BASE+va+fs,bytes(ms-fs))
env=dict(os.environ); env['LD_LIBRARY_PATH']=str((pathlib.Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')).resolve())
for line in subprocess.check_output(['nm','-D','-C',str(MODEL)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=BASE+int(parts[0],16)
dis=subprocess.check_output([str(oc.OBJDUMP),'-d','-C','-j','.plt',str(MODEL)],text=True,env=env)
for addr,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[BASE+int(addr,16)]=name
# Map the pinned Android libc++ runtime needed by Base String/locale methods.
CPP=LIBS/'libc++_shared.so'; CPP_BASE=0x8000000; cb=CPP.read_bytes(); assert hashlib.sha256(cb).hexdigest()=='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
coff=struct.unpack_from('<Q',cb,32)[0]; cent,cnum=struct.unpack_from('<HH',cb,54); cextent=0
for i in range(cnum):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',cb,coff+i*cent)
 if typ==1:cextent=max(cextent,va+ms)
p.u.mem_map(CPP_BASE,(cextent+0xfff)&~0xfff)
for i in range(cnum):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',cb,coff+i*cent)
 if typ==1 and ms:
  p.u.mem_write(CPP_BASE+va,cb[off:off+fs])
  if ms>fs:p.u.mem_write(CPP_BASE+va+fs,bytes(ms-fs))
for line in subprocess.check_output(['nm','-D','-C',str(CPP)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=CPP_BASE+int(parts[0],16)
cdis=subprocess.check_output([str(oc.OBJDUMP),'-d','-C','-j','.plt',str(CPP)],text=True,env=env)
for addr,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',cdis,re.M):p.plt[CPP_BASE+int(addr,16)]=name
# Native loader relocation for Model references; code imports continue through bounded PLT resolution.
shoff=struct.unpack_from('<Q',raw,40)[0]; se,sn,ss=struct.unpack_from('<HHH',raw,58); secs=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*se) for i in range(sn)]; names=raw[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; symbols=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,shndx,val,size=struct.unpack_from('<IBBHQQ',raw,off); symbols.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',raw,off); typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(BASE+where,struct.pack('<Q',BASE+add))
  elif typ in (0x402,0x401,0x1025,0x101):
   name,val=symbols[idx]; target=BASE+val if val else p.syms.get(name,0)
   if target:p.u.mem_write(BASE+where,struct.pack('<Q',target+add if typ in (0x401,0x101) else target))

# Rebase dynamic relocations for mapped DSOs used by the actual constructors.
paths={
 'FountainPen':LIBS/'libSPenFountainPen.so','PenCommon':LIBS/'libSPenPenCommon.so',
 'Base':LIBS/'libSPenBase.so','Drawing':LIBS/'libSPenDrawing.so',
 'Graphics':LIBS/'libSPenGraphics.so','InkPen2':LIBS/'libSPenInkPen2.so',
 'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
 'Graphics':0x4000000,'InkPen2':0x5000000,'Composer':0x6000000,'Model':BASE}
rawsyms={}
for lib,path in paths.items():
 for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split()
  if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'): rawsyms.setdefault(parts[2],bases[lib]+int(parts[0],16))
for lib,path in paths.items():
 b=path.read_bytes(); base=bases[lib]; shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; ns=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={ns[x[0]:ns.find(b'\0',x[0])].decode():x for x in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
 for snm in ('.rela.dyn','.rela.plt'):
  sec=by.get(snm)
  if not sec:continue
  for off in range(sec[4],sec[4]+sec[5],sec[9]):
   where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
   if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
   elif typ in (0x401,0x402,0x403,0x101,0x1025,0x1026):
    name,val=sy[idx]; target=base+val if val else rawsyms.get(name,0)
    if not target:target=p.syms.get(name,0)
    if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
    elif lib=='InkPen2' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))
    if lib=='InkPen2' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())

def bind_exact_plt(path,base):
 for offset,raw_symbol in elf_plt.resolve(path).items():
  symbol=subprocess.check_output(['c++filt','-n',raw_symbol],text=True).strip() if raw_symbol.startswith('_Z') else raw_symbol
  p.plt[base+offset]=symbol
for _lib,_path in paths.items(): bind_exact_plt(_path,bases[_lib])
bind_exact_plt(CPP,CPP_BASE)

for name in ['std::__ndk1::recursive_mutex::lock()','std::__ndk1::recursive_mutex::unlock()','std::__ndk1::recursive_mutex::recursive_mutex()','std::__ndk1::recursive_mutex::~recursive_mutex()','std::__ndk1::mutex::lock()','std::__ndk1::mutex::unlock()','std::__ndk1::mutex::mutex()','std::__ndk1::mutex::~mutex()']: p.callbacks[name]=lambda:None
p.callbacks['operator new(unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
p.callbacks['operator new[](unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
p.callbacks['strlen']=lambda:p.x(0,len(bytes(p.u.mem_read(p.x(0),256)).split(b'\0',1)[0]))
p.callbacks['atoi']=lambda:p.x(0,int(bytes(p.u.mem_read(p.x(0),64)).split(b'\0',1)[0] or b'0'))
# Native Base defines the MotionEvent constructor and every accessor used by
# RedrawPen. fountain_native normally substitutes endpoint properties; remove
# that whole surface so this saved caller reads the event built from ObjectStroke.
for inherited in [name for name in p.callbacks if name.startswith('SPen::MotionEvent::')]:
 p.callbacks.pop(inherited)
p._newlog=[]
def native_new():
 size=p.x(0); out=p.alloc(size); p._newlog.append((out,size)); p.x(0,out)
p.callbacks['operator new(unsigned long)']=native_new
p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue)); p.callbacks['__cxa_atexit']=lambda:p.x(0,0)
loader_calls=[]; DSO_HANDLE=0x50000001
def read_cstr(addr): return bytes(p.u.mem_read(addr,256)).split(bytes([0]),1)[0].decode('ascii','replace')
def host_dlopen():
 name=read_cstr(p.u.reg_read(R.UC_ARM64_REG_X0)); flags=p.u.reg_read(R.UC_ARM64_REG_X1); loader_calls.append({'fn':'dlopen','name':name,'flags':flags});
 if name!='libSPenInkPen2.so': raise RuntimeError(f'unapproved dlopen {name!r}')
 p.u.reg_write(R.UC_ARM64_REG_X0,DSO_HANDLE)
def host_dlsym():
 handle=p.u.reg_read(R.UC_ARM64_REG_X0); name=read_cstr(p.u.reg_read(R.UC_ARM64_REG_X1)); loader_calls.append({'fn':'dlsym','handle':hex(handle),'name':name})
 if handle!=DSO_HANDLE: raise RuntimeError(f'unknown dso handle {handle:#x}')
 result=p.syms.get(name,0)
 if not result: raise RuntimeError(f'unresolved dlsym {name!r}')
 p.u.reg_write(R.UC_ARM64_REG_X0,result)
p.callbacks['dlopen']=host_dlopen; p.callbacks['dlsym']=host_dlsym; p.callbacks['dlerror']=lambda:p.x(0,0)

def strncmp():
 a,b,n=p.x(0),p.x(1),p.x(2);x=bytes(p.u.mem_read(a,n));y=bytes(p.u.mem_read(b,n));out=0
 for aa,bb in zip(x,y):
  if aa!=bb:out=aa-bb;break
  if aa==0:break
 p.x(0,out)
p.callbacks['strncmp']=strncmp
p.callbacks['getauxval']=lambda:p.x(0,{6:4096,16:256,26:0}.get(p.x(0),0))
p.callbacks['__system_property_get']=lambda:(p.u.mem_write(p.x(1),b'arm64-v8a0'),p.x(0,9))
p.callbacks['__cxa_thread_atexit']=lambda:p.x(0,0)
p._emutls={}
def emutls():
 key=p.x(0); size,align,existing,template=struct.unpack('<QQQQ',p.u.mem_read(key,32))
 if not 0<size<=0x100000 or not 0<align<=0x10000 or align&(align-1): raise RuntimeError(f'invalid emutls descriptor {size} {align}')
 template_bytes=bytes(p.u.mem_read(template,size)) if template else bytes(size)
 value=p._emutls.get(key)
 if value is None:
  if existing: raise RuntimeError(f'unowned preexisting emutls pointer {existing:#x}')
  rawp=p.alloc(size+align); value=(rawp+align-1)&-align; p.u.mem_write(value,template_bytes); p.u.mem_write(key+16,struct.pack('<Q',value)); p._emutls[key]=value
 p.x(0,value)
p.callbacks['__emutls_get_address']=emutls
p._guards=set()
def guard_acquire():
 key=p.x(0); value=struct.unpack('<Q',p.u.mem_read(key,8))[0]
 p.x(0,int(not value&1 and key not in p._guards));
 if p.x(0):p._guards.add(key)
def guard_release():
 key=p.x(0)
 if key not in p._guards:raise RuntimeError(f'guard release without acquire {key:#x}')
 p._guards.remove(key);p.u.mem_write(key,b'\1')
def guard_abort():p._guards.discard(p.x(0))
p.callbacks['__cxa_guard_acquire']=guard_acquire;p.callbacks['__cxa_guard_release']=guard_release;p.callbacks['__cxa_guard_abort']=guard_abort
def host_malloc():
 n=p.u.reg_read(R.UC_ARM64_REG_X0)
 if n>0x1000000: raise RuntimeError(f'malloc bound {n:#x}')
 p.u.reg_write(R.UC_ARM64_REG_X0,p.alloc(max(n,1)))
def host_calloc():
 n=p.u.reg_read(R.UC_ARM64_REG_X0)*p.u.reg_read(R.UC_ARM64_REG_X1)
 if n>0x1000000: raise RuntimeError(f'calloc bound {n:#x}')
 p.u.reg_write(R.UC_ARM64_REG_X0,p.alloc(max(n,1)))
for name in ['malloc','__libc_malloc']:p.callbacks[name]=host_malloc
p.callbacks['calloc']=host_calloc
for name in ['free','__libc_free']:p.callbacks[name]=lambda:p.x(0,0)
p.callbacks['realloc']=lambda:p.x(0,p.alloc(p.x(1)))
for name in ['pthread_mutex_init','pthread_mutexattr_init','pthread_mutexattr_destroy','pthread_mutexattr_settype','pthread_mutex_destroy','pthread_mutex_lock','pthread_mutex_unlock','pthread_cond_init','pthread_cond_destroy','pthread_cond_wait','pthread_cond_signal','pthread_cond_broadcast']:
 p.callbacks[name]=lambda:p.x(0,0)
# Single-thread pthread_once with actual init callback execution thunk.
ONCE=0x200f2080
p.u.mem_write(ONCE,bytes.fromhex('080040f9a80000b5fd7bbfa9fd030091280080d2080000f920003fd6fd7bc1a8000080d2c0035fd6'))
p.syms['pthread_once']=ONCE
p._newlog=[]
def native_new():
 size=p.x(0); out=p.alloc(size); p._newlog.append((out,size)); p.x(0,out)
p.callbacks['operator new(unsigned long)']=native_new
# Run every mapped non-Composer dependency initializer; any failure aborts the probe.
def run_init(path,base):
 b=path.read_bytes(); sh=struct.unpack_from('<Q',b,40)[0]; es,n,si=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,sh+i*es) for i in range(n)]; names=b[secs[si][4]:secs[si][4]+secs[si][5]]; sec=next(x for x in secs if names[x[0]:names.find(b'\0',x[0])]==b'.init_array');
 for i in range(0,sec[5],8):
  target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+i,8))[0]
  if not target:continue
  for j in range(4):p.x(j,0)
  p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
  try:p.u.emu_start(target,oc.STOP,count=3000000)
  except Exception as e: print('INIT EXC',path.name,hex(target),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),type(e).__name__,e); raise
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'init failed {path.name} {target:#x} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
print('symadvanced',p.syms.get('SPen::Pen::SetAdvancedSetting(SPen::String const*)'))
for path,base in [(CPP,CPP_BASE),(LIBS/'libSPenBase.so',0x2000000),(LIBS/'libSPenPenCommon.so',0x1000000),(MODEL,BASE),(LIBS/'libSPenInkPen2.so',0x5000000),(LIBS/'libSPenDrawing.so',0x3000000),(LIBS/'libSPenGraphics.so',0x4000000)]:run_init(path,base)
def emulate_lse(u,a,z,d):
 if a==0x2098c60:
  x1=u.reg_read(R.UC_ARM64_REG_X1); old=struct.unpack('<I',u.mem_read(x1,4))[0]; new=(old+u.reg_read(R.UC_ARM64_REG_W0))&0xffffffff; u.mem_write(x1,struct.pack('<I',new)); u.reg_write(R.UC_ARM64_REG_W0,old); u.reg_write(R.UC_ARM64_REG_PC,a+4)
 if a in (0x104caf0,0x104c400):
  x1=u.reg_read(R.UC_ARM64_REG_X1); old=struct.unpack('<Q',u.mem_read(x1,8))[0]; u.mem_write(x1,struct.pack('<Q',(old+u.reg_read(R.UC_ARM64_REG_X0))&0xffffffffffffffff)); u.reg_write(R.UC_ARM64_REG_X0,old); u.reg_write(R.UC_ARM64_REG_PC,a+4)
p.u.hook_add(UC_HOOK_CODE,emulate_lse)
def dltrace(u,a,z,d):
 if a==0x104ece4:
  x0=u.reg_read(R.UC_ARM64_REG_X0); x1=u.reg_read(R.UC_ARM64_REG_X1); data=bytes(u.mem_read(x0,200)).split(b'\\0',1)[0] if x0 else b''; print('DLOPEN args',hex(x0),data,hex(x1))
p.u.hook_add(UC_HOOK_CODE,dltrace)
object_stroke_snapshots=[]
factory_call_returns={}
factory_created_pens=[]
rt_identity_by_address={}
selected_rt_id=None
native_draw_entries=[]
native_motion_getter_returns=[]
pending_motion_getters={}
method_trace_names_by_pc={}
inkpen2_buffer_events=[]
drawable_threshold_entries=[]
drawable_entry_snapshots=[]
paint_color_calls=[]
pending_inkpen2_buffer_returns={}
for _name,_address in p.syms.items():
 oblique_method=('InkPen2StrokeDrawableGLV' in _name and any(token in _name for token in ('::Draw(','::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen('))) or _name in ('SPen::Pen::SetColor(int)','SPen::PenDrawableRT::SetPenData(float, int)')
 motion_getter=_name.startswith('SPen::MotionEvent::Get') and ('()' in _name or '(int)' in _name)
 addpoint_method=_name.startswith('SPen::InkPen2StrokeDrawableRTV') and '::AddPoint(' in _name
 if oblique_method or motion_getter or addpoint_method:
  method_trace_names_by_pc.setdefault(_address,[]).append(_name)
phase='setup'
screen_resolution_calls=[]
source_size_bits=None
def rt_identity(address):
 if address not in rt_identity_by_address:
  rt_identity_by_address[address]=f'rt{len(rt_identity_by_address)}'
 return rt_identity_by_address[address]

def snapshot_object_stroke(label, obj):
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); size=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); count=p.x(0)
 p.call('SPen::ObjectStroke::IsCurveEnabled() const',[obj]); curve=p.x(0)
 p.call('SPen::ObjectStroke::GetToolType() const',[obj]); tool=p.x(0)
 p.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke=p.x(0)
 def get_i(name):
  p.call(name,[obj]); return p.x(0)
 def get_f(name):
  p.call(name,[obj]); return struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 state={'label':label,'size_bits':size,'point_count':count,'curve_enabled':curve,'tool_type':tool,'stroke_type':stroke,
        'shape':get_i('SPen::ObjectStroke::IsShape() const'),
        'line_type':get_i('SPen::ObjectStroke::GetLineType() const'),
        'fixed_width_enabled':get_i('SPen::ObjectStroke::IsFixedWidthEnabled() const'),
        'fixed_width_bits':get_f('SPen::ObjectStroke::GetFixedWidth() const'),
        'rainbow_enabled':get_i('SPen::ObjectStroke::IsRainbowEffectEnabled() const'),
        'rainbow_distance':get_i('SPen::ObjectStroke::GetRainbowDistance() const'),
        'rainbow_offset_bits':get_f('SPen::ObjectStroke::GetRainbowOffset() const')}
 object_stroke_snapshots.append(state)
 print('OBJECT_STROKE_STATE',state)
 return state

def snapshot_source_arrays(obj):
 p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); count=p.x(0)
 arrays={}
 for name,signature,element_size in (
  ('points','SPen::ObjectStroke::GetPoint() const',8),
  ('pressures','SPen::ObjectStroke::GetPressure() const',4),
  ('timestamps','SPen::ObjectStroke::GetTimeStamp() const',4),
  ('tilts','SPen::ObjectStroke::GetTilt() const',4),
  ('orientations','SPen::ObjectStroke::GetOrientation() const',4),
 ):
  p.call(signature,[obj]); data=p.x(0); size=count*element_size
  if size>1_000_000 or (count and not data): raise RuntimeError(f'invalid {signature} data pointer/count {data:#x}/{count}')
  arrays[name]={'count':count,'element_size':element_size,'data_hex':p.u.mem_read(data,size).hex() if size else ''}
 return arrays

def log_vcall(u,a,z,d):
 if a in pending_motion_getters and pending_motion_getters[a]:
  call=pending_motion_getters[a].pop(0)
  call['return']={'x0':hex(u.reg_read(R.UC_ARM64_REG_X0)),'s0':hex(u.reg_read(R.UC_ARM64_REG_S0)),'d0':hex(u.reg_read(R.UC_ARM64_REG_D0))}
  call['return_pc']=hex(a); call['phase_return']=phase
  native_motion_getter_returns.append(call)
 if a==p.syms.get('createPenInst'):
  factory_call_returns[u.reg_read(R.UC_ARM64_REG_LR)]=True
 if a in factory_call_returns:
  factory_call_returns.pop(a)
  factory_created_pens.append(u.reg_read(R.UC_ARM64_REG_X0))
  print('FACTORY_RETURN pen',hex(factory_created_pens[-1]))
 if a in method_trace_names_by_pc:
  regs={f'x{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)}
  regs.update({f's{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(8)})
  pointers={}
  for i in (0,1,2,3,4):
   ptr=u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))
   try:pointers[f'x{i}_head']=u.mem_read(ptr,96).hex() if ptr else None
   except Exception:pointers[f'x{i}_head']='unmapped'
  record={'signatures':method_trace_names_by_pc[a],'phase':phase,'pc':hex(a),'regs':regs,'pointer_heads':pointers}
  if 'SPen::Pen::SetColor(int)' in method_trace_names_by_pc[a]:
   paint_color_calls.append({'method':'SPen::Pen::SetColor(int)','phase':phase,'entry_pc':hex(a),'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'argb_u32':hex(u.reg_read(R.UC_ARM64_REG_W1))})
  if 'SPen::PenDrawableRT::SetPenData(float, int)' in method_trace_names_by_pc[a]:
   paint_color_calls.append({'method':'SPen::PenDrawableRT::SetPenData(float, int)','phase':phase,'entry_pc':hex(a),'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'size_f32_bits':hex(u.reg_read(R.UC_ARM64_REG_S0)),'argb_u32':hex(u.reg_read(R.UC_ARM64_REG_W1))})
  if any(name.startswith('SPen::InkPen2StrokeDrawableGLV9::RedrawPen(') for name in method_trace_names_by_pc[a]):
   this=u.reg_read(R.UC_ARM64_REG_X0)
   snapshot={'phase':phase,'entry_pc':hex(a),'receiver_address':hex(this),'receiver_role':'primary V9 drawable after native non-virtual thunk adjustment'}
   for label,offset in (('tolerance_plus_44_f32_bits',44),('spacing_plus_88_f32_bits',88),('residual_plus_92_f32_bits',92),('tool_threshold_plus_96_f32_bits',96)):
    snapshot[label]=bytes(u.mem_read(this+offset,4)).hex()
   snapshot['alternation_plus_100_u8']=u.mem_read(this+100,1)[0]
   drawable_entry_snapshots.append(snapshot)
  if any('::drawLine(' in name for name in method_trace_names_by_pc[a]):
   this=u.reg_read(R.UC_ARM64_REG_X0)
   record['drawline_plus44_threshold_f32_bits']=bytes(u.mem_read(this+44,4)).hex()
   pen_ptr=factory_created_pens[-1] if factory_created_pens else 0
   primary_ptr=struct.unpack('<Q',u.mem_read(pen_ptr+112,8))[0] if pen_ptr else 0
   record['drawline_plus96_tool_threshold_f32_bits']=bytes(u.mem_read(this+96,4)).hex()
   drawable_threshold_entries.append({'phase':phase,'entry_pc':hex(a),'receiver_address':hex(this),'primary_drawable_address':hex(primary_ptr) if primary_ptr else None,'receiver_minus_primary':this-primary_ptr if primary_ptr else None,'plus44_tolerance_f32_bits':record['drawline_plus44_threshold_f32_bits'],'plus96_tool_threshold_f32_bits':record['drawline_plus96_tool_threshold_f32_bits']})
  if any('::endPen(' in name for name in method_trace_names_by_pc[a]):
   event_ptr=u.reg_read(R.UC_ARM64_REG_X1)
   record['motion_event_entry_bytes']=u.mem_read(event_ptr,64).hex() if event_ptr else None
  native_draw_entries.append(record)
  for signature in method_trace_names_by_pc[a]:
   if signature.startswith('SPen::MotionEvent::Get'):
    lr=u.reg_read(R.UC_ARM64_REG_LR)
    pending_motion_getters.setdefault(lr,[]).append({'signature':signature,'phase_entry':phase,'entry_pc':hex(a),'return_pc_expected':hex(lr),'args':{f'x{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(4)}})
  print('NATIVE_METHOD_ENTRY',record)
 if a in (p.syms.get('SPen::InkPen2StrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'), p.syms.get('SPen::InkPen2StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float, SPen::Vector3<float>)')):
  this=u.reg_read(R.UC_ARM64_REG_X0); vector_obj=struct.unpack('<Q',u.mem_read(this+80,8))[0]; bounds=struct.unpack('<3Q',u.mem_read(vector_obj,24)) if vector_obj else (0,0,0); begin,end,capacity=bounds; n=end-begin
  if not (begin <= end <= capacity) or n>1_000_000 or n%4: raise RuntimeError(f'invalid InkPen2 callback vector descriptor {bounds!r}')
  record={'phase':phase,'rt':hex(this),'vector_object':hex(vector_obj),'before_count_bytes':n,'before_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)],'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}
  pending_inkpen2_buffer_returns[u.reg_read(R.UC_ARM64_REG_LR)]=(vector_obj,record)
 if a in pending_inkpen2_buffer_returns:
  vector_obj,record=pending_inkpen2_buffer_returns.pop(a); begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); n=end-begin; before=record['before_count_bytes']
  if not (begin<=end<=capacity) or n>1_000_000 or n%4 or n<before: raise RuntimeError(f'invalid InkPen2 callback vector return {begin:#x}/{end:#x}/{capacity:#x}')
  record.update({'after_count_bytes':n,'after_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'delta_bytes':bytes(u.mem_read(begin+before,n-before)).hex() if n>before else ''}); inkpen2_buffer_events.append(record)
p.u.hook_add(UC_HOOK_CODE,log_vcall)
def cpptrace(u,a,z,d):
 if a in (CPP_BASE+0xccc80,CPP_BASE+0xd17fc,CPP_BASE+0xa0058,CPP_BASE+0xa0068,p.syms.get('__cxa_allocate_exception'),p.syms.get('__cxa_throw'),p.syms.get('__cxa_begin_catch')):
  print('CPPTRACE',hex(a),'lr',hex(u.reg_read(R.UC_ARM64_REG_LR)),'x0',hex(u.reg_read(R.UC_ARM64_REG_X0)),'x1',hex(u.reg_read(R.UC_ARM64_REG_X1)),'x2',hex(u.reg_read(R.UC_ARM64_REG_X2)))
p.u.hook_add(UC_HOOK_CODE,cpptrace)
def taskqueue(u,a,z,d):
 if a==0x20001040:
  print('managed register',hex(p.x(0)),hex(p.x(1))); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1); vt=struct.unpack('<Q',u.mem_read(task,8))[0]; method=struct.unpack('<Q',u.mem_read(vt+16,8))[0]; caller=u.reg_read(R.UC_ARM64_REG_LR); print('queue task',hex(p.x(0)),hex(task),hex(vt),hex(method),'words',u.mem_read(task,64).hex())
  if not method: raise RuntimeError('task has null Run')
  p._task_returns.append(caller); p.x(0,task); u.reg_write(R.UC_ARM64_REG_LR,0x20001020); u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:
  caller=p._task_returns.pop(); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)

def screen_trace(u,a,z,d):
 if a==0x10460b4:
  call={'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)}; screen_resolution_calls.append(call); print('Pen::SetScreenResolution entry',call)
p.u.hook_add(UC_HOOK_CODE,screen_trace)
p.u.hook_add(UC_HOOK_CODE,taskqueue)

obj=p.alloc(0x100); impl=p.alloc(0x400)
try:
 p.call('SPen::ObjectStroke::ObjectStroke()', [obj])
 p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj])
 p.put(obj+32,'Q',impl)
 print('model ctors ok',hex(obj),hex(impl),bytes(p.u.mem_read(obj,64)).hex())
 pen=p.alloc(0x400)
 p.call('SPen::InkPen2::InkPen2()', [pen], limit=5000000)
 manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.u.mem_write(p.context_vtable+16,struct.pack('<Q',0x20001040)); p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6')); p.put(qvt+16,'Q',0x20001000); p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue))
 p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager]); p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]); p.put(pen+8,'Q',manager); p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]); print('manager queue',hex(p.x(0)),hex(queue))
 print('allocs first',p._newlog[:30]); print('canvasprobe addrs',{k:hex(getattr(p,k)) for k in ('drawing','context','bitmap','draw_canvas','pen_canvas','oblique','drawable_vtable','pen','canvas_reference')}); print('inkpen2 ctor ok',hex(pen),bytes(p.u.mem_read(pen,0xa0)).hex()); p.drawing=p.alloc(256); p.u.mem_write(p.drawing,bytes(256)); p.put(p.context,'Q',p.context_vtable); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0); p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile)); p.u.mem_write(0x200f20e0,mov_w0_ret(args.width)); p.u.mem_write(0x200f20f0,mov_w0_ret(args.height)); p.put(p.context_vtable+16,'Q',0x200f20d0); p.put(p.context_vtable+80,'Q',0x200f20c0); p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6')); p.put(p.context_vtable+144,'Q',0x200f20a0); p.put(p.context_vtable+152,'Q',0x200f20b0); p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6')); p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6')); p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[p.drawing,p.context]); print('drawing constructed',p.u.mem_read(p.drawing,128).hex()); p.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[p.drawing,p.pen_canvas,pen,0],limit=5000000); d=struct.unpack('<Q',p.u.mem_read(pen+112,8))[0]; print('drawing setIPenBitmap drawable',hex(d),bytes(p.u.mem_read(d,0x180)).hex() if d else None);
 p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]); p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native ObjectStroke GetPenSize S0 bits',source_size_bits); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,input_case['stroke_type']]); source_color_argb=None
 if args.case.endswith('_color'):
  source_color_argb=0xff336699; p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,source_color_argb]); p.call('SPen::ObjectStroke::GetColor() const',[obj]); source_color_getter=p.x(0)
  if source_color_getter!=source_color_argb: raise RuntimeError(f'ObjectStroke::GetColor returned {source_color_getter:#x}, expected {source_color_argb:#x}')
 p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,int(input_case['curve_enabled'])]); print('SetCurveEnabled post-call x0',p.x(0)); points=input_case['points']; count=len(points); pts=p.alloc(count*8); prs=p.alloc(count*4); ts=p.alloc(count*4); tilt=p.alloc(count*4); orient=p.alloc(count*4); p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(prs,struct.pack('<'+'f'*count,*input_case['pressure'])); p.u.mem_write(ts,struct.pack('<'+'i'*count,*input_case['timestamps'])); p.u.mem_write(tilt,struct.pack('<'+'f'*count,*input_case['tilt'])); p.u.mem_write(orient,struct.pack('<'+'f'*count,*input_case['orientation'])); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,orient,count]); rect=p.alloc(32); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.call('SPen::ObjectBase::IsVisible() const',[obj]); print('visible',p.x(0)); p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname initial',hex(p.x(0))); pname=p.alloc(32); p.call('SPen::String::String()',[pname]); p.call('SPen::String::Construct()',[pname]); cstr=p.alloc(80); p.u.mem_write(cstr,b'com.samsung.android.sdk.pen.pen.preload.InkPen2\0'); p.call('SPen::String::Set(char const*)',[pname,cstr]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);
 if args.advanced_setting in ('empty','9'):
  advanced=p.alloc(32); p.call('SPen::String::String()',[advanced]); p.call('SPen::String::Construct()',[advanced]); advanced_cstr=p.alloc(8); p.u.mem_write(advanced_cstr,b'9;\0' if args.advanced_setting=='9' else b'\0'); p.call('SPen::String::Set(char const*)',[advanced,advanced_cstr]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,advanced]);
 p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0); advanced_getter_bytes=p.u.mem_read(advanced_getter,32).hex() if advanced_getter else None
 advanced_utf8=None
 if advanced_getter:
  p.call('SPen::String::GetUTF8Size() const',[advanced_getter]); advanced_size=p.x(0); advanced_buf=p.alloc(max(advanced_size+1,1)); p.call('SPen::String::GetUTF8(char*, int) const',[advanced_getter,advanced_buf,advanced_size+1]); advanced_utf8=bytes(p.u.mem_read(advanced_buf,advanced_size+1)).split(b'\0',1)[0].decode('ascii','replace')
 p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname set',hex(p.x(0)),p.u.mem_read(p.x(0),24).hex()); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('count pre',p.x(0)); print('after setcanvas fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)]); p.call('SPen::PenStrokeDataManager::GetInstance()',[]); print('real cache manager',hex(p.x(0)),p.u.mem_read(p.x(0),64).hex()); cachemanager=p.x(0); p.call('SPen::PenStrokeDataManager::GetStrokeData(int)',[cachemanager,0]); print('cache data runtime0',hex(p.x(0)),p.u.mem_read(p.x(0),32).hex() if p.x(0) else None); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('pointcount',p.x(0),'drawable_fields',[hex(struct.unpack('<Q',p.u.mem_read(d+i,8))[0]) for i in range(0,128,8)]); snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj); phase='draw_object_stroke';
 try: p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 except Exception as error:
  print('DRAW_FAILURE',type(error).__name__,str(error),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'x0_x7',[hex(p.u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)],_always=True); raise
 draw_result=p.x(0); draw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('drawObjectStroke result',p.x(0),'loader calls',loader_calls,'drawer',p.u.mem_read(p.drawing,48).hex());
 if not factory_created_pens:
  print(json.dumps({'draw_result':p.x(0),'stroke_type':args.stroke_type,'factory_count':0,'loader_calls':loader_calls,'redraw_entries':[r for r in native_draw_entries if 'RedrawPen(' in ' '.join(r['signatures'])]},indent=2),_always=True); raise SystemExit(0)
 selected_pen=factory_created_pens[-1]; p.call('SPen::InkPen2::GetStrokeDrawableGL()',[selected_pen]); selected_drawable=p.x(0); selected_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable,8))[0] if selected_drawable else 0; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0] if selected_drawable else 0; selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0] if selected_sub_vptr else 0; selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]; selected_drawable_version=next((re.search(r'InkPen2StrokeDrawableGLV(\d+)',name).group(1) for name in selected_object_redraw_names if re.search(r'InkPen2StrokeDrawableGLV(\d+)',name)), 'unknown'); print('SELECTED_VTABLE',hex(selected_vptr),hex(selected_sub_vptr),hex(selected_object_redraw_target),selected_object_redraw_names); selected_rt=struct.unpack('<Q',p.u.mem_read(selected_drawable+16,8))[0] if selected_drawable else 0; selected_rt_id=rt_identity(selected_rt) if selected_rt else None; print('DRAW_SELECTED_NATIVE pen/drawable/rt',hex(selected_pen),hex(selected_drawable),hex(selected_rt),selected_rt_id); actual_pen=factory_created_pens[-1]; actual_drawable=selected_drawable; print('Drawing-selected redraw pen/drawable',hex(actual_pen),hex(actual_drawable));
 expected_version={'null':'1','empty':'9','9':'9'}[args.advanced_setting]
 if selected_drawable_version!=expected_version: raise RuntimeError(f'{args.advanced_setting} setting selected V{selected_drawable_version}, expected V{expected_version}')
 drawing_entries=[row for row in native_draw_entries if row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
 if not drawing_entries: raise RuntimeError('Drawing did not enter the selected drawable RedrawPen vtable target')
 snapshot_object_stroke('before_saved_RedrawPen',obj); p.call('SPen::Pen::GetSize()', [actual_pen]); pen_size_after_draw_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native Pen::GetSize after draw S0 bits',pen_size_after_draw_bits); phase='saved_redraw'
 if not selected_object_redraw_target: raise RuntimeError('selected drawable has no ObjectStroke RedrawPen vtable slot')
 call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect]); saved_redraw_result=p.x(0); saved_redraw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('redraw result',hex(p.x(0)), 'rect',p.u.mem_read(rect,16).hex()); p.call('SPen::InkPen2StrokeDrawableGL::getSize()', [actual_drawable]); print('native drawable size after redraw S0',struct.unpack('<f',struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)))[0]); inverse_scale_observed=None
 if selected_drawable_version=='9':
  p.call('SPen::InkPen2StrokeDrawableGLV9::getInverseScale()', [actual_drawable]); inverse_scale_observed=[struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(),struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S1)).hex()]; print('native inverse scale after redraw',inverse_scale_observed)
 else: print('inverse scale method not available on selected V'+selected_drawable_version)
 rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]; vp=struct.unpack('<Q',p.u.mem_read(rt+264,8))[0]; print('rt',hex(rt),'vector_ptr',hex(vp),'vector_raw',p.u.mem_read(rt+264,24).hex()); vv=struct.unpack('<3Q',p.u.mem_read(vp,24)) if vp else (0,0,0); print('vector',tuple(hex(v) for v in vv))
 # The native AddPoint return hook copies the complete callback vector before
 # its caller submits/clears it. This is a synthetic explicit context profile.
 import json
 callbacks=[]
 p.call('SPen::InkPen2StrokeDrawableGL::getSize()', [actual_drawable]); drawable_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 inverse_scale_bits=inverse_scale_observed
 drawable_native_state={'identity':'selected drawable returned from actual Drawing factory route','address':hex(actual_drawable),'drawable_vtable':hex(selected_vptr),'drawable_size_f32_bits':drawable_size_bits,'primary_object_plus_44_raw_bytes':bytes(p.u.mem_read(actual_drawable+44,4)).hex(),'inverse_scale_f32_bits':inverse_scale_bits}
 if selected_drawable_version=='9':
  drawable_native_state.update({'spacing_plus_88_f32_bits':bytes(p.u.mem_read(actual_drawable+88,4)).hex(),'residual_plus_92_f32_bits':bytes(p.u.mem_read(actual_drawable+92,4)).hex(),'tool_threshold_plus_96_f32_bits':bytes(p.u.mem_read(actual_drawable+96,4)).hex(),'alternation_plus_100_u8':p.u.mem_read(actual_drawable+100,1)[0]})
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')
 if any(pending_motion_getters.values()): raise RuntimeError(f'MotionEvent getter calls missed native returns: {pending_motion_getters}')
 p.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke=p.x(0)
 native_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()}
 native_hashes['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest()
 native_hashes['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
 traced_methods=[{k:v for k,v in row.items() if k in ('signatures','phase','pc','regs','motion_event_entry_bytes','pointer_heads')} for row in native_draw_entries if any(token in name for name in row['signatures'] for token in ('::RedrawPen(','::endPen(','::drawPoint('))]
 case_id=args.case+'_'+('null_v1' if args.advanced_setting=='null' else ('empty_v9' if args.advanced_setting=='empty' else 'explicit9_v9'))
 result={'schema':'sdocx-inkpen2-saved-caller-case-v1','case_id':case_id,'profile':{'width':args.width,'height':args.height,'canvas_model_matrix_f32_bits':[struct.pack('<f',v).hex() for v in (1,0,0,0,1,0,0,0,1)]},'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.InkPen2','stroke_type':stroke,'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'pen_size_f32_bits':source_size_bits,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']],'advanced_setting_state':args.advanced_setting, 'advanced_setting':('9;' if args.advanced_setting=='9' else ('empty-string' if args.advanced_setting=='empty' else None)), 'advanced_getter_pointer_nonnull':bool(advanced_getter), 'advanced_getter_object_hex':advanced_getter_bytes, 'advanced_getter_utf8':advanced_utf8},'native_calls':{'ObjectDrawing::drawObjectStroke_return':draw_result,'selected_drawable_object_redraw_return':saved_redraw_result,'drawObjectStroke_rect_f32_bits':draw_rect,'separate_saved_RedrawPen_rect_f32_bits':saved_redraw_rect,'route':{'advanced_setting_state':args.advanced_setting, 'advanced_setting':('9;' if args.advanced_setting=='9' else ('empty-string' if args.advanced_setting=='empty' else None)), 'advanced_getter_pointer_nonnull':bool(advanced_getter), 'advanced_getter_object_hex':advanced_getter_bytes, 'advanced_getter_utf8':advanced_utf8,'drawing_selected_drawable_version':selected_drawable_version,'saved_redraw_invoked_version':selected_drawable_version},'source_ObjectStroke_arrays':source_object_arrays,'motion_event_getter_returns':native_motion_getter_returns,'loader':loader_calls,'pen_screen_resolution':screen_resolution_calls,'source_ObjectStroke_GetPenSize_f32_bits':source_size_bits,'ObjectStroke_getter_snapshots':object_stroke_snapshots,'selected_drawable_redraw_lifecycle_method_entries':traced_methods,'selected_drawable_state':drawable_native_state,'drawing_selected_rt_id':selected_rt_id,'pen_GetSize_after_draw_f32_bits':pen_size_after_draw_bits,'drawable_getSize_f32_bits':drawable_size_bits,'drawable_getInverseScale_f32_bits':inverse_scale_bits},'native_method_entries':traced_methods, 'callback_buffer_events':inkpen2_buffer_events,'libraries':native_hashes,'host_boundaries':['ObjectDrawing context vslot +16 supplies the screen-profile object; its vtable +32/+40 returns explicit width/height','context vslot +80 is an explicit identity color-conversion host callback','context vslots +144/+152 return explicit flags/version values 1 and 1.0','canvas/bitmap/context/resource/ownership adapters and managed task queue registration are host; queued task Run methods and Drawing/InkPen2 geometry execute natively'],'scope':'Native Drawing caller plus direct saved RedrawPen under an explicit synthetic profile. This does not exercise SDK deserialization, Samsung device defaults, renderer pixels, or SVG/PDF export.'}
 if source_color_argb is not None:
  result['input']['color_argb_u32']=source_color_argb
  result['native_calls']['source_ObjectStroke_GetColor_u32']=source_color_getter
  result['native_calls']['paint_color_calls']=paint_color_calls
 result['native_calls']['drawline_threshold_entries']=drawable_threshold_entries
 result['native_calls']['selected_drawable_redraw_entry_state']=drawable_entry_snapshots
 result['native_calls']['selected_drawable_object_redraw_entry']=selected_object_redraw_names[0] if selected_object_redraw_names else None
 result['native_calls']['selected_drawable_primary_vtable_id']='InkPen2StrokeDrawableGLV'+selected_drawable_version
 result['scope']=f'Native Drawing caller plus direct RedrawPen through selected GLV{selected_drawable_version} vtable slot under an explicit synthetic profile. No serialized SDocx parser, device-default profile, renderer pixels, or SVG/PDF export.'
 if source_color_argb is not None:
  rgb=tuple(((source_color_argb >> shift)&255)/255.0 for shift in (16,8,0))
  reference=json.loads((ROOT/'conformance/inkpen2-saved.json').read_text())['cases'][0]['native']['rtv3']
  expected_geometry=[struct.pack('<6f',*row[:6]) for row in reference]
  validation={}
  for phase_name in ('draw_object_stroke','saved_redraw'):
   events=[row for row in inkpen2_buffer_events if row['phase']==phase_name]
   raw=bytes.fromhex(events[-1]['after_bytes'])
   if len(raw)%36 or len(raw)//36!=len(reference): raise RuntimeError(f'colored {phase_name} record count mismatch')
   rows=[struct.unpack_from('<9f',raw,index) for index in range(0,len(raw),36)]
   if [struct.pack('<6f',*row[:6]) for row in rows]!=expected_geometry: raise RuntimeError(f'colored {phase_name} geometry differs from numeric reference')
   rgb_bits=[struct.pack('<f',value) for value in rgb]
   if any([struct.pack('<f',value) for value in row[6:]]!=rgb_bits for row in rows): raise RuntimeError(f'colored {phase_name} RGB lanes differ from source ARGB {source_color_argb:#x}')
   validation[phase_name]={'record_count':len(rows),'constant_rgb_f32_bits':[struct.pack('<f',value).hex() for value in rgb],'geometry_first_six_match_numeric_fixture':True}
  if not any(row['method']=='SPen::Pen::SetColor(int)' and row['argb_u32']==hex(source_color_argb) and row['phase']=='draw_object_stroke' for row in paint_color_calls): raise RuntimeError('native Drawing did not forward the source color to Pen::SetColor')
  if not any(row['method']=='SPen::PenDrawableRT::SetPenData(float, int)' and row['argb_u32']==hex(source_color_argb) for row in paint_color_calls): raise RuntimeError('native PenDrawableRT::SetPenData did not receive the source ARGB')
  result['native_calls']['color_contract_validation']=validation
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 if draw_result != 1 or saved_redraw_result != 1:
  raise RuntimeError(f'native saved caller/replay failed: {draw_result}, {saved_redraw_result}')
 expected_version={'null':'1','empty':'9','9':'9'}[args.advanced_setting]
 if selected_drawable_version != expected_version:
  raise RuntimeError(f'{args.advanced_setting} advanced setting selected V{selected_drawable_version}, expected V{expected_version}')
 if not any(row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):
  raise RuntimeError('Drawing did not enter the selected InkPen2 RedrawPen vtable target')
 if not any(row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):
  raise RuntimeError('explicit selected RedrawPen replay was not observed')
 for phase_name in ('draw_object_stroke','saved_redraw'):
  if not [row for row in inkpen2_buffer_events if row['phase']==phase_name]:
   raise RuntimeError(f'missing native RTV AddPoint buffers for {phase_name}')
 draw_terminal=next(row for row in reversed(inkpen2_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes']
 saved_terminal=next(row for row in reversed(inkpen2_buffer_events) if row['phase']=='saved_redraw')['after_bytes']
 if draw_terminal != saved_terminal:
  raise RuntimeError('Drawing callback bytes differ from selected saved RedrawPen replay')
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 if args.write_fixture:
  fixture=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else {'schema':'sdocx-inkpen2-saved-caller-native-v1','cases':{}}
  if fixture.get('schema')!='sdocx-inkpen2-saved-caller-native-v1': raise RuntimeError('unexpected caller fixture schema')
  fixture['cases'][case_id]=result
  FIXTURE.write_text(json.dumps(fixture,indent=2,sort_keys=True)+'\n')
  print(f'wrote {FIXTURE.relative_to(ROOT)} case {case_id}',_always=True)
 elif args.check_fixture:
  if not FIXTURE.exists(): raise SystemExit('InkPen2 native caller fixture missing')
  fixture=json.loads(FIXTURE.read_text())
  if fixture.get('cases',{}).get(case_id)!=result: raise SystemExit(f'native caller fixture mismatch for {case_id}')
  print(f'verified {FIXTURE.relative_to(ROOT)} case {case_id}',_always=True)
 else:
  print(encoded,end='',_always=True)
except Exception:
 raise
