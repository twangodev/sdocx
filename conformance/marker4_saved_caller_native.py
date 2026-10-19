"""Native Model ObjectStroke -> Drawing -> Marker4 saved-caller conformance witness.

Run both setting controls with the project's pinned Unicorn and AArch64 toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/marker4_saved_caller_native.py --check-fixture
"""
import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json,math
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0,str(ROOT/'conformance'))
import oblique_canvas_native as oc
oc.native.HASHES['Marker4']='23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff'
_original_native_init=oc.native.NativeFountain.__init__
def _marker4_native_init(self, library_dir, objdump, additional_libraries=()):
 _original_native_init(self, library_dir, objdump, tuple('Marker4' if item=='ObliquePen' else item for item in additional_libraries))
oc.native.NativeFountain.__init__=_marker4_native_init
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
PINNED_DSO_SHA256={'Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Marker4':'23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
_PIN_PATHS={'Base':LIBS/'libSPenBase.so','Composer':LIBS/'libSPenComposer.so','Drawing':LIBS/'libSPenDrawing.so','FountainPen':LIBS/'libSPenFountainPen.so','Graphics':LIBS/'libSPenGraphics.so','Marker4':LIBS/'libSPenMarker4.so','Model':MODEL,'PenCommon':LIBS/'libSPenPenCommon.so','libSPenComposer.so':LIBS/'libSPenComposer.so','libc++_shared.so':LIBS/'libc++_shared.so'}
for _name,_path in _PIN_PATHS.items():
 _actual=hashlib.sha256(_path.read_bytes()).hexdigest()
 if _actual!=PINNED_DSO_SHA256[_name]: raise SystemExit(f'pinned {_name} DSO SHA-256 mismatch: {_actual}')
FIXTURE=ROOT/'conformance/marker4-saved-caller-native.json'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--advanced-setting',choices=('7','8'),default='7')
parser.add_argument('--pen-name',choices=('Marker4','StraightHighlighter'),default='Marker4')
parser.add_argument('--capture-one',action='store_true',help=argparse.SUPPRESS)
parser.add_argument('--check-fixture',action='store_true')
parser.add_argument('--write-fixture',action='store_true')
parser.add_argument('--size',type=float,default=10.0)
parser.add_argument('--omit-size',action='store_true')
parser.add_argument('--tool',type=int,choices=(0,2),default=2)
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true')
args=parser.parse_args()

def fixture_driver():
 if args.check_fixture and args.write_fixture: raise SystemExit("choose only one fixture mode")
 if not (args.check_fixture or args.write_fixture):
  if args.capture_one: return False
  args.check_fixture=True
 cases=[]
 for setting in ("7","8"):
  for omitted,size in ((True,10.0),(False,0.0),(False,0.1),(False,2.0),(False,10.0)):
   cases.append(("Marker4",setting,2,size,omitted))
 for setting in ("7","8"):
  cases.append(("StraightHighlighter",setting,2,10.0,False))
 for name in ("Marker4","StraightHighlighter"):
  cases.append((name,"8",0,10.0,False))
 helper_paths={"oblique_canvas_native.py":ROOT/"conformance/oblique_canvas_native.py","fountain_native.py":ROOT/"conformance/fountain_native.py","elf_plt.py":ROOT/"conformance/elf_plt.py"}
 helper_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in helper_paths.items()}
 runner_hash=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
 captured=[]
 for name,setting,tool,size,omitted in cases:
  cmd=[sys.executable,str(pathlib.Path(__file__).resolve()),"--capture-one","--pen-name",name,"--advanced-setting",setting,"--tool",str(tool),"--size",str(size)]
  if omitted: cmd.append("--omit-size")
  child=subprocess.run(cmd,cwd=ROOT,check=True,text=True,capture_output=True)
  try: captured.append(json.loads(child.stdout))
  except json.JSONDecodeError as e: raise RuntimeError(f"native capture did not return JSON for {name} V{setting}: {child.stderr[-4000:]}") from e
 if args.write_fixture:
  FIXTURE.write_text(json.dumps({"schema":"marker4-saved-caller-native-v1","runner_sha256":runner_hash,"executed_helper_sha256":helper_hashes,"limits":["Synthetic ObjectStroke; no Samsung document deserialization.","Context/canvas/queue/color host adapters; native Drawing factory, saved RedrawPen and native RT AddPoint entry execute.","AddPoint returns at native entry, so these are logical stamp streams, not raster pixels."],"cases":captured},indent=2,sort_keys=True)+"\n")
  print(f"wrote {FIXTURE.relative_to(ROOT)} with {len(captured)} native caller cases")
 else:
  expected=json.loads(FIXTURE.read_text())
  if expected.get("schema")!="marker4-saved-caller-native-v1" or expected.get("runner_sha256")!=runner_hash or expected.get("executed_helper_sha256")!=helper_hashes or expected.get("cases")!=captured:
   raise SystemExit("Marker4 saved-caller native fixture differs; inspect capture changes, then use --write-fixture explicitly")
  print(f"checked {FIXTURE.relative_to(ROOT)}: {len(captured)} cases match pinned native captures")
 return True

if fixture_driver(): raise SystemExit(0)
if not args.capture_one: raise SystemExit("use --check-fixture, --write-fixture, or internal --capture-one")
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
case_id=f'tool{args.tool}_size{args.size}_omit{int(args.omit_size)}_v{args.advanced_setting}'
input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':args.size,'curve_enabled':True,'tool_type':args.tool,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}
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
def _f32reg(index): return struct.unpack('<f',struct.pack('<I',p.u.reg_read(getattr(R,f'UC_ARM64_REG_S{index}'))))[0]
p.callbacks['atan2f']=lambda:p.f(0,math.atan2(_f32reg(0),_f32reg(1)))
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
 'Graphics':LIBS/'libSPenGraphics.so','Marker4':LIBS/'libSPenMarker4.so',
 'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
 'Graphics':0x4000000,'Marker4':0x5000000,'Composer':0x6000000,'Model':BASE}
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
    elif lib=='Marker4' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))
    if lib=='Marker4' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())

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
 if name!='libSPenMarker4.so': raise RuntimeError(f'unapproved dlopen {name!r}')
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
for path,base in [(CPP,CPP_BASE),(LIBS/'libSPenBase.so',0x2000000),(LIBS/'libSPenPenCommon.so',0x1000000),(MODEL,BASE),(LIBS/'libSPenMarker4.so',0x5000000),(LIBS/'libSPenDrawing.so',0x3000000),(LIBS/'libSPenGraphics.so',0x4000000)]:run_init(path,base)
MODEL_GETTERS=('SPen::ObjectStroke::GetPenName() const','SPen::ObjectStroke::GetAdvancedPenSetting() const','SPen::ObjectStroke::GetPointCount() const','SPen::ObjectStroke::GetPoint() const','SPen::ObjectStroke::GetPressure() const','SPen::ObjectStroke::GetTimeStamp() const','SPen::ObjectStroke::GetTilt() const','SPen::ObjectStroke::GetOrientation() const','SPen::ObjectStroke::GetPenSize() const','SPen::ObjectStroke::GetToolType() const','SPen::ObjectStroke::GetStrokeType() const','SPen::ObjectStroke::GetColor() const')
model_getter_contract={name:{'native_symbol_rva':hex(p.syms[name]-BASE) if name in p.syms else None,'host_callback_present':name in p.callbacks} for name in MODEL_GETTERS}
if any(row['native_symbol_rva'] is None or row['host_callback_present'] for row in model_getter_contract.values()): raise RuntimeError(f'Model getter boundary mismatch: {model_getter_contract}')
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
marker4_buffer_events=[]
drawable_threshold_entries=[]
drawable_entry_snapshots=[]
paint_color_calls=[]
pending_marker4_buffer_returns={}
for _name,_address in p.syms.items():
 oblique_method=('Marker4StrokeDrawableGLV' in _name and any(token in _name for token in ('::Draw(','::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen('))) or _name in ('SPen::Pen::SetColor(int)','SPen::PenDrawableRT::SetPenData(float, int)')
 motion_getter=_name.startswith('SPen::MotionEvent::Get') and ('()' in _name or '(int)' in _name)
 motion_ctor=_name.startswith('SPen::MotionEvent::MotionEvent(')
 addpoint_method=_name.startswith('SPen::Marker4StrokeDrawableRT') and any(token in _name for token in ('::AddPoint(','::ResetPointOffset(','::GetPointOffset('))
 if oblique_method or motion_getter or motion_ctor or addpoint_method:
  method_trace_names_by_pc.setdefault(_address,[]).append(_name)
phase='setup'
screen_resolution_calls=[]
event_order=[]
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
 state={'label':label,'size_bits':size,'point_count':count,'curve_enabled':curve,'tool_type':tool,'stroke_type':stroke,'color_argb':(lambda: (p.call('SPen::ObjectStroke::GetColor() const',[obj]),p.x(0))[1])(),
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
  if any(name.startswith('SPen::Marker4StrokeDrawableGLV4::RedrawPen(SPen::ObjectStroke const*') for name in method_trace_names_by_pc[a]):
   this=u.reg_read(R.UC_ARM64_REG_X0)
   snapshot={'phase':phase,'entry_pc':hex(a),'receiver_address':hex(this),'receiver_role':'selected primary drawable after native non-virtual thunk adjustment'}
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
p.u.hook_add(UC_HOOK_CODE,log_vcall)
marker4_stamps=[]
pen_set_size_entries=[]
pending_pen_set_size_returns={}
marker4_addpoint_targets={}
for _version in (3,4):
 _method=f'SPen::Marker4StrokeDrawableRTV{_version}::AddPoint(SPen::Vector2<float>, float, float)' if _version==3 else 'SPen::Marker4StrokeDrawableRTV4::AddPoint(SPen::Vector2<float>, float)'
 if _method in p.syms: marker4_addpoint_targets[p.syms[_method]]=_method
def capture_marker4_addpoint(u,a,z,d):
 if a==p.syms.get('SPen::Pen::SetSize(float)'):
  record={'phase':phase,'pen':hex(u.reg_read(R.UC_ARM64_REG_X0)),'input_f32_bits':hex(u.reg_read(R.UC_ARM64_REG_S0)),'return_address':hex(u.reg_read(R.UC_ARM64_REG_LR))}
  pen_set_size_entries.append(record); pending_pen_set_size_returns.setdefault(u.reg_read(R.UC_ARM64_REG_LR),[]).append(record)
 if a in pending_pen_set_size_returns and pending_pen_set_size_returns[a]:
  record=pending_pen_set_size_returns[a].pop(0); record['return_x0']=hex(u.reg_read(R.UC_ARM64_REG_X0))
 if a not in marker4_addpoint_targets: return
 lane_count=3 if 'RTV4::AddPoint' in marker4_addpoint_targets[a] else 4
 row={'phase':phase,'method':marker4_addpoint_targets[a],'receiver':hex(u.reg_read(R.UC_ARM64_REG_X0)),'float_registers':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(lane_count)]}
 marker4_stamps.append(row)
 u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,capture_marker4_addpoint)
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
 if a==p.syms.get('SPen::Pen::SetScreenResolution(int, int)'):
  call={'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)}; screen_resolution_calls.append(call); event_order.append({'event':'Pen::SetScreenResolution','phase':phase,'args':call,'pc':hex(a)}); print('Pen::SetScreenResolution entry',call)
p.u.hook_add(UC_HOOK_CODE,screen_trace)
p.u.hook_add(UC_HOOK_CODE,taskqueue)

obj=p.alloc(0x100); impl=p.alloc(0x400)
try:
 p.call('SPen::ObjectStroke::ObjectStroke()', [obj])
 p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj])
 p.put(obj+32,'Q',impl)
 print('model ctors ok',hex(obj),hex(impl),bytes(p.u.mem_read(obj,64)).hex())
 pen=p.alloc(0x400)
 p.call('SPen::Marker4::Marker4()', [pen], limit=5000000)
 manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.u.mem_write(p.context_vtable+16,struct.pack('<Q',0x20001040)); p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6')); p.put(qvt+16,'Q',0x20001000); p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue))
 p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager]); p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]); p.put(pen+8,'Q',manager); p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]); print('manager queue',hex(p.x(0)),hex(queue))
 print('allocs first',p._newlog[:30]); print('canvasprobe addrs',{k:hex(getattr(p,k)) for k in ('drawing','context','bitmap','draw_canvas','pen_canvas','oblique','drawable_vtable','pen','canvas_reference')}); print('marker4 ctor ok',hex(pen),bytes(p.u.mem_read(pen,0xa0)).hex()); p.drawing=p.alloc(256); p.u.mem_write(p.drawing,bytes(256)); p.put(p.context,'Q',p.context_vtable); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0); p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile)); p.u.mem_write(0x200f20e0,mov_w0_ret(args.width)); p.u.mem_write(0x200f20f0,mov_w0_ret(args.height)); p.put(p.context_vtable+16,'Q',0x200f20d0); p.put(p.context_vtable+80,'Q',0x200f20c0); p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6')); p.put(p.context_vtable+144,'Q',0x200f20a0); p.put(p.context_vtable+152,'Q',0x200f20b0); p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6')); p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6')); p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[p.drawing,p.context]); print('drawing constructed',p.u.mem_read(p.drawing,128).hex()); p.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[p.drawing,p.pen_canvas,pen,0],limit=5000000); d=struct.unpack('<Q',p.u.mem_read(pen+112,8))[0]; print('drawing setIPenBitmap drawable',hex(d),bytes(p.u.mem_read(d,0x180)).hex() if d else None);
 if args.omit_size:
  handler=p.alloc(256); input_ptr=p.alloc(8); empty_payload=p.alloc(4); p.call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)',[handler,impl]); p.put(input_ptr,'Q',empty_payload); p.f(0,1.0); p.call('SPen::ObjectStrokeBinaryHandler::m_ApplyBinary_FlexibleData(unsigned char**, int, unsigned int, float) const',[handler,input_ptr,0,0])
  if p.x(0)!=1 or p.get(input_ptr,'Q')[0]!=empty_payload: raise RuntimeError('native empty flexible reader failed')
 else: p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj])
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native ObjectStroke GetPenSize S0 bits',source_size_bits); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,input_case['stroke_type']])
 p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,int(input_case['curve_enabled'])]); print('SetCurveEnabled post-call x0',p.x(0)); points=input_case['points']; count=len(points); pts=p.alloc(count*8); prs=p.alloc(count*4); ts=p.alloc(count*4); tilt=p.alloc(count*4); orient=p.alloc(count*4); p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(prs,struct.pack('<'+'f'*count,*input_case['pressure'])); p.u.mem_write(ts,struct.pack('<'+'i'*count,*input_case['timestamps'])); p.u.mem_write(tilt,struct.pack('<'+'f'*count,*input_case['tilt'])); p.u.mem_write(orient,struct.pack('<'+'f'*count,*input_case['orientation'])); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,orient,count]); rect=p.alloc(32); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.call('SPen::ObjectBase::IsVisible() const',[obj]); print('visible',p.x(0)); p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname initial',hex(p.x(0))); pname=p.alloc(32); p.call('SPen::String::String()',[pname]); p.call('SPen::String::Construct()',[pname]); cstr=p.alloc(80); p.u.mem_write(cstr,('com.samsung.android.sdk.pen.pen.preload.'+args.pen_name).encode('ascii')+b'\0'); p.call('SPen::String::Set(char const*)',[pname,cstr]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);
 if args.advanced_setting in ('7','8'):
  advanced=p.alloc(32); p.call('SPen::String::String()',[advanced]); p.call('SPen::String::Construct()',[advanced]); empty=p.alloc(1); p.u.mem_write(empty,args.advanced_setting.encode('ascii')+b';\0'); p.call('SPen::String::Set(char const*)',[advanced,empty]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,advanced]);
 p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0); advanced_getter_bytes=p.u.mem_read(advanced_getter,32).hex() if advanced_getter else None
 advanced_utf8=None
 if advanced_getter:
  p.call('SPen::String::GetUTF8Size() const',[advanced_getter]); advanced_size=p.x(0); advanced_buf=p.alloc(max(advanced_size+1,1)); p.call('SPen::String::GetUTF8(char*, int) const',[advanced_getter,advanced_buf,advanced_size+1]); advanced_utf8=bytes(p.u.mem_read(advanced_buf,advanced_size+1)).split(b'\0',1)[0].decode('ascii','replace')
 p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname set',hex(p.x(0)),p.u.mem_read(p.x(0),24).hex()); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('count pre',p.x(0)); snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj)
 expected_arrays={'points':struct.pack('<'+'f'*(count*2),*(v for xy in input_case['points'] for v in xy)).hex(),'pressures':struct.pack('<'+'f'*count,*input_case['pressure']).hex(),'timestamps':struct.pack('<'+'i'*count,*input_case['timestamps']).hex(),'tilts':struct.pack('<'+'f'*count,*input_case['tilt']).hex(),'orientations':struct.pack('<'+'f'*count,*input_case['orientation']).hex()}
 for _array,_expected in expected_arrays.items():
  if source_object_arrays[_array]['count']!=count or source_object_arrays[_array]['data_hex']!=_expected: raise RuntimeError(f'ObjectStroke native {_array} storage differs from requested input')
 phase='draw_object_stroke';
 try: p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 except Exception as error:
  print('DRAW_FAILURE',type(error).__name__,str(error),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'x0_x7',[hex(p.u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)],_always=True); raise
 draw_result=p.x(0); draw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('drawObjectStroke result',p.x(0),'loader calls',loader_calls,'drawer',p.u.mem_read(p.drawing,48).hex());
 if draw_result != 1 or not factory_created_pens:
  raise RuntimeError(f'Drawing failed to select Marker4: result={draw_result}, loader={loader_calls}')
 selected_pen=factory_created_pens[-1]; p.call('SPen::Pen::getSettingData()',[selected_pen]); actual_setting_data=p.x(0); setting_size_bits=bytes(p.u.mem_read(actual_setting_data,4)).hex(); setting_resolution=list(struct.unpack('<ii',p.u.mem_read(actual_setting_data+32,8)));
 if setting_resolution != [args.width,args.height]: raise RuntimeError(f'native Pen settings screen mismatch: {setting_resolution}')
 p.call('SPen::Pen::getSettingData()',[selected_pen]);
 if p.x(0)!=actual_setting_data: raise RuntimeError('Pen::getSettingData pointer changed')
 p.call('SPen::Marker4::GetStrokeDrawableGL()',[selected_pen]); selected_drawable=p.x(0); selected_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable,8))[0] if selected_drawable else 0; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0] if selected_drawable else 0; selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0] if selected_sub_vptr else 0; selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]; selected_drawable_version=next((f'v{v}' for v in range(1,10) if any(f'Marker4StrokeDrawableGLV{v}::RedrawPen' in name for name in selected_object_redraw_names)),'unknown'); print('SELECTED_VTABLE',hex(selected_vptr),hex(selected_sub_vptr),hex(selected_object_redraw_target),selected_object_redraw_names); selected_rt=struct.unpack('<Q',p.u.mem_read(selected_drawable+16,8))[0] if selected_drawable else 0; selected_rt_id=rt_identity(selected_rt) if selected_rt else None; print('DRAW_SELECTED_NATIVE pen/drawable/rt',hex(selected_pen),hex(selected_drawable),hex(selected_rt),selected_rt_id); actual_pen=factory_created_pens[-1]; actual_drawable=selected_drawable; print('Drawing-selected redraw pen/drawable',hex(actual_pen),hex(actual_drawable));
 expected_version={'7':'v7','8':'v8'}[args.advanced_setting]
 if selected_drawable_version!=expected_version: raise RuntimeError(f'unexpected selected drawable profile {selected_drawable_version!r}')
 drawing_entries=[row for row in native_draw_entries if row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
 if not drawing_entries: raise RuntimeError('Drawing did not enter the selected drawable RedrawPen vtable target')
 snapshot_object_stroke('before_saved_RedrawPen',obj); p.call('SPen::Marker4StrokeDrawableGL::getSize()', [actual_drawable]); selected_drawable_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); p.call('SPen::Pen::GetSize()', [actual_pen]); pen_size_after_draw_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native Pen::GetSize after draw S0 bits',pen_size_after_draw_bits); phase='saved_redraw'
 if not selected_object_redraw_target: raise RuntimeError('selected drawable has no ObjectStroke RedrawPen vtable slot')
 call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect]); saved_redraw_result=p.x(0); saved_redraw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('redraw result',hex(p.x(0)), 'rect',p.u.mem_read(rect,16).hex());
 selected_rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]
 p.call('SPen::Pen::GetSize()', [actual_pen]); final_pen_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 drawable_native_state={'identity':'selected drawable returned from actual Drawing factory route','address':hex(actual_drawable),'drawable_vtable':hex(selected_vptr),'native_size_getter':'SPen::Pen::GetSize() for selected factory pen','pen_size_f32_bits_after_draw':final_pen_size_bits}
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')
 if any(pending_motion_getters.values()): raise RuntimeError(f'MotionEvent getter calls missed native returns: {pending_motion_getters}')
 p.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke=p.x(0)
 native_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()}
 native_hashes['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest()
 native_hashes['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
 traced_methods=[{k:v for k,v in row.items() if k in ('signatures','phase','pc','regs','motion_event_entry_bytes','pointer_heads')} for row in native_draw_entries if any(token in name for name in row['signatures'] for token in ('::RedrawPen(','::endPen(','::drawPoint('))]
 draw_stream=[row for row in marker4_stamps if row['phase']=='draw_object_stroke']
 saved_stream=[row for row in marker4_stamps if row['phase']=='saved_redraw']
 if not draw_stream or not saved_stream: raise RuntimeError(f'expected RT AddPoint calls in both phases, got {len(draw_stream)} and {len(saved_stream)}')
 normalize=lambda rows:[(row['method'],row['float_registers']) for row in rows]
 if normalize(draw_stream)!=normalize(saved_stream): raise RuntimeError('Drawing and explicit saved RedrawPen stamp streams differ')
 event_summary=[{'event':'Pen::SetScreenResolution','phase':row['phase'],'width':row['args']['width'],'height':row['args']['height']} if row['event']=='Pen::SetScreenResolution' else {'event':row['event'],'phase':row['phase']} for row in event_order]
 result={'schema':'marker4-caller-control-v1','case_id':args.pen_name.lower()+'_tool'+str(args.tool)+'_size'+str(args.size)+'_omit'+str(int(args.omit_size))+'_v'+args.advanced_setting,'profile':{'width':args.width,'height':args.height},'input':{'kind':'synthetic native ObjectStroke setters; no serialized SDocx input','pen_name':'com.samsung.android.sdk.pen.pen.preload.'+args.pen_name,'advanced_setting':args.advanced_setting+';','size_setter_called':not args.omit_size,'omitted_size_materialized_by':'native ObjectStrokeBinaryHandler::m_ApplyBinary_FlexibleData(empty buffer)' if args.omit_size else None,'requested_size':args.size,'native_Model_GetPenSize_f32_bits_before_Drawing':source_size_bits,'tool_type':args.tool,'stroke_type':stroke,'curve_enabled':True,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in input_case['points']],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']]},'route':{'factory_symbol':'createPenInst','selected_pen_address':hex(selected_pen),'selected_drawable_address':hex(actual_drawable),'selected_rt_address':hex(selected_rt),'selected_drawable_vtable':hex(selected_vptr),'selected_drawable_class':selected_object_redraw_names[0] if selected_object_redraw_names else None,'selected_drawable_version':selected_drawable_version,'drawing_result':draw_result,'saved_redraw_result':saved_redraw_result,'native_Pen_GetSize_after_drawing_f32_bits':final_pen_size_bits,'selected_drawable_GetSize_f32_bits':selected_drawable_size_bits,'native_Pen_SetSize_calls':pen_set_size_entries},'source_arrays':source_object_arrays,'object_stroke_snapshots':object_stroke_snapshots,'pen_setting_data':{'size_bits':setting_size_bits,'screen_resolution':setting_resolution},'streams':{'drawObjectStroke_AddPoint_count':len(draw_stream),'saved_RedrawPen_AddPoint_count':len(saved_stream),'drawObjectStroke_sha256':hashlib.sha256(json.dumps(draw_stream,separators=(',',':')).encode()).hexdigest(),'saved_RedrawPen_sha256':hashlib.sha256(json.dumps(saved_stream,separators=(',',':')).encode()).hexdigest(),'normalized_streams_equal':normalize(draw_stream)==normalize(saved_stream),'drawObjectStroke_addpoint_registers':draw_stream,'saved_RedrawPen_addpoint_registers':saved_stream},'libraries':native_hashes,'helper_hashes':{'adapted_from':'conformance/inkpen_saved_caller_native.py','adapted_from_sha256':hashlib.sha256((ROOT/'conformance/inkpen_saved_caller_native.py').read_bytes()).hexdigest(),'runner_sha256':hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),'executed_helpers_sha256':{'oblique_canvas_native.py':hashlib.sha256((ROOT/'conformance/oblique_canvas_native.py').read_bytes()).hexdigest(),'fountain_native.py':hashlib.sha256((ROOT/'conformance/fountain_native.py').read_bytes()).hexdigest(),'elf_plt.py':hashlib.sha256((ROOT/'conformance/elf_plt.py').read_bytes()).hexdigest()},'Model_sha256':hashlib.sha256(MODEL.read_bytes()).hexdigest(),'Marker4_sha256':hashlib.sha256((LIBS/'libSPenMarker4.so').read_bytes()).hexdigest()},'host_boundaries':['Native Model ObjectStroke constructor/setters/getters and native ObjectDrawing::drawObjectStroke execute. Synthetic ObjectStroke style is represented by native setter calls; there is no public Rust Stroke.pen_width in this ABI. Native Marker4 GLV7/GLV8 RedrawPen executes; RT AddPoint is intercepted at its native entry to capture the complete logical stamp stream and return before raster/GPU work. Context resolution, color conversion, queue registration and canvases remain host adapters.'],'scope':'Native Drawing caller control for Marker4 with explicit native Model style setters and settings 7;/8;. This is not SDK deserialization; it does not establish Rust public-width derivation or pixels.'}
 if draw_result != 1 or saved_redraw_result != 1:
  raise RuntimeError(f'native saved caller/replay failed: {draw_result}, {saved_redraw_result}')
 expected_version={'7':'v7','8':'v8'}[args.advanced_setting]
 if selected_drawable_version != expected_version:
  raise RuntimeError(f'unexpected selected drawable profile {selected_drawable_version!r}')
 if not any(row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries): raise RuntimeError('Drawing did not enter selected Marker4 saved RedrawPen target')
 if not any(row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries): raise RuntimeError('explicit Marker4 RedrawPen replay was not observed')
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 print(encoded,end='',_always=True)
except Exception:
 raise
