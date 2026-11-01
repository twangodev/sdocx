"""Bounded attempt to open the retained saved note using native Model."""
import state_probe_capture as probe

probe.SCENE = r'''
    attempt={'scope':'native NoteDoc constructor using retained saved archive; host virtual cache I/O remains bounded. Mode 0 is exploratory, not established SDK behavior.','steps':[]}
    def invoke(label, symbol, args):
        try:
            result=native_call(p,symbol,args,5_000_000)
            attempt['steps'].append({'step':label,'result':hex(result)})
            return result
        except Exception as exc:
            attempt['steps'].append({'step':label,'error_type':type(exc).__name__,'error':str(exc)})
            raise
    def string(value):
        obj=p.alloc(64); text=value.encode()+b'\0'; ptr=p.alloc(len(text)); p.u.mem_write(ptr,text)
        invoke('String::String','SPen::String::String()',[obj])
        invoke('String::Construct','SPen::String::Construct(char const*)',[obj,ptr])
        return obj
    host_os=__import__('os'); opened={}
    note_type=int(host_os.environ.get('SDOCX_NOTE_TYPE','0'))
    if note_type not in (0,2): raise RuntimeError('unapproved note type')
    case=host_os.environ.get('SDOCX_NOTE_PROBE_CASE','handwritten')
    if case not in ('handwritten','quiz'): raise RuntimeError('unapproved probe case')
    archive=ROOT/'tmp/stroke-conformance'/f'{case}.sdocx'
    if note_type==2 and case!='handwritten':raise RuntimeError('type 2 control is pinned to handwritten')
    pinned={'handwritten':'77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3','quiz':'38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a'}
    if hashlib.sha256(archive.read_bytes()).hexdigest()!=pinned[case]:raise RuntimeError('saved archive hash drift')
    for name,want in [('libSPenModel.so','4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'),('libSPenZlib.so','7796339a9c41c3218bef47c17bc1573c544bc431a8d312da390dce40c64adcd7')]:
        if hashlib.sha256((LIB/name).read_bytes()).hexdigest()!=want:raise RuntimeError(f'{name} hash drift')
    attempt['inputs']={'archive_sha256':pinned[case],'native_note_type':note_type,'sdk_mode':0,'model_sha256':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','spen_zlib_sha256':'7796339a9c41c3218bef47c17bc1573c544bc431a8d312da390dce40c64adcd7'}
    from collections import deque
    pcs=deque(maxlen=1500); tracing=[False]
    pending=[]; parser_return=[None]; parser_pcs=deque(maxlen=160)
    def last_pcs(u,address,size,data):
        if tracing[0]:
            pcs.append(hex(address))
            if address==0x747ac20:
                attempt['steps'].append({'step':'native Error::SetError','code':p.x(0),'line':p.x(1),'source':cstring(p.x(2)) if p.x(2) else None,'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'preceding_pcs':list(pcs)[-40:]})
            if parser_return[0] is not None: parser_pcs.append(hex(address))
            if parser_return[0] is not None and address==parser_return[0]:
                attempt['binary_parser_return']={'x0':hex(p.x(0)),'pcs':list(parser_pcs)}; parser_return[0]=None
            if address==0x72a7df8:
                n=p.x(2)
                attempt['steps'].append({'step':'EndTag signature compare','length':n,'actual_hex':bytes(p.u.mem_read(p.x(0),n)).hex(),'expected_hex':bytes(p.u.mem_read(p.x(1),n)).hex()})
            if address==0x72a7d20:
                attempt['steps'].append({'step':'EndTag type','field_0x114':p.get(p.x(0)+0x114,'I')[0]})
                parser_return[0]=u.reg_read(R.UC_ARM64_REG_LR); parser_pcs.clear()
                pointer,length=p.x(1),p.x(2)
                attempt['steps'].append({'step':'EndTag binary parser entry','length':length,'data_hex':bytes(p.u.mem_read(pointer,min(length,256))).hex() if pointer and length<4096 else None})
            if address==0x72a77b4:
                lr=u.reg_read(R.UC_ARM64_REG_LR); pending.append(lr)
                attempt['steps'].append({'step':'EndTag::ParseImpl entry','args':[hex(p.x(i)) for i in range(4)],'lr':hex(lr)})
            elif pending and address==pending[-1]:
                pending.pop(); attempt['steps'].append({'step':'EndTag::ParseImpl return','x0':hex(p.x(0))})
    p.u.hook_add(UC_HOOK_CODE,last_pcs)
    def open_readonly():
        pointer=p.x(0); raw=bytearray()
        for offset in range(4096):
            b=bytes(p.u.mem_read(pointer+offset,1))
            if b==b'\0': break
            raw.extend(b)
        path=raw.decode(); flags=p.x(1)
        if path!=str(archive) or flags!=0:
            raise RuntimeError(f'unapproved open {path!r} flags={flags:#x}')
        if len(opened)>=16:raise RuntimeError('archive descriptor bound')
        fd=host_os.open(path,host_os.O_RDONLY); opened[fd]=path; p.x(0,fd)
        attempt['steps'].append({'step':'open','path':path,'flags':flags})
    def read_file():
        fd,dst,size=p.x(0),p.x(1),p.x(2)
        if fd not in opened or size>1048576: raise RuntimeError('unapproved read')
        data=host_os.read(fd,size); p.u.mem_write(dst,data); p.x(0,len(data)); attempt['steps'].append({'step':'read','size':size,'actual':len(data),'sha256':hashlib.sha256(data).hexdigest()})
    def read_checked():
        if p.x(2)>p.x(3): raise RuntimeError('read_chk exceeds buffer')
        read_file()
    def close_file():
        fd=p.x(0)
        if fd not in opened: raise RuntimeError('unapproved close')
        host_os.close(fd); del opened[fd]; p.x(0,0)
    def seek_file():
        fd,offset,whence=p.x(0),p.x(1),p.x(2)
        if fd not in opened: raise RuntimeError('unapproved seek')
        if offset>=1<<63: offset-=1<<64
        position=host_os.lseek(fd,offset,whence); p.x(0,position); attempt['steps'].append({'step':'lseek','offset':offset,'whence':whence,'result':position})
    p.callbacks.update({'open':open_readonly,'read':read_file,'__read_chk':read_checked,'close':close_file,'lseek':seek_file,'lseek64':seek_file})
    # Native minizip DSO and exact-path, read-only FILE adapter. Cache writes
    # continue to use the preexisting bounded, in-memory renderer filesystem.
    zlib_base=0x0a000000
    zlib_info=load_extra_dso(p, LIB/'libSPenZlib.so',zlib_base,env)
    install_exact_plt_map(p,env,(('Renderer',RENDERER_BASE),('libc++_shared',LIBCPP_BASE),('Zlib',zlib_base)))
    attempt['zlib_loader']=zlib_info
    file_handles={}
    original_fns={name:p.callbacks[name] for name in ['fopen','fopen64','fread','fseek','fseek64','ftell','ftell64','fclose','feof','ferror']}
    def cstring(ptr):
        raw=bytearray()
        for i in range(4096):
            b=bytes(p.u.mem_read(ptr+i,1))
            if b==b'\0': return raw.decode()
            raw.extend(b)
        raise RuntimeError('unterminated file path')
    def open_stream():
        path,mode=cstring(p.x(0)),cstring(p.x(1))
        if path==str(archive) and mode in ('r','rb'):
            if len(file_handles)>=16:raise RuntimeError('archive stream bound')
            fd=host_os.open(path,host_os.O_RDONLY); handle=p.alloc(16); file_handles[handle]=fd; p.x(0,handle)
            attempt['steps'].append({'step':'fopen','path':path,'mode':mode})
        else: original_fns['fopen']()
    def fread_stream():
        dst,size,count,handle=[p.x(i) for i in range(4)]
        if handle not in file_handles: return original_fns['fread']()
        if size*count>1048576: raise RuntimeError('stream read bound')
        data=host_os.read(file_handles[handle],size*count); p.u.mem_write(dst,data); p.x(0,len(data)//size if size else 0)
    def seek_stream():
        handle,offset,whence=p.x(0),p.x(1),p.x(2)
        if handle not in file_handles:return original_fns['fseek']()
        if offset>=1<<63: offset-=1<<64
        host_os.lseek(file_handles[handle],offset,whence); p.x(0,0)
    def tell_stream():
        handle=p.x(0)
        if handle not in file_handles:return original_fns['ftell']()
        p.x(0,host_os.lseek(file_handles[handle],0,1))
    def close_stream():
        handle=p.x(0)
        if handle not in file_handles:return original_fns['fclose']()
        host_os.close(file_handles.pop(handle));p.x(0,0)
    def eof_stream():
        handle=p.x(0)
        if handle not in file_handles:return original_fns['feof']()
        fd=file_handles[handle];p.x(0,int(host_os.lseek(fd,0,1)>=host_os.fstat(fd).st_size))
    def error_stream():
        if p.x(0) not in file_handles:return original_fns['ferror']()
        p.x(0,0)
    p.callbacks.update({'fopen':open_stream,'fopen64':open_stream,'fread':fread_stream,'fseek':seek_stream,'fseek64':seek_stream,'ftell':tell_stream,'ftell64':tell_stream,'fclose':close_stream,'feof':eof_stream,'ferror':error_stream})
    # Host zlib stream state is never exposed as an ARM64 pointer. Guest
    # buffers are copied under explicit bounds; no guest allocator is invoked.
    class HostZStream(C.Structure):
        _fields_=[('next_in',C.c_void_p),('avail_in',C.c_uint),('total_in',C.c_ulong),('next_out',C.c_void_p),('avail_out',C.c_uint),('total_out',C.c_ulong),('msg',C.c_void_p),('state',C.c_void_p),('zalloc',C.c_void_p),('zfree',C.c_void_p),('opaque',C.c_void_p),('data_type',C.c_int),('adler',C.c_ulong),('reserved',C.c_ulong)]
    libz=C.CDLL('libz.so.1'); libz.zlibVersion.restype=C.c_char_p
    libz.inflateInit2_.argtypes=[C.POINTER(HostZStream),C.c_int,C.c_char_p,C.c_int]
    libz.inflate.argtypes=[C.POINTER(HostZStream),C.c_int];libz.inflateEnd.argtypes=[C.POINTER(HostZStream)]
    streams={}
    def inflate_init():
        guest,window,version,size=[p.x(i) for i in range(4)]
        if size!=C.sizeof(HostZStream) or p.get(guest+64,'Q')[0] or p.get(guest+72,'Q')[0]:raise RuntimeError('unsupported zlib ABI or guest allocator')
        if window>=1<<31:window-=1<<32
        stream=HostZStream();ret=libz.inflateInit2_(C.byref(stream),window,libz.zlibVersion(),size)
        if ret==0:streams[guest]=stream;p.put(guest+56,'Q',1)
        p.x(0,ret);attempt['steps'].append({'step':'inflateInit2','window':window,'result':ret})
    def inflate():
        guest,flush=p.x(0),p.x(1)
        if guest not in streams:raise RuntimeError('unknown inflate stream')
        stream=streams[guest];src,nin,dst,nout=p.get(guest,'Q')[0],p.get(guest+8,'I')[0],p.get(guest+24,'Q')[0],p.get(guest+32,'I')[0]
        if nin>1048576 or nout>1048576:raise RuntimeError('inflate buffer bound')
        inp=C.create_string_buffer(bytes(p.u.mem_read(src,nin))) if nin else None;out=C.create_string_buffer(nout) if nout else None
        stream.next_in=C.cast(inp,C.c_void_p).value if inp else None;stream.avail_in=nin
        stream.next_out=C.cast(out,C.c_void_p).value if out else None;stream.avail_out=nout
        oldin,oldout=stream.total_in,stream.total_out;ret=libz.inflate(C.byref(stream),flush)
        consumed,produced=stream.total_in-oldin,stream.total_out-oldout
        if produced:p.u.mem_write(dst,C.string_at(out,produced))
        p.put(guest,'Q',src+consumed);p.put(guest+8,'I',stream.avail_in);p.put(guest+16,'Q',stream.total_in)
        p.put(guest+24,'Q',dst+produced);p.put(guest+32,'I',stream.avail_out);p.put(guest+40,'Q',stream.total_out)
        p.put(guest+88,'I',stream.data_type);p.put(guest+96,'Q',stream.adler);p.x(0,ret)
    def inflate_end():
        guest=p.x(0)
        if guest not in streams:raise RuntimeError('unknown inflate end')
        ret=libz.inflateEnd(C.byref(streams.pop(guest)));p.put(guest+56,'Q',0);p.x(0,ret)
    def crc():
        checksum,ptr,n=p.x(0),p.x(1),p.x(2)
        if n>1048576:raise RuntimeError('crc bound')
        p.x(0,zlib.crc32(bytes(p.u.mem_read(ptr,n)) if n else b'',checksum)&0xffffffff)
    p.callbacks.update({'inflateInit2_':inflate_init,'inflate':inflate,'inflateEnd':inflate_end,'crc32':crc})
    def checked_vsnprintf():
        if p.x(1)>p.x(3):raise RuntimeError('vsnprintf_chk capacity')
        dst,capacity,fmtptr,vaptr=p.x(0),p.x(1),p.x(4),p.x(5)
        fmt=cstring(fmtptr)
        if fmt not in ('%x@','%x','%s/%s'):raise RuntimeError(f'unhandled checked format {fmt!r}')
        stack,gr_top,vr_top,gr_off,vr_off=struct.unpack('<QQQii',bytes(p.u.mem_read(vaptr,32)))
        if gr_off<0 and gr_top: arg=p.get(gr_top+gr_off,'Q')[0]
        elif stack: arg=p.get(stack,'Q')[0]
        else: raise RuntimeError('invalid va_list')
        if fmt=='%s/%s':
            first=arg;off=gr_off+8
            second=p.get(gr_top+off,'Q')[0] if off<0 and gr_top else p.get(stack,'Q')[0]
            rendered=(cstring(first)+'/'+cstring(second)).encode()
        else:rendered=(f'{arg & 0xffffffff:x}' + ('@' if fmt.endswith('@') else '')).encode()
        n=min(len(rendered),capacity-1) if capacity else 0
        if capacity:p.u.mem_write(dst,rendered[:n]+b'\0')
        p.x(0,len(rendered));attempt['steps'].append({'step':'vsnprintf_chk','format':fmt,'value':arg & 0xffffffff,'result':rendered.decode()})
    p.callbacks['__vsnprintf_chk']=checked_vsnprintf
    cache_root='/tmp/sdocx-fountain-note-cache';cache_dirs={cache_root};cache_data={}
    orig_access=p.callbacks['access'];orig_mkdir=p.callbacks['mkdir']
    def in_cache(path):
        return (path==cache_root or path.startswith(cache_root+'/')) and '\\' not in path and all(part not in ('.','..') for part in path.split('/'))
    def cache_access():
        path=cstring(p.x(0));mode=p.x(1)
        if not in_cache(path):return orig_access()
        exists=path in cache_dirs or path in cache_data
        if not exists:set_virtual_errno(2)
        p.x(0,0 if exists else -1);attempt['steps'].append({'step':'cache access','path':path,'mode':mode,'exists':exists})
    def cache_mkdir():
        path=cstring(p.x(0));mode=p.x(1)
        if not in_cache(path):return orig_mkdir()
        if path in cache_dirs:set_virtual_errno(17);p.x(0,-1)
        else:
            if len(cache_dirs)>=64:raise RuntimeError('cache directory count bound')
            cache_dirs.add(path);p.x(0,0)
        attempt['steps'].append({'step':'cache mkdir','path':path,'mode':mode})
    p.callbacks.update({'access':cache_access,'mkdir':cache_mkdir})
    cache_handles={};cache_io=__import__('io')
    from cache_budget import within_budget, write_reservation
    def cache_reserve(extra):
        # Pessimistically reserve twice each live payload/copy for allocation slack.
        live=sum(map(len,cache_data.values()))+sum(len(item[2].getbuffer()) for item in cache_handles.values())
        if not within_budget(live,extra):raise RuntimeError('aggregate cache memory budget')
    stream_fns={k:p.callbacks[k] for k in ('fopen','fopen64','fread','fwrite','fseek','fseek64','ftell','ftell64','fclose','feof','ferror','fflush')}
    def cache_fopen():
        path,mode=cstring(p.x(0)),cstring(p.x(1))
        if not in_cache(path):return stream_fns['fopen']()
        if mode not in ('rb','r','wb','w','w+b','r+b','ab'):raise RuntimeError(f'unsupported cache fopen mode {mode!r}')
        if mode.startswith('r') and path not in cache_data:set_virtual_errno(2);p.x(0,0);return
        if len(cache_handles)>=32 or len(set(cache_data) | {item[0] for item in cache_handles.values()} | {path})>64:raise RuntimeError('cache file count bound')
        data=cache_data.get(path,b'') if not mode.startswith('w') else b''
        cache_reserve(len(data))
        handle=p.alloc(16);stream=cache_io.BytesIO(data)
        if mode.startswith('a'):stream.seek(0,2)
        cache_handles[handle]=(path,mode,stream);p.x(0,handle);attempt['steps'].append({'step':'cache fopen','path':path,'mode':mode})
    def cache_fread():
        dst,size,count,handle=[p.x(i) for i in range(4)]
        if handle not in cache_handles:return stream_fns['fread']()
        if size*count>1048576:raise RuntimeError('cache read bound')
        cache_reserve(size*count)
        path,mode,stream=cache_handles[handle];offset=stream.tell();data=stream.read(size*count)
        attempt['steps'].append({'step':'cache fread','path':path,'offset':offset,'size':size,'count':count,'actual':len(data),'prefix_hex':data[:32].hex()})
        if data:p.u.mem_write(dst,data)
        p.x(0,len(data)//size if size else 0)
    def cache_fwrite():
        src,size,count,handle=[p.x(i) for i in range(4)]
        if handle not in cache_handles:return stream_fns['fwrite']()
        path,mode,stream=cache_handles[handle]
        if not mode.startswith(('w','a','r+')) or size*count>1048576 or stream.tell()+size*count>16777216:raise RuntimeError('cache write bound')
        cache_reserve(write_reservation(stream.tell(),len(stream.getbuffer()),size*count))
        stream.write(bytes(p.u.mem_read(src,size*count)));p.x(0,count)
    def cache_seek():
        handle,offset,whence=p.x(0),p.x(1),p.x(2)
        if handle not in cache_handles:return stream_fns['fseek']()
        if offset>=1<<63:offset-=1<<64
        if cache_handles[handle][2].seek(offset,whence)>16777216:raise RuntimeError('cache seek bound')
        p.x(0,0)
    def cache_tell():
        handle=p.x(0)
        if handle not in cache_handles:return stream_fns['ftell']()
        p.x(0,cache_handles[handle][2].tell())
    def cache_close():
        handle=p.x(0)
        if handle not in cache_handles:return stream_fns['fclose']()
        path,mode,stream=cache_handles[handle]
        if mode.startswith(('w','a','r+')):cache_reserve(len(stream.getbuffer()))
        cache_handles.pop(handle)
        if mode.startswith(('w','a','r+')):
            cache_data[path]=stream.getvalue();attempt['steps'].append({'step':'cache stored','path':path,'size':len(cache_data[path]),'sha256':hashlib.sha256(cache_data[path]).hexdigest()})
        stream.close();p.x(0,0)
    def cache_eof():
        handle=p.x(0)
        if handle not in cache_handles:return stream_fns['feof']()
        stream=cache_handles[handle][2];p.x(0,int(stream.tell()>=len(stream.getbuffer())))
    def cache_error():
        if p.x(0) not in cache_handles:return stream_fns['ferror']()
        p.x(0,0)
    def cache_flush():
        if p.x(0) not in cache_handles:return stream_fns['fflush']()
        p.x(0,0)
    p.callbacks.update({'fopen':cache_fopen,'fopen64':cache_fopen,'fread':cache_fread,'fwrite':cache_fwrite,'fseek':cache_seek,'fseek64':cache_seek,'ftell':cache_tell,'ftell64':cache_tell,'fclose':cache_close,'feof':cache_eof,'ferror':cache_error,'fflush':cache_flush})
    prior_strchr=p.callbacks['strchr']
    def cache_strchr():
        if not 0x2096180<=p.u.reg_read(R.UC_ARM64_REG_LR)<0x20961b0:return prior_strchr()
        pointer,char=p.x(0),p.x(1)&255;data=cstring(pointer).encode();i=data.find(bytes([char]));p.x(0,pointer+i if i>=0 else 0)
    p.callbacks['strchr']=cache_strchr
    dir_handles={}
    def cache_opendir():
        path=cstring(p.x(0))
        if not in_cache(path):raise RuntimeError(f'unapproved opendir {path!r}')
        if path not in cache_dirs:set_virtual_errno(2);p.x(0,0);return
        children=[v for v in cache_dirs | set(cache_data) if v.startswith(path+'/') and '/' not in v[len(path)+1:]]
        if children:raise RuntimeError(f'nonempty cache opendir unsupported {path!r}')
        if len(dir_handles)>=32:raise RuntimeError('cache directory handle bound')
        handle=p.alloc(16);dir_handles[handle]=path;p.x(0,handle);attempt['steps'].append({'step':'cache opendir','path':path})
    def cache_readdir():
        if p.x(0) not in dir_handles:raise RuntimeError('unapproved readdir')
        p.x(0,0)
    def cache_closedir():
        if p.x(0) not in dir_handles:raise RuntimeError('unapproved closedir')
        del dir_handles[p.x(0)];p.x(0,0)
    p.callbacks.update({'opendir':cache_opendir,'readdir':cache_readdir,'closedir':cache_closedir})
    prior_strlen_chk=p.callbacks['__strlen_chk']
    def cache_strlen_chk():
        if not 0x20cca00<=p.u.reg_read(R.UC_ARM64_REG_LR)<0x20ccb00:return prior_strlen_chk()
        pointer,bound=p.x(0),p.x(1)
        if bound>4096:raise RuntimeError('strlen bound')
        data=bytes(p.u.mem_read(pointer,bound));index=data.find(b'\0')
        if index<0:raise RuntimeError('unterminated checked string')
        p.x(0,index)
    p.callbacks['__strlen_chk']=cache_strlen_chk
    def cache_fwrite_chk():
        if p.x(1)*p.x(2)>p.x(4):raise RuntimeError('fwrite_chk source bound')
        cache_fwrite()
    p.callbacks['__fwrite_chk']=cache_fwrite_chk
    def cache_mktime():
        pointer=p.x(0);fields=p.get(pointer,'9i');sec,minute,hour,day,month,year,wday,yday,isdst=fields
        if not (0<=sec<=60 and 0<=minute<60 and 0<=hour<24 and 1<=day<=31 and 0<=month<12 and 0<=year<=300):raise RuntimeError(f'unexpected tm fields {fields}')
        time_module=__import__('time');value=int(time_module.mktime((year+1900,month+1,day,hour,minute,sec,wday,yday,isdst)))
        p.x(0,value);attempt['steps'].append({'step':'mktime','tm':list(fields),'result':value,'timezone':time_module.tzname})
    p.callbacks['mktime']=cache_mktime
    cache_times={}
    def cache_utime():
        path=cstring(p.x(0));pointer=p.x(1)
        if not in_cache(path):raise RuntimeError(f'unapproved utime {path!r}')
        if path not in cache_data and path not in cache_dirs:set_virtual_errno(2);p.x(0,-1);return
        times=p.get(pointer,'2q') if pointer else None;cache_times[path]=times;p.x(0,0);attempt['steps'].append({'step':'cache utime','path':path,'times':times})
    p.callbacks['utime']=cache_utime
    # Unicorn lacks the LSE atomic instruction selected by this environment's
    # synthetic CPU flag. Select the library's own LL/SC fallback.
    cpu_flag=0x74b7b98
    attempt['atomic_capability_before']=p.get(cpu_flag,'B')[0]
    p.put(cpu_flag,'B',0)
    try:
        cache=string('/tmp/sdocx-fountain-note-cache')
        path=string(str(archive))
        note=p.alloc(512)
        invoke('NoteDoc::NoteDoc','SPen::NoteDoc::NoteDoc()',[note])
        tracing[0]=True
        invoke('NoteDoc::Construct','SPen::NoteDoc::Construct(SPen::String const*, SPen::String const*, SPen::String const*, int, SPen::NoteDoc::Mode, bool, SPen::NoteType)',[note,cache,path,0,1848,0,0,note_type])
        tracing[0]=False
        attempt['construct_terminal_pcs']=list(pcs)
        count=invoke('NoteDoc::GetPageCount','SPen::NoteDoc::GetPageCount() const',[note])
        if count: invoke('NoteDoc::GetPage(0)','SPen::NoteDoc::GetPage(int) const',[note,0])
    except Exception:
        tracing[0]=False
        attempt['construct_terminal_pcs']=list(pcs)
    errors=[row for row in attempt['steps'] if 'error' in row]
    if errors:raise RuntimeError(f'unexpected native/probe error {errors}')
    signature=[row for row in attempt['steps'] if row['step']=='EndTag signature compare']
    if len(signature)!=1:raise RuntimeError('unexpected signature count')
    expected=b'Document for S-Pen SDK'
    actual=bytes.fromhex(signature[0]['actual_hex']);target=bytes.fromhex(signature[0]['expected_hex'])
    if not actual.endswith(expected) or ((actual==target)!=(note_type==2)):raise RuntimeError('unexpected signature control')
    constructions=[row for row in attempt['steps'] if row['step']=='NoteDoc::Construct']
    counts=[row for row in attempt['steps'] if row['step']=='NoteDoc::GetPageCount']
    if len(constructions)!=1 or constructions[0].get('result')!='0x0' or len(counts)!=1 or counts[0].get('result')!='0x0':raise RuntimeError('unexpected native NoteDoc result')
    if note_type==2:
        import zipfile as zip_module
        member_checks=[]
        with zip_module.ZipFile(archive) as z:
            for name in z.namelist():
                matches=[(path,data) for path,data in cache_data.items() if path.endswith('/'+name)]
                if len(matches)!=1:raise RuntimeError(f'cache member missing {name!r}')
                data=matches[0][1];digest=hashlib.sha256(data).hexdigest()
                if digest!=hashlib.sha256(z.read(name)).hexdigest():raise RuntimeError(f'cache member mismatch {name!r}')
                member_checks.append({'name':name,'size':len(data),'sha256':digest})
        attempt['verified_members']=member_checks
        native_errors=[row for row in attempt['steps'] if row['step']=='native Error::SetError']
        if len(native_errors)!=1 or (native_errors[0]['code'],native_errors[0]['line'],native_errors[0]['lr'])!=(11,776,'0x728a6d8'):raise RuntimeError('unexpected native error')
        cache_reads=[row for row in attempt['steps'] if row['step']=='cache fread']
        if not cache_reads or (cache_reads[-1]['offset'],cache_reads[-1]['size'],cache_reads[-1]['actual'])!=(8,234,123):raise RuntimeError('unexpected media read')
    attempt['host_zlib_version']=libz.zlibVersion().decode()
    for stream in streams.values():libz.inflateEnd(C.byref(stream))
    for fd in list(opened): host_os.close(fd)
    for fd in file_handles.values(): host_os.close(fd)
    Path(f'/tmp/sdocx-fountain-note-open-{case}-type{note_type}.json').write_text(json.dumps(attempt,indent=2)+'\n')
    raise SystemExit(0)
'''
probe.main()
