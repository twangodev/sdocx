"""Native Model ObjectStroke -> Drawing -> Pencil3 version-selected caller witness.

Run both setting controls with the project's pinned Unicorn and AArch64 toolchain:
  PYTHONPATH=scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin \
  LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu \
    python3 conformance/pencil3_saved_caller_native.py
"""
import sys,pathlib,hashlib,struct,subprocess,re,os,argparse,json
ROOT=pathlib.Path(__file__).resolve().parents[1]
os.chdir(ROOT)
sys.path.insert(0,str(ROOT/'conformance'))
import oblique_canvas_native as oc
oc.native.HASHES['Pencil3']='41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359'
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_CODE
LIBS=ROOT/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
FIXTURE=ROOT/'conformance/pencil3-saved-caller-native.json'
parser=argparse.ArgumentParser(description=__doc__)
mode=parser.add_mutually_exclusive_group()
mode.add_argument('--print-capture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
mode.add_argument('--write-fixture',action='store_true')

parser.add_argument('--advanced-setting',choices=('null','empty'),default='null')
parser.add_argument('--width',type=int,default=1920)
parser.add_argument('--height',type=int,default=1080)
parser.add_argument('--verbose',action='store_true',help='print native setup and instruction-boundary diagnostics')

args=parser.parse_args()
if not (args.print_capture or args.check_fixture or args.write_fixture):
 args.check_fixture=True
if args.check_fixture or args.write_fixture:
 cases={}
 for setting in ('null','empty'):
  command=[sys.executable,str(pathlib.Path(__file__).resolve()),'--print-capture','--advanced-setting',setting,'--width',str(args.width),'--height',str(args.height)]
  run=subprocess.run(command,cwd=ROOT,env=os.environ.copy(),text=True,capture_output=True,check=True)
  cases['straight_curve_tool2_'+setting]=json.loads(run.stdout)
 fixture_data={'schema':'sdocx-pencil3-saved-caller-native-fixture-v1','cases':cases}
 if args.write_fixture:
  FIXTURE.write_text(json.dumps(fixture_data,indent=2,sort_keys=True)+'\n')
  print('wrote '+str(FIXTURE.relative_to(ROOT)))
 else:
  if not FIXTURE.exists(): raise SystemExit('Pencil3 native saved-caller fixture missing; use --write-fixture to create it')
  existing=json.loads(FIXTURE.read_text())
  if existing!=fixture_data: raise SystemExit('Pencil3 native saved-caller fixture mismatch')
  print('verified Pencil3 null/V1 and empty/V3 native caller cases')
 raise SystemExit(0)
import builtins
_console_print=builtins.print
def print(*values,**kwargs):
 always=kwargs.pop('_always',False)
 if args.verbose or always:_console_print(*values,**kwargs)
case_id='straight_curve_tool2_'+args.advanced_setting
input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}
if not (0 < args.width < 65536 and 0 < args.height < 65536): raise SystemExit('profile dimensions must be positive u16 values')
def mov_w0_ret(value):
 if not 0 <= value <= 0xffff: raise ValueError('screen dimension exceeds MOVZ immediate')
 return struct.pack('<II',0x52800000|(value<<5),0xd65f03c0)
def call_address(address, args, limit=5_000_000):
 for index,value in enumerate(args):p.x(index,value)
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200EF000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
 p.u.emu_start(address,oc.STOP,count=limit)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'address call {address:#x} did not return')
class PencilCanvasProbe(oc.CanvasProbe):
 def __init__(self):
  # Load the Pencil3 DSO explicitly; do not reuse the ObliquePen slot from
  # the inherited probe constructor.
  oc.native.NativeFountain.__init__(self,oc.LIBRARY_DIR,oc.OBJDUMP,('Drawing','Graphics','Pencil3'))
  self.load_composer()
  self.draw_object_base=self.syms[oc.DRAW_OBJECT]-0x80100
  self.composer_base=self.syms['SPen::ObjectStrokePdfExporter::ExportObject()']-0x34F684
  self.set_matrix_calls=0; self.get_matrix_calls=0
  self.drawing=self.alloc(128)
  self.context,self.context_vtable=self.alloc(32),self.alloc(256)
  self.bitmap,self.bitmap_vtable=self.alloc(128),self.alloc(256)
  self.draw_canvas=self.make_canvas(); self.pen_canvas=self.make_canvas()
  self.oblique=self.alloc(512); self.drawable_vtable=self.alloc(256)
  self.u.mem_write(self.drawing,bytes(128)); self.u.mem_write(self.oblique,bytes(512))
  self.u.mem_write(self.oblique,struct.pack('<Q',self.drawable_vtable))
  self.u.mem_write(self.drawable_vtable+16,struct.pack('<Q',self.syms[oc.PEN_COMMON_SET_CANVAS]))
  self.u.mem_write(self.drawable_vtable+200,struct.pack('<Q',oc.STUB_CANVAS_TAG))
  self.pen,self.pen_vtable=self.alloc(256),self.alloc(256)
  self.u.mem_write(self.pen,struct.pack('<Q',self.pen_vtable))
  self.u.mem_write(self.pen_vtable+232,struct.pack('<Q',oc.STUB_GET_DRAWABLE))
  self.u.mem_write(self.context,struct.pack('<Q',self.context_vtable))
  self.u.mem_write(self.bitmap,struct.pack('<Q',self.bitmap_vtable))
  self.u.mem_write(self.context_vtable+24,struct.pack('<Q',oc.STUB_GET_CONTEXT))
  self.u.mem_write(self.bitmap_vtable+24,struct.pack('<Q',oc.STUB_GET_CONTEXT))
  self.canvas_reference,reference_vtable=self.alloc(32),self.alloc(128)
  self.u.mem_write(self.canvas_reference,struct.pack('<Q',reference_vtable))
  self.u.mem_write(reference_vtable+72,struct.pack('<Q',oc.STUB_REFERENCE_RELEASE))
  self.u.mem_write(reference_vtable+80,struct.pack('<Q',oc.STUB_REFERENCE_RELEASE))
  for canvas in (self.draw_canvas,self.pen_canvas):
   vtable=struct.unpack('<Q',self.u.mem_read(canvas,8))[0]
   self.u.mem_write(vtable+184,struct.pack('<Q',oc.STUB_CANVAS_DATA))
   self.u.mem_write(vtable+192,struct.pack('<Q',oc.STUB_CANVAS_REFERENCE))
   self.u.mem_write(vtable+200,struct.pack('<Q',oc.STUB_CANVAS_TAG))
  self.u.mem_write(self.u.reg_read(R.UC_ARM64_REG_TPIDR_EL0)+40,struct.pack('<Q',0x123456789ABCDEF0))
  self.callbacks.update({
   'SPen::SPGraphicsFactory::CreateCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)':lambda:self.x(0,self.draw_canvas),
   'SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)':lambda:self.x(0,self.pen_canvas),
   'SPen::SPGraphicsFactory::ReleaseCanvas(SPen::ISPCanvasBase*)':lambda:self.x(0,0),
   'SPen::ObjectBase::GetType() const':lambda:self.x(0,0xFFFF),
   'SPen::ObjectBase::IsVisible() const':lambda:self.x(0,0),
   '__android_log_print':lambda:self.x(0,0),
   'memcmp':self.memcmp,
  })
  self.u.hook_add(UC_HOOK_CODE,self.hook_canvas)
p=PencilCanvasProbe()
import pencil_native as _pn
_rng=_pn.BionicRandom(); _rng_trace=[]
def _seed_pencil_rng():
 _seed=p.x(0); _rng.seed(_seed); _rng_trace.append({'phase':phase,'function':'srand','seed':_seed})
def _next_pencil_rng():
 _value=_rng.next(); _rng_trace.append({'phase':phase,'function':'rand','value':_value}); return p.x(0,_value)
p.callbacks['srand']=_seed_pencil_rng; p.callbacks['rand']=_next_pencil_rng
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
 'Graphics':LIBS/'libSPenGraphics.so','Pencil3':LIBS/'libSPenPencil3.so',
 'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
 'Graphics':0x4000000,'Pencil3':0x5000000,'Composer':0x6000000,'Model':BASE}
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
    elif lib=='Pencil3' and where in (0x6c278,0x6c280):print('unresolved',hex(where),name,hex(val),hex(typ))
    if lib=='Pencil3' and where in (0x6c278,0x6c280):print('reloc',hex(where),name,hex(target),hex(base+where),p.u.mem_read(base+where,8).hex())

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
 if name!='libSPenPencil3.so': raise RuntimeError(f'unapproved dlopen {name!r}')
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
for path,base in [(CPP,CPP_BASE),(LIBS/'libSPenBase.so',0x2000000),(LIBS/'libSPenPenCommon.so',0x1000000),(MODEL,BASE),(LIBS/'libSPenPencil3.so',0x5000000),(LIBS/'libSPenDrawing.so',0x3000000),(LIBS/'libSPenGraphics.so',0x4000000)]:run_init(path,base)
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
factory_return_snapshots=[]
rt_identity_by_address={}
selected_rt_id=None
native_draw_entries=[]
native_motion_getter_returns=[]
pending_motion_getters={}
method_trace_names_by_pc={}
pencil_buffer_events=[]
drawable_threshold_entries=[]
drawable_entry_snapshots=[]
paint_color_calls=[]
pending_pencil_buffer_returns={}
for _name,_address in p.syms.items():
 oblique_method=('Pencil3StrokeDrawableGLV' in _name and any(token in _name for token in ('::Draw(','::RedrawPen(','::drawLine(','::drawPoint(','::startPen(','::endPen('))) or _name in ('SPen::Pen::SetColor(int)','SPen::PenDrawableRT::SetPenData(float, int)')
 motion_getter=_name.startswith('SPen::MotionEvent::Get') and ('()' in _name or '(int)' in _name)
 motion_ctor=_name.startswith('SPen::MotionEvent::MotionEvent(')
 addpoint_method=_name.startswith('SPen::Pencil3StrokeDrawableRT') and any(token in _name for token in ('::AddPoint(','::ResetPointOffset(','::GetPointOffset('))
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
  pen=factory_created_pens[-1]
  # Capture native constructor state before Drawing applies ObjectStroke data.
  # This is raw memory evidence; it is not interpreted as a user/device default.
  factory_return_snapshots.append({'pen':hex(pen),'first_160_bytes':bytes(u.mem_read(pen,160)).hex()})
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
  if any(name.startswith('SPen::Pencil3StrokeDrawableGLV') and '::RedrawPen(SPen::ObjectStroke const*' in name for name in method_trace_names_by_pc[a]):
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
 if a==p.syms.get('SPen::Pencil3StrokeDrawableRTV1::ResetPointOffset()'):
  event_order.append({'event':'ResetPointOffset','phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'pc':hex(a)})
 if a==p.syms.get('SPen::Pencil3StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)'):
  event_order.append({'event':'AddPoint','phase':phase,'this':hex(u.reg_read(R.UC_ARM64_REG_X0)),'pc':hex(a)})
  this=u.reg_read(R.UC_ARM64_REG_X0); vector_obj=struct.unpack('<Q',u.mem_read(this+80,8))[0]; bounds=struct.unpack('<3Q',u.mem_read(vector_obj,24)) if vector_obj else (0,0,0); begin,end,capacity=bounds; n=end-begin
  if not (begin <= end <= capacity) or n>1_000_000 or n%4: raise RuntimeError(f'invalid Pencil3 callback vector descriptor {bounds!r}')
  record={'phase':phase,'rt':hex(this),'vector_object':hex(vector_obj),'before_count_bytes':n,'before_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'float_args':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))) for i in range(4)],'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}
  pending_pencil_buffer_returns[u.reg_read(R.UC_ARM64_REG_LR)]=(vector_obj,record)
 if a in pending_pencil_buffer_returns:
  vector_obj,record=pending_pencil_buffer_returns.pop(a); begin,end,capacity=struct.unpack('<3Q',u.mem_read(vector_obj,24)); n=end-begin; before=record['before_count_bytes']
  if not (begin<=end<=capacity) or n>1_000_000 or n%4 or n<before: raise RuntimeError(f'invalid Pencil3 callback vector return {begin:#x}/{end:#x}/{capacity:#x}')
  record.update({'after_count_bytes':n,'after_bytes':bytes(u.mem_read(begin,n)).hex() if n else '', 'delta_bytes':bytes(u.mem_read(begin+before,n-before)).hex() if n>before else ''}); pencil_buffer_events.append(record)
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
 if a==p.syms.get('SPen::Pen::SetScreenResolution(int, int)'):
  call={'width':u.reg_read(R.UC_ARM64_REG_W1),'height':u.reg_read(R.UC_ARM64_REG_W2)}; screen_resolution_calls.append(call); event_order.append({'event':'Pen::SetScreenResolution','phase':phase,'args':call,'pc':hex(a)}); print('Pen::SetScreenResolution entry',call)
p.u.hook_add(UC_HOOK_CODE,screen_trace)
p.u.hook_add(UC_HOOK_CODE,taskqueue)

obj=p.alloc(0x100); impl=p.alloc(0x400)
try:
 p.call('SPen::ObjectStroke::ObjectStroke()', [obj])
 p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj])
 p.put(obj+32,'Q',impl)
 p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); model_default_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 p.call('SPen::ObjectStroke::GetToolType() const',[obj]); model_default_tool_type=p.x(0)
 print('model ctors ok',hex(obj),hex(impl),bytes(p.u.mem_read(obj,64)).hex())
 pen=p.alloc(0x400)
 p.call('SPen::Pencil3::Pencil3()', [pen], limit=5000000)
 p.call('SPen::Pen::GetSize()', [pen])
 pencil_ctor_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.u.mem_write(p.context_vtable+16,struct.pack('<Q',0x20001040)); p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6')); p.put(qvt+16,'Q',0x20001000); p._task_returns=[]; p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:(print('GMSGQ',hex(p.x(0)),hex(queue)),p.x(0,queue))
 p.call('SPen::PenGLDataManager::PenGLDataManager()', [manager]); p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]); p.put(pen+8,'Q',manager); p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]); print('manager queue',hex(p.x(0)),hex(queue))
 print('allocs first',p._newlog[:30]); print('canvasprobe addrs',{k:hex(getattr(p,k)) for k in ('drawing','context','bitmap','draw_canvas','pen_canvas','oblique','drawable_vtable','pen','canvas_reference')}); print('Pencil3 ctor ok',hex(pen),bytes(p.u.mem_read(pen,0xa0)).hex()); p.drawing=p.alloc(256); p.u.mem_write(p.drawing,bytes(256)); p.put(p.context,'Q',p.context_vtable); profile=p.alloc(64); profile_vt=p.alloc(64); p.put(profile,'Q',profile_vt); p.put(profile_vt+32,'Q',0x200f20e0); p.put(profile_vt+40,'Q',0x200f20f0); p.u.mem_write(0x200f20d0,bytes.fromhex('40000058c0035fd6')+struct.pack('<Q',profile)); p.u.mem_write(0x200f20e0,mov_w0_ret(args.width)); p.u.mem_write(0x200f20f0,mov_w0_ret(args.height)); p.put(p.context_vtable+16,'Q',0x200f20d0); p.put(p.context_vtable+80,'Q',0x200f20c0); p.u.mem_write(0x200f20c0,bytes.fromhex('e003012ac0035fd6')); p.put(p.context_vtable+144,'Q',0x200f20a0); p.put(p.context_vtable+152,'Q',0x200f20b0); p.u.mem_write(0x200f20a0,bytes.fromhex('20008052c0035fd6')); p.u.mem_write(0x200f20b0,bytes.fromhex('00102e1ec0035fd6')); p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]); p.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)',[p.drawing,p.context]); print('drawing constructed',p.u.mem_read(p.drawing,128).hex()); p.call('SPen::ObjectDrawing::setIPenBitmap(SPen::ISPPenCanvas*, SPen::IPen*, SPen::StrokeType)',[p.drawing,p.pen_canvas,pen,0],limit=5000000); d=struct.unpack('<Q',p.u.mem_read(pen+112,8))[0]; print('drawing setIPenBitmap drawable',hex(d),bytes(p.u.mem_read(d,0x180)).hex() if d else None);
 p.f(0,input_case['size']); p.call('SPen::ObjectStroke::SetPenSize(float)',[obj]); p.call('SPen::ObjectStroke::GetPenSize() const',[obj]); source_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native ObjectStroke GetPenSize S0 bits',source_size_bits); p.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,input_case['tool_type']]); p.call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,input_case['stroke_type']])
 p.call('SPen::ObjectStroke::SetCurveEnabled(bool)',[obj,int(input_case['curve_enabled'])]); print('SetCurveEnabled post-call x0',p.x(0)); points=input_case['points']; count=len(points); pts=p.alloc(count*8); prs=p.alloc(count*4); ts=p.alloc(count*4); tilt=p.alloc(count*4); orient=p.alloc(count*4); p.u.mem_write(pts,struct.pack('<'+'f'*(count*2),*(v for xy in points for v in xy))); p.u.mem_write(prs,struct.pack('<'+'f'*count,*input_case['pressure'])); p.u.mem_write(ts,struct.pack('<'+'i'*count,*input_case['timestamps'])); p.u.mem_write(tilt,struct.pack('<'+'f'*count,*input_case['tilt'])); p.u.mem_write(orient,struct.pack('<'+'f'*count,*input_case['orientation'])); p.call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,orient,count]); rect=p.alloc(32); p.call('SPen::ObjectBase::SetVisibility(bool)',[obj,1]); p.call('SPen::ObjectBase::IsVisible() const',[obj]); print('visible',p.x(0)); p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname initial',hex(p.x(0))); pname=p.alloc(32); p.call('SPen::String::String()',[pname]); p.call('SPen::String::Construct()',[pname]); cstr=p.alloc(80); p.u.mem_write(cstr,b'com.samsung.android.sdk.pen.pen.preload.Pencil3\0'); p.call('SPen::String::Set(char const*)',[pname,cstr]); p.call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,pname]);
 if args.advanced_setting=='empty':
  advanced=p.alloc(32); p.call('SPen::String::String()',[advanced]); p.call('SPen::String::Construct()',[advanced]); empty=p.alloc(1); p.u.mem_write(empty,b'\0'); p.call('SPen::String::Set(char const*)',[advanced,empty]); p.call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,advanced]);
 p.call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_getter=p.x(0); advanced_getter_bytes=p.u.mem_read(advanced_getter,32).hex() if advanced_getter else None
 advanced_utf8=None
 if advanced_getter:
  p.call('SPen::String::GetUTF8Size() const',[advanced_getter]); advanced_size=p.x(0); advanced_buf=p.alloc(max(advanced_size+1,1)); p.call('SPen::String::GetUTF8(char*, int) const',[advanced_getter,advanced_buf,advanced_size+1]); advanced_utf8=bytes(p.u.mem_read(advanced_buf,advanced_size+1)).split(b'\0',1)[0].decode('ascii','replace')
 p.call('SPen::ObjectStroke::GetPenName() const',[obj]); print('penname set',hex(p.x(0)),p.u.mem_read(p.x(0),24).hex()); p.call('SPen::ObjectStroke::GetPointCount() const',[obj]); print('count pre',p.x(0)); snapshot_object_stroke('before_drawObjectStroke',obj); source_object_arrays=snapshot_source_arrays(obj); phase='draw_object_stroke';
 try: p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)
 except Exception as error:
  print('DRAW_FAILURE',type(error).__name__,str(error),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'x0_x7',[hex(p.u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)],_always=True); raise
 draw_result=p.x(0); draw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('drawObjectStroke result',p.x(0),'loader calls',loader_calls,'drawer',p.u.mem_read(p.drawing,48).hex());
 if draw_result != 1 or not factory_created_pens:
  raise RuntimeError(f'Drawing failed to select Pencil3: result={draw_result}, loader={loader_calls}')
 selected_pen=factory_created_pens[-1]; p.call('SPen::Pen::getSettingData()',[selected_pen]); actual_setting_data=p.x(0); setting_size_bits=bytes(p.u.mem_read(actual_setting_data,4)).hex(); setting_resolution=list(struct.unpack('<ii',p.u.mem_read(actual_setting_data+32,8)));
 if setting_resolution != [args.width,args.height] or setting_size_bits != source_size_bits: raise RuntimeError(f'native Pen settings mismatch: size={setting_size_bits} screen={setting_resolution}')
 p.call('SPen::Pen::getSettingData()',[selected_pen]);
 if p.x(0)!=actual_setting_data: raise RuntimeError('Pen::getSettingData pointer changed')
 p.call('SPen::Pencil3::GetStrokeDrawableGL()',[selected_pen]); selected_drawable=p.x(0); selected_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable,8))[0] if selected_drawable else 0; selected_sub_vptr=struct.unpack('<Q',p.u.mem_read(selected_drawable+8,8))[0] if selected_drawable else 0; selected_object_redraw_target=struct.unpack('<Q',p.u.mem_read(selected_sub_vptr+40,8))[0] if selected_sub_vptr else 0; selected_object_redraw_names=[name for name,address in p.syms.items() if address==selected_object_redraw_target]; selected_drawable_version=next((f'v{v}' for v in (1,2,3,4) if any(f'Pencil3StrokeDrawableGLV{v}::RedrawPen' in name for name in selected_object_redraw_names)),'unknown'); print('SELECTED_VTABLE',hex(selected_vptr),hex(selected_sub_vptr),hex(selected_object_redraw_target),selected_object_redraw_names); selected_rt=struct.unpack('<Q',p.u.mem_read(selected_drawable+16,8))[0] if selected_drawable else 0; selected_rt_id=rt_identity(selected_rt) if selected_rt else None; print('DRAW_SELECTED_NATIVE pen/drawable/rt',hex(selected_pen),hex(selected_drawable),hex(selected_rt),selected_rt_id); actual_pen=factory_created_pens[-1]; actual_drawable=selected_drawable; print('Drawing-selected redraw pen/drawable',hex(actual_pen),hex(actual_drawable));
 # Capture observed version; no Pencil version expectation supplied.
 drawing_entries=[row for row in native_draw_entries if row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target)]
 if not drawing_entries: raise RuntimeError('Drawing did not enter the selected drawable RedrawPen vtable target')
 snapshot_object_stroke('before_saved_RedrawPen',obj); p.call('SPen::Pen::GetSize()', [actual_pen]); pen_size_after_draw_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(); print('native Pen::GetSize after draw S0 bits',pen_size_after_draw_bits); phase='saved_redraw'
 if not selected_object_redraw_target: raise RuntimeError('selected drawable has no ObjectStroke RedrawPen vtable slot')
 call_address(selected_object_redraw_target,[actual_drawable+8,obj,rect,rect]); saved_redraw_result=p.x(0); saved_redraw_rect=bytes(p.u.mem_read(rect,16)).hex()
 print('redraw result',hex(p.x(0)), 'rect',p.u.mem_read(rect,16).hex()); p.call('SPen::Pencil3StrokeDrawableGL::getSize()', [actual_drawable]); print('native drawable size after redraw S0',struct.unpack('<f',struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)))[0])
 rt=struct.unpack('<Q',p.u.mem_read(actual_drawable+16,8))[0]; vp=struct.unpack('<Q',p.u.mem_read(rt+80,8))[0]; print('rt',hex(rt),'vector_ptr',hex(vp),'vector_raw',p.u.mem_read(rt+80,24).hex()); vv=struct.unpack('<3Q',p.u.mem_read(vp,24)) if vp else (0,0,0); print('vector',tuple(hex(v) for v in vv))
 # The native AddPoint return hook copies the complete callback vector before
 # its caller submits/clears it. This is a synthetic explicit context profile.
 import json
 callbacks=[]
 p.call('SPen::Pencil3StrokeDrawableGL::getSize()', [actual_drawable]); drawable_size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 drawable_native_state={'identity':'selected drawable returned from actual Drawing factory route','address':hex(actual_drawable),'drawable_vtable':hex(selected_vptr),'native_size_getter':'SPen::Pencil3StrokeDrawableGL::getSize()','drawable_size_f32_bits':drawable_size_bits}
 if screen_resolution_calls != [{'width':args.width,'height':args.height}]: raise RuntimeError(f'unexpected native screen profile calls: {screen_resolution_calls}')
 if any(pending_motion_getters.values()): raise RuntimeError(f'MotionEvent getter calls missed native returns: {pending_motion_getters}')
 p.call('SPen::ObjectStroke::GetStrokeType() const',[obj]); stroke=p.x(0)
 native_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in paths.items()}
 native_hashes['libc++_shared.so']=hashlib.sha256(CPP.read_bytes()).hexdigest()
 native_hashes['libSPenComposer.so']=hashlib.sha256((LIBS/'libSPenComposer.so').read_bytes()).hexdigest()
 traced_methods=[{k:v for k,v in row.items() if k in ('signatures','phase','pc','regs','motion_event_entry_bytes','pointer_heads')} for row in native_draw_entries if any(token in name for name in row['signatures'] for token in ('::RedrawPen(','::endPen(','::drawPoint('))]
 case_id='straight_curve_tool2_'+args.advanced_setting
 draw_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes'])
 saved_stream=bytes.fromhex(next(row for row in reversed(pencil_buffer_events) if row['phase']=='saved_redraw')['after_bytes'])
 if not draw_stream or len(draw_stream)%20 or draw_stream!=saved_stream: raise RuntimeError(f'expected matching nonempty 20-byte AddPoint records, got {len(draw_stream)} and {len(saved_stream)} bytes')
 phase_counts={}
 for phase_name in ('draw_object_stroke','saved_redraw'):
  events=[row for row in pencil_buffer_events if row['phase']==phase_name]
  if len(events)!=len(draw_stream)//20 or any(int(row['rt'],16)!=selected_rt for row in events):
   raise RuntimeError(f'callback count/receiver does not match full stream in {phase_name}')
  phase_counts[phase_name]=len(events)
 if not event_order or event_order[0]['event']!='Pen::SetScreenResolution':
  raise RuntimeError('native screen profile was not applied before geometry')
 motion_ctors=[row for row in native_draw_entries if any(sig.startswith('SPen::MotionEvent::MotionEvent(') for sig in row['signatures'])]
 event_summary=[{'event':'Pen::SetScreenResolution','phase':row['phase'],'width':row['args']['width'],'height':row['args']['height']} if row['event']=='Pen::SetScreenResolution' else {'event':row['event'],'phase':row['phase']} for row in event_order]
 rng_by_phase={name:{'seed_values':[row['seed'] for row in _rng_trace if row['phase']==name and row['function']=='srand'],'rand_values':[row['value'] for row in _rng_trace if row['phase']==name and row['function']=='rand']} for name in ('draw_object_stroke','saved_redraw')}
 if selected_drawable_version=='v1' and any(len(profile['seed_values'])!=1 or not profile['rand_values'] for profile in rng_by_phase.values()): raise RuntimeError(f'incomplete modeled RNG trace for V1: {rng_by_phase}')
 if selected_drawable_version=='v3' and any(profile['seed_values'] or profile['rand_values'] for profile in rng_by_phase.values()): raise RuntimeError(f'unexpected RNG import calls for V3: {rng_by_phase}')
 result={'schema':'pencil3-saved-caller-native-v1','case_id':case_id,'profile':{'width':args.width,'height':args.height,'canvas_model_matrix_f32_bits':[struct.pack('<f',v).hex() for v in (1,0,0,0,1,0,0,0,1)]},'input':{'kind':'synthetic native ObjectStroke setter input; no serialized SDocx parser input','pen_name':'com.samsung.android.sdk.pen.pen.preload.Pencil3','stroke_type':stroke,'tool_type':input_case['tool_type'],'curve_enabled':input_case['curve_enabled'],'pen_size_f32_bits':source_size_bits,'points_f32_bits':[[struct.pack('<f',v).hex() for v in xy] for xy in points],'pressure_f32_bits':[struct.pack('<f',v).hex() for v in input_case['pressure']],'timestamps_i32':input_case['timestamps'],'tilt_f32_bits':[struct.pack('<f',v).hex() for v in input_case['tilt']],'orientation_f32_bits':[struct.pack('<f',v).hex() for v in input_case['orientation']],'advanced_setting_state':args.advanced_setting,'advanced_getter_pointer_nonnull':bool(advanced_getter),'advanced_getter_utf8':advanced_utf8,'advanced_getter_object_hex':advanced_getter_bytes},'libraries':native_hashes,'native_model_getters':model_getter_contract,'rng':{'profile':'AOSP bionic TYPE_3 model used to answer intercepted srand/rand imports','execution_boundary':'callbacks are Python model outputs; Pencil3 saved redraw/addpoint geometry is native Unicorn execution; this does not validate a Samsung device libc/runtime','calls':_rng_trace,'by_phase':rng_by_phase},'helper_hashes':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in (pathlib.Path(__file__).resolve(),pathlib.Path(oc.__file__).resolve(),pathlib.Path(oc.native.__file__).resolve(),pathlib.Path(elf_plt.__file__).resolve(),pathlib.Path(_pn.__file__).resolve())},'native_model_default_before_setters':{'ObjectStroke_GetPenSize_f32_bits':model_default_size_bits,'ObjectStroke_GetToolType':model_default_tool_type},'pen_constructor_return_state_raw':factory_return_snapshots,'pencil3_constructor_get_size_f32_bits':pencil_ctor_size_bits,'pen_setting_data':{'native_getter':'SPen::Pen::getSettingData()','pen_size_f32_bits_offset_0':setting_size_bits,'screen_resolution_i32_offsets_32_36':setting_resolution,'screen_min_dimension':min(setting_resolution),'derived_movement_resolution_scale_f32_bits':struct.pack('<f',1440.0/min(setting_resolution)).hex() if min(setting_resolution)>0 else struct.pack('<f',1.).hex()},'drawable_native_state':drawable_native_state,'native_calls':{'ObjectStroke_getter_snapshots':object_stroke_snapshots,'source_ObjectStroke_arrays':source_object_arrays,'MotionEvent_getter_returns':native_motion_getter_returns,'MotionEvent_constructors': [{'phase':row['phase'],'signatures':row['signatures'],'pc':row['pc'],'regs':row['regs']} for row in motion_ctors]},'loader':loader_calls,'route':{'factory_symbol':'createPenInst','selected_drawable_class':f'SPen::Pencil3StrokeDrawableGL{selected_drawable_version.upper()}' if selected_drawable_version!='unknown' else None,'selected_drawable_vtable_id':selected_drawable_version,'redraw_entry':selected_object_redraw_names[0] if selected_object_redraw_names else None,'drawing_return':draw_result,'saved_redraw_return':saved_redraw_result,'same_rt_id':selected_rt_id,'selected_rt_address':hex(selected_rt)},'screen_resolution':screen_resolution_calls,'runtime_event_order':event_summary,'streams':{'draw_object_stroke':{'callback_count':phase_counts['draw_object_stroke'],'float_count':len(draw_stream)//4,'sha256':hashlib.sha256(draw_stream).hexdigest(),'values_f32_bits':[draw_stream[i:i+4].hex() for i in range(0,len(draw_stream),4)]},'saved_redraw':{'callback_count':phase_counts['saved_redraw'],'float_count':len(saved_stream)//4,'sha256':hashlib.sha256(saved_stream).hexdigest(),'same_rt_as_drawing_phase':True,'same_bytes_as_drawing_phase':draw_stream==saved_stream}},'host_boundaries':['Synthetic ObjectStroke values are supplied through native Model setters; Model getters and source-array reads, Drawing selection, native MotionEvent constructors/getters, selected Pencil3 saved redraw, and RTV1 AddPoint geometry execute natively. The selected drawable version is recorded per setting state.','The constructor-return bytes are captured before this ObjectStroke is applied; they are raw state, not asserted to be a Samsung device default. `Pen::GetSize()` is observed on a separately constructed Pencil3 before stroke application; the model default getter is also recorded before setters; the selected tool remains explicit synthetic input.',f'Native Pen::SetScreenResolution and Pen::getSettingData execute against the explicit IContext profile supplied {args.width}x{args.height} before AddPoint; identity color conversion, flags/version, canvases, bitmap/resource adapters, and task-queue registration are host stubs.'],'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.'}
 getter_phases={row['phase_entry'] for row in native_motion_getter_returns}
 required_getters={'SPen::MotionEvent::GetX(int) const','SPen::MotionEvent::GetY(int) const','SPen::MotionEvent::GetToolType(int) const'}
 seen_getters={row['signature'] for row in native_motion_getter_returns}
 if not motion_ctors or not required_getters<=seen_getters or getter_phases!={'draw_object_stroke','saved_redraw'}:
  raise RuntimeError(f'MotionEvent native constructor/getter evidence incomplete: ctors={len(motion_ctors)} getters={seen_getters} phases={getter_phases}')
 result['motion_event_constructors']=[{'phase':row['phase'],'signatures':row['signatures'],'pc':row['pc'],'regs':row['regs']} for row in motion_ctors]
 result['motion_event_getter_returns']=native_motion_getter_returns
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 if draw_result != 1 or saved_redraw_result != 1:
  raise RuntimeError(f'native saved caller/replay failed: {draw_result}, {saved_redraw_result}')
 # Capture observed version; no Pencil version expectation supplied.
 if not any(row['phase']=='draw_object_stroke' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):
  raise RuntimeError('Drawing did not enter the selected Pencil3 RedrawPen vtable target')
 if not any(row['phase']=='saved_redraw' and row['pc']==hex(selected_object_redraw_target) for row in native_draw_entries):
  raise RuntimeError('explicit selected RedrawPen replay was not observed')
 for phase_name in ('draw_object_stroke','saved_redraw'):
  if not [row for row in pencil_buffer_events if row['phase']==phase_name]:
   raise RuntimeError(f'missing native RTV AddPoint buffers for {phase_name}')
 draw_terminal=next(row for row in reversed(pencil_buffer_events) if row['phase']=='draw_object_stroke')['after_bytes']
 saved_terminal=next(row for row in reversed(pencil_buffer_events) if row['phase']=='saved_redraw')['after_bytes']
 if draw_terminal != saved_terminal:
  raise RuntimeError('Drawing callback bytes differ from selected saved RedrawPen replay')
 encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
 print(encoded,end='',_always=True)
except Exception:
 raise
