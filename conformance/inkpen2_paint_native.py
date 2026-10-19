"""Bounded native RTV3 paint stages on Mesa; never a production renderer."""
import argparse, contextlib, hashlib, io, json, os, re, struct, subprocess, sys
import zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'conformance'))
import brushpen_paint_positive_native as bp
import brushpen_saved_native as bs
import inkpen_paint_native as ip
import fountain_native as fn
import elf_plt
from unicorn import arm64_const as R
from unicorn import UC_HOOK_MEM_INVALID
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
OBJ=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
INK=LIB/'libSPenInkPen2.so'; BASE=0x7000000
CPP=LIB/'libc++_shared.so'; CPP_BASE=0x8000000
assert hashlib.sha256(INK.read_bytes()).hexdigest()=='61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c'

def call(p,name,x=(),f=(),limit=5000000):
    p.call(name,list(x),list(f),instruction_limit=limit)

SCENARIOS = {
    'single': [(64.0, 48.0, (0.8, 0.2, 0.1))],
    'coincident_same': [(64.0, 48.0, (0.8, 0.2, 0.1)), (64.0, 48.0, (0.8, 0.2, 0.1))],
    'separated': [(64.0, 48.0, (0.8, 0.2, 0.1)), (96.0, 48.0, (0.8, 0.2, 0.1))],
    'coincident_different': [(64.0, 48.0, (0.8, 0.2, 0.1)), (64.0, 48.0, (0.2, 0.8, 0.3))],
    'single_half_alpha': [(64.0, 48.0, (0.8, 0.2, 0.1))],
    'coincident_different_half_alpha': [(64.0, 48.0, (0.8, 0.2, 0.1)), (64.0, 48.0, (0.2, 0.8, 0.3))],
}

def main(case_id):
    point_inputs = SCENARIOS[case_id]
    input_argb = 0x80336699 if case_id.endswith("half_alpha") else 0xff336699
    old_uc=fn.Uc
    def max_uc(*args):
        u=old_uc(*args);u.ctl_set_cpu_model(R.UC_CPU_ARM64_MAX);return u
    fn.Uc=max_uc
    try:p=bp.PositiveProbe()
    finally:fn.Uc=old_uc
    p.u.hook_add(UC_HOOK_MEM_INVALID,lambda u,a,addr,size,val,d:print('INVALID_MEM',hex(a),hex(addr),size,'pc',hex(u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(u.reg_read(R.UC_ARM64_REG_LR))))
    C=bp.C
    g=p.mesa.gl; f=p.mesa.f
    f.update({
      'glGenFramebuffers':ip.cfunc(g,'glGenFramebuffers',None,[C.c_int,C.POINTER(C.c_uint)]),
      'glBindFramebuffer':ip.cfunc(g,'glBindFramebuffer',None,[C.c_uint,C.c_uint]),
      'glFramebufferTexture2D':ip.cfunc(g,'glFramebufferTexture2D',None,[C.c_uint,C.c_uint,C.c_uint,C.c_uint,C.c_int]),
      'glCheckFramebufferStatus':ip.cfunc(g,'glCheckFramebufferStatus',C.c_uint,[C.c_uint]),
      'glGetBooleanv':ip.cfunc(g,'glGetBooleanv',None,[C.c_uint,C.POINTER(C.c_ubyte)]),
      'glGetIntegerv':ip.cfunc(g,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)]),
      'glGetFramebufferAttachmentParameteriv':ip.cfunc(g,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)]),
      'glReadPixels':ip.cfunc(g,'glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p]),
      'glViewport':ip.cfunc(g,'glViewport',None,[C.c_int,C.c_int,C.c_int,C.c_int]),
      'glClearColor':ip.cfunc(g,'glClearColor',None,[C.c_float,C.c_float,C.c_float,C.c_float]),
      'glClear':ip.cfunc(g,'glClear',None,[C.c_uint]),
      'glInvalidateFramebuffer':ip.cfunc(g,'glInvalidateFramebuffer',None,[C.c_uint,C.c_int,C.POINTER(C.c_uint)]),
      'glCreateShader':ip.cfunc(g,'glCreateShader',C.c_uint,[C.c_uint]),
      'glCreateProgram':ip.cfunc(g,'glCreateProgram',C.c_uint,[]),
      'glCompileShader':ip.cfunc(g,'glCompileShader',None,[C.c_uint]),
      'glAttachShader':ip.cfunc(g,'glAttachShader',None,[C.c_uint,C.c_uint]),
      'glDetachShader':ip.cfunc(g,'glDetachShader',None,[C.c_uint,C.c_uint]),
      'glLinkProgram':ip.cfunc(g,'glLinkProgram',None,[C.c_uint]),
      'glDeleteShader':ip.cfunc(g,'glDeleteShader',None,[C.c_uint]),
      'glDeleteProgram':ip.cfunc(g,'glDeleteProgram',None,[C.c_uint]),
      'glBindAttribLocation':ip.cfunc(g,'glBindAttribLocation',None,[C.c_uint,C.c_uint,C.c_char_p]),
      'glGetAttribLocation':ip.cfunc(g,'glGetAttribLocation',C.c_int,[C.c_uint,C.c_char_p]),
      'glGetProgramiv':ip.cfunc(g,'glGetProgramiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)]),
      'glGetShaderiv':ip.cfunc(g,'glGetShaderiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)]),
      'glGetProgramInfoLog':ip.cfunc(g,'glGetProgramInfoLog',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.c_char_p]),
      'glGetProgramBinary':ip.cfunc(g,'glGetProgramBinary',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p]),
      'glProgramBinary':ip.cfunc(g,'glProgramBinary',None,[C.c_uint,C.c_uint,C.c_void_p,C.c_int]),
      'glGetShaderInfoLog':ip.cfunc(g,'glGetShaderInfoLog',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.c_char_p]),
      'glShaderSource':ip.cfunc(g,'glShaderSource',None,[C.c_uint,C.c_int,C.POINTER(C.c_char_p),C.POINTER(C.c_int)]),
      'glGetUniformLocation':ip.cfunc(g,'glGetUniformLocation',C.c_int,[C.c_uint,C.c_char_p]),
      'glUseProgram':ip.cfunc(g,'glUseProgram',None,[C.c_uint]),
      'glDrawArrays':ip.cfunc(g,'glDrawArrays',None,[C.c_uint,C.c_int,C.c_int]),
      'glUniformMatrix4fv':ip.cfunc(g,'glUniformMatrix4fv',None,[C.c_int,C.c_int,C.c_ubyte,C.POINTER(C.c_float)]),
      'glUniform1i':ip.cfunc(g,'glUniform1i',None,[C.c_int,C.c_int]),
      'glUniform1f':ip.cfunc(g,'glUniform1f',None,[C.c_int,C.c_float]),
      'glUniform2f':ip.cfunc(g,'glUniform2f',None,[C.c_int,C.c_float,C.c_float]),
      'glUniform4f':ip.cfunc(g,'glUniform4f',None,[C.c_int,C.c_float,C.c_float,C.c_float,C.c_float]),
      'glUniform4fv':ip.cfunc(g,'glUniform4fv',None,[C.c_int,C.c_int,C.POINTER(C.c_float)]),
      'glGetActiveUniform':ip.cfunc(g,'glGetActiveUniform',None,[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_char_p]),
      'glGetActiveAttrib':ip.cfunc(g,'glGetActiveAttrib',None,[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_char_p]),
    })
    def cstr(ptr,limit=65536):
        raw=bytearray()
        while len(raw)<limit:
            ch=bytes(p.u.mem_read(ptr+len(raw),1))
            if ch==b'\0':return bytes(raw)
            raw+=ch
        raise RuntimeError('GL string exceeded bound')
    shader_sources=[]
    shader_rows={}
    program_rows={}
    def create_shader():
        shader_type=p.x(0); shader=f['glCreateShader'](shader_type)
        shader_rows[shader]={'type':shader_type,'sources':[],'compiled':None}
        p.x(0,shader)
    def create_program():
        program=f['glCreateProgram'](); program_rows[program]={'shaders':[],'linked':None}
        p.x(0,program)
    def attach_shader():
        program,shader=p.x(0),p.x(1); f['glAttachShader'](program,shader)
        program_rows.setdefault(program,{'shaders':[],'linked':None})['shaders'].append(shader)
    def link_program():
        program=p.x(0); f['glLinkProgram'](program); status=C.c_int(); f['glGetProgramiv'](program,0x8B82,C.byref(status))
        row=program_rows.setdefault(program,{'shaders':[],'linked':None})
        row['linked']=bool(status.value)
        row['shader_snapshots']=[dict(shader_rows.get(shader,{})) for shader in row['shaders']]
    def program_binary_miss():
        # Explicit synthetic host cache miss; force the vendor's source compile path.
        length,fmt=p.x(2),p.x(3)
        if length:p.put(length,'i',0)
        if fmt:p.put(fmt,'I',0)
    def program_binary_unexpected():
        raise RuntimeError('native shader attempted to consume a host program binary')
    def shader_source():
        sh,count,arr,lens=(p.x(i) for i in range(4)); keep=[]; ptrs=[]; lengths=[]; contents=[]
        for i in range(count):
            ptr=struct.unpack('<Q',p.u.mem_read(arr+8*i,8))[0]
            size=struct.unpack('<i',p.u.mem_read(lens+4*i,4))[0] if lens else -1
            raw=bytes(p.u.mem_read(ptr,size)) if size>=0 else cstr(ptr)
            buf=C.create_string_buffer(raw); keep.append(buf); ptrs.append(C.cast(buf,C.c_char_p)); lengths.append(len(raw)); contents.append(raw)
        shader_sources.extend(contents)
        shader_rows.setdefault(sh,{'type':None,'sources':[],'compiled':None})['sources'].extend(x.decode('utf-8','replace') for x in contents)
        arr_c=(C.c_char_p*count)(*ptrs); lens_c=(C.c_int*count)(*lengths)
        f['glShaderSource'](sh,count,arr_c,lens_c)
    def active_gl_name(which):
        program,index,bufsize,lengthp,sizep,typep,namep=[p.x(i) for i in range(7)]
        length,size,kind=C.c_int(),C.c_int(),C.c_uint(); name=C.create_string_buffer(min(max(bufsize,1),4096))
        f[which](program,index,bufsize,C.byref(length) if lengthp else None,C.byref(size) if sizep else None,C.byref(kind) if typep else None,name)
        if lengthp:p.put(lengthp,'i',length.value)
        if sizep:p.put(sizep,'i',size.value)
        if typep:p.put(typep,'I',kind.value)
        if namep:p.u.mem_write(namep,name.raw[:min(max(bufsize,0),4096)])
    def get_boolean():
        pname,outptr=p.x(0),p.x(1);count=4 if pname in (0x0C23,0x0C22) else 1
        out=(C.c_ubyte*count)();f['glGetBooleanv'](pname,out);p.u.mem_write(outptr,bytes(out))
    p.callbacks.update({
      'glCreateShader':create_shader,
      'glCreateProgram':create_program,
      'glCompileShader':lambda:f['glCompileShader'](p.x(0)),
      'glAttachShader':attach_shader,
      'glDetachShader':lambda:f['glDetachShader'](p.x(0),p.x(1)),
      'glLinkProgram':link_program,
      'glDeleteShader':lambda:f['glDeleteShader'](p.x(0)),
      'glDeleteProgram':lambda:f['glDeleteProgram'](p.x(0)),
      'glBindAttribLocation':lambda:f['glBindAttribLocation'](p.x(0),p.x(1),cstr(p.x(2))),
      'glGetAttribLocation':lambda:p.x(0,f['glGetAttribLocation'](p.x(0),cstr(p.x(1)))),
      'glGetUniformLocation':lambda:p.x(0,f['glGetUniformLocation'](p.x(0),cstr(p.x(1)))),
      'glGetActiveUniform':lambda:active_gl_name('glGetActiveUniform'),
      'glGetActiveAttrib':lambda:active_gl_name('glGetActiveAttrib'),
      'glUseProgram':lambda:f['glUseProgram'](p.x(0)),
      'glDrawArrays':lambda:f['glDrawArrays'](p.x(0),p.x(1),p.x(2)),
      'glUniformMatrix4fv':lambda:(lambda loc,n,tr,ptr:(lambda vals,arr:(f['glUniformMatrix4fv'](loc,n,tr,arr),None))(p.get(ptr,'f'*(16*n)),(C.c_float*(16*n))(*p.get(ptr,'f'*(16*n)))))(p.x(0),p.x(1),p.x(2),p.x(3)),
      'glUniform1i':lambda:f['glUniform1i'](p.x(0),p.x(1)),
      'glUniform1f':lambda:f['glUniform1f'](p.x(0),p.f(0)),
      'glUniform2f':lambda:f['glUniform2f'](p.x(0),p.f(0),p.f(1)),
      'glUniform4f':lambda:f['glUniform4f'](p.x(0),p.f(0),p.f(1),p.f(2),p.f(3)),
      'glUniform4fv':lambda:(lambda loc,n,ptr:(lambda vals:f['glUniform4fv'](loc,n,(C.c_float*(4*n))(*vals)))(p.get(ptr,'f'*(4*n))))(p.x(0),p.x(1),p.x(2)),
      'glGetProgramiv':lambda:(lambda v:(f['glGetProgramiv'](p.x(0),p.x(1),C.byref(v)),p.put(p.x(2),'I',v.value),program_rows.setdefault(p.x(0),{'shaders':[],'linked':None}).update(linked=bool(v.value) if p.x(1)==0x8B82 else program_rows.get(p.x(0),{}).get('linked'))))(C.c_int()),
      'glGetShaderiv':lambda:(lambda v:(f['glGetShaderiv'](p.x(0),p.x(1),C.byref(v)),p.put(p.x(2),'I',v.value),shader_rows.setdefault(p.x(0),{'type':None,'sources':[],'compiled':None}).update(compiled=bool(v.value) if p.x(1)==0x8B81 else shader_rows.get(p.x(0),{}).get('compiled'))))(C.c_int()),
      'glGetProgramInfoLog':lambda:(lambda b:(f['glGetProgramInfoLog'](p.x(0),p.x(1),None,b),p.u.mem_write(p.x(3),b.raw)))(C.create_string_buffer(min(p.x(1),4096))),
      'glGetProgramBinary':program_binary_miss,
      'glProgramBinary':program_binary_unexpected,
      'glGetShaderInfoLog':lambda:(lambda b:(f['glGetShaderInfoLog'](p.x(0),p.x(1),None,b),p.u.mem_write(p.x(3),b.raw)))(C.create_string_buffer(min(p.x(1),4096))),
      'glShaderSource':shader_source,
      'glGenFramebuffers':lambda:(lambda n,out:(lambda ids:(f['glGenFramebuffers'](n,ids),[p.put(out+4*i,'I',v) for i,v in enumerate(ids)]))( (C.c_uint*n)()))(p.x(0),p.x(1)),
      'glBindFramebuffer':lambda:f['glBindFramebuffer'](p.x(0),p.x(1)),
      'glFramebufferTexture2D':lambda:f['glFramebufferTexture2D'](*[p.x(i) for i in range(5)]),
      'glCheckFramebufferStatus':lambda:p.x(0,f['glCheckFramebufferStatus'](p.x(0))),
      'glGetBooleanv':get_boolean,
      'glViewport':lambda:f['glViewport'](*[p.x(i) for i in range(4)]),
      'glClearColor':lambda:f['glClearColor'](*[p.f(i) for i in range(4)]),
      'glClear':lambda:f['glClear'](p.x(0)),
      'glInvalidateFramebuffer':lambda:(lambda target,n,ptr:(lambda vals:f['glInvalidateFramebuffer'](target,n,vals))((C.c_uint*n)(*p.get(ptr,'I'*n))))(p.x(0),p.x(1),p.x(2)),
    })
    b=INK.read_bytes(); phoff=struct.unpack_from('<Q',b,32)[0]; ent,n=struct.unpack_from('<HH',b,54)
    extent=max(va+ms for i in range(n) if (ph:=struct.unpack_from('<II6Q',b,phoff+i*ent))[0]==1 for va,ms in [(ph[3],ph[6])])
    p.u.mem_map(BASE,(extent+0xfff)&~0xfff)
    for i in range(n):
        typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
        if typ==1 and ms:
            p.u.mem_write(BASE+va,b[off:off+fs])
            if ms>fs:p.u.mem_write(BASE+va+fs,bytes(ms-fs))
    env={**os.environ,'LD_LIBRARY_PATH':str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
    raw_symbols={}
    for path,base in [(LIB/'libSPenFountainPen.so',0),(LIB/'libSPenPenCommon.so',0x1000000),(LIB/'libSPenBase.so',0x2000000),(LIB/'libSPenBrushPen.so',0x3000000),(LIB/'libSPenGraphics.so',0x4000000),(LIB/'libSPenRenderer.so',0x5000000),(LIB/'libSPenModel.so',0x6000000),(INK,BASE),(CPP,CPP_BASE)]:
        for line in subprocess.check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
            z=line.split()
            if len(z)==3 and z[1].upper() in ('T','W','D','B','R','V'):raw_symbols.setdefault(z[2],base+int(z[0],16))
    for line in subprocess.check_output(['nm','-D','-C',str(INK)],text=True,env=env).splitlines():
        z=line.strip().split(' ',2)
        if len(z)==3 and z[1] in ('T','W'):p.syms[z[2]]=BASE+int(z[0],16)
    name_by_pc={address:name for name,address in p.syms.items() if BASE<=address<BASE+0x200000}; entry_trace=[]
    from unicorn import UC_HOOK_CODE
    def trace_entry(u,a,z,d):
        if a in name_by_pc:
            entry_trace.append((hex(a),name_by_pc[a]))
            if len(entry_trace)>40:del entry_trace[:-40]
    p.u.hook_add(UC_HOOK_CODE,trace_entry)
    for offset,rawname in elf_plt.resolve(INK).items():
        name=subprocess.check_output(['c++filt','-n',rawname],text=True,env=env).strip() if rawname.startswith('_Z') else rawname
        p.plt[BASE+offset]=name
    # Map pinned libc++ because native RTV3 shader/cache setup uses std::string
    # and unordered-map code. Run its three actual DSO init-array entries.
    cb=CPP.read_bytes(); assert hashlib.sha256(cb).hexdigest()=='4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
    cph=struct.unpack_from('<Q',cb,32)[0]; ce,cn=struct.unpack_from('<HH',cb,54)
    cextent=max(va+ms for i in range(cn) if (ph:=struct.unpack_from('<II6Q',cb,cph+i*ce))[0]==1 for va,ms in [(ph[3],ph[6])])
    p.u.mem_map(CPP_BASE,(cextent+0xfff)&~0xfff)
    for i in range(cn):
        typ,flags,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',cb,cph+i*ce)
        if typ==1 and ms:
            p.u.mem_write(CPP_BASE+va,cb[off:off+fs])
            if ms>fs:p.u.mem_write(CPP_BASE+va+fs,bytes(ms-fs))
    for line in subprocess.check_output(['nm','-D','-C',str(CPP)],text=True,env=env).splitlines():
        z=line.strip().split(' ',2)
        if len(z)==3 and z[1] in ('T','W'):p.syms[z[2]]=CPP_BASE+int(z[0],16)
    for offset,rawname in elf_plt.resolve(CPP).items():
        name=subprocess.check_output(['c++filt','-n',rawname],text=True,env=env).strip() if rawname.startswith('_Z') else rawname
        p.plt[CPP_BASE+offset]=name
    # Replace objdump label guesses for every resident Android DSO with the
    # reviewed ADRP/LDR/GOT/RELA resolver, including BTI/NOP veneer entries.
    resident=[(LIB/'libSPenFountainPen.so',0),(LIB/'libSPenPenCommon.so',0x1000000),(LIB/'libSPenBase.so',0x2000000),(LIB/'libSPenBrushPen.so',0x3000000),(LIB/'libSPenGraphics.so',0x4000000),(LIB/'libSPenRenderer.so',0x5000000),(LIB/'libSPenModel.so',0x6000000)]
    for path,base in resident:
        for old in [a for a in p.plt if base<=a<base+0x200000]:p.plt.pop(old)
        for offset,rawname in elf_plt.resolve(path).items():
            name=subprocess.check_output(['c++filt','-n',rawname],text=True,env=env).strip() if rawname.startswith('_Z') else rawname
            p.plt[base+offset]=name
    def relocate_dso(blob,base):
        shoff=struct.unpack_from('<Q',blob,40)[0]; se,sn,ss=struct.unpack_from('<HHH',blob,58)
        secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*se) for i in range(sn)]; ns=blob[secs[ss][4]:secs[ss][4]+secs[ss][5]]
        by={ns[s[0]:ns.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=blob[st[4]:st[4]+st[5]]; sy=[]
        for off in range(ds[4],ds[4]+ds[5],ds[9]):
            ni,info,other,idx,val,size=struct.unpack_from('<IBBHQQ',blob,off); sy.append((strings[ni:strings.find(b'\0',ni)].decode() if ni else '',idx,val))
        for secname in ('.rela.dyn',):
            sec=by.get(secname)
            if not sec:continue
            for off in range(sec[4],sec[4]+sec[5],sec[9]):
                where,info,add=struct.unpack_from('<QQq',blob,off); typ,si=info&0xffffffff,info>>32
                if typ==0x403:val=base+add
                elif typ in (0x101,0x401):
                    name,idx,symval=sy[si]; val=(base+symval if idx and symval else raw_symbols.get(name,0))+add
                else:continue
                if val:p.put(base+where,'Q',val)
    relocate_dso(cb,CPP_BASE)
    guards=set()
    def guard_acquire():
        ptr=p.x(0); word=p.get(ptr,'Q')[0]
        if word&1:p.x(0,0)
        else:p.put(ptr,'Q',2);p.x(0,1);guards.add(ptr)
    def guard_release():p.put(p.x(0),'Q',1)
    p.callbacks['__cxa_guard_acquire']=guard_acquire; p.callbacks['__cxa_guard_release']=guard_release; p.callbacks['__cxa_guard_abort']=lambda:p.put(p.x(0),'Q',0)
    for imported in ('pthread_mutex_init','pthread_mutex_destroy','pthread_mutex_lock','pthread_mutex_unlock','pthread_mutexattr_init','pthread_mutexattr_destroy','pthread_mutexattr_settype','pthread_rwlock_rdlock','pthread_rwlock_wrlock','pthread_rwlock_unlock','pthread_cond_broadcast','pthread_cond_signal','pthread_cond_init','pthread_cond_destroy'):
        p.callbacks[imported]=lambda:p.x(0,0)
    # Apply the InkPen2 DSO's RELATIVE and defined/cross-DSO pointer relocations.
    shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58)
    secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]
    names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]
    by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}
    ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
    for off in range(ds[4],ds[4]+ds[5],ds[9]):
        ni,info,other,idx,val,size=struct.unpack_from('<IBBHQQ',b,off)
        sy.append((strings[ni:strings.find(b'\0',ni)].decode() if ni else '',idx,val))
    for secname in ('.rela.dyn','.rela.plt'):
        sec=by.get(secname)
        if not sec:continue
        for off in range(sec[4],sec[4]+sec[5],sec[9]):
            where,info,add=struct.unpack_from('<QQq',b,off); typ,si=info&0xffffffff,info>>32
            if typ==0x403: value=BASE+add
            elif typ in (0x101,0x401,0x402):
                name,idx,val=sy[si]
                value=(BASE+val if idx and val else raw_symbols.get(name,0)) + (add if typ in (0x101,0x401) else 0)
            else:continue
            if value:p.put(BASE+where,'Q',value)
            elif typ in (0x101,0x401,0x402):print('unresolved relocation',secname,hex(where),sy[si][0],hex(typ))
    # Native Renderer factory path and native blend objects; Mesa remains the GL boundary.
    p.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)',None)
    p.callbacks['crc32']=lambda:p.x(0,zlib.crc32(bytes(p.u.mem_read(p.x(1),p.x(2))),p.x(0)))
    def bounded_log_format():
        dst,cap,fmt=p.x(0),p.x(1),p.x(2)
        raw=bytearray()
        while len(raw)<4096:
            c=p.u.mem_read(fmt+len(raw),1)
            if c==b'\0':break
            raw+=c
        if len(raw)>=4096 or cap>4096:raise RuntimeError('unbounded renderer log format')
        print('VSNPRINTF',bytes(raw),[hex(p.x(i)) for i in range(4)],bytes(p.u.mem_read(p.x(3),32)).hex() if p.x(3) else None,bytes(p.u.mem_read(0x200efa70,96)).hex())
        if cap:p.u.mem_write(dst,bytes(raw[:cap-1])+b'\0')
        p.x(0,len(raw))
    p.callbacks['vsnprintf']=bounded_log_format
    p.callbacks['getauxval']=lambda:p.x(0,{6:4096,16:256,17:100,23:0,26:0,51:0}.get(p.x(0),0))
    def property_get():
        key=cstr(p.x(0),256); value=b'arm64-v8a' if key==b'ro.arch' else b''
        p.u.mem_write(p.x(1),value+b'\0');p.x(0,len(value))
    p.callbacks['__system_property_get']=property_get
    def strncmp():
        a,b,n=p.x(0),p.x(1),p.x(2); x=bytes(p.u.mem_read(a,n)); y=bytes(p.u.mem_read(b,n)); k=0
        for l,r in zip(x,y):
            if l!=r:k=l-r;break
            if l==0:break
        p.x(0,k)
    p.callbacks['strncmp']=strncmp
    locale=p.alloc(64); locale_name=p.alloc(2); p.u.mem_write(locale_name,b'C\0'); current_locale=[0x7009200]
    def newlocale():
        name=cstr(p.x(1),128)
        if name not in (b'C',b'POSIX'):raise RuntimeError(f'unsupported locale request {name!r}')
        p.x(0,locale)
    def uselocale():
        old=current_locale[0]
        if p.x(0):current_locale[0]=p.x(0)
        p.x(0,old)
    p.callbacks['newlocale']=newlocale; p.callbacks['uselocale']=uselocale; p.callbacks['freelocale']=lambda:p.x(0,0)
    p.callbacks['setlocale']=lambda:p.x(0,locale_name)
    def mbtowc():
        out,src,n=p.x(0),p.x(1),p.x(2)
        if not src or not n:p.x(0,0xffffffffffffffff);return
        b=p.u.mem_read(src,1)[0]
        if b>=0x80:p.x(0,0xffffffffffffffff);return
        if out:p.put(out,'I',b)
        p.x(0,0 if b==0 else 1)
    p.callbacks['mbtowc']=mbtowc
    file_objects={}; file_rows=[]
    def fopen_mem():
        name,mode=cstr(p.x(0),1024),cstr(p.x(1),32)
        if mode not in (b'wb',b'rb'):raise RuntimeError(f'unsupported native cache file mode {mode!r}')
        token=p.alloc(64);file_objects[token]=bytearray();file_rows.append({'name':name.decode(errors='replace'),'mode':mode.decode(),'bytes':file_objects[token]});p.x(0,token)
    def fwrite_mem():
        ptr,size,count,token=(p.x(i) for i in range(4))
        if token not in file_objects or size*count>1<<20:raise RuntimeError('invalid native cache fwrite')
        raw=bytes(p.u.mem_read(ptr,size*count));file_objects[token].extend(raw);file_rows[-1]['bytes']=file_objects[token];p.x(0,count)
    def close_mem():
        token=p.x(0)
        if token not in file_objects:raise RuntimeError('unknown native cache fclose')
        p.x(0,0)
    p.callbacks['fopen']=fopen_mem; p.callbacks['fwrite']=fwrite_mem; p.callbacks['fclose']=close_mem; p.callbacks['fflush']=lambda:p.x(0,0); p.callbacks['__errno']=lambda:p.x(0,locale)
    p.callbacks['setbuf']=lambda:None
    p.callbacks['fileno']=lambda:p.x(0,0)
    def strchr():
        ptr,ch=p.x(0),p.x(1)&255;raw=bytes(p.u.mem_read(ptr,4096));pos=raw.find(bytes([ch]));p.x(0,ptr+pos if pos>=0 else 0)
    p.callbacks['strchr']=strchr
    def memchr():
        ptr,ch,n=p.x(0),p.x(1)&255,p.x(2);raw=bytes(p.u.mem_read(ptr,n));off=raw.find(bytes([ch]));p.x(0,ptr+off if off>=0 else 0)
    p.callbacks['memchr']=memchr
    p.callbacks['fsync']=lambda:p.x(0,0)
    def strlen_chk():
        raw=cstr(p.x(0),min(p.x(1),65536));p.x(0,len(raw))
    p.callbacks['__strlen_chk']=strlen_chk
    def strncpy_chk2():
        dst,src,n,dsize,ssize=(p.x(i) for i in range(5))
        if n>4096 or n>dsize or n>ssize:raise RuntimeError(f'fortified strncpy exceeds bound: n={n} dst={dsize} src={ssize}')
        raw=bytes(p.u.mem_read(src,n));zero=raw.find(b'\0')
        if zero>=0:raw=raw[:zero]+bytes(n-zero)
        p.u.mem_write(dst,raw);p.x(0,dst)
    p.callbacks['__strncpy_chk2']=strncpy_chk2
    # Single-thread pthread_once thunk executes the native initializer callback.
    p.u.mem_write(0x20000080,bytes([0xf9,0x00,0x00,0x08,0xe8,0x00,0x00,0xb5,0xfd,0x7b,0xbf,0xa9,0xfd,0x03,0x00,0x91,0x28,0x00,0x80,0xd2,0x08,0x00,0x00,0xf9,0x20,0x00,0x3f,0xd6,0xfd,0x7b,0xc1,0xa8,0x00,0x00,0x80,0xd2,0xc0,0x03,0x5f,0xd6]))
    p.syms['pthread_once']=0x20000080; p.callbacks.pop('pthread_once',None)
    p.callbacks['__cxa_atexit']=lambda:p.x(0,0)
    def init_arrays(path,base):
        blob=path.read_bytes(); sh=struct.unpack_from('<Q',blob,40)[0]; es,num,si=struct.unpack_from('<HHH',blob,58)
        sections=[struct.unpack_from('<IIQQQQIIQQ',blob,sh+i*es) for i in range(num)]; names=blob[sections[si][4]:sections[si][4]+sections[si][5]]
        init=next((s for s in sections if names[s[0]:names.find(b'\0',s[0])]==b'.init_array'),None)
        if not init:return 0
        called=0
        for off in range(0,init[5],8):
            target=p.get(base+init[3]+off,'Q')[0]
            if not target:continue
            for i in range(4):p.x(i,0)
            p.u.reg_write(R.UC_ARM64_REG_SP,0x200f0000);p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
            try:p.u.emu_start(target,0x200f1000,count=3000000)
            except Exception as exc:raise RuntimeError(f'{path.name} init entry {target:#x} failed pc={p.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={p.x(30):#x}: {exc}') from exc
            if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError(f'{path.name} initializer did not return: {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
            called+=1
        return called
    init_counts={'libc++_shared.so':init_arrays(CPP,CPP_BASE)}
    for path,base in resident:
        init_counts[path.name]=init_arrays(path,base)
    init_counts['libSPenInkPen2.so']=init_arrays(INK,BASE)
    gl_trace=[]
    pre_invalidate=[]
    gl_state={'framebuffer':0,'program':0,'blend_equation':None,'blend_function':None,'array_buffer':0,'last_draw':None}
    gl_errors=[]
    def check_gl(label):
        found=[]
        for _ in range(32):
            code=p.mesa.f['glGetError']()
            if not code:break
            found.append(int(code))
        else:raise RuntimeError(f'Mesa GL error queue did not drain after {label}')
        gl_errors.append({'phase':label,'errors':found})
        if found:raise RuntimeError(f'Mesa GL errors after {label}: {found}')
    gl_arg_counts={'glBindFramebuffer':2,'glFramebufferTexture2D':5,'glCheckFramebufferStatus':1,'glViewport':4,'glClearColor':0,'glClear':1,'glInvalidateFramebuffer':3,'glBindTexture':2,'glActiveTexture':1,'glUseProgram':1,'glUniform1i':2,'glUniform1f':1,'glUniform2f':1,'glUniform4f':1,'glUniform4fv':3,'glUniformMatrix4fv':4,'glBlendEquationSeparate':2,'glBlendFuncSeparate':4,'glEnable':1,'glDisable':1,'glColorMask':4,'glDrawArrays':3,'glDrawArraysInstanced':4,'glGenFramebuffers':2,'glGenTextures':2,'glTexImage2D':8,'glTexSubImage2D':8,'glTexParameteri':3,'glVertexAttribPointer':6,'glVertexAttribDivisor':2,'glEnableVertexAttribArray':1,'glDisableVertexAttribArray':1,'glBufferData':4,'glBufferSubData':4,'glBindBuffer':2}
    for glname,argc in gl_arg_counts.items():
        if glname not in p.callbacks:continue
        callback=p.callbacks[glname]
        def wrapped(name=glname,n=argc,fn=callback):
            row={'call':name,'x':[hex(p.x(i)) for i in range(n)],'s':[p.f(i) for i in range(8)],'state_before':dict(gl_state)}
            if name=='glClearColor':row['s']=[p.f(i) for i in range(4)]
            if name=='glBufferData':row['buffer_bytes']=bytes(p.u.mem_read(p.x(2),min(p.x(1),256))).hex() if p.x(2) and p.x(1)<=256 else None
            if name=='glBufferSubData':row['buffer_bytes']=bytes(p.u.mem_read(p.x(3),min(p.x(2),256))).hex() if p.x(3) and p.x(2)<=256 else None
            if name=='glVertexAttribPointer':row['attrib']={'index':p.x(0),'size':p.x(1),'type':hex(p.x(2)),'stride':p.x(4),'offset':hex(p.x(5))}
            if name=='glUniformMatrix4fv':row['matrix_values']=list(p.get(p.x(3),'16f'))
            if name=='glUniform4fv':row['uniform_values']=list(struct.unpack('<4f',p.u.mem_read(p.x(2),16)))
            if name=='glUniform4f':row['uniform_values']=[p.f(i) for i in range(4)]
            if name=='glGenFramebuffers' and 0<p.x(0)<=16:row['ids_before']=[p.get(p.x(1)+4*i,'I')[0] for i in range(p.x(0))]
            if name=='glInvalidateFramebuffer':
                fb=C.c_int(); f['glGetIntegerv'](0x8CA6,C.byref(fb))
                objtype,objname=C.c_int(),C.c_int()
                f['glGetFramebufferAttachmentParameteriv'](0x8D40,0x8CE0,0x8CD0,C.byref(objtype))
                f['glGetFramebufferAttachmentParameteriv'](0x8D40,0x8CE0,0x8CD1,C.byref(objname))
                pix=C.create_string_buffer(128*96*4); f['glPixelStorei'](0x0D05,1); f['glReadPixels'](0,0,128,96,0x1908,0x1401,pix)
                rgba=bytes(pix.raw); pixels=[rgba[i:i+4] for i in range(0,len(rgba),4)]
                pre_invalidate.append({'framebuffer':fb.value,'attachment_type':objtype.value,'attachment_name':objname.value,'rgba_sha256':hashlib.sha256(rgba).hexdigest(),'rgba_bytes':rgba.hex(),'center_rgba':list(rgba[(48*128+64)*4:(48*128+64)*4+4]),'nonzero_rgb':sum(1 for r,g,b,a in pixels if r or g or b),'max_rgb':[max(p[c] for p in pixels) for c in range(3)],'sum_rgb':[sum(p[c] for p in pixels) for c in range(3)],'nonzero_xy_rgba':[(i%128,i//128,list(p)) for i,p in enumerate(pixels) if p[0] or p[1] or p[2]][:32],'draw_state':dict(gl_state)})
            fn()
            if name=='glGenFramebuffers' and 0<p.x(0)<=16:row['ids_after']=[p.get(p.x(1)+4*i,'I')[0] for i in range(p.x(0))]
            if name=='glCheckFramebufferStatus':row['status']=p.x(0)
            if name=='glDrawArraysInstanced':row['instance_count']=row['x'][3]
            if name=='glFramebufferTexture2D':row['attachment_args']=row['x']
            if name=='glBindFramebuffer':gl_state['framebuffer']=p.x(1)
            if name=='glUseProgram':
                gl_state['program']=p.x(0)
            if name=='glBindBuffer' and p.x(0)==0x8892:gl_state['array_buffer']=p.x(1)
            if name=='glBlendEquationSeparate':gl_state['blend_equation']=row['x']
            if name=='glBlendFuncSeparate':gl_state['blend_function']=row['x']
            if name=='glVertexAttribPointer':row['vertex_buffer']=gl_state['array_buffer']
            if name in ('glDrawArrays','glDrawArraysInstanced'):
                row['draw_state']=dict(gl_state)
                gl_state['last_draw']={'call':name,'framebuffer':gl_state['framebuffer'],'program':gl_state['program'],'blend_equation':gl_state['blend_equation'],'blend_function':gl_state['blend_function'],'args':row['x']}
            gl_trace.append(row)
        p.callbacks[glname]=wrapped
    manager=p.alloc(256); call(p,'SPen::PenGLDataManager::PenGLDataManager()',[manager])
    queue=p.alloc(32); qvt=p.alloc(64); p.put(queue,'Q',qvt); p.queue=queue
    p.put(qvt+16,'Q',0x20001020); p.u.mem_write(0x20001020,bytes.fromhex('1f2003d5c0035fd6'))
    queue_calls=[]
    def trace_queue_stub(u,address,size,data):
        if address==0x20001020:
            queue_calls.append({'x0_x3':[u.reg_read(r) for r in (R.UC_ARM64_REG_X0,R.UC_ARM64_REG_X1,R.UC_ARM64_REG_X2,R.UC_ARM64_REG_X3)],'lr':u.reg_read(R.UC_ARM64_REG_LR)})
    p.u.hook_add(UC_HOOK_CODE,trace_queue_stub)
    p.callbacks['SPen::PenGLDataManager::GetMsgQueue()']=lambda:p.x(0,queue)
    call(p,'SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue])
    rt=p.alloc(0x300)
    call(p,'SPen::InkPen2StrokeDrawableRTV3::InkPen2StrokeDrawableRTV3(SPen::PenGLDataManager*)',[rt,manager])
    try:call(p,'SPen::InkPen2StrokeDrawableRTV3::Init()',[rt])
    except Exception:
        print('ENTRY_TRACE',entry_trace[-40:]);raise
    check_gl('Init')
    call(p,'SPen::InkPen2StrokeDrawableRTV3::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',[rt,128,96,queue])
    check_gl('CreatePenCanvas')
    canvas=p.get(rt+200,'Q')[0]
    call(p,'SPen::SPPenCanvasRT::GetBitmap()',[canvas]); bitmap=p.x(0)
    call(p,'SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]); subbitmap=p.x(0)
    inv=p.alloc(8);p.put(inv,'2f',1.0,1.0);call(p,'SPen::InkPen2StrokeDrawableRTV3::SetInverseScale(SPen::Vector2<float> const&)',[rt,inv])
    color_vec=p.alloc(24); color_buf=p.alloc(4096); p.put(color_vec,'3Q',color_buf,color_buf,color_buf+4096); p.put(rt+80,'Q',color_vec); p.put(rt+88,'Q',color_vec)
    p.put(rt+296,'2f',1.0,1.0)
    settings=p.alloc(64);p.put(settings,'fI',6.0,input_argb);p.put(settings+44,'f',1.0);p.put(settings+52,'B',1)
    call(p,'SPen::PenDrawableRT::SetPenSettingData(SPen::PenSettingData*)',[rt,settings])
    add_color='SPen::InkPen2StrokeDrawableRTV3::AddPoint(SPen::Vector2<float>, float, SPen::Vector3<float>)'
    for x,y,rgb in point_inputs:
        call(p,add_color,[rt],(x,y,0.7,*rgb))
    color_begin,color_end,color_cap=p.get(color_vec,'3Q'); color_raw=bytes(p.u.mem_read(color_begin,color_end-color_begin))
    records=[list(struct.unpack_from('<9f',color_raw,i)) for i in range(0,len(color_raw),36)]
    call(p,'SPen::InkPen2StrokeDrawableRTV3::Update(bool)',[rt,0])
    matrix=p.alloc(64);p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
    try:call(p,'SPen::InkPen2StrokeDrawableRTV3::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,subbitmap,matrix])
    except Exception:
        print('DRAW_ENTRY_TRACE',entry_trace[-40:]);raise
    check_gl('Update+Draw+readback')
    if len(pre_invalidate)!=5:
        raise RuntimeError(f'expected two setup and three draw-stage invalidations, found {len(pre_invalidate)}')
    stage_names=['max_alpha_mask','source_over_canvas','final_composite']
    stage_readbacks=[]
    for stage,name in zip(pre_invalidate[2:],stage_names):
        stage_readbacks.append({'stage':name,**stage})
    source_records=bytes(p.u.mem_read(color_begin,color_end-color_begin))
    upload_rows=[r for r in gl_trace if r['call']=='glBufferData' and r.get('buffer_bytes')]
    matching_uploads=[r for r in upload_rows if bytes.fromhex(r['buffer_bytes'])[:len(source_records)]==source_records]
    if not matching_uploads:
        raise RuntimeError('native RTV3 record bytes were not found in a native glBufferData upload')
    used_program_ids=sorted({d['program'] for d in (r['draw_state'] for r in gl_trace if r['call'] in ('glDrawArrays','glDrawArraysInstanced'))})
    programs={}
    for program in used_program_ids:
        row=program_rows.get(program,{'shaders':[],'linked':None})
        snapshots=row.get('shader_snapshots',[])
        programs[str(program)]={'linked':row['linked'],'shaders':[{'id':shader,'type':snapshot.get('type'),'compiled':snapshot.get('compiled'),'source':'\n'.join(snapshot.get('sources',[]))} for shader,snapshot in zip(row['shaders'],snapshots)]}
    version_fn=p.mesa.gl.glGetString; version_fn.argtypes=[C.c_uint]; version_fn.restype=C.c_char_p
    host_gl={name:(version_fn(pname) or b'').decode('utf-8','replace') for name,pname in [('vendor',0x1F00),('renderer',0x1F01),('version',0x1F02),('shading_language_version',0x8B8C)]}
    library_paths=[LIB/name for name in ('libSPenFountainPen.so','libSPenPenCommon.so','libSPenBase.so','libSPenBrushPen.so','libSPenGraphics.so','libSPenRenderer.so','libSPenModel.so','libSPenInkPen2.so','libc++_shared.so')]
    helper_paths=[ROOT/'conformance'/name for name in ('brushpen_paint_positive_native.py','brushpen_saved_native.py','inkpen_paint_native.py','fountain_native.py','elf_plt.py')]
    relevant_calls=('glTexImage2D','glBindFramebuffer','glFramebufferTexture2D','glCheckFramebufferStatus','glViewport','glClearColor','glClear','glInvalidateFramebuffer','glBindTexture','glActiveTexture','glUseProgram','glUniform1i','glUniform1f','glUniform2f','glUniform4f','glUniform4fv','glUniformMatrix4fv','glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask','glDrawArrays','glDrawArraysInstanced','glVertexAttribPointer','glVertexAttribDivisor','glEnableVertexAttribArray','glDisableVertexAttribArray','glBufferData','glBufferSubData','glBindBuffer')
    fixture_case={
        'id':case_id,
        'input':{'canvas_width':128,'canvas_height':96,'inverse_scale':[1.0,1.0],'set_pen_setting_data':{'allocation_size':64,'field_layout':{'size_f32':0,'argb_u32':4,'field_44_f32':44,'branch_flag_u8':52},'raw_hex':bytes(p.u.mem_read(settings,56)).hex(),'size':6.0,'argb':input_argb,'field_44':1.0,'branch_flag':1},'points':[{'position':[x,y],'pressure':0.7,'rgb':list(rgb)} for x,y,rgb in point_inputs]},
        'host_setup':{'drawable':'native InkPen2StrokeDrawableRTV3 constructed and initialized by pinned DSO','buffer_wiring':'test host allocates a bounded std::vector<float> ABI buffer and installs its pointer at RTV3 offsets +80 and +88','canvas':'native RTV3::CreatePenCanvas, native SPPenCanvasRT::GetBitmap, native SPTextureBitmap::GetSubBitmap(0)','queue':'host IGLMsgQueue slot+16 is a bounded synchronous no-op return; no task delivery is claimed','queue_slot_calls':queue_calls,'saved_drawing_or_device_profile':False},
        'native_calls':['InkPen2StrokeDrawableRTV3::InkPen2StrokeDrawableRTV3','InkPen2StrokeDrawableRTV3::Init','InkPen2StrokeDrawableRTV3::CreatePenCanvas(128,96)','InkPen2StrokeDrawableRTV3::SetInverseScale([1,1])','PenDrawableRT::SetPenSettingData','InkPen2StrokeDrawableRTV3::AddPoint(Vector2,float,Vector3) per input point','InkPen2StrokeDrawableRTV3::Update(false)','InkPen2StrokeDrawableRTV3::Draw(subbitmap,matrix)'],
        'input_vector_records':records,
        'input_vector_bytes_hex':source_records.hex(),
        'input_vector_sha256':hashlib.sha256(source_records).hexdigest(),
        'gpu_uploads':[{'size_bytes':int(r['x'][1],16),'bytes_hex':r['buffer_bytes'],'sha256':hashlib.sha256(bytes.fromhex(r['buffer_bytes'])).hexdigest(),'usage':r['x'][3]} for r in matching_uploads],
        'shader_programs':programs,
        'texture_allocations':[{'target':r['x'][0],'level':r['x'][1],'internal_format':r['x'][2],'width':r['x'][3],'height':r['x'][4],'format':r['x'][6],'type':r['x'][7]} for r in gl_trace if r['call']=='glTexImage2D'],
        'framebuffer_attachments':[{'state_before':r['state_before'],'args':r['x']} for r in gl_trace if r['call']=='glFramebufferTexture2D'],
        'draw_calls':[r['draw_state'] for r in gl_trace if r['call'] in ('glDrawArrays','glDrawArraysInstanced')],
        'attributes':[{'attrib':r['attrib'],'array_buffer':r.get('vertex_buffer'),'state_before':r['state_before']} for r in gl_trace if r['call']=='glVertexAttribPointer'],
        'uniform_calls':[{'call':r['call'],'args':r['x'],'values':r.get('uniform_values',r.get('matrix_values')),'state_before':r['state_before']} for r in gl_trace if r['call'] in ('glUniformMatrix4fv','glUniform4fv','glUniform4f','glUniform1f','glUniform2f','glUniform1i')],
        'blend_calls':[{'equation':r['x'],'state_before':r['state_before']} if r['call']=='glBlendEquationSeparate' else {'function':r['x'],'state_before':r['state_before']} for r in gl_trace if r['call'] in ('glBlendEquationSeparate','glBlendFuncSeparate')],
        'stage_readbacks_before_invalidate':stage_readbacks,
        'gl_error_checkpoints':gl_errors,
        'init_array_counts':init_counts,
        'host_gl':host_gl,
        'native_libraries_sha256':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in library_paths},
        'conformance_helper_sha256':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in helper_paths},
        'scope_note':'Synthetic native RTV3 input and settings; Mesa GLES output from the pinned ARM64 library behavior. The RGBA8 source-over canvas and the GL_R8 max-mask/final-composite target are separate products. The final composite target red lane is premultiplied output color, not a stand-alone alpha mask. This is not saved Drawing, parser, device, or production-dispatch evidence.'
    }
    return fixture_case

def capture(case_id):
    buf=io.StringIO()
    with contextlib.redirect_stdout(buf):
        return main(case_id)

def capture_all():
    env=dict(os.environ)
    env['PYTHONPATH']=str(ROOT/'scratch/apk-analysis-runtime/python')+os.pathsep+env.get('PYTHONPATH','')
    cases=[]
    for case_id in SCENARIOS:
        result=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--capture',case_id],cwd=ROOT,env=env,text=True,capture_output=True)
        if result.returncode:
            raise RuntimeError(f'{case_id} native capture failed ({result.returncode}):\n{result.stdout}\n{result.stderr}')
        marker=next((line[len('CAPTURE_JSON='):] for line in result.stdout.splitlines() if line.startswith('CAPTURE_JSON=')),None)
        if marker is None:raise RuntimeError(f'{case_id} native capture produced no JSON marker: {result.stdout[-2000:]}')
        cases.append(json.loads(marker))
    by_id={case['id']:case for case in cases}
    if set(by_id)!=set(SCENARIOS):raise RuntimeError('native paint control set is incomplete')
    def stage(case_id,name):
        return next(row for row in by_id[case_id]['stage_readbacks_before_invalidate'] if row['stage']==name)
    if stage('single','max_alpha_mask')['rgba_bytes']!=stage('coincident_same','max_alpha_mask')['rgba_bytes']:
        raise RuntimeError('coincident same-RGB max mask changed from single stamp')
    if stage('single','final_composite')['rgba_bytes']!=stage('coincident_same','final_composite')['rgba_bytes']:
        raise RuntimeError('same-RGB overlap changed final composite color')
    def first_stamp_pixels(case_id,name):
        row=stage(case_id,name)
        return [(x,y,pixel) for x,y,pixel in row['nonzero_xy_rgba'] if x<80]
    if first_stamp_pixels('single','final_composite')!=first_stamp_pixels('separated','final_composite'):
        raise RuntimeError('separated control changed the first stamp output pixels')
    if stage('coincident_different','final_composite')['rgba_bytes']==stage('single','final_composite')['rgba_bytes']:
        raise RuntimeError('different-RGB overlap failed to expose source-over color normalization')
    for low, full in [('single_half_alpha', 'single'), ('coincident_different_half_alpha', 'coincident_different')]:
        if stage(low, 'max_alpha_mask')['rgba_bytes'] != stage(full, 'max_alpha_mask')['rgba_bytes']:
            raise RuntimeError('input alpha changed the native geometric max mask')
        if stage(low, 'final_composite')['rgba_bytes'] == stage(full, 'final_composite')['rgba_bytes']:
            raise RuntimeError('input alpha failed to change native final composition')
    for case in cases:
        argb = case['input']['set_pen_setting_data']['argb']
        rgba = [struct.unpack('<f', struct.pack('<f', channel / 255.0))[0] for channel in ((argb >> 16) & 255, (argb >> 8) & 255, argb & 255, argb >> 24)]
        color_uniforms = [row['values'] for row in case['uniform_calls'] if row['call'] == 'glUniform4fv']
        if color_uniforms != [rgba, rgba]:
            raise RuntimeError(f"{case['id']} native shader uniforms did not preserve packed ARGB")
        if len(case['draw_calls'])!=3 or len(case['stage_readbacks_before_invalidate'])!=3:
            raise RuntimeError(f"{case['id']} did not traverse all three native paint stages")
        if any(row['errors'] for row in case['gl_error_checkpoints']):
            raise RuntimeError(f"{case['id']} has a Mesa GL error")
        if case['host_setup']['queue_slot_calls']:
            raise RuntimeError(f"{case['id']} unexpectedly entered the bounded no-op queue callback")
        expected_size=case['input']['canvas_width']*case['input']['canvas_height']*4
        if any(len(bytes.fromhex(row['rgba_bytes']))!=expected_size for row in case['stage_readbacks_before_invalidate']):
            raise RuntimeError(f"{case['id']} has an incomplete full-target readback")
        if any(len(bytes.fromhex(upload['bytes_hex']))!=upload['size_bytes'] for upload in case['gpu_uploads']):
            raise RuntimeError(f"{case['id']} has a truncated native vector upload")
    return {
        'schema':'sdocx.inkpen2.paint.native.v1',
        'paint_model':{
            'evidence':'derived from the captured native shader source, blend state, and Mesa readbacks',
            'max_mask':'M = max_i(alpha_i) per covered pixel (GL_MAX, ONE/ONE)',
            'source_over':'P_i = c_i * alpha_i; P = P_i + P_prev * (1 - alpha_i); A = alpha_i + A_prev * (1 - alpha_i)',
            'final_composite':'C = (P / A) * (M * input_alpha), when A > 0; the GL_R8 target R is premultiplied output color, not standalone alpha',
            'uniform_rgb_reduction':'when every record has the same RGB c, P/A = c, so final RGB is c * M * input_alpha; same-RGB overlap should preserve final color while differing RGB exposes source-over normalization',
            'scope':'This is the observed synthetic native RTV3 paint path, not a saved Drawing or production admission claim.'
        },
        'cases':cases,
    }

def cli():
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture',action='store_true')
    mode.add_argument('--check-fixture',action='store_true')
    mode.add_argument('--capture',choices=tuple(SCENARIOS))
    parser.add_argument('--fixture',type=Path,default=ROOT/'conformance/inkpen2-paint-native.json')
    args=parser.parse_args()
    if args.capture:
        print('CAPTURE_JSON='+json.dumps(capture(args.capture),sort_keys=True,separators=(',',':')))
        return
    actual=capture_all()
    serialized=json.dumps(actual,indent=2,sort_keys=True)+'\n'
    if args.write_fixture:
        args.fixture.write_text(serialized)
        print(f'wrote {args.fixture} ({len(actual["cases"])} native captures)')
    else:
        if not args.fixture.exists():raise SystemExit(f'missing fixture: {args.fixture}')
        expected=args.fixture.read_text()
        if json.loads(expected)!=actual:raise SystemExit(f'native output mismatch: {args.fixture}')
        print(f'checked {args.fixture} ({len(actual["cases"])} native captures)')

if __name__=='__main__':cli()
