"""Bounded Unicorn oracle for InkPen2 V9 saved redraw and RTV3 output.

Requires the hash-pinned Android DSOs and project-local Unicorn/binutils.
All DSO init arrays run; unresolved reached imports fail closed. The host
provides explicit single-thread locale, syscall, and mutex context.
"""
import struct, hashlib, subprocess, time, json, argparse, math
from unicorn import UC_HOOK_CODE
from pathlib import Path
from fountain_native import NativeFountain
from unicorn import UC_HOOK_MEM_INVALID
from unicorn import arm64_const
UC_ARM64_REG_SP=arm64_const.UC_ARM64_REG_SP; UC_ARM64_REG_LR=arm64_const.UC_ARM64_REG_LR; UC_ARM64_REG_PC=arm64_const.UC_ARM64_REG_PC
import fountain_native

CPP=Path('scratch/apk-analysis-native/arm64-v8a/libc++_shared.so')
INK=Path('scratch/apk-analysis-native/arm64-v8a/libSPenInkPen2.so')

def elf(path):
 b=path.read_bytes(); shoff=struct.unpack_from('<Q',b,40)[0]; es,n,si=struct.unpack_from('<HHH',b,58)
 sh=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*es) for i in range(n)]; ns=b[sh[si][4]:sh[si][4]+sh[si][5]]; by={}
 for s in sh:
  e=ns.find(b'\0',s[0]); by[ns[s[0]:e].decode()]=s
 d=by['.dynsym']; strings=b[by['.dynstr'][4]:by['.dynstr'][4]+by['.dynstr'][5]]; sy=[]
 for off in range(d[4],d[4]+d[5],d[9]):
  ni,inf,o,idx,val,size=struct.unpack_from('<IBBHQQ',b,off); e=strings.find(b'\0',ni); sy.append((strings[ni:e].decode() if ni else '',idx,val))
 return b,sh,by,sy

class N(NativeFountain):
 fountain_native.HASHES['InkPen2']='61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c'
 def __init__(self):
  if hashlib.sha256(CPP.read_bytes()).hexdigest() != '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4':
   raise ValueError('unverified libc++_shared.so')
  original_uc=fountain_native.Uc
  def make_uc(*args):
   u=original_uc(*args); u.ctl_set_cpu_model(arm64_const.UC_CPU_ARM64_MAX); return u
  fountain_native.Uc=make_uc
  try: super().__init__(Path('scratch/apk-analysis-native/arm64-v8a'), Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'),('InkPen2',))
  finally: fountain_native.Uc=original_uc
  self.callbacks['__cxa_atexit']=lambda:self.x(0,0)
  self.callbacks['getauxval']=self.getauxval
  self.callbacks['__system_property_get']=self.property_get
  self.callbacks['strncmp']=self.strncmp
  self.callbacks['fmodf']=self.fmodf
  self.callbacks['pthread_mutex_init']=self.pthread_noop
  self.callbacks['pthread_mutexattr_init']=self.pthread_noop
  self.callbacks['pthread_mutexattr_destroy']=self.pthread_noop
  self.callbacks['pthread_mutexattr_settype']=self.pthread_noop
  self.callbacks['pthread_mutex_destroy']=self.pthread_noop
  self.callbacks['pthread_mutex_lock']=self.pthread_noop
  self.callbacks['pthread_mutex_unlock']=self.pthread_noop
  self.callbacks['pthread_cond_init']=self.pthread_noop
  self.callbacks['pthread_cond_destroy']=self.pthread_noop
  self.callbacks['pthread_cond_wait']=self.pthread_noop
  self.callbacks['pthread_cond_signal']=self.pthread_noop
  self.callbacks['pthread_cond_broadcast']=self.pthread_noop
  self.callbacks['pthread_key_create']=self.pthread_key_create
  self.callbacks['pthread_key_delete']=self.pthread_noop
  self.callbacks['pthread_setspecific']=self.pthread_noop
  self.callbacks['newlocale']=self.newlocale
  self.callbacks['uselocale']=self.uselocale
  self.callbacks['freelocale']=self.pthread_noop
  self.callbacks['setlocale']=self.setlocale
  self.current_locale=0x7009210
  self.callbacks['posix_memalign']=self.posix_memalign
  self.callbacks['wcscoll_l']=self.wcscoll_l
  self.callbacks['mbtowc']=self.mbtowc
  self.callbacks['clock_gettime']=self.clock_gettime
  self.callbacks['syscall']=self.syscall
  self.callbacks['operator new(unsigned long, std::nothrow_t const&)']=lambda:self.x(0,self.alloc(self.x(0)))
  self.callbacks['__cxa_guard_acquire']=self.guard_acquire
  self.callbacks['__cxa_guard_release']=self.guard_release
  self.callbacks['__cxa_guard_abort']=self.guard_abort
  self.callbacks['__cxa_thread_atexit']=lambda:self.x(0,0)
  self.external_stubs={}
  self.next_external=0x7000000
  self.u.mem_map(0x7000000,0x100000)
  self.u.mem_write(0x7009220,b'C\0')
  self.u.mem_write(0x7000080,bytes([0xf9,0x00,0x00,0x08,0xe8,0x00,0x00,0xb5,0xfd,0x7b,0xbf,0xa9,0xfd,0x03,0x00,0x91,0x28,0x00,0x80,0xd2,0x08,0x00,0x00,0xf9,0x20,0x00,0x3f,0xd6,0xfd,0x7b,0xc1,0xa8,0x00,0x00,0x80,0xd2,0xc0,0x03,0x5f,0xd6]))
  self.u.hook_add(UC_HOOK_MEM_INVALID, lambda u,a,addr,size,val,d: (_ for _ in ()).throw(RuntimeError(f'invalid memory access {a:#x} {addr:#x} size={size}')))
  self.init_arrays = {}
  self.host_calls = {}
  self.load_cpp()
  self.init_array(CPP,0x4000000)
  self.apply(INK,0x3000000)
  self.apply(Path('scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so'),0x1000000)
  self.apply(Path('scratch/apk-analysis-native/arm64-v8a/libSPenBase.so'),0x2000000)
  self.init_array(Path('scratch/apk-analysis-native/arm64-v8a/libSPenBase.so'),0x2000000)
  self.init_array(Path('scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so'),0x1000000)
  self.init_array(INK,0x3000000)
  assert self.get(0x3000000+0x71230,'8Q')[1] > self.get(0x3000000+0x71230,'8Q')[0], 'InkPen2 map initializer did not populate table'
  assert any(self.get(0x10001530,'9Q')), 'PenCommon map initializer did not populate table'
  assert self.init_arrays == {'libc++_shared.so':3,'libSPenBase.so':59,'libSPenPenCommon.so':47,'libSPenInkPen2.so':35}, self.init_arrays
  self.initialization_host_calls = dict(self.host_calls)
 def property_get(self):
  key=self.read_cstr(self.x(0)); val=b'arm64-v8a' if key=='ro.arch' else b''; self.u.mem_write(self.x(1),val+b'\0'); self.x(0,len(val))
 def read_cstr(self,p):
  out=bytearray()
  while len(out)<128:
   b=self.u.mem_read(p+len(out),1)[0]
   if not b: break
   out.append(b)
  return out.decode(errors='replace')
 def syscall(self):
  args=[self.x(i) for i in range(6)]; self.x(0,1234 if args[0]==178 else (1<<64)-38)
 def clock_gettime(self):
  p=self.x(1); ns=1_000_000_000; self.put(p,'qq',ns//1_000_000_000,ns%1_000_000_000); self.x(0,0)
 def wcscoll_l(self):
  a,b=self.x(0),self.x(1); print('wcscoll',hex(a),hex(b),hex(self.x(2)),'lr',hex(self.u.reg_read(arm64_const.UC_ARM64_REG_LR))); result=0
  if not a or not b: self.x(0,0); return
  for i in range(256):
   x=struct.unpack('<I',self.u.mem_read(a+4*i,4))[0]; y=struct.unpack('<I',self.u.mem_read(b+4*i,4))[0]
   if x!=y: result=-1 if x<y else 1; break
   if x==0: break
  self.x(0,result)
 def mbtowc(self):
  out,src,n=self.x(0),self.x(1),self.x(2)
  if src==0 or n==0: self.x(0,0xffffffff); return
  value=self.u.mem_read(src,1)[0]
  if out: self.u.mem_write(out,struct.pack('<I',value))
  self.x(0,0 if value==0 else 1)
 def newlocale(self):
  name=self.read_cstr(self.x(1)); assert name in ('C','POSIX') and self.x(0)==0x1fbf and self.x(2)==0; self.x(0,0x7009200)
 def uselocale(self):
  old=self.current_locale
  if self.x(0): self.current_locale=self.x(0)
  self.x(0,old)
 def setlocale(self):
  if self.x(1): assert self.read_cstr(self.x(1)) in ('C','POSIX')
  self.x(0,0x7009220)
 def pthread_key_create(self): self.x(1,1); self.x(0,0)
 def posix_memalign(self):
  out,align,size=self.x(0),self.x(1),self.x(2); ptr=self.alloc(max(size,1)); self.u.mem_write(out,struct.pack('<Q',ptr)); self.x(0,0)
 def pthread_noop(self): self.x(0,0)
 def fmodf(self):
  x,y=self.f(0),self.f(1)
  self.f(0, math.fmod(x,y) if y else float('nan'))
 def strncmp(self):
  a,b,n=self.x(0),self.x(1),self.x(2)
  aa=bytes(self.u.mem_read(a,n));bb=bytes(self.u.mem_read(b,n)); v=0
  for x,y in zip(aa,bb):
   if x!=y: v=x-y; break
   if x==0: break
  self.x(0,v)
 def getauxval(self):
  key=self.x(0); val={6:4096,16:1<<8,26:0}.get(key,0); print('getauxval',key,'->',hex(val)); self.x(0,val)
 def guard_acquire(self):
  p=self.x(0); v=struct.unpack('<Q',self.u.mem_read(p,8))[0]
  if v&1: self.x(0,0)
  else: self.u.mem_write(p,struct.pack('<Q',2)); self.x(0,1)
 def guard_release(self): self.u.mem_write(self.x(0),struct.pack('<Q',1))
 def guard_abort(self): self.u.mem_write(self.x(0),struct.pack('<Q',0))
 def load_cpp(self):
  b,sh,by,sy=elf(CPP); base=0x4000000; self.u.mem_map(base,0x2000000)
  off=struct.unpack_from('<Q',b,32)[0]; pe,pn=struct.unpack_from('<HH',b,54)
  for i in range(pn):
   kind,flags,fo,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,off+i*pe)
   if kind==1 and ms:
    self.u.mem_write(base+va,b[fo:fo+fs]);
    if ms>fs:self.u.mem_write(base+va+fs,bytes(ms-fs))
  for name,idx,val in sy:
   if idx and val:self.syms[name]=base+val
  for line in subprocess.check_output(["nm","-D","-C",str(CPP)],text=True).splitlines():
   parts=line.strip().split(" ",2)
   if len(parts)==3 and parts[1] in ("T","W"):self.syms[parts[2]]=base+int(parts[0],16)
  self.apply(CPP,base)
 def apply(self,path,base):
  b,sh,by,sy=elf(path)
  for s in sh:
   if s[1]!=4:continue
   for off in range(s[4],s[4]+s[5],s[9]):
    where,info,a=struct.unpack_from('<QQq',b,off); typ,si=info&0xffffffff,info>>32
    if typ==0x403:t=base+a
    elif typ in (0x101,0x401):
     name,idx,val=sy[si];t=base+val+a if idx else self.syms.get(name,0)+a
    elif typ==0x402:
     name,idx,val=sy[si]; t=base+val+a if idx and val else self.syms.get(name) or (0x7000080 if name=='pthread_once' else self.external_stub(name))
     if not t: raise RuntimeError(f'unresolved JUMP_SLOT {path}:{name}')
    else:continue
    self.u.mem_write(base+where,struct.pack('<Q',t))
 def external_stub(self,name):
  if name not in self.external_stubs:
   addr=self.next_external; self.next_external+=16; self.external_stubs[addr]=name; self.u.mem_write(addr,struct.pack('<I',0xd65f03c0))
  return next(addr for addr,n in self.external_stubs.items() if n==name)
 def hook(self,u,a,size,data):
  if a in self.external_stubs:
   name=self.external_stubs[a]; aliases={'_Znwm':'operator new(unsigned long)','_Znam':'operator new[](unsigned long)','_ZnwmRKSt9nothrow_t':'operator new(unsigned long, std::nothrow_t const&)','_ZdlPv':'operator delete(void*)','_ZdaPv':'operator delete[](void*)','_ZdlPvm':'operator delete(void*, unsigned long)','_ZdaPvm':'operator delete[](void*)'}; key=aliases.get(name,name)
   cb=self.callbacks.get(key)
   if cb is None: raise RuntimeError('unhandled native import '+name)
   self.host_calls[name] = self.host_calls.get(name, 0) + 1
   cb(); u.reg_write(arm64_const.UC_ARM64_REG_PC,u.reg_read(arm64_const.UC_ARM64_REG_LR)); return
  if a in self.plt and self.plt[a] in self.callbacks:
   name=self.plt[a]; self.host_calls[name] = self.host_calls.get(name, 0) + 1
  super().hook(u,a,size,data)
 def init_array(self,path,base):
  b,sh,by,sy=elf(path); s=by['.init_array']; start=base+s[3]
  ptrs=[struct.unpack('<Q',self.u.mem_read(start+i,8))[0] for i in range(0,s[5],8)]
  called=0
  for ptr in ptrs:
   if not ptr:continue
   self.x(0,0)
   self.x(1,0)
   self.x(2,0)
   self.x(3,0)
   self.u.reg_write(UC_ARM64_REG_SP,0x200f0000)
   self.u.reg_write(UC_ARM64_REG_LR,0x200f1000)
   try:
    self.u.emu_start(ptr,0x200f1000,count=3000000)
    if self.u.reg_read(UC_ARM64_REG_PC) != 0x200f1000: raise RuntimeError(f'initializer budget exhausted at {self.u.reg_read(UC_ARM64_REG_PC):#x}')
   except Exception as e: raise RuntimeError(f'initializer failed {path.name} {ptr:#x} pc={self.u.reg_read(UC_ARM64_REG_PC):#x}: {e}') from e
   called+=1
  self.init_arrays[path.name]=called
 def cap(self,u,a,size,data):
  if a==self.syms['SPen::InkPen2StrokeDrawableGLV9::drawLine(float, float, long long, SPen::RectF*, bool, bool)']:
   self.captured.append(('line',self.f(0),self.f(1),self.x(2)))
  elif a==self.syms['SPen::InkPen2StrokeDrawableGLV9::endPen(SPen::MotionEvent const*, SPen::RectF*, bool)']:
   self.captured.append(('end',))
  elif a==self.syms['SPen::InkPen2StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float)']:
   self.captured.append(('point',self.f(0),self.f(1),self.f(2)))

def capture_one(case):
 n=N(); n.captured=[]; n.host_calls={}; n.u.hook_add(UC_HOOK_CODE,n.cap)
 n.callbacks['SPen::ObjectStroke::GetPointCount() const']=lambda:n.x(0,len(pts))
 pts=[tuple(p) for p in case['points']]; times=case['timestamps']; pres=case['pressures']
 def ch(name,fmt,vals):
  p=n.alloc(struct.calcsize('<'+fmt*len(vals)));n.put(p,fmt*len(vals),*vals);n.callbacks['SPen::ObjectStroke::'+name+'() const']=lambda p=p:n.x(0,p)
 p=n.alloc(8*len(pts));n.put(p,'f'*(2*len(pts)),*(x for pt in pts for x in pt));n.callbacks['SPen::ObjectStroke::GetPoint() const']=lambda:n.x(0,p)
 ch('GetTimeStamp','i',times);ch('GetPressure','f',pres);ch('GetTilt','f',case.get('tilts',[0.]*len(pts)));ch('GetOrientation','f',case.get('orientations',[0.]*len(pts)))
 n.callbacks['SPen::ObjectStroke::IsShape() const']=lambda:n.x(0,int(case.get('is_shape',False)));n.callbacks['SPen::ObjectStroke::GetToolType() const']=lambda:n.x(0,case.get('tool_type',2))
 n.callbacks['SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)']=lambda:None
 n.callbacks['SPen::MotionEvent::~MotionEvent()']=lambda:None
 for name,val in [('GetX(int) const',pts[-1][0]),('GetY(int) const',pts[-1][1])]:n.callbacks['SPen::MotionEvent::'+name]=lambda val=val:n.d(0,val)
 for name,val in [('GetToolType(int) const',case.get('tool_type',2)),('GetSource() const',0),('GetEventTime() const',times[-1])]:n.callbacks['SPen::MotionEvent::'+name]=lambda val=val:n.x(0,val)
 for name,val in [('GetPressure(int) const',pres[-1]),('GetTilt(int) const',case.get('tilts',[0.]*len(pts))[-1])]:n.callbacks['SPen::MotionEvent::'+name]=lambda val=val:n.f(0,val)
 data,settings,mgr,draw,rect=[n.alloc(s) for s in (256,128,256,1024,32)]
 n.put(data,'Q',settings);n.put(settings,'f',case['size']);n.put(settings+32,'ii',*case['resolution']);n.put(data+8,'BB',int(case['curve_enabled']),0);n.put(data+16,'i',0)
 n.call('SPen::InkPen2StrokeDrawableGLV9::InkPen2StrokeDrawableGLV9(SPen::InkPen2Data*, SPen::PenGLDataManager*)',[draw,data,mgr],instruction_limit=5_000_000)
 rt=n.get(draw+16,'Q')[0];v=n.alloc(24);storage=n.alloc(4*9*4096);n.put(v,'QQQ',storage,storage,storage+4*9*4096);n.put(rt+80,'Q',v);n.put(rt+296,'f',case['inverse_scale'][0]);n.put(rt+300,'f',case['inverse_scale'][1]);n.put(draw+88,'f',case['spacing']);n.put(draw+100,'B',int(case['initial_alternation']))
 assert case['tolerance'] == 0.0, 'oracle currently supports only saved tolerance 0.0'
 stroke=n.alloc(512);n.put(rect,'4f',0.,0.,0.,0.)

 try:n.call('SPen::InkPen2StrokeDrawableGLV9::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)',[draw,stroke,rect,1],instruction_limit=2_000_000)
 except Exception as e:print('redraw-error',e,hex(n.u.reg_read(UC_ARM64_REG_PC)),[hex(n.u.reg_read(r)) for r in (arm64_const.UC_ARM64_REG_X0,arm64_const.UC_ARM64_REG_X1,arm64_const.UC_ARM64_REG_X2,arm64_const.UC_ARM64_REG_X3,arm64_const.UC_ARM64_REG_X8,arm64_const.UC_ARM64_REG_X9)]);raise
 start,end,cap=n.get(v,'QQQ'); assert start <= end <= cap and cap-start <= 4*9*4096, 'invalid RTV3 bounds'; assert (end-start)%36 == 0, 'RTV3 output is not complete nine-float records'; vals=n.get(start,'f'*((end-start)//4)); assert all(math.isfinite(v) for v in vals), 'non-finite RTV3 output'; return {'input_callback_trace':n.captured,'rtv3': [list(vals[i:i+9]) for i in range(0,len(vals),9)],'bounds':list(n.get(rect,'4f')),'initializers':n.init_arrays,'host_calls_during_initialization':n.initialization_host_calls,'host_calls_during_redraw':n.host_calls,'runtime_context':{'cpu_model':'UC_CPU_ARM64_MAX','hwcap':256,'hwcap2':0,'pagesize':4096,'arch':'arm64-v8a','locale':'C','gettid':1234,'mutex_model':'single-thread no-op mutex/condition operations'}}

if __name__=='__main__':
 ap=argparse.ArgumentParser(description=__doc__); ap.add_argument('--fixture',type=Path,default=Path('conformance/inkpen2-saved.json')); group=ap.add_mutually_exclusive_group(required=True); group.add_argument('--write-fixture',action='store_true'); group.add_argument('--check-fixture',action='store_true'); args=ap.parse_args()
 fixture=json.loads(args.fixture.read_text())
 if fixture['source']['apk_sha256'] != 'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667': raise ValueError('fixture APK provenance differs')
 fixture['source']['libcxx_sha256']='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
 fixture['oracle']={'scope':'InkPen2 V9 saved type-0 redraw with bounded Android-host context','apk_sha256':'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667'}
 differences=[]
 for case in fixture['cases']:
  result=json.loads(json.dumps(capture_one(case)))
  if args.check_fixture and case.get('native') != result: differences.append(case['name'])
  else: case['native']=result
  print(f"{case['name']}: {len(result['rtv3'])} actual RTV3 records; constructors={result['initializers']}")
 if args.check_fixture and differences: raise SystemExit('fixture mismatch: '+', '.join(differences))
 if args.write_fixture: args.fixture.write_text(json.dumps(fixture,indent=2)+'\n')
