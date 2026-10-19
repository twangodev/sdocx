"""Shared pinned Drawing/Model/libc++ Unicorn runtime for caller oracles.

This helper executes only project-local, hash-checked native libraries. It
provides the initialized Oblique CanvasProbe used as a native host scaffold;
individual callers map their own profile DSO and install their own callbacks.
"""
from pathlib import Path
from types import SimpleNamespace

def create_runtime():
    import sys,hashlib,struct,subprocess,os
    root=Path(__file__).resolve().parents[1]
    os.chdir(root)  # CanvasProbe currently resolves its shipped-artifact paths from cwd.
    sys.path.insert(0,str(root))
    sys.path.insert(0,str(root/'conformance'))
    from conformance.elf_plt import resolve as resolve_plt
    import oblique_canvas_native as oc
    from unicorn import arm64_const as R
    from unicorn import UC_HOOK_CODE
    LIBS=root/'scratch/apk-analysis-native/arm64-v8a'; MODEL=LIBS/'libSPenModel.so'; BASE=0x7000000
    p=oc.CanvasProbe()
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
    env=dict(os.environ); env['LD_LIBRARY_PATH']=str((root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
    for line in subprocess.check_output(['nm','-D','-C',str(MODEL)],text=True,env=env).splitlines():
     parts=line.strip().split(' ',2)
     if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=BASE+int(parts[0],16)
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
    # Rebase dynamic relocations for mapped DSOs used by the actual constructors.
    paths={
     'FountainPen':LIBS/'libSPenFountainPen.so','PenCommon':LIBS/'libSPenPenCommon.so',
     'Base':LIBS/'libSPenBase.so','Drawing':LIBS/'libSPenDrawing.so',
     'Graphics':LIBS/'libSPenGraphics.so','ObliquePen':LIBS/'libSPenObliquePen.so',
     'Composer':LIBS/'libSPenComposer.so','Model':MODEL}
    bases={'FountainPen':0,'PenCommon':0x1000000,'Base':0x2000000,'Drawing':0x3000000,
     'Graphics':0x4000000,'ObliquePen':0x5000000,'Composer':0x6000000,'Model':BASE}
    rawsyms={}
    for lib,path in paths.items():
     for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
      parts=line.strip().split()
      if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'): rawsyms.setdefault(parts[2],bases[lib]+int(parts[0],16))
    for line in subprocess.check_output(['nm','-D',str(CPP)],text=True,env=env).splitlines():
     parts=line.strip().split()
     if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'): rawsyms.setdefault(parts[2],CPP_BASE+int(parts[0],16))
    # These three imports are outside the pinned DSO closure. Resource
    # callbacks are not exercised by saved-stroke capture; stdio is likewise
    # unused. Their relocation targets are explicit host boundaries.
    release_stub=0x200f2500
    p.u.mem_write(release_stub,bytes.fromhex('000020d4'))  # BRK if capture unexpectedly needs resource release.
    stdio_stub=p.alloc(256)
    host_stubs={
     '_ZN4SPen13ManagedObject9OnReleaseEv':release_stub,
     '_ZN4SPen13ManagedObject9OnRestoreEv':release_stub,
     '__sF':stdio_stub,
    }
    rawsyms.update(host_stubs)
    unresolved_imports=[]
    # Composer is mapped only for CanvasProbe's image metadata. This caller
    # does not execute Composer code, so leave that DSO's external UI vtables
    # untouched instead of manufacturing a broad host ABI.
    relocation_paths={name:path for name,path in paths.items() if name!='Composer'}
    relocation_paths['libc++']=CPP
    relocation_bases={name:base for name,base in bases.items() if name!='Composer'}
    relocation_bases['libc++']=CPP_BASE
    for lib,path in relocation_paths.items():
     b=path.read_bytes(); base=relocation_bases[lib]; shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; ns=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={ns[x[0]:ns.find(b'\0',x[0])].decode():x for x in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
     for off in range(ds[4],ds[4]+ds[5],ds[9]):
      no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
     for snm in ('.rela.dyn','.rela.plt'):
      sec=by.get(snm)
      if not sec:continue
      for off in range(sec[4],sec[4]+sec[5],sec[9]):
       where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
       if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
       elif typ in (0x401,0x402,0x403,0x101,0x1025,0x1026):
        name,val=sy[idx]; target=base+val if val else p.syms.get(name,0)
        if not target:target=rawsyms.get(name,0)
        if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
        elif typ in (0x101,0x401):
         unresolved_imports.append((lib,where,name,typ))
        # Unresolved JUMP_SLOT imports enter an explicitly mapped PLT callback.

    def install_verified_plt(path,base,span):
     for address in tuple(p.plt):
      if base<=address<base+span: p.plt.pop(address)
     entries=resolve_plt(path)
     names=tuple(dict.fromkeys(entries.values()))
     pretty=subprocess.check_output(['c++filt','-n'],input='\n'.join(names)+'\n',text=True).splitlines()
     demangled=dict(zip(names,pretty,strict=True))
     p.plt.update({base+offset:demangled[symbol] for offset,symbol in entries.items()})
    install_verified_plt(MODEL,BASE,(extent+0xfff)&~0xfff)
    install_verified_plt(CPP,CPP_BASE,(cextent+0xfff)&~0xfff)

    for name in ['std::__ndk1::recursive_mutex::lock()','std::__ndk1::recursive_mutex::unlock()','std::__ndk1::recursive_mutex::recursive_mutex()','std::__ndk1::recursive_mutex::~recursive_mutex()','std::__ndk1::mutex::lock()','std::__ndk1::mutex::unlock()','std::__ndk1::mutex::mutex()','std::__ndk1::mutex::~mutex()']: p.callbacks[name]=lambda:None
    p.callbacks['operator new(unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
    p.callbacks['operator new[](unsigned long, std::nothrow_t const&)']=lambda:p.x(0,p.alloc(p.x(0)))
    p.callbacks['strlen']=lambda:p.x(0,len(bytes(p.u.mem_read(p.x(0),256)).split(b'\0',1)[0]))
    p.callbacks['atoi']=lambda:p.x(0,int(bytes(p.u.mem_read(p.x(0),64)).split(b'\0',1)[0] or b'0'))
    p._runtime_queue = 0
    p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:p.x(0,p._runtime_queue); p.callbacks['__cxa_atexit']=lambda:p.x(0,0)
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
    p._emutls_descriptors=[]
    def emutls():
     key=p.x(0); size,align,existing,template=struct.unpack('<QQQQ',p.u.mem_read(key,32))
     if not 0<size<=0x100000 or not 0<align<=0x10000 or align&(align-1): raise RuntimeError(f'invalid emutls descriptor {size} {align}')
     template_bytes=bytes(p.u.mem_read(template,size)) if template else bytes(size)
     p._emutls_descriptors.append({'size':size,'align':align,'existing_nonnull':bool(existing),'template_nonnull':bool(template),'template_sha256':hashlib.sha256(template_bytes).hexdigest(),'template_nonzero':any(template_bytes),'template_bytes_hex':template_bytes.hex() if any(template_bytes) else None})
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
    # Expose bounded init-array execution for callers that explicitly need it.
    # This helper does not execute dependency arrays itself; the Calligraphy
    # caller runner below executes only its profile plugin's init array.
    def run_init(path,base):
     b=path.read_bytes(); sh=struct.unpack_from('<Q',b,40)[0]; es,n,si=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,sh+i*es) for i in range(n)]; names=b[secs[si][4]:secs[si][4]+secs[si][5]]; sec=next(x for x in secs if names[x[0]:names.find(b'\0',x[0])]==b'.init_array');
     for i in range(0,sec[5],8):
      target=struct.unpack('<Q',p.u.mem_read(base+sec[3]+i,8))[0]
      if not target:continue
      for j in range(4):p.x(j,0)
      p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,oc.STOP)
      p.u.emu_start(target,oc.STOP,count=3000000)
      if p.u.reg_read(R.UC_ARM64_REG_PC)!=oc.STOP:raise RuntimeError(f'init failed {path.name} {target:#x} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
    # CanvasProbe's generic drawing defaults suppress real Model visibility
    # and geometry. Once Model and Base are mapped, caller runners must execute
    # the shipped getters and MotionEvent implementation.
    for callback_name in tuple(p.callbacks):
     if callback_name.startswith('SPen::MotionEvent::') or callback_name in ('SPen::ObjectBase::GetType() const','SPen::ObjectBase::IsVisible() const'):
      p.callbacks.pop(callback_name)
    if any(name.startswith('SPen::MotionEvent::') for name in p.callbacks):
     raise RuntimeError('stale synthetic MotionEvent callback remains in drawing runtime')
    return SimpleNamespace(p=p, oc=oc, R=R, UC_HOOK_CODE=UC_HOOK_CODE,
                           LIBS=LIBS, MODEL=MODEL, BASE=BASE, CPP=CPP,
                           CPP_BASE=CPP_BASE, paths=paths, bases=bases,
                           rawsyms=rawsyms, host_stubs=host_stubs,
                           unresolved_imports=unresolved_imports,
                           emutls_descriptors=p._emutls_descriptors,
                           run_init=run_init, env=env)
