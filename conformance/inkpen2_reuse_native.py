"""Capture native InkPen2 V9 drawable reuse and fresh controls.

Run with the project's pinned Unicorn and AArch64 toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
    python3 conformance/inkpen2_reuse_native.py --write-fixture --scenario fresh --tool 1
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
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE, UC_MEM_READ, UC_MEM_WRITE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
FIXTURE=ROOT/'conformance/inkpen2-reuse-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group(required=True)
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--print-capture',action='store_true')

parser.add_argument('--scenario',choices=('all','reuse','fresh'),default='all')
parser.add_argument('--first-path',choices=('short','long'),default='short')
parser.add_argument('--tool',type=int,choices=(1,2),default=1,help='tool for the fresh-control stroke')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')

args=parser.parse_args()
if args.scenario=='all':
 action='--write-fixture' if args.write_fixture else ('--check-fixture' if args.check_fixture else '--print-capture')
 runs=[('fresh','--tool','1'),('fresh','--tool','2'),('reuse','--first-path','short'),('reuse','--first-path','long')]
 for scenario,*extra in runs:
  command=[sys.executable,str(pathlib.Path(__file__).resolve()),action,'--scenario',scenario,*extra,'--width',str(args.width),'--height',str(args.height)]
  if args.verbose: command.append('--verbose')
  subprocess.run(command,cwd=ROOT,check=True,env=os.environ.copy())
 raise SystemExit(0)
args.case='line0_curve_short_history'
args.advanced_setting='9'
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
if args.case == 'route_control':
 INPUT_CASES={'route_control': {'points':[[0.0,0.0],[2.0,0.0]],'size':1.0,'curve_enabled':True,'tool_type':0,'stroke_type':0,'pressure':[0.5,0.75],'timestamps':[100,110],'tilt':[0.1,0.2],'orientation':[0.0,0.3]}}
else:
 INPUT_CASES={args.case: {'points':[[0.0,0.0],[8.0,0.0],[16.0,3.0],[24.0,3.0]],'size':4.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[0.2,0.4,0.6,0.8],'timestamps':[0,10,20,30],'tilt':[0.0,0.0,0.0,0.0],'orientation':[0.0,0.0,0.0,0.0]}}
input_case=dict(INPUT_CASES[args.case])
input_case.update({'tool_type':1,'points':[[0.0,0.0],[2.0,0.0]],'pressure':[0.2,0.4],'timestamps':[0,10],'tilt':[0.0]*2,'orientation':[0.0]*2})
if args.scenario=='fresh':
 input_case.update({'tool_type':args.tool,'points':[[0.0,0.0],[8.0,0.0],[16.0,3.0],[24.0,3.0]],'pressure':[0.2,0.4,0.6,0.8],'timestamps':[0,10,20,30],'tilt':[0.0]*4,'orientation':[0.0]*4})
elif args.first_path=='long':
 input_case.update({'tool_type':1,'points':[[0.0,0.0],[8.0,0.0],[16.0,3.0],[24.0,3.0],[32.0,5.0]],'pressure':[0.2,0.4,0.6,0.8,1.0],'timestamps':[0,10,20,30,40],'tilt':[0.0]*5,'orientation':[0.0]*5})
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
redraw_cache_branches=[]
redraw_returns=[]
pending_redraw_returns={}
active_memory_trace=None
watch_drawable=None
redraw_memory_traces=[]
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

def persist_case(case_id, phases, scenario):
 streams={}
 for phase_name in phases:
  events=[row for row in inkpen2_buffer_events if row['phase']==phase_name]
  if not events: raise RuntimeError(f'missing native AddPoint buffers for {phase_name}')
  raw=bytes.fromhex(events[-1]['after_bytes'])
  if len(raw)%36: raise RuntimeError(f'{phase_name} stream is not complete 9-float records')
  streams[phase_name]={'record_count':len(raw)//36,'byte_count':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'full_bytes_hex':raw.hex(),
   'addpoint_returns':[{'before_count_bytes':row['before_count_bytes'],'after_count_bytes':row['after_count_bytes'],'delta_bytes':row['delta_bytes']} for row in events]}
 states=[]
 for row in redraw_returns:
  if row['phase'] not in phases: continue
  states.append({'phase':row['phase'],'receiver':hex(row['receiver']),'entry_alternation_u8':row['entry_alternation'],
   'return_alternation_u8':row['after_alternation'],'entry_fields_f32_bits':row['entry_fields'],'return_fields_f32_bits':row['after_fields']})
 if len(states)!=len(phases): raise RuntimeError(f'expected one native RedrawPen state record for phases {phases}, got {len(states)}')
 result_by_phase={'draw_object_stroke':draw_result}
 if 'second_result' in globals(): result_by_phase['second_tool1_baseline']=second_result
 if 'third_result' in globals(): result_by_phase['third_tool2_baseline']=third_result
 phase_identities={'draw_object_stroke':{'pen':hex(first_pen),'drawable':hex(first_drawable),'rt':hex(first_rt)}}
 if 'second_pen' in globals(): phase_identities['second_tool1_baseline']={'pen':hex(second_pen),'drawable':hex(second_drawable),'rt':hex(second_rt)}
 if 'third_pen' in globals(): phase_identities['third_tool2_baseline']={'pen':hex(third_pen),'drawable':hex(third_drawable),'rt':hex(third_rt)}
 if selected_drawable_version!='9' or not selected_redraw_names: raise RuntimeError('selected drawable vtable is not native V9 RedrawPen')
 case={'schema':'sdocx-inkpen2-reuse-case-v1','case_id':case_id,'scenario':scenario,
  'profile':{'width':args.width,'height':args.height,'advanced_setting':'9;','selected_drawable':'InkPen2StrokeDrawableGLV9'},
  'input':{'kind':'synthetic native ObjectStroke setters; no SDocx deserializer','first_stroke':{'tool_type':input_case['tool_type'],'size_f32_bits':source_size_bits,
   'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in input_case['points']],
   'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps']},
   'baseline_tool1':{'tool_type':1,'points':[[0,0],[8,0],[16,3],[24,3]],'pressure':[0.2,0.4,0.6,0.8],'timestamps':[0,10,20,30]},
   'baseline_tool2':{'tool_type':2,'points':[[0,0],[8,0],[16,3],[24,3]],'pressure':[0.2,0.4,0.6,0.8],'timestamps':[0,10,20,30]}},
  'native':{'draw_results':result_by_phase,'drawable_states':states,'streams':streams,'phase_object_identities':phase_identities,
   'selected_redraw_entries':[{'phase':phase_name,'pc':hex(selected_redraw_target),'symbols':selected_redraw_names} for phase_name in phases],
   'persistent_field_contract':{'offset':100,'width_bytes':1,'constructor_init_va':'0x4cc00','initial_value_u8':0,'redraw_resets_field':False},
   'drawline_runtime_entries':[row for row in drawable_threshold_entries if row['phase'] in phases],
   'screen_resolution_calls':[row for row in screen_resolution_calls],
   'object_identity':{'drawing':hex(p.drawing),'object_stroke':hex(obj),'pen_manager':hex(manager),'pen':hex(first_pen),'drawable':hex(first_drawable),'rt':hex(first_rt)},
   'first_addpoint_watchpoints':[{k:v for k,v in row.items() if k in ('phase','receiver','entry_pc','first_addpoint_pc','first_addpoint_rt','first_addpoint_before_vector','events')} for row in redraw_memory_traces if row['phase'] in phases],
   'watchpoint_extent':'Each receiver memory trace starts at RedrawPen entry and is deliberately stopped by the first RTV3 AddPoint entry. It is not a full-Redraw trace; complete +100 entry/return values come from the native method return snapshots.',
   'cache_branch_trace':[row for row in redraw_cache_branches if row['phase'] in phases],
   'libraries':{name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()},
   'libc++_shared.so':hashlib.sha256(CPP.read_bytes()).hexdigest()},
  'scope':f'Native synthetic ObjectStroke Drawing caller with explicit {args.width}x{args.height} profile and saved advanced setting 9;. Host canvas/context/queue/resource ownership adapters remain. Fresh controls are isolated emulator executions. The causal +100 restoration intervention is not part of these native sequences.'}
 fixture=json.loads(FIXTURE.read_text()) if FIXTURE.exists() else {'schema':'sdocx-inkpen2-reuse-native-v1','cases':{}}
 if fixture.get('schema')!='sdocx-inkpen2-reuse-native-v1': raise RuntimeError('unexpected reuse fixture schema')
 if args.write_fixture:
  fixture['cases'][case_id]=case; FIXTURE.write_text(json.dumps(fixture,indent=2,sort_keys=True)+'\n')
  print(f'wrote {FIXTURE.relative_to(ROOT)} case {case_id}',_always=True)
 elif args.check_fixture:
  if fixture.get('cases',{}).get(case_id)!=case: raise SystemExit(f'native reuse fixture mismatch for {case_id}')
  if args.check_fixture and scenario=='same-Drawing same-drawable reuse':
   cases=fixture['cases']
   for required in ('fresh_tool1','fresh_tool2','short_then_tool1_tool2','long_then_tool1_tool2'):
    if required not in cases: raise SystemExit(f'reuse fixture missing comparison case {required}')
   def digest(case_name, phase_name): return cases[case_name]['native']['streams'][phase_name]['sha256']
   if digest('short_then_tool1_tool2','second_tool1_baseline')==digest('fresh_tool1','draw_object_stroke'):
    raise SystemExit('short-history tool1 witness unexpectedly matches its fresh control')
   if digest('long_then_tool1_tool2','second_tool1_baseline')!=digest('fresh_tool1','draw_object_stroke'):
    raise SystemExit('long-history tool1 output differs from fresh control')
   for reuse_name in ('short_then_tool1_tool2','long_then_tool1_tool2'):
    if digest(reuse_name,'third_tool2_baseline')!=digest('fresh_tool2','draw_object_stroke'):
     raise SystemExit(f'{reuse_name} tool2 output differs from fresh control')
   for reuse_name,expected_first_return in (('short_then_tool1_tool2',1),('long_then_tool1_tool2',0)):
    states=cases[reuse_name]['native']['drawable_states']
    if states[0]['return_alternation_u8']!=expected_first_return or states[1]['entry_alternation_u8']!=expected_first_return:
     raise SystemExit(f'{reuse_name} retained +100 state does not match the native first-stroke contract')
   print('verified fresh/reuse cross-case stream and +100 state controls',_always=True)
  print(f'verified {FIXTURE.relative_to(ROOT)} case {case_id}',_always=True)
 else: print(json.dumps(case,indent=2,sort_keys=True),_always=True)

def log_vcall(u,a,z,d):
 global active_memory_trace
 redraw_sym=p.syms.get('SPen::InkPen2StrokeDrawableGLV9::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)')
 draw_object_pc=p.syms.get('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)')
 if a==draw_object_pc and watch_drawable:
  active_memory_trace={'phase':phase,'receiver':watch_drawable,'draw_object_entry_pc':hex(a),'events':[]}
 if a==redraw_sym:
  if active_memory_trace: active_memory_trace['redraw_entry_pc']=hex(a)
  else: active_memory_trace={'phase':phase,'receiver':u.reg_read(R.UC_ARM64_REG_X0),'entry_pc':hex(a),'events':[]}
 addpoint_trace_pcs={p.syms.get('SPen::InkPen2StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float)'), p.syms.get('SPen::InkPen2StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float, SPen::Vector3<float>)')}
 if a in addpoint_trace_pcs and active_memory_trace:
  active_memory_trace.update({'first_addpoint_pc':hex(a),'first_addpoint_rt':hex(u.reg_read(R.UC_ARM64_REG_X0)),'first_addpoint_before_vector':active_memory_trace['events'][-1]['vector_state'] if active_memory_trace['events'] and 'vector_state' in active_memory_trace['events'][-1] else None})
  redraw_memory_traces.append(active_memory_trace); active_memory_trace=None
 if a in pending_redraw_returns:
  state=pending_redraw_returns.pop(a)
  state.update({'return_pc':hex(a),'return_x0':hex(u.reg_read(R.UC_ARM64_REG_X0)),'after_fields':{str(offset):bytes(u.mem_read(state['receiver']+offset,4)).hex() for offset in (44,88,92,96)},'after_alternation':u.mem_read(state['receiver']+100,1)[0]})
  redraw_returns.append(state)
 redraw_symbol=p.syms.get('SPen::InkPen2StrokeDrawableGLV9::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)')
 if redraw_symbol and a in (redraw_symbol+0x130,redraw_symbol+0x160,redraw_symbol+0x164,redraw_symbol+0x230,redraw_symbol+0x238,redraw_symbol+0x270,redraw_symbol+0x2b4):
  redraw_cache_branches.append({'phase':phase,'pc':hex(a),'offset':hex(a-redraw_symbol),'w0':hex(u.reg_read(R.UC_ARM64_REG_W0)),'w23':hex(u.reg_read(R.UC_ARM64_REG_W23)),'w26':hex(u.reg_read(R.UC_ARM64_REG_W26)),'x0':hex(u.reg_read(R.UC_ARM64_REG_X0)),'x19':hex(u.reg_read(R.UC_ARM64_REG_X19)),'x24':hex(u.reg_read(R.UC_ARM64_REG_X24))})
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
  if any(name.startswith('SPen::InkPen2StrokeDrawableGLV9::RedrawPen(SPen::ObjectStroke') for name in method_trace_names_by_pc[a]):
   this=u.reg_read(R.UC_ARM64_REG_X0)
   ret=u.reg_read(R.UC_ARM64_REG_LR); pending_redraw_returns[ret]={'phase':phase,'entry_pc':hex(a),'receiver':this,'entry_fields':{str(offset):bytes(u.mem_read(this+offset,4)).hex() for offset in (44,88,92,96)},'entry_alternation':u.mem_read(this+100,1)[0]}
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
def trace_drawable_memory(u,access,address,size,value,data):
 if active_memory_trace and active_memory_trace['receiver'] <= address < active_memory_trace['receiver']+0x300:
  selfptr=active_memory_trace['receiver']; row={'kind':'read' if access==UC_MEM_READ else 'write','pc':hex(u.reg_read(R.UC_ARM64_REG_PC)),'offset':address-selfptr,'size':size,'old_hex':bytes(u.mem_read(address,size)).hex()}
  if access==UC_MEM_WRITE: row['new_value_hex']=(int(value)&((1<<(size*8))-1)).to_bytes(size,'little',signed=False).hex()
  active_memory_trace['events'].append(row)
p.u.hook_add(UC_HOOK_MEM_READ|UC_HOOK_MEM_WRITE,trace_drawable_memory)
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
 # Reuse exactly the same ObjectDrawing, ObjectStroke, PenManager, and GL objects.
 first_pen=factory_created_pens[-1]; first_drawable=struct.unpack('<Q',p.u.mem_read(first_pen+112,8))[0]; first_rt=struct.unpack('<Q',p.u.mem_read(first_drawable+16,8))[0]; watch_drawable=redraw_returns[-1]['receiver']
 p.call('SPen::InkPen2::GetStrokeDrawableGL()',[first_pen]); selected_drawable=p.x(0); selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0]; selected_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0]
 selected_redraw_names=[name for name,address in p.syms.items() if address==selected_redraw_target and 'InkPen2StrokeDrawableGLV9' in name and 'RedrawPen' in name]
 selected_drawable_version='9' if selected_redraw_names else 'unknown'
 first_terminal=next(row for row in reversed(inkpen2_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes']
 if draw_result!=1: raise RuntimeError(f'native Drawing failed: {draw_result}')
 if args.scenario=='fresh':
  if args.tool not in (1,2): raise RuntimeError('invalid fresh tool')
  persist_case(f'fresh_tool{args.tool}',('draw_object_stroke',),'fresh-control')
  raise SystemExit(0)
 def set_points(points,pressure,times):
  count=len(points); pt=p.alloc(count*8); pr=p.alloc(count*4); ts=p.alloc(count*4); ti=p.alloc(count*4); ori=p.alloc(count*4)
  p.u.mem_write(pt,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(pr,struct.pack('<'+'f'*count,*pressure)); p.u.mem_write(ts,struct.pack('<'+'i'*count,*times))
  p.u.mem_write(ti,bytes(count*4)); p.u.mem_write(ori,bytes(count*4)); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pt,pr,ts,ti,ori,count])
 baseline_points=[[0.0,0.0],[8.0,0.0],[16.0,3.0],[24.0,3.0]]; baseline_pressure=[0.2,0.4,0.6,0.8]; baseline_times=[0,10,20,30]
 set_points(baseline_points,baseline_pressure,baseline_times); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,1]); snapshot_object_stroke('second_tool1_baseline',obj); phase='second_tool1_baseline'
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000); second_result=p.x(0); second_pen=factory_created_pens[-1]; second_drawable=struct.unpack('<Q',p.u.mem_read(second_pen+112,8))[0]; second_rt=struct.unpack('<Q',p.u.mem_read(second_drawable+16,8))[0]; second_terminal=next(row for row in reversed(inkpen2_buffer_events) if row['phase']=='second_tool1_baseline')['after_bytes']
 set_points(baseline_points,baseline_pressure,baseline_times); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]); snapshot_object_stroke('third_tool2_baseline',obj); phase='third_tool2_baseline'
 p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000); third_result=p.x(0); third_pen=factory_created_pens[-1]; third_drawable=struct.unpack('<Q',p.u.mem_read(third_pen+112,8))[0]; third_rt=struct.unpack('<Q',p.u.mem_read(third_drawable+16,8))[0]; third_terminal=next(row for row in reversed(inkpen2_buffer_events) if row['phase']=='third_tool2_baseline')['after_bytes']
 if any(value!=1 for value in (second_result,third_result)): raise RuntimeError(f'native Drawing reuse failed: {second_result}, {third_result}')
 if (second_pen,second_drawable,second_rt)!=(first_pen,first_drawable,first_rt) or (third_pen,third_drawable,third_rt)!=(first_pen,first_drawable,first_rt):
  raise RuntimeError('Drawing replaced the selected Pen/drawable/RT during the reuse sequence')
 if any(row['receiver']!=watch_drawable for row in redraw_returns if row['phase'] in ('draw_object_stroke','second_tool1_baseline','third_tool2_baseline')):
  raise RuntimeError('native RedrawPen receiver identity changed during sequence')
 first_key='short_then_tool1_tool2' if args.first_path=='short' else 'long_then_tool1_tool2'
 persist_case(first_key,('draw_object_stroke','second_tool1_baseline','third_tool2_baseline'),'same-Drawing same-drawable reuse')
 raise SystemExit(0)
except Exception:
 raise
