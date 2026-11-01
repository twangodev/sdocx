import ast
import hashlib
from pathlib import Path
if not __debug__: raise RuntimeError('optimized Python is unsupported')
if hashlib.sha256(Path(__file__).with_name('state_probe_capture.py').read_bytes()).hexdigest()!='052d3d7f3677e56788c119df22ce38f31dfc50ddc9bb367c4c39198b2aad4695': raise RuntimeError('state probe drift')
import state_probe_capture as probe

source = Path(__file__).with_name('note_open_probe.py').read_text()
if hashlib.sha256(source.encode()).hexdigest() != 'eb358c7276f4de5fbcd42a3d26c3b3c031d50da59b517afcde7c5936e2e98ce9':
    raise RuntimeError('base probe drift')
wdoc = Path(__file__).resolve().parents[2] / 'scratch/apk-analysis-native/arm64-v8a/libSPenWDoc.so'
if hashlib.sha256(wdoc.read_bytes()).hexdigest() != '1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6':
    raise RuntimeError('WDoc binary drift')
assignment = next(n for n in ast.parse(source).body if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Attribute) and n.targets[0].attr == 'SCENE')
scene = ast.literal_eval(assignment.value)
def checked_replace(value, old, new):
    if value.count(old)!=1: raise RuntimeError('probe replacement count mismatch')
    return value.replace(old,new)
scene = checked_replace(scene, "    pcs=deque(maxlen=1500); tracing=[False]", "    pcs=deque(maxlen=1500); tracing=[False]; progress_count=[0]")
scene = checked_replace(scene, "        if tracing[0]:\n            pcs.append(hex(address))", "        if tracing[0]:\n            progress_count[0]+=1\n            if progress_count[0]%1000000==0: print(f'WNote instructions={progress_count[0]} pc={address:#x}',flush=True)\n            pcs.append(hex(address))")
scene = checked_replace(scene, 'native_call(p,symbol,args,5_000_000)', 'native_call(p,symbol,args,500_000_000)')
scene = checked_replace(scene, "    attempt['zlib_loader']=zlib_info", "    attempt['zlib_loader']=zlib_info\n    wdoc_base=0x0b000000\n    attempt['wdoc_loader']=load_extra_dso(p,LIB/'libSPenWDoc.so',wdoc_base,env)\n    install_exact_plt_map(p,env,(('Renderer',RENDERER_BASE),('libc++_shared',LIBCPP_BASE),('Zlib',zlib_base),('WDoc',wdoc_base)))\n    for index in range(39):\n        addr=p.get(wdoc_base+0x105d88+8*index,'Q')[0]\n        p.syms['wdoc_init']=addr\n        invoke('WDoc init '+str(index),'wdoc_init',[])")
scene=checked_replace(scene, "    p.callbacks['utime']=cache_utime", """    p.callbacks['utime']=cache_utime
    def bounded_stat():
        path=cstring(p.x(0)); dst=p.x(1)
        if path==str(archive):
            size=host_os.stat(path).st_size; mode=0o100444
        elif in_cache(path) and (path in cache_data or path in cache_dirs):
            size=len(cache_data.get(path,b'')); mode=0o100444 if path in cache_data else 0o40755
        elif in_cache(path):
            set_virtual_errno(2);p.x(0,-1);return
        else:raise RuntimeError(f'unapproved stat {path!r}')
        p.u.mem_write(dst,bytes(128));p.put(dst+16,'I',mode);p.put(dst+48,'q',size)
        p.x(0,0);attempt['steps'].append({'step':'bounded stat','path':path,'size':size,'mode':mode})
    p.callbacks['stat']=bounded_stat
""")
# Limit all archive/cache I/O to the exact archive and virtual cache.  The
# inherited helpers may have broader fallbacks, so never delegate unknown paths.
scene=checked_replace(scene, "        else: original_fns['fopen']()", "        else: raise RuntimeError('unapproved non-cache fopen')")
scene=checked_replace(scene, "        if not in_cache(path):return orig_access()", "        if not in_cache(path):\n            if path != str(archive) or mode != 0: raise RuntimeError('unapproved access')\n            p.x(0,0);return")
scene=checked_replace(scene, "        if not in_cache(path):return orig_mkdir()", "        if not in_cache(path):raise RuntimeError('unapproved mkdir')")
scene=checked_replace(scene, "    Path(f'/tmp/sdocx-fountain-note-open-{case}-type{note_type}.json').write_text(json.dumps(attempt,indent=2)+'\\n')", "    safe={'inputs':attempt['inputs'],'identity':attempt['identity'],'identity_verified':attempt['identity_verified'],'steps':[row for row in attempt['steps'] if row['step'] in ('WNote::Construct','WNote::GetPageCount','WPage::LoadObject','WPage::GetObjectCount')]}\n    Path('/tmp/sdocx-fountain-wnote-handwritten-safe.json').write_text(json.dumps(safe,indent=2)+'\\n')")

start=scene.index("        note=p.alloc(512)")
end=scene.index("    except Exception:",start)
scene=scene[:start]+'''        note=p.alloc(4096)
        invoke('WNote::WNote','SPen::WNote::WNote()',[note])
        tracing[0]=True
        invoke('WNote::Construct','SPen::WNote::Construct(SPen::String const*, SPen::String const*, int, int, bool)',[note,cache,path,1848,0,0])
        tracing[0]=False
        attempt['construct_terminal_pcs']=list(pcs)
        if attempt['steps'][-1].get('result')!='0x1':raise RuntimeError('WNote construction failed')
        count=invoke('WNote::GetPageCount','SPen::WNote::GetPageCount() const',[note])
        if count!=1:raise RuntimeError('page count mismatch')
        if count:
            page=invoke('WNote::GetPage(0)','SPen::WNote::GetPage(int) const',[note,0])
            if not page:raise RuntimeError('missing page')
            if invoke('WPage::LoadObject','SPen::WPage::LoadObject()',[page])!=1:raise RuntimeError('page load failed')
            object_count=invoke('WPage::GetObjectCount','SPen::WPage::GetObjectCount(bool)',[page,1])
            if object_count!=2769:raise RuntimeError('object count mismatch')
            identity_hashes=[]
            for i in range(object_count):
                obj=native_call(p,'SPen::WPage::GetObject(int)',[page,i])
                if not obj or native_call(p,'SPen::ObjectBase::GetType() const',[obj])!=1:raise RuntimeError('object identity/type mismatch')
                uuid=native_call(p,'SPen::ObjectBase::GetUuid() const',[obj])
                identity_hashes.append(hashlib.sha256(bytes(p.u.mem_read(uuid,36))).hexdigest())
            attempt['identity']={'count':len(identity_hashes),'distinct':len(set(identity_hashes)),'ordered_uuid_hashes_sha256':hashlib.sha256(''.join(identity_hashes).encode('ascii')).hexdigest()}
            if len(set(identity_hashes))!=2769 or attempt['identity']['ordered_uuid_hashes_sha256']!='963f09365784936859772c332222fd69d25cd71011f2c950349f57c25cc4d567':raise RuntimeError('identity mismatch')
            if hashlib.sha256(archive.read_bytes()).hexdigest()!=pinned[case]:raise RuntimeError('archive changed')
            attempt['identity_verified']=True

''' + scene[end:]
start=scene.index("    errors=[row for row in attempt['steps']")
end=scene.index("    attempt['host_zlib_version']",start)
scene=scene[:start]+"    if not attempt.get('identity_verified'): raise RuntimeError('native object verification failed')\n"+scene[end:]
probe.SCENE=scene
try:
    probe.main()
except Exception as exc:
    raise RuntimeError('native probe failed: '+type(exc).__name__) from None
