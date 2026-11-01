"""Experimental original archive WNote loader; retains bounded host adapters."""
import ast
import builtins
import json
import os
import subprocess
import hashlib
from pathlib import Path
import state_probe_capture as probe

ROOT = Path(__file__).resolve().parents[2]
for relative, expected_sha in {
    'tools/fountain_v14/state_probe_capture.py': '052d3d7f3677e56788c119df22ce38f31dfc50ddc9bb367c4c39198b2aad4695',
    'tools/fountain_v14/cache_budget.py': 'a363e11365daef4a1da3f4e8d648e03a59c3ac147e21d61fc1bc5b876850a0cb',
    'conformance/fountain-v14-saved-scene-inputs.json': 'b8bb2d8a17412e80b67dc8456b9d8dbd061ebf0afcb443c32ff09cbb9b59182f',
}.items():
    if hashlib.sha256((ROOT / relative).read_bytes()).hexdigest() != expected_sha:
        raise RuntimeError(f'dependency drift: {relative}')
source = (ROOT / 'tools/fountain_v14/note_open_probe.py').read_text()
if hashlib.sha256(source.encode()).hexdigest() != 'eb358c7276f4de5fbcd42a3d26c3b3c031d50da59b517afcde7c5936e2e98ce9':
    raise RuntimeError('base probe drift')
wdoc = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenWDoc.so'
if hashlib.sha256(wdoc.read_bytes()).hexdigest() != '1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6':
    raise RuntimeError('WDoc binary drift')
assignment = next(n for n in ast.parse(source).body if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Attribute) and n.targets[0].attr == 'SCENE')
scene = ast.literal_eval(assignment.value)
scene = scene.replace("    case=host_os.environ.get('SDOCX_NOTE_PROBE_CASE','handwritten')", "    case='small_marker'")
scene = scene.replace("    if case not in ('handwritten','quiz'): raise RuntimeError('unapproved probe case')", "")
scene = scene.replace("    archive=ROOT/'tmp/stroke-conformance'/f'{case}.sdocx'", "    archive=ROOT/'hf/04-marker4-highlighter.sdocx'")
scene = scene.replace("    pinned={'handwritten':", "    pinned={'small_marker':'8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2','handwritten':")
scene = scene.replace("    pcs=deque(maxlen=1500); tracing=[False]", "    pcs=deque(maxlen=1500); tracing=[False]; progress_count=[0]")
scene = scene.replace("        if tracing[0]:\n            pcs.append(hex(address))", "        if tracing[0]:\n            progress_count[0]+=1\n            if progress_count[0]%1000000==0: print(f'WNote instructions={progress_count[0]} pc={address:#x}',flush=True)\n            pcs.append(hex(address))")
scene = scene.replace('native_call(p,symbol,args,5_000_000)', 'native_call(p,symbol,args,15_000_000)')
scene = scene.replace("    attempt['zlib_loader']=zlib_info", "    attempt['zlib_loader']=zlib_info\n    wdoc_base=0x0b000000\n    attempt['wdoc_loader']=load_extra_dso(p,LIB/'libSPenWDoc.so',wdoc_base,env)\n    install_exact_plt_map(p,env,(('Renderer',RENDERER_BASE),('libc++_shared',LIBCPP_BASE),('Zlib',zlib_base),('WDoc',wdoc_base)))\n    for index in range(39):\n        addr=p.get(wdoc_base+0x105d88+8*index,'Q')[0]\n        p.syms['wdoc_init']=addr\n        invoke('WDoc init '+str(index),'wdoc_init',[])")
scene=scene.replace("    p.callbacks['utime']=cache_utime", """    p.callbacks['utime']=cache_utime
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
# Fail closed on paths outside the selected archive and virtual cache.
scene = scene.replace("        if not in_cache(path):return orig_access()", "        if not in_cache(path):\n            if path == str(archive) and mode in (0, 4): p.x(0, 0); return\n            raise RuntimeError('unapproved access')")
scene = scene.replace("        if not in_cache(path):return orig_mkdir()", "        if not in_cache(path):raise RuntimeError('unapproved mkdir')")
scene = scene.replace("        if not in_cache(path):return stream_fns['fopen']()", "        if not in_cache(path):\n            if path == str(archive) and mode in ('r', 'rb'): return stream_fns['fopen']()\n            raise RuntimeError('unapproved fopen')")
start=scene.index("        note=p.alloc(512)")
end=scene.index("    except Exception:",start)
scene=scene[:start]+'''        note=p.alloc(4096)
        invoke('WNote::WNote','SPen::WNote::WNote()',[note])
        tracing[0]=True
        assert invoke('WNote::Construct','SPen::WNote::Construct(SPen::String const*, SPen::String const*, int, int, bool)',[note,cache,path,1848,0,0]) == 1
        tracing[0]=False
        attempt['construct_terminal_pcs']=list(pcs)
        count=invoke('WNote::GetPageCount','SPen::WNote::GetPageCount() const',[note])
        assert count == 2
        if count:
            page=invoke('WNote::GetPage(0)','SPen::WNote::GetPage(int) const',[note,0])
            assert page != 0
            assert invoke('WPage::LoadObject','SPen::WPage::LoadObject()',[page]) == 1
            object_count=invoke('WPage::GetObjectCount','SPen::WPage::GetObjectCount(bool)',[page,1])
            assert object_count == 41
            for i in range(object_count):
                obj=invoke('WPage::GetObject '+str(i),'SPen::WPage::GetObject(int)',[page,i])
                if obj:
                    invoke('ObjectBase::GetType '+str(i),'SPen::ObjectBase::GetType() const',[obj])
                    uuid=invoke('ObjectBase::GetUuid '+str(i),'SPen::ObjectBase::GetUuid() const',[obj])
                    attempt['steps'].append({'step':'UUID raw '+str(i),'value':hashlib.sha256(bytes(p.u.mem_read(uuid,36))).hexdigest()})
''' + scene[end:]
start=scene.index("    errors=[row for row in attempt['steps']")
end=scene.index("    attempt['host_zlib_version']",start)
scene=scene[:start]+scene[end:]
disk_write = "    Path(f'/tmp/sdocx-fountain-note-open-{case}-type{note_type}.json').write_text(json.dumps(attempt,indent=2)+'\\n')"
if disk_write not in scene:
    raise RuntimeError('base output drift')
scene = scene.replace(disk_write, "    __import__('builtins')._sdocx_wnote_identity = {'inputs': attempt['inputs'], 'steps': [row for row in attempt['steps'] if row['step'].startswith(('UUID raw ', 'ObjectBase::GetType ', 'WNote::Construct', 'WNote::GetPageCount', 'WNote::GetPage(0)', 'WPage::LoadObject', 'WPage::GetObjectCount'))]}")
probe.SCENE=scene
try:
    probe.main()
except SystemExit as exc:
    if exc.code != 0:
        raise
native = builtins._sdocx_wnote_identity
del builtins._sdocx_wnote_identity
source_example = ROOT / 'crates/sdocx/examples/saved_ink_inputs.rs'
if hashlib.sha256(source_example.read_bytes()).hexdigest() != 'e4494fc13e18c131e6eaec55c04108fe7fa1d0ed1b8a8ceefb4edbc20b02525c':
    raise RuntimeError('Rust input example drift')
env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0')
rust = json.loads(subprocess.check_output(['cargo', 'run', '--quiet', '-p', 'sdocx', '--features', 'serde', '--example', 'saved_ink_inputs', '--', str(ROOT / 'hf/04-marker4-highlighter.sdocx')], cwd=ROOT, env=env))
rows = native['steps']
expected = {'WNote::Construct': '0x1', 'WNote::GetPageCount': '0x2', 'WPage::LoadObject': '0x1', 'WPage::GetObjectCount': '0x29'}
for step, result in expected.items():
    found = [row for row in rows if row['step'] == step]
    if len(found) != 1 or found[0].get('result') != result:
        raise RuntimeError(f'native {step} failed')
page_rows = [row for row in rows if row['step'] == 'WNote::GetPage(0)']
if len(page_rows) != 1 or int(page_rows[0].get('result', '0'), 16) == 0:
    raise RuntimeError('native page missing')
hashes = [row['value'] for row in rows if row['step'].startswith('UUID raw ')]
rust_hashes = [hashlib.sha256(row['rendering']['metadata']['uuid'].encode('ascii')).hexdigest() for row in rust['strokes']]
if rust['document_sha256'] != native['inputs']['archive_sha256']:
    raise RuntimeError('archive identity mismatch')
if len(hashes) != 41 or hashes != rust_hashes or len(set(hashes)) != 41:
    raise RuntimeError('object identity mismatch')
types = [row for row in rows if row['step'].startswith('ObjectBase::GetType ')]
if len(types) != 41 or any(row.get('result') != '0x1' for row in types):
    raise RuntimeError('object type mismatch')
summary = {'archive_sha256': rust['document_sha256'], 'native_object_count': int(next(row['result'] for row in rows if row['step'] == 'WPage::GetObjectCount'), 16), 'rust_stroke_count': len(rust_hashes), 'ordered_full_uuid_sha256_matches': sum(a == b for a, b in zip(hashes, rust_hashes)), 'distinct_uuid_sha256_count': len(set(hashes)), 'ordered_uuid_hashes_sha256': hashlib.sha256(''.join(hashes).encode('ascii')).hexdigest()}
print(json.dumps(summary, indent=2))
