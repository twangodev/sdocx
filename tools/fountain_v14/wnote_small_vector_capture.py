"""Experimental original archive WNote loader; retains bounded host adapters."""
import ast
import builtins
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/fountain_v14"))
import hashlib
from pathlib import Path
if not __debug__: raise RuntimeError('optimized Python is unsupported')
if hashlib.sha256((ROOT/'tools/fountain_v14/state_probe_capture.py').read_bytes()).hexdigest()!='052d3d7f3677e56788c119df22ce38f31dfc50ddc9bb367c4c39198b2aad4695': raise RuntimeError('state probe drift')
import state_probe_capture as probe

source = (ROOT / 'tools/fountain_v14/note_open_probe.py').read_text()
if hashlib.sha256(source.encode()).hexdigest() != 'eb358c7276f4de5fbcd42a3d26c3b3c031d50da59b517afcde7c5936e2e98ce9':
    raise RuntimeError('base probe drift')
wdoc = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenWDoc.so'
if hashlib.sha256(wdoc.read_bytes()).hexdigest() != '1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6':
    raise RuntimeError('WDoc binary drift')
assignment = next(n for n in ast.parse(source).body if isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Attribute) and n.targets[0].attr == 'SCENE')
scene = ast.literal_eval(assignment.value)
def checked_replace(value, old, new, expected=1):
    if value.count(old) != expected:
        raise RuntimeError('probe replacement count mismatch')
    return value.replace(old, new)
scene = checked_replace(scene, "    case=host_os.environ.get('SDOCX_NOTE_PROBE_CASE','handwritten')", "    case='small_marker'")
scene = checked_replace(scene, "    if case not in ('handwritten','quiz'): raise RuntimeError('unapproved probe case')", "")
scene = checked_replace(scene, "    archive=ROOT/'tmp/stroke-conformance'/f'{case}.sdocx'", "    archive=ROOT/'hf/04-marker4-highlighter.sdocx'")
scene = checked_replace(scene, "    pinned={'handwritten':", "    pinned={'small_marker':'8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2','handwritten':")
scene = checked_replace(scene, "    pcs=deque(maxlen=1500); tracing=[False]", "    pcs=deque(maxlen=1500); tracing=[False]; progress_count=[0]")
scene = checked_replace(scene, "        if tracing[0]:\n            pcs.append(hex(address))", "        if tracing[0]:\n            progress_count[0]+=1\n            if progress_count[0]%1000000==0: print(f'WNote instructions={progress_count[0]} pc={address:#x}',flush=True)\n            pcs.append(hex(address))")
scene = checked_replace(scene, 'native_call(p,symbol,args,5_000_000)', 'native_call(p,symbol,args,15_000_000)')
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
scene=checked_replace(scene, "    p.callbacks.update({'inflateInit2_':inflate_init,'inflate':inflate,'inflateEnd':inflate_end,'crc32':crc})", "    shader_crc=p.callbacks['crc32']\n    p.callbacks.update({'inflateInit2_':inflate_init,'inflate':inflate,'inflateEnd':inflate_end,'crc32':crc})")
scene=checked_replace(scene, "mode.startswith(('w','a','r+'))", "mode in ('w','wb','w+b','r+b','ab')", 3)
scene=checked_replace(scene, "raise RuntimeError('cache write bound')", "raise RuntimeError(f'cache write bound mode={mode} bytes={size*count} stream_type={type(stream).__name__}')")
scene=checked_replace(scene, "cache_handles", "note_cache_handles", 20)
scene=checked_replace(scene, "    p.callbacks['utime']=cache_utime", "    p.callbacks['utime']=cache_utime\n    shader_fileno=p.callbacks['fileno']\n    def virtual_fileno():\n        if p.x(0) in note_cache_handles: set_virtual_errno(9);p.x(0,-1);return\n        return shader_fileno()\n    p.callbacks['fileno']=virtual_fileno")
scene=checked_replace(scene, "        else: original_fns['fopen']()", "        else: raise RuntimeError('unapproved non-cache fopen')")
scene=checked_replace(scene, "        if not in_cache(path):return orig_access()", "        if not in_cache(path):\n            if path != str(archive) or mode != 0: raise RuntimeError('unapproved access')\n            p.x(0,0);return")
scene=checked_replace(scene, "        if not in_cache(path):return orig_mkdir()", "        if not in_cache(path):raise RuntimeError('unapproved mkdir')")
start=scene.index("        note=p.alloc(512)")
end=scene.index("    except Exception:",start)
scene=scene[:start]+'''        note=p.alloc(4096)
        invoke('WNote::WNote','SPen::WNote::WNote()',[note])
        tracing[0]=True
        invoke('WNote::Construct','SPen::WNote::Construct(SPen::String const*, SPen::String const*, int, int, bool)',[note,cache,path,1848,0,0])
        tracing[0]=False
        attempt['construct_terminal_pcs']=list(pcs)
        count=invoke('WNote::GetPageCount','SPen::WNote::GetPageCount() const',[note])
        if count:
            page=invoke('WNote::GetPage(0)','SPen::WNote::GetPage(int) const',[note,0])
            invoke('WPage::LoadObject','SPen::WPage::LoadObject()',[page])
            invoke('WPage::GetObjectCount','SPen::WPage::GetObjectCount(bool)',[page,1])
            loaded=invoke('WPage::GetObject(0)','SPen::WPage::GetObject(int)',[page,0])
            invoke('ObjectBase::GetType','SPen::ObjectBase::GetType() const',[loaded])
            assert invoke('ObjectStroke::GetPointCount','SPen::ObjectStroke::GetPointCount() const',[loaded])==121
            uuid=native_call(p,'SPen::ObjectBase::GetUuid() const',[loaded])
            uuid_hash=hashlib.sha256(bytes(p.u.mem_read(uuid,36))).hexdigest()
            native_color=native_call(p,'SPen::ObjectStroke::GetColor() const',[loaded])&0xffffffff
            native_call(p,'SPen::ObjectStroke::GetPenSize() const',[loaded])
            native_size_bits=p.u.reg_read(R.UC_ARM64_REG_S0)
            p.callbacks.update(stream_fns);p.callbacks.update(original_fns)
            rt_v5_entries=[0]
            def observe_rt_v5(u,address,size,data): rt_v5_entries[0]+=1
            p.u.hook_add(UC_HOOK_CODE,observe_rt_v5,begin=0x9f700,end=0x9f700)
            p.callbacks['glDrawArraysInstanced']=original_instanced
            p.callbacks['crc32']=shader_crc
            p.callbacks.pop('SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)',None)
            matrix=p.alloc(36);p.put(matrix,'9f',1,0,0,0,-1,0,-80,h+145,1)
            native_call(p,'SPen::SPPenCanvas::SetMatrix(SPen::Matrix3<float> const&)',[pen_canvas,matrix])
            bind_fb(0x8d40,binding.value);bind_tex(0x0de1,target_texture);storage=C.create_string_buffer(bytes([252,252,252,255])*(w*h));upload(0x0de1,0,0,0,w,h,0x1908,0x1401,storage);read_pixels(0,0,w,h,0x1908,0x1401,read);before=read.raw
            before_draw=len(draw_attribute_joins);before_stamps=len(native_streams)
            tracing[0]=True
            invoke('ObjectDrawing::drawObjectStroke','SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,canvas,pen_canvas,loaded,1])
            read_pixels(0,0,w,h,0x1908,0x1401,read);after=read.raw
            (Path(host_os.environ['SDOCX_VECTOR_TMP'])/'native-before.rgba').write_bytes(before)
            (Path(host_os.environ['SDOCX_VECTOR_TMP'])/'native-after.rgba').write_bytes(after)
            attempt['loaded_drawing']={'uuid_sha256':uuid_hash,'color_argb':native_color,'pen_size_bits':native_size_bits,'matrix':[1,0,0,0,-1,0,-80,h+145,1],'gl_error':mesa_scope['gf']['glGetError'](),'native_draw_count':len(draw_attribute_joins)-before_draw,'rtv5_addpoint_entries':rt_v5_entries[0],'draw':{key:draw_attribute_joins[-1][key] for key in ('mode','first','count','instances','framebuffer_binding')},'target_framebuffer':binding.value,'before_sha256':hashlib.sha256(before).hexdigest(),'after_sha256':hashlib.sha256(after).hexdigest(),'changed_bytes':sum(a!=b for a,b in zip(before,after)),'output_sha256':hashlib.sha256(after).hexdigest()}

''' + scene[end:]
start=scene.index("    errors=[row for row in attempt['steps']")
end=scene.index("    attempt['host_zlib_version']",start)
scene=scene[:start]+scene[end:]

probe.base.TAG=b'com.samsung.android.sdk.pen.pen.preload.FountainPen;0;RED_UINT8;'
disk_write = "    Path(f'/tmp/sdocx-fountain-note-open-{case}-type{note_type}.json').write_text(json.dumps(attempt,indent=2)+'\\n')"
if disk_write not in scene:
    raise RuntimeError('base output drift')
scene=checked_replace(scene, disk_write, "    __import__('builtins')._sdocx_wnote_drawing = {'inputs':attempt['inputs'], 'steps':[row for row in attempt['steps'] if row['step'].startswith(('WNote::Construct','WNote::GetPageCount','WPage::LoadObject','WPage::GetObjectCount','ObjectBase::GetType','ObjectStroke::GetPointCount','ObjectDrawing::drawObjectStroke'))], 'drawing':attempt.get('loaded_drawing')}")
probe.SCENE=scene
try:
    probe.main()
except SystemExit as exc:
    if exc.code != 0: raise RuntimeError('native probe failed') from None
except Exception as exc:
    raise RuntimeError('native probe failed: '+type(exc).__name__) from None
result=builtins._sdocx_wnote_drawing
del builtins._sdocx_wnote_drawing
steps=result['steps']
if any('error' in row for row in steps): raise RuntimeError('native drawing failed: '+repr([(row['step'],row.get('error_type')) for row in steps if 'error' in row]))
if not result['drawing'] or result['drawing']['native_draw_count']!=1 or result['drawing']['changed_bytes']<=0 or result['drawing']['gl_error']!=0 or result['drawing']['rtv5_addpoint_entries']!=59 or result['drawing']['draw']!={'mode':5,'first':0,'count':4,'instances':59,'framebuffer_binding':result['drawing']['target_framebuffer']}: raise RuntimeError('no observed frame change')
expected={'uuid_sha256':'3d1819b137be66c5d3955b112bceb22a8f46c48d7d3960e43118c15b478c40c5','color_argb':4278387459,'pen_size_bits':1091690090,'before_sha256':'8646cc31146f6ece0d4b7c15f717088fa58a9e89534510b8f75da877b128b6ad','after_sha256':'e7b4fb6f401d07430aaae786739fd314f0f7d4cf1cd4ff496abf6484a1bdbaf3'}
if any(result['drawing'].get(k)!=v for k,v in expected.items() if k not in ('before_sha256','after_sha256')): raise RuntimeError('native source mismatch')
if hashlib.sha256((ROOT/'hf/04-marker4-highlighter.sdocx').read_bytes()).hexdigest()!='8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2': raise RuntimeError('archive changed')
print(json.dumps(result,indent=2))
