"""Native Marker2 Drawing admission and saved-redraw conformance oracle.

A bare --check-fixture validates the exact five-case envelope and replays all
five native profiles. Pass --case to check one profile. Refresh each record
with --write-fixture and the matching profile arguments below. Requires the
project-pinned Unicorn runtime, AArch64 binutils, and pinned Android DSOs; host
canvas/context adapters are part of the recorded boundary.

  marker2_nontrivial_saved3 --advanced-setting 2 --size 10 --tool 2 --curve 1
  marker2_nontrivial_saved3 --advanced-setting 2 --size 10 --tool 0 --curve 1
  marker2_nontrivial_saved3 --advanced-setting 2 --size 0 --tool 1 --curve 0
  marker2_nontrivial_saved3 --advanced-setting 2 --size 0 --tool 1 --curve 1
  marker2_nontrivial_saved3 --advanced-setting null --omit-size --tool 0 --curve 1
"""
import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0,str(ROOT/'conformance'))
import oblique_canvas_native as oc
oc.native.HASHES['Marker2']='f1b2ceea921baac78b722cbd5423732f42d7bec19ccb63d428cf429582412269'
_original_native_init=oc.native.NativeFountain.__init__
def _marker2_native_init(self, library_dir, objdump, additional_libraries=()):
 _original_native_init(self, library_dir, objdump, tuple('Marker2' if item=='ObliquePen' else item for item in additional_libraries))
oc.native.NativeFountain.__init__=_marker2_native_init
class Marker2CanvasProbe(oc.CanvasProbe):
 def __init__(self):
  super().__init__()
  self.marker2=self.oblique
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
FIXTURE=ROOT/'conformance/marker2-saved-caller-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
parser.add_argument('--case',choices=('marker2_linked_saved1','marker2_nontrivial_saved3'),help='select one profile; without --case, --check-fixture replays the complete five-case matrix')
parser.add_argument('--advanced-setting',choices=('null','empty','2'),default='null',help='ObjectStroke advanced setting: null pointer, explicit empty string, or explicit 2;')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--size',type=float,help='override the case pen size before the native ObjectStroke setter')
parser.add_argument('--omit-size',action='store_true',help='leave ObjectStroke at its native constructor size')
parser.add_argument('--tool',type=int,choices=(0,1,2),help='override the saved ObjectStroke tool type')
parser.add_argument('--curve',type=int,choices=(0,1),help='override the saved curve-enabled property')
parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')
args=parser.parse_args()
if args.check_fixture and args.case is None:
 if args.advanced_setting!='null' or args.size is not None or args.omit_size or args.tool is not None or args.curve is not None or args.width!=1920 or args.height!=1080:
  parser.error('profile overrides require --case; bare --check-fixture validates the fixed five-case matrix')
 matrix=[
  ('marker2_nontrivial_saved3','2','10','2','1',False),
  ('marker2_nontrivial_saved3','2','10','0','1',False),
  ('marker2_nontrivial_saved3','2','0','1','0',False),
  ('marker2_nontrivial_saved3','2','0','1','1',False),
  ('marker2_nontrivial_saved3','null',None,'0','1',True),
 ]
 fixture=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else None
 if fixture is None or fixture.get('schema')!='sdocx-marker2-saved-caller-native-v1':
  raise SystemExit('native caller fixture missing or has an unsupported schema')
 expected_slots=set()
 for case,advanced,size,tool,curve,omit in matrix:
  size_key='omit' if omit else struct.pack('<f',float(size)).hex()
  expected_slots.add(f'{case}:{advanced}:tool{tool}:curve{curve}:size{size_key}')
 if set(fixture.get('cases',{}))!=expected_slots:
  raise SystemExit(f'native caller fixture envelope mismatch: expected {sorted(expected_slots)}, found {sorted(fixture.get("cases",{}))}')
 for case,advanced,size,tool,curve,omit in matrix:
  command=[sys.executable,str(pathlib.Path(__file__).resolve()),'--check-fixture','--case',case,'--advanced-setting',advanced,'--tool',tool,'--curve',curve]
  if size is not None: command.extend(['--size',size])
  if omit: command.append('--omit-size')
  subprocess.run(command,check=True,env=os.environ.copy())
 print('verified exact five-case Marker2 caller fixture envelope')
 raise SystemExit(0)
if args.case is None:
 if args.write_fixture: parser.error('--case is required with --write-fixture')
 parser.error('--case is required for this mode')
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
INPUT_CASES={
 'marker2_linked_saved1': {'points':[[0.0,0.0],[20.0,0.0]],'size':8.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[0.05,1.0],'timestamps':[0,8],'tilt':[0.0,0.0],'orientation':[0.0,0.0]},
 'marker2_nontrivial_saved3': {'points':[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]],'size':10.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[1.0,0.05,0.8,0.4],'timestamps':[0,8,16,24],'tilt':[0.0]*4,'orientation':[0.0]*4},
}
input_case=dict(INPUT_CASES[args.case])
if args.tool is not None: input_case['tool_type']=args.tool
if args.curve is not None: input_case['curve_enabled']=bool(args.curve)
requested_size=args.size if args.size is not None else input_case['size']
if not (0 < args.width < 65536 and 0 < args.height < 65536): raise SystemExit('profile dimensions must be positive u16 values')
def mov_w0_ret(value):
 if not 0 <= value <= 0xffff: raise ValueError('screen dimension exceeds MOVZ immediate')
 return struct.pack('<II',0x52800000|(value<<5),0xd65f03c0)
def call_address(address, args, limit=5_000_000):
 for index,value in enumerate(args):p.x(index,value)
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200EF000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
 p.u.emu_start(address,oc.STOP,count=limit)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'address call {address:#x} did not return')
p=Marker2CanvasProbe()
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
 'Graphics':LIBS/'libSPenGraphics.so','Marker2':LIBS/'libSPenMarker2.so',
 'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
 'Graphics':0x4000000,'Marker2':0x5000000,'Composer':0x6000000,'Model':BASE}
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
    elif lib=='Marker2' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))
    if lib=='Marker2' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())

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
 if name!='libSPenMarker2.so': raise RuntimeError(f'unapproved dlopen {name!r}')
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
for path,base in [(CPP,CPP_BASE),(LIBS/'libSPenBase.so',0x2000000),(LIBS/'libSPenPenCommon.so',0x1000000),(MODEL,BASE),(LIBS/'libSPenMarker2.so',0x5000000),(LIBS/'libSPenDrawing.so',0x3000000),(LIBS/'libSPenGraphics.so',0x4000000)]:run_init(path,base)
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
pending_add_returns={}
captured_callbacks=[]
captured_final_vectors={}
object_stroke_snapshots=[]
color_conversion_calls=[]
pen_set_color_calls=[]
factory_call_returns={}
factory_created_pens=[]
phase_rt_addresses={}
rt_identity_by_address={}
selected_rt_id=None
native_draw_entries=[]
native_motion_getter_returns=[]
pending_motion_getters={}
method_trace_names_by_pc={}
marker2_addpoints={p.syms[name]:name for name in ('SPen::Marker2StrokeDrawableRTV1::AddPoint(SPen::Vector2<float>)','SPen::Marker2StrokeDrawableRTV2::AddPoint(SPen::Vector2<float>)')}
pending_marker2_addpoint={}
marker2_addpoint_plt={addr:name for addr,name in p.plt.items() if name in marker2_addpoints.values()}
marker2_drawline_plt={addr:name for addr,name in p.plt.items() if name in ('SPen::Marker2StrokeDrawableGLV1::drawLine(float, float, bool)','SPen::Marker2StrokeDrawableGLV2::drawLine(float, float, bool)')}
pending_marker2_drawline={}
marker2_sample_ends=[]
for _name,_address in p.syms.items():
 marker2_method='Marker2StrokeDrawableGLV' in _name and any(token in _name for token in ('::Draw(','::RedrawPen(','::drawPoint(','::startPen(','::endPen('))
 motion_getter=_name.startswith('SPen::MotionEvent::Get') and ('()' in _name or '(int)' in _name)
 if marker2_method or motion_getter:
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
 state={'label':label,'size_bits':size,'point_count':count,'curve_enabled':curve,'tool_type':tool,'stroke_type':stroke}
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
 if a==0x200f20c0:
  color_conversion_calls.append({'phase':phase,'input_argb_w1':f'0x{u.reg_read(R.UC_ARM64_REG_W1):08x}','result_argb_w0':None})
  u.reg_write(R.UC_ARM64_REG_W0,u.reg_read(R.UC_ARM64_REG_W1))
  u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  color_conversion_calls[-1]['result_argb_w0']=f'0x{u.reg_read(R.UC_ARM64_REG_W0):08x}'
  return
 if a==p.syms.get('SPen::Pen::SetColor(int)'):
  pen_set_color_calls.append({'phase':phase,'color_argb_w1':f'0x{u.reg_read(R.UC_ARM64_REG_W1):08x}'})
 if a in pending_marker2_addpoint:
  this,record=pending_marker2_addpoint.pop(a)
  vec=struct.unpack('<Q',u.mem_read(this+80,8))[0]
  begin,end,cap=struct.unpack('<3Q',u.mem_read(vec,24))
  size=end-begin
  if size>0x100000 or size%8: raise RuntimeError(f'invalid Marker2 RT interleaved coordinate vector {size}')
  flat=list(struct.unpack('<'+'f'*(size//4),u.mem_read(begin,size))) if size else []
  record['vector_count']=len(flat)//2; record['xy']=[flat[index:index+2] for index in range(0,len(flat),2)]
  packed=bytes(u.mem_read(begin,size)) if size else b''
  captured_callbacks.append(record)
  captured_final_vectors[record['phase']]={'vector_count':record['vector_count'],'vector_bytes':packed.hex(),'sha256':hashlib.sha256(packed).hexdigest()}
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
  if any('::endPen(' in name for name in method_trace_names_by_pc[a]):
   event_ptr=u.reg_read(R.UC_ARM64_REG_X1)
   record['motion_event_entry_bytes']=u.mem_read(event_ptr,64).hex() if event_ptr else None
  native_draw_entries.append(record)
  for signature in method_trace_names_by_pc[a]:
   if signature.startswith('SPen::MotionEvent::Get'):
    lr=u.reg_read(R.UC_ARM64_REG_LR)
    pending_motion_getters.setdefault(lr,[]).append({'signature':signature,'phase_entry':phase,'entry_pc':hex(a),'return_pc_expected':hex(lr),'args':{f'x{i}':hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(4)}})
  print('NATIVE_METHOD_ENTRY',record)
 if a in marker2_addpoint_plt:
  this=u.reg_read(R.UC_ARM64_REG_X0); caller=u.reg_read(R.UC_ARM64_REG_LR)
  pending_marker2_addpoint[caller]=(this,{'phase':phase,'rt_id':rt_identity(this),'implementation':marker2_addpoint_plt[a],'xy_bits':[hex(u.reg_read(R.UC_ARM64_REG_S0)),hex(u.reg_read(R.UC_ARM64_REG_S1))]})
 if a in marker2_drawline_plt:
  pending_marker2_drawline[u.reg_read(R.UC_ARM64_REG_LR)]={'phase':phase,'implementation':marker2_drawline_plt[a],'this':u.reg_read(R.UC_ARM64_REG_X0)}
 if a in pending_marker2_drawline:
  row=pending_marker2_drawline.pop(a); gl=row['this']; rt=struct.unpack('<Q',u.mem_read(gl+16,8))[0]; vec=struct.unpack('<Q',u.mem_read(rt+80,8))[0]; begin,end,cap=struct.unpack('<3Q',u.mem_read(vec,24)); size=end-begin
  if size>0x100000 or size%8: raise RuntimeError(f'invalid Marker2 drawLine return vector size {size}')
  marker2_sample_ends.append({'phase':row['phase'],'implementation':row['implementation'],'sample_end':size//8,'gl':hex(gl),'rt':rt_identity(rt)})
 if a==0x5039f74: print('helper entry x0-x5',[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(6)],'memx0',u.mem_read(u.reg_read(R.UC_ARM64_REG_X0),32).hex())
 if a==0x5039fd4:
  x=lambda i:u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}')); print('vcall getRepeat x0',hex(x(0)),'vptr',u.mem_read(x(0),8).hex(),'x9',hex(x(9)),'slot',u.mem_read(x(9)+16,8).hex(),'x19',hex(x(19)),'x20',hex(x(20)),'x24',hex(x(24)))
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
 p.call('SPen::Marker2::Marker2()', [pen], limit=5000000)
 manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.u.mem_write(p.context_vtable+16,struct.pack('<Q',0x20001040)); p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6')); p.put(qvt+16,'Q',0x20001000); p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue))
 p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager]); p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]); p.put(pen+8,'Q',manager); p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]); print('manager queue',hex(p.x(0)),hex(queue))
 print('allocs first',p._newlog[:30]); print('canvasprobe addrs',{k:hex(getattr(p,k)) for k in ('drawing','context','bitmap','draw_canvas','pen_canvas','marker2','drawable_vtable','pen','canvas_reference')}); print('marker2 ctor ok',hex(pen),bytes(p.u.mem_read(pen,0xa0)).hex()); p.drawing=p.alloc(256); p.u.mem_write(p.drawing,bytes(256)); p.put(p.context,'Q',p.context_vtable); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0); p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile)); p.u.mem_write(0x200f20e0,mov_w0_ret(args.width)); p.u.mem_write(0x200f20f0,mov_w0_ret(args.height)); p.put(p.context_vtable+16,'Q',0x200f20d0); p.put(p.context_vtable+80,'Q',0x200f20c0); p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6')); p.put(p.context_vtable+144,'Q',0x200f20a0); p.put(p.context_vtable+152,'Q',0x200f20b0); p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6')); p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6')); p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[p.drawing,p.context]); print('drawing constructed',p.u.mem_read(p.drawing,128).hex()); p.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[p.drawing,p.pen_canvas,pen,0],limit=5000000); d=struct.unpack('<Q',p.u.mem_read(pen+112,8))[0]; print('drawing setIPenBitmap drawable',hex(d),bytes(p.u.mem_read(d,0x180)).hex() if d else None);
 if not args.omit_size:
  p.f(0,requested_size); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj])
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native ObjectStroke GetPenSize S0 bits',source_size_bits); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,input_case['stroke_type']]); p.call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,0xff336699]); p.call('SPen::ObjectStroke::GetColor() const',[obj]); source_color=f'0x{p.x(0):08x}'; p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,int(input_case['curve_enabled'])]); print('SetCurveEnabled post-call x0',p.x(0)); points=input_case['points']; count=len(points); pts=p.alloc(count*8); prs=p.alloc(count*4); ts=p.alloc(count*4); tilt=p.alloc(count*4); orient=p.alloc(count*4); p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(prs,struct.pack('<'+'f'*count,*input_case['pressure'])); p.u.mem_write(ts,struct.pack('<'+'i'*count,*input_case['timestamps'])); p.u.mem_write(tilt,struct.pack('<'+'f'*count,*input_case['tilt'])); p.u.mem_write(orient,struct.pack('<'+'f'*count,*input_case['orientation'])); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,orient,count]); rect=p.alloc(32); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.call('SPen::ObjectBase::IsVisible() const',[obj]); print('visible',p.x(0)); p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname initial',hex(p.x(0))); pname=p.alloc(32); p.call('SPen::String::String()',[pname]); p.call('SPen::String::Construct()',[pname]); cstr=p.alloc(80); p.u.mem_write(cstr,b'com.samsung.android.sdk.pen.pen.preload.Marker2\0'); p.call('SPen::String::Set(char const*)',[pname,cstr]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);
 if args.advanced_setting!='null':
  advanced=p.alloc(32); p.call('SPen::String::String()',[advanced]); p.call('SPen::String::Construct()',[advanced]); advanced_cstr=p.alloc(8); p.u.mem_write(advanced_cstr,{'empty':b'\0','2':b'2;\0'}[args.advanced_setting]); p.call('SPen::String::Set(char const*)',[advanced,advanced_cstr]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,advanced]);
 p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_ptr=p.x(0); advanced_value=None
 if advanced_ptr:
  p.call('SPen::String::GetUTF8Size() const',[advanced_ptr]); advanced_len=p.x(0)
  if not 0<=advanced_len<=4096: raise RuntimeError(f'invalid native advanced-setting length {advanced_len}')
  advanced_buf=p.alloc(advanced_len+1); p.call('SPen::String::GetUTF8(char*, int) const',[advanced_ptr,advanced_buf,advanced_len+1]); advanced_value=bytes(p.u.mem_read(advanced_buf,advanced_len)).split(b'\0',1)[0].decode('utf-8')
 p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname pointer',hex(p.x(0))); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('count pre',p.x(0)); snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj); phase='draw_object_stroke'; p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000); drawing_return=p.x(0); print('drawObjectStroke result',drawing_return,'loader calls',loader_calls,'drawer',p.u.mem_read(p.drawing,48).hex());
 if drawing_return!=1 or not loader_calls or len(factory_created_pens)!=1: raise RuntimeError(f'native Drawing admission did not complete factory path: result={drawing_return}, loader={loader_calls}, factories={len(factory_created_pens)}')
 selected_pen=factory_created_pens[-1]; p.call('SPen::Marker2::GetStrokeDrawableGL()',[selected_pen]); selected_drawable=p.x(0); selected_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable,8))[0] if selected_drawable else 0; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0] if selected_drawable else 0; selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0] if selected_sub_vptr else 0; selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]; selected_drawable_version=next((re.search(r'Marker2StrokeDrawableGLV(\d+)',name).group(1) for name in selected_object_redraw_names if re.search(r'Marker2StrokeDrawableGLV(\d+)',name)), 'unknown'); print('SELECTED_VTABLE',hex(selected_vptr),hex(selected_sub_vptr),hex(selected_object_redraw_target),selected_object_redraw_names); selected_rt=struct.unpack('<Q',p.u.mem_read(selected_drawable+16,8))[0] if selected_drawable else 0; selected_rt_id=rt_identity(selected_rt) if selected_rt else None; print('DRAW_SELECTED_NATIVE pen/drawable/rt/callbacks',hex(selected_pen),hex(selected_drawable),hex(selected_rt),selected_rt_id,[hex(v) for v in phase_rt_addresses.get('draw_object_stroke',[])]); actual_pen=factory_created_pens[-1]; actual_drawable=selected_drawable; print('Drawing-selected redraw pen/drawable',hex(actual_pen),hex(actual_drawable));
 if selected_drawable_version=='unknown':
  raise RuntimeError('Drawing selected an unrecognized Marker2 drawable vtable')
 drawing_entries=[row for row in native_draw_entries if row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
 if not drawing_entries: raise RuntimeError('Drawing did not enter the selected drawable RedrawPen vtable target')
 snapshot_object_stroke('before_saved_RedrawPen',obj); p.call('SPen::Pen::GetSize()', [actual_pen]); pen_size_after_draw_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native Pen::GetSize after draw S0 bits',pen_size_after_draw_bits); phase='saved_redraw'
 if not selected_object_redraw_target: raise RuntimeError('selected drawable has no ObjectStroke RedrawPen vtable slot')
 call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect])
 if p.x(0)!=1: raise RuntimeError(f'selected saved RedrawPen returned {p.x(0)}, expected 1')
 print('redraw result',hex(p.x(0)), 'rect',p.u.mem_read(rect,16).hex()); p.call('SPen::Marker2StrokeDrawableGL::getSize()', [actual_drawable]); print('native drawable size after redraw S0',struct.unpack('<f',struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)))[0]); rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]
 # The native AddPoint return hook copies the complete callback vector before
 # its caller submits/clears it. This is a synthetic explicit context profile.
 import json
 if pending_marker2_addpoint: raise RuntimeError(f'Marker2 RT AddPoint return hooks missed: {pending_marker2_addpoint}')
 if any(len(row['xy']) != row['vector_count'] for row in captured_callbacks): raise RuntimeError('incomplete Marker2 RT x/y vectors')
 callbacks=[{k:v for k,v in row.items() if k not in ('vector_ptr','vector_bounds')} for row in captured_callbacks]
 p.call('SPen::Marker2StrokeDrawableGL::getSize()', [actual_drawable]); drawable_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 inverse_scale_bits=None
 expected_profiles=1
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]*expected_profiles: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')
 if not callbacks or not {'draw_object_stroke','saved_redraw'} <= set(r['phase'] for r in callbacks): raise RuntimeError(f'missing callback phases: {[r["phase"] for r in callbacks]}')
 if any(pending_motion_getters.values()): raise RuntimeError(f'MotionEvent getter calls missed native returns: {pending_motion_getters}')
 if not native_motion_getter_returns or not {'draw_object_stroke','saved_redraw'} <= set(r['phase_entry'] for r in native_motion_getter_returns): raise RuntimeError('missing live MotionEvent getter return evidence')
 # Verify the full interleaved coordinate stream across native Drawing and
 # the selected saved-redraw entry point for every captured profile.
 draw_phase='draw_object_stroke'
 if captured_final_vectors[draw_phase]['vector_bytes']!=captured_final_vectors['saved_redraw']['vector_bytes']: raise RuntimeError('Drawing and selected-vtable saved RedrawPen final callback streams differ')
 expected_version='1' if args.advanced_setting=='null' else '2'
 if selected_drawable_version!=expected_version: raise RuntimeError(f'actual Drawing admission selected V{selected_drawable_version}, expected measured V{expected_version} for setting {args.advanced_setting}')
 native_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()}
 native_hashes['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest()
 native_hashes['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
 traced_methods=[{k:v for k,v in row.items() if k in ('signatures','phase','pc','regs','motion_event_entry_bytes')} for row in native_draw_entries if any(token in name for name in row['signatures'] for token in ('::RedrawPen(','::endPen(','::drawPoint('))]
 source_hashes={name:hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in ('conformance/marker2_saved_caller_native.py','conformance/oblique_canvas_native.py','conformance/fountain_native.py','conformance/elf_plt.py')}
 objdump_path=pathlib.Path(oc.OBJDUMP)
 toolchain={'python':sys.version.split()[0],'unicorn':__import__('unicorn').__version__,'objdump_path':str(objdump_path.relative_to(ROOT)),'objdump_sha256':hashlib.sha256(objdump_path.read_bytes()).hexdigest(),'nm_version':subprocess.check_output(['nm','--version'],text=True,env=env).splitlines()[0]}
 result={'schema':'sdocx-marker2-saved-caller-native-case-v1','harness_and_helper_sha256':source_hashes,'toolchain':toolchain,'case_id':args.case,'profile':{'width':args.width,'height':args.height,'canvas_model_matrix_f32_bits':[struct.pack('<f',v).hex() for v in (1,0,0,0,1,0,0,0,1)]},'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.Marker2','stroke_type':input_case['stroke_type'],'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'pen_size_f32_bits':source_size_bits,'size_input':('omitted' if args.omit_size else requested_size),'color_argb':source_color,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']],'advanced_setting':advanced_value},'native_calls':{'ObjectDrawing::drawObjectStroke_return':1,'selected_drawable_object_redraw_return':1,'route':{'runner_mode':'selected','advanced_setting':advanced_value,'drawing_selected_drawable_version':selected_drawable_version,'saved_redraw_invoked_version':selected_drawable_version},'source_ObjectStroke_arrays':source_object_arrays,'motion_event_getter_returns':native_motion_getter_returns,'loader':loader_calls,'pen_screen_resolution':screen_resolution_calls,'source_ObjectStroke_GetPenSize_f32_bits':source_size_bits,'ObjectStroke_GetColor_argb':source_color,'context_color_conversion_calls':color_conversion_calls,'Pen::SetColor_calls':pen_set_color_calls,'ObjectStroke_getter_snapshots':object_stroke_snapshots,'selected_drawable_redraw_lifecycle_method_entries':traced_methods,'drawing_selected_rt_id':selected_rt_id,'phase_rt_ids':{ph:sorted({rt_identity(v) for v in addresses}) for ph,addresses in phase_rt_addresses.items()},'pen_GetSize_after_draw_f32_bits':pen_size_after_draw_bits,'drawable_getSize_f32_bits':drawable_size_bits,'drawable_getInverseScale_f32_bits':inverse_scale_bits},'callback_add_point_calls':callbacks,'phase_final_vectors':captured_final_vectors,'sample_boundaries_from_native_drawLine_returns':marker2_sample_ends,'libraries':native_hashes,'host_boundaries':['ObjectDrawing context vslot +16 supplies the screen-profile object; its vtable +32/+40 returns explicit width/height','context vslot +80 is a host identity ARGB callback (entry input and returned W0 captured)','context vslots +144/+152 return explicit flags/version values 1 and 1.0','canvas/bitmap/context/resource/ownership adapters and managed task queue registration are host; queued task Run methods, Drawing selection, Marker2 wrapper, and saved geometry execute natively'],'scope':'Native Drawing caller selects the real Marker2 drawable via native factory and invokes its saved ObjectStroke RedrawPen; ObjectStroke is constructed through pinned Model setters and context/profile/queue remain explicit host boundaries. This does not claim SDK deserialization, actual Samsung device defaults, or renderer pixels.'}
 if selected_object_redraw_target:
  result['native_calls']['selected_drawable_object_redraw_entry']=selected_object_redraw_names[0] if selected_object_redraw_names else None
  result['native_calls']['selected_drawable_primary_vtable_id']='Marker2StrokeDrawableGLV'+selected_drawable_version
  result['scope']=f'Native Drawing caller plus direct RedrawPen through the selected GLV{selected_drawable_version} vtable slot under an explicit synthetic profile. This does not exercise SDK deserialization, Samsung device defaults, renderer pixels, or SVG/PDF export.'
 result['native_calls']['ObjectStroke_GetAdvancedPenSetting']={'is_null':advanced_ptr==0,'utf8':advanced_value}
 result['native_calls']['Drawing_factory_admission']={'requested_canonical_pen_name':'com.samsung.android.sdk.pen.pen.preload.Marker2','loader_calls':loader_calls,'factory_created_pen_count':len(factory_created_pens),'factory_returned_pen':hex(selected_pen),'selected_drawable_object':hex(selected_drawable),'selected_vtable_entry':selected_object_redraw_names,'selected_marker2_version_actual':selected_drawable_version}
 result['runtime_context']={'profile_size': [args.width,args.height], 'canvas_model_matrix_f32':[1,0,0,0,1,0,0,0,1], 'profile_dimensions_and_context_flags':'host-provided explicitly; not device defaults', 'color_conversion':'identity host callback'}
 if source_color!='0xff336699': raise RuntimeError(f'native ObjectStroke color getter changed ARGB: {source_color}')
 if not color_conversion_calls or any(row['input_argb_w1']!=source_color or row['result_argb_w0']!=source_color for row in color_conversion_calls): raise RuntimeError(f'native caller did not preserve ARGB through explicit identity color callback: {color_conversion_calls}')
 if not pen_set_color_calls or pen_set_color_calls[-1]['color_argb_w1']!=source_color: raise RuntimeError(f'native Pen::SetColor did not finish with the source ARGB: {pen_set_color_calls}')
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 if args.write_fixture:
  if FIXTURE.exists():
   old=json.loads(FIXTURE.read_text())
   if old.get('schema')!='sdocx-marker2-saved-caller-native-v1': raise SystemExit('refusing to append to a foreign fixture schema')
   cases=old.get('cases',{})
  else: cases={}
  slot=f'{args.case}:{args.advanced_setting}:tool{input_case["tool_type"]}:curve{int(input_case["curve_enabled"])}:size{"omit" if args.omit_size else struct.pack("<f",requested_size).hex()}'
  cases[slot]=result
  FIXTURE.write_text(json.dumps({'schema':'sdocx-marker2-saved-caller-native-v1','cases':cases},indent=2,sort_keys=True)+'\n')
  print(f'wrote {FIXTURE.relative_to(ROOT)} case {slot}',_always=True)
 elif args.check_fixture:
  if not FIXTURE.exists(): raise SystemExit('native caller fixture missing')
  fixture=json.loads(FIXTURE.read_text())
  slot=f'{args.case}:{args.advanced_setting}:tool{input_case["tool_type"]}:curve{int(input_case["curve_enabled"])}:size{"omit" if args.omit_size else struct.pack("<f",requested_size).hex()}'
  expected=fixture.get('cases',{}).get(slot)
  if expected!=result: raise SystemExit(f'native caller fixture mismatch for {slot}')
  print(f'verified {FIXTURE.relative_to(ROOT)} case {args.case}',_always=True)
 else: print(encoded,end='',_always=True)
except Exception:
 raise
