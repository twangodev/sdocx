"""Bounded saved Drawing caller with native-paper and explicit host-paper profiles.

This reuses the pinned Pencil3 native caller harness without modifying it. The
bitmap object is an explicit host adapter; Drawing, DrawingUtil, Pencil3 V3 and
RTV1 setters execute as native AArch64 code in Unicorn.
"""
from pathlib import Path
import contextlib, hashlib, io, json, os, struct, sys, subprocess, tempfile, zlib

ROOT = Path(__file__).resolve().parents[1]
os.chdir(ROOT)

# The public invocation is one deterministic three-profile run. Each child
# executes the same pinned native caller once, and the aggregate fixture keeps
# the complete native-pattern, explicit-white, and zero-paper evidence.
if os.environ.get('PENCIL_PAPER_PROFILE_CHILD') != '1':
    import argparse
    _driver=argparse.ArgumentParser(description='Replay native Pencil3 paper paint profiles')
    _driver.add_argument('--fixture',type=Path,default=ROOT/'conformance/pencil3-paper-paint-native.json')
    _driver_mode=_driver.add_mutually_exclusive_group()
    _driver_mode.add_argument('--write-fixture',action='store_true')
    _driver_mode.add_argument('--check-fixture',action='store_true')
    _driver_args=_driver.parse_args()
    _fixture=_driver_args.fixture if _driver_args.fixture.is_absolute() else ROOT/_driver_args.fixture
    _profiles={}
    with tempfile.TemporaryDirectory(prefix='pencil-paper-paint-',dir=ROOT/'tmp') as _temporary:
        for _mode in ('native','white','zero'):
            _profile_path=Path(_temporary)/f'{_mode}.json'
            _env=dict(os.environ,PENCIL_PAPER_PROFILE_CHILD='1',PENCIL_PAPER_MODE=_mode)
            _child=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--write-fixture','--fixture',str(_profile_path)],cwd=ROOT,env=_env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
            if _child.returncode:
                raise SystemExit(f'{_mode} child failed (exit {_child.returncode}):\n'+_child.stdout[-6000:])
            _profiles[_mode]=json.loads(_profile_path.read_text())
    _runner_sha=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    _source_pins=_profiles['native']['pinned_source_hashes']
    _library_pins=_profiles['native']['libraries']
    if any(row['runner_sha256']!=_runner_sha for row in _profiles.values()):
        raise SystemExit('profile runner hashes differ from the aggregate runner')
    if any(row['pinned_source_hashes']!=_source_pins or row['libraries']!=_library_pins for row in _profiles.values()):
        raise SystemExit('profile helper or native library pins differ')
    for _mode,_profile in _profiles.items():
        _paper=_profile['native_paint_trace']['paper']['depthmap_input']
        _raw=bytes.fromhex(_paper['source_raw_hex']); _rgba=bytes.fromhex(_paper['rgba_hex'])
        if len(_rgba)!=_paper['rgba_bytes'] or hashlib.sha256(_rgba).hexdigest()!=_paper['rgba_sha256']:
            raise SystemExit(f'{_mode} RGBA source payload length or hash mismatch')
        if len(_raw)!=_paper['source_raw_bytes'] or hashlib.sha256(_raw).hexdigest()!=_paper['source_raw_sha256']:
            raise SystemExit(f'{_mode} source payload length or hash mismatch')
        if _mode=='native':
            _expanded=b''.join(bytes((_value,0,0,0)) for _value in _raw)
            if _paper['source']!='native libSPenPencil3.so g_backgroundPtn@0xf3c8c' or _rgba!=_expanded or (_paper['width'],_paper['height'])!=(150,150):
                raise SystemExit('native g_backgroundPtn source-to-RGBA expansion contract failed')
        elif _paper['source']!='explicit synthetic host paper pixels' or _rgba!=_raw:
            raise SystemExit(f'{_mode} host source labeling/bytes mismatch')
        _draws=_profile['native_paint_trace']['gl']['draw_submissions']
        if not _draws: raise SystemExit(f'{_mode} has no native Draw submissions')
        for _draw in _draws:
            if _draw['framebuffer']!=_draw['read_framebuffer'] or not _draw['framebuffer'] or _draw['draw_color_attachment_name']!=_draw['read_color_attachment_name'] or not _draw['draw_color_attachment_name']:
                raise SystemExit(f'{_mode} lacks a joined Mesa READ/DRAW framebuffer attachment')
            _read_state=_draw['readback_framebuffer_state']; _readback=_draw['readback']; _pixels=zlib.decompress(bytes.fromhex(_readback['rgba_zlib_hex']))
            if _readback['bytes']!=_readback['width']*_readback['height']*4 or len(_pixels)!=_readback['bytes'] or hashlib.sha256(_pixels).hexdigest()!=_readback['sha256']:
                raise SystemExit(f'{_mode} full RGBA readback payload length or hash mismatch')
            if _draw['post_read_gl_error']!='0x0' or _read_state['draw_framebuffer']!=_read_state['read_framebuffer'] or _read_state['draw_framebuffer']!=_draw['framebuffer'] or _read_state['draw_attachment_name']!=_read_state['read_attachment_name'] or _read_state['draw_attachment_name']!=_draw['draw_color_attachment_name'] or _read_state['draw_status']!='0x8cd5':
                raise SystemExit(f'{_mode} immediate pre-read framebuffer state or post-read GL error mismatch')
    _runtime_pins=_profiles['native']['runtime_pins']; _tool_pins=_profiles['native']['analysis_tool_pins']; _host_gles_pins=_profiles['native']['host_gles_pins']
    if any(row['runtime_pins']!=_runtime_pins or row['analysis_tool_pins']!=_tool_pins or row['host_gles_pins']!=_host_gles_pins for row in _profiles.values()):
        raise SystemExit('profile runtime/tool/GLES pins differ')
    _aggregate={'schema':'pencil3-paper-paint-native-v1','runner_sha256':_runner_sha,'pinned_source_hashes':_source_pins,'libraries':_library_pins,'runtime_pins':_runtime_pins,'analysis_tool_pins':_tool_pins,'host_gles_pins':_host_gles_pins,'profiles':_profiles,'scope':'The saved Drawing caller produces and queues native Pencil3 work; actual native SPTextureBitmap readable interfaces feed native RTV1 Draw. GLES/EGL, native texture-object ownership, and source bitmap storage are explicit host adapters. The native profile uses bytes extracted from Pencil3 g_backgroundPtn; white and zero are explicit synthetic inputs. Mesa readbacks are host renderer evidence, not Samsung device pixels or full application paint.'}
    _encoded=json.dumps(_aggregate,indent=2,sort_keys=True)+'\n'
    if _driver_args.write_fixture:
        _fixture.parent.mkdir(parents=True,exist_ok=True)
        _fixture.write_text(_encoded)
        print(f'wrote {_fixture.relative_to(ROOT)}')
    else:
        if not _fixture.exists(): raise SystemExit(f'fixture missing: {_fixture}')
        if _fixture.read_text()!=_encoded: raise SystemExit(f'fixture mismatch: {_fixture}')
        print(f'verified {_fixture.relative_to(ROOT)} (native Draw and all three Mesa paper profiles reproduced)')
    raise SystemExit(0)

sys.path.insert(0, str(ROOT / "conformance"))
sys.path.insert(0, str(ROOT / "scratch/apk-analysis-runtime/python"))

source_path = ROOT / "conformance/pencil3_saved_caller_native.py"
source = source_path.read_text()
SOURCE_PINS={
 "pencil3_saved_caller_native.py":"129001ce587c104e22e1675e759df9a04d942e884a93fb747b195d001b6a776e",
 "oblique_canvas_native.py":"1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2",
 "calligraphy_draw_native.py":"e98afde895bc83d3a4667578383b43e3f648a337b463a95c6a0451e17ff4a788",
 "fountain_native.py":"40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30",
 "elf_plt.py":"816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b",
 "pencil_native.py":"b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c",
 "calligraphy_saved_native.py":"663f15668825ce6ed219830f3e671c408b85ad103477b444201f68f0b12c6517",
 "pencil_texture_native.py":"84f6860aaf16ec5ef15d13b3af1929710419bb2dd96e22550f10de80f4cf7a93",
 "inkpen_paint_native.py":"a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae",
}
for name,digest in SOURCE_PINS.items():
 path=ROOT/'conformance'/name
 if hashlib.sha256(path.read_bytes()).hexdigest()!=digest: raise RuntimeError(f'pinned source mismatch before patch: {name}')
_PREFLIGHT_LIBDIR=ROOT/'scratch/apk-analysis-native/arm64-v8a'
_PREFLIGHT_DSO_PINS={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Pencil3':'41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
for _name,_digest in _PREFLIGHT_DSO_PINS.items():
 _path=_PREFLIGHT_LIBDIR/('libSPen'+_name+'.so' if _name!='libc++_shared.so' else _name)
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_digest: raise RuntimeError(f'pinned native library mismatch before imports: {_name}')
TOOL_PINS={'python3.13':'889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9','nm':'ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1','aarch64-linux-gnu-objdump':'b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb'}
_tool_paths={'python3.13':Path(sys.executable).resolve(),'nm':Path('/usr/bin/nm'),'aarch64-linux-gnu-objdump':ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'}
for _name,_digest in TOOL_PINS.items():
 if hashlib.sha256(_tool_paths[_name].read_bytes()).hexdigest()!=_digest: raise RuntimeError(f'pinned analysis tool mismatch before imports: {_name}')
RUNTIME_PINS={'unicorn/__init__.py':'9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b','unicorn/unicorn.py':'e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7','unicorn/unicorn_py3/unicorn.py':'62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0','unicorn/arm64_const.py':'92f91a2b0a36cd34deee16db4f8da4ac0c9ae35c3ea1089296e5ef9d170a5be9','unicorn/lib/libunicorn.so.2':'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c'}
for _relative,_digest in RUNTIME_PINS.items():
 if hashlib.sha256((ROOT/'scratch/apk-analysis-runtime/python'/_relative).read_bytes()).hexdigest()!=_digest: raise RuntimeError(f'pinned Unicorn runtime mismatch before imports: {_relative}')
HOST_GLES_PINS={'libEGL.so.1':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6','libGLESv2.so.2':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812','swrast_dri.so':'0d8044a389ab67ef838b1bf13a6b6ce34547a45540a8a994b46f7fdd5e7292d0'}
_host_gles_paths={'libEGL.so.1':Path('/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0'),'libGLESv2.so.2':Path('/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0'),'swrast_dri.so':Path('/usr/lib/x86_64-linux-gnu/dri/swrast_dri.so')}
for _name,_digest in HOST_GLES_PINS.items():
 if hashlib.sha256(_host_gles_paths[_name].read_bytes()).hexdigest()!=_digest: raise RuntimeError(f'pinned host GLES resource mismatch before imports: {_name}')
source_loader = "oc.native.NativeFountain.__init__(self,oc.LIBRARY_DIR,oc.OBJDUMP,('Drawing','Graphics','Pencil3'))"
if source.count(source_loader) != 1:
    raise RuntimeError("Pencil3 loader anchor changed")
source = source.replace(source_loader, source_loader.replace("'Pencil3'))", "'Pencil3','Renderer'))"))
source_points_anchor="'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]]"
if source.count(source_points_anchor)!=1: raise RuntimeError('saved-caller synthetic point anchor changed')
source=source.replace(source_points_anchor,"'points':[[100.,100.],[110.,100.],[120.,100.],[130.,110.]]")
# Renderer is appended at the non-overlapping base 0x6000000. The caller
# harness separately maps Composer; move Composer to 0x9000000 below.
source = source.replace("'Composer':0x6000000", "'Composer':0x9000000")
import oblique_canvas_native as _oc
import calligraphy_draw_native as _cd
_oc.native.HASHES['Renderer'] = 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
_oc.native.HASHES['Graphics'] = 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621'
DSO_PINS={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Pencil3':'41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
LIBDIR=ROOT/'scratch/apk-analysis-native/arm64-v8a'
for name,digest in DSO_PINS.items():
 path=LIBDIR/('libSPen'+name+'.so' if name not in ('libc++_shared.so',) else name)
 if hashlib.sha256(path.read_bytes()).hexdigest()!=digest: raise RuntimeError(f'pinned native library mismatch before execution: {name}')
def _load_composer_at_nine(self):
 import hashlib, re, struct, subprocess
 path=_oc.LIBRARY_DIR/'libSPenComposer.so'; blob=path.read_bytes(); base=0x9000000
 if hashlib.sha256(blob).hexdigest()!=_oc.COMPOSER_SHA256: raise RuntimeError('unverified Composer DSO')
 off=struct.unpack_from('<Q',blob,32)[0]; ent,num=struct.unpack_from('<HH',blob,54); extent=0
 for i in range(num):
  typ,flags,fo,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,off+i*ent)
  if typ==1: extent=max(extent,va+ms)
 self.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(num):
  typ,flags,fo,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,off+i*ent)
  if typ==1 and ms:
   self.u.mem_write(base+va,blob[fo:fo+fs])
   if ms>fs:self.u.mem_write(base+va+fs,bytes(ms-fs))
 env=dict(os.environ); runtime=ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'
 if runtime.is_dir():env['LD_LIBRARY_PATH']=str(runtime.resolve())
 for line in subprocess.check_output(['nm','-D','-C',path],text=True,env=env).splitlines():
  z=line.strip().split(' ',2)
  if len(z)==3 and z[1] in ('T','W'):self.syms[z[2]]=base+int(z[0],16)
 dis=subprocess.check_output([str(_oc.OBJDUMP),'-d','-C','-j','.plt',path],text=True,env=env)
 for a,n in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):self.plt[base+int(a,16)]=n
_oc.CanvasProbe.load_composer = _load_composer_at_nine
needle = "phase='draw_object_stroke';"
inject = r'''
# Create a second actual native SPTextureBitmap so the writable render target
# and sampled depthmap are distinct GL textures. Its source bytes are the
# explicit synthetic host paper input; GetBitmapReadable/+0xc0 remain native.
if paper_mode=='native':
 from pencil_texture_native import read_va as _read_pencil_va
 _paper_raw=_read_pencil_va(ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so',0xf3c8c,150*150)
 if len(_paper_raw)!=150*150: raise RuntimeError(f'native g_backgroundPtn source has unexpected length: {len(_paper_raw)}')
 paper_input_rgba=b''.join(bytes((v,0,0,0)) for v in _paper_raw); paper_width=paper_height=150
else:
 _paper_raw=native_surface; paper_input_rgba=native_surface; paper_width=paper_height=None
paper_width,paper_height=(320,180) if paper_width is None else (paper_width,paper_height)
paper_input={'mode':paper_mode,'width':paper_width,'height':paper_height,'rgba_bytes':len(paper_input_rgba),'rgba_sha256':hashlib.sha256(paper_input_rgba).hexdigest(),'rgba_hex':paper_input_rgba.hex(),'source_raw_bytes':len(_paper_raw),'source_raw_sha256':hashlib.sha256(_paper_raw).hexdigest(),'source_raw_hex':_paper_raw.hex(),'expansion':'each native pattern byte v becomes RGBA(v,0,0,0)' if paper_mode=='native' else 'RGBA bytes are the explicit synthetic host source','mask_control':os.environ.get('PENCIL_MASK_CONTROL','native'),'source':'native libSPenPencil3.so g_backgroundPtn@0xf3c8c' if paper_mode=='native' else 'explicit synthetic host paper pixels'}
depthmap_tag=p.alloc(48); p.u.mem_write(depthmap_tag,b'pencil-consumer-depthmap\0')
paper_source_ptr=p.alloc(len(paper_input_rgba)); p.u.mem_write(paper_source_ptr,paper_input_rgba)
p.u.mem_write(0x200ef000,struct.pack('<II',0,0)+bytes(24))
p.call(native_bitmap_factory,[queue,paper_width,paper_height,depthmap_tag,paper_source_ptr,0,0,0],limit=5000000)
paper_bitmap=p.x(0)
p.call('SPen::ObjectDrawing::SetDepthmap(SPen::ISPBitmap*)',[p.drawing,paper_bitmap])
p.call('SPen::SPTextureBitmap::GetTextureBitmap()',[paper_bitmap])
paper_texture=p.x(0)
paper_gl_id=texture_objects_for_mesa[-1]['mesa_texture_id']
paper_surface_sha256=texture_objects_for_mesa[-1]['sha256']
p.call('SPen::ObjectDrawing::SetBackground(SPen::ISPBitmap*)',[p.drawing,native_pencil_bitmap])
background_bitmap=native_pencil_bitmap
p.call('SPen::SPTextureBitmap::GetTextureBitmap()',[background_bitmap])
background_texture=p.x(0)
background_gl_id=texture_objects_for_mesa[0]['mesa_texture_id']
background_surface_sha256=texture_objects_for_mesa[0]['sha256']
paper_depthmap_injected={'drawing_depthmap_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+48,8))[0]),'depthmap_bitmap':hex(paper_bitmap),'depthmap_texture':hex(paper_texture),'depthmap_dimensions':[paper_width,paper_height],'drawing_background_field':hex(struct.unpack('<Q',p.u.mem_read(p.drawing+40,8))[0]),'background_bitmap':hex(background_bitmap),'background_texture':hex(background_texture),'background_dimensions':[320,180],'depthmap_and_background_are_distinct':paper_bitmap!=background_bitmap and paper_texture!=background_texture}
native_bitmap_interface_trace=[]
_readable_getter=p.syms['SPen::SPTextureBitmap::GetBitmapReadable()']
_texture_activate_rt=p.syms['SPen::SPTextureBitmap::ActivateTextureRT(int, bool)']
_texture_activate_thunk=p.syms['non-virtual thunk to SPen::SPTextureBitmap::ActivateTextureRT(int, bool)']
_readable_returns={}
def _trace_native_bitmap_interfaces(_uc,address,_size,_data):
 if address==_readable_getter:
  this=p.x(0); lr=p.u.reg_read(R.UC_ARM64_REG_LR); row={'event':'GetBitmapReadable-entry','this':hex(this),'lr':hex(lr)}; native_bitmap_interface_trace.append(row); _readable_returns.setdefault(lr,[]).append(row)
 if address in _readable_returns and _readable_returns[address]:
  row=_readable_returns[address].pop(); value=p.x(0); row.update({'event_return':hex(value),'returned_interface_bytes':bytes(p.u.mem_read(value,16)).hex(),'returned_vptr':hex(struct.unpack('<Q',p.u.mem_read(value,8))[0])})
  if not _readable_returns[address]: _readable_returns.pop(address,None)
 if address in (_texture_activate_rt,_texture_activate_thunk):
  this=p.x(0); native_bitmap_interface_trace.append({'event':'ActivateTextureRT-thunk-entry' if address==_texture_activate_thunk else 'ActivateTextureRT-entry','this':hex(this),'unit':p.x(1),'release':p.x(2),'vptr':hex(struct.unpack('<Q',p.u.mem_read(this,8))[0]),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
p.u.hook_add(UC_HOOK_CODE,_trace_native_bitmap_interfaces)
from unicorn import UC_HOOK_MEM_WRITE
penreturn_symbols={n:a for n,a in p.syms.items() if 'SPen::PenReturnCallback::' in n and '@plt' not in n}
penreturn_ctor_pcs={a for n,a in penreturn_symbols.items() if 'PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)' in n}
penreturn_dtor_pcs={a for n,a in penreturn_symbols.items() if '~PenReturnCallback()' in n and '@plt' not in n}
penreturn_callback_objects=[]
penreturn_callback_events=[]
def trace_penreturn_callbacks(_uc,address,_size,_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj.get('return_pc')==address:
   obj['active']=False
   penreturn_callback_events.append({'event':'destructor_return','phase':phase,'pc':hex(address),'this':hex(obj['this'])})
 if address in penreturn_ctor_pcs:
  this=p.x(0); penreturn_callback_objects.append({'this':this,'phase':phase,'active':True,'return_pc':None}); penreturn_callback_events.append({'event':'constructor','phase':phase,'pc':hex(address),'this':hex(this)})
 elif address in penreturn_dtor_pcs:
  this=p.x(0)
  matches=[row for row in penreturn_callback_objects if row['this']==this and row.get('active')]
  if matches:
   retpc=p.u.reg_read(R.UC_ARM64_REG_LR)
   matches[-1]['return_pc']=retpc
   penreturn_callback_events.append({'event':'destructor_entry','phase':phase,'pc':hex(address),'return_pc':hex(retpc),'this':hex(this),'bytes_0_80':p.u.mem_read(this,80).hex()})
def trace_callback_writes(_uc,_access,address,size,value,_user_data):
 for obj in penreturn_callback_objects:
  if obj.get('active') and obj['this'] <= address < obj['this']+80:
   penreturn_callback_events.append({'event':'field_write','phase':phase,'pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'this':hex(obj['this']),'offset':address-obj['this'],'size':size,'value_hex':(int(value) & ((1 << (8*size))-1)).to_bytes(size,'little').hex()})
p.u.hook_add(UC_HOOK_CODE,trace_penreturn_callbacks)
p.u.hook_add(UC_HOOK_MEM_WRITE,trace_callback_writes)
paper_setter_trace=[]
paper_setter_pcs={p.syms['SPen::Pencil3StrokeDrawableGLV3::SetDepthMapBitmap(SPen::ISPBitmap*)'],p.syms['SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)'],0x510d48c}
def trace_paper_setters(_uc,address,_size,_data):
 if address in paper_setter_pcs:
  row={'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]}; paper_setter_trace.append(row)
p.u.hook_add(UC_HOOK_CODE,trace_paper_setters)
scale_after=[]
def trace_scale_return(_uc,address,_size,_data):
 if address==0x510d490: scale_after.append({'phase':phase,'x0':hex(p.x(0)),'scale_bits':p.u.mem_read(p.x(0)+264,4).hex()})
p.u.hook_add(UC_HOOK_CODE,trace_scale_return)
paint_entries=[]
queued_tasks=[]
executed_task_runs=[]
task_run_seen=[]
texture_vtable_calls=[]
set_pen_data_calls=[]
pending_set_pen_data_returns={}
set_pen_task_returns=[]
SET_PEN_DATA_TASK_VPTR=0x511b3c0
SET_PEN_DATA_TASK_RETURN=0x200f2250
set_pen_data_pc=p.syms.get('SPen::PenDrawableRT::SetPenData(float, int)')
texture_adapter_events=[]
def trace_queue_enqueue(_uc,address,_size,_data):
 for frame in pending_set_pen_data_returns.get(address,[]):
  obj=int(frame['this'],16); frame['rt_plus_44_after']=bytes(p.u.mem_read(obj+44,4)).hex(); frame['return_pc']=hex(address)
 if address in pending_set_pen_data_returns: pending_set_pen_data_returns.pop(address)
 if set_pen_data_pc and address==set_pen_data_pc:
  obj=p.x(0); row={'phase':phase,'pc':hex(address),'this':hex(obj),'size_f32':p.f(0),'color_argb':hex(p.x(1)),'rt_plus_44_before':bytes(p.u.mem_read(obj+44,4)).hex(),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))}; set_pen_data_calls.append(row); pending_set_pen_data_returns.setdefault(p.u.reg_read(R.UC_ARM64_REG_LR),[]).append(row)
 if address in (0x510a694,0x510cce0): paint_entries.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2))})
 if address in (0x51089d4,0x5108a00,0x5108a28,0x5108a54,0x510d48c,0x510c7e4): task_run_seen.append({'phase':phase,'pc':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'s0_f32':struct.unpack('<f',p.u.reg_read(R.UC_ARM64_REG_Q0).to_bytes(16,'little')[:4])[0]})
 if address in (0x605e53c,0x605e8e8,0x605ee8c,0x605ec64,0x605ec74,0x605f840):
  native_texture_object_events.append({'phase':phase,'entry':hex(address),'x0':hex(p.x(0)),'x1':hex(p.x(1)),'x2':hex(p.x(2)),'x3':hex(p.x(3)),'x4':hex(p.x(4)),'x5':hex(p.x(5)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 elif address==0x20001000:
  task=p.x(1); vt=struct.unpack('<Q',p.u.mem_read(task,8))[0]
  slots=struct.unpack('<6Q',p.u.mem_read(vt,48))
  queued_tasks.append({'phase':phase,'task':hex(task),'vtable':hex(vt),'vtable_slots':[hex(v) for v in slots],'raw_qwords_0_64':[hex(v) for v in struct.unpack('<8Q',p.u.mem_read(task,64))]})
  if vt==SET_PEN_DATA_TASK_VPTR:
   run=slots[2]
   if not run: raise RuntimeError('native SetPenData task has null vtable+16')
   target=struct.unpack('<Q',p.u.mem_read(task+16,8))[0]
   before=bytes(p.u.mem_read(target+44,4)).hex()
   caller=p.u.reg_read(R.UC_ARM64_REG_LR)
   set_pen_task_returns.append({'task':task,'target':target,'caller':caller,'vtable':vt,'run':run,'before':before})
   task_run_seen.append({'phase':phase,'pc':hex(run),'task':hex(task),'target':hex(target),'vtable':hex(vt),'role':'WorkerTask::Member2<PenDrawableRT, SetPenData(float,int)>::Run'})
   p.x(0,task); p.u.reg_write(R.UC_ARM64_REG_LR,SET_PEN_DATA_TASK_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,run)
 elif address==SET_PEN_DATA_TASK_RETURN:
  if not set_pen_task_returns: raise RuntimeError('SetPenData task returned without a pending queue task')
  frame=set_pen_task_returns.pop(); after=bytes(p.u.mem_read(frame['target']+44,4)).hex()
  set_pen_data_calls.append({'phase':phase,'task':hex(frame['task']),'vtable':hex(frame['vtable']),'run_pc':hex(frame['run']),'target_rt':hex(frame['target']),'rt_plus_44_before':frame['before'],'rt_plus_44_after':after,'queue_accept_return':'0x1'})
  p.x(0,1); p.u.reg_write(R.UC_ARM64_REG_PC,frame['caller'])
 elif address in (0x200f2310,0x200f2320,0x200f2330):
  texture_vtable_calls.append({'phase':phase,'stub':hex(address),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'this':hex(p.x(0)),'x1':hex(p.x(1)),'object_header':bytes(p.u.mem_read(p.x(0),64)).hex() if p.x(0) else None})
 elif address in (TEXTURE_ACTIVATE_STUB,TEXTURE_PARAM_STUB,TEXTURE_UPDATE_STUB,TEXTURE_TAG_STUB):
  obj=p.x(0)
  target,texture_id=struct.unpack('<II',p.u.mem_read(obj+40,8))
  if address==TEXTURE_ACTIVATE_STUB:
   unit=p.x(1)
   if unit>31: raise RuntimeError(f'unsupported texture unit {unit}')
   mesa_gl['glActiveTexture'](0x84c0+unit); mesa_gl['glBindTexture'](target,texture_id)
   event={'event':'Activate','phase':phase,'object':hex(obj),'target':hex(target),'texture_id':texture_id,'unit':unit}
  elif address==TEXTURE_PARAM_STUB:
   param,value=p.x(1),p.x(2)
   pnames={0:0x2802,1:0x2803,2:0x8072,4:0x2800,5:0x2801}; value_maps={0:{0:0x8370,1:0x812f,2:0x812d,3:0x2901},1:{0:0x8370,1:0x812f,2:0x812d,3:0x2901},2:{0:0x8370,1:0x812f,2:0x812d,3:0x2901},4:{0:0x2600,1:0x2601},5:{0:0x2600,1:0x2601}}
   if param not in pnames or value not in value_maps.get(param,{}): raise RuntimeError(f'unsupported native texture SetParam {param=} {value=}')
   gl_value=value_maps[param][value]; mesa_gl['glBindTexture'](target,texture_id); mesa_gl['glTexParameteri'](target,pnames[param],gl_value)
   event={'event':'SetParam','phase':phase,'object':hex(obj),'target':hex(target),'texture_id':texture_id,'parameter':param,'value':value,'gl_pname':hex(pnames[param]),'gl_value':hex(gl_value)}
  elif address==TEXTURE_UPDATE_STUB:
   size=p.x(2)
   if size==0:
    event={'event':'Update','phase':phase,'object':hex(obj),'bytes':0,'data_pointer':hex(p.x(1)),'native_operation':'zero-size update; no Mesa upload'}
   else:
    width,height=struct.unpack('<II',p.u.mem_read(obj+64,8))
    if width*height*4!=size or size>16*1024*1024: raise RuntimeError(f'texture Update size mismatch {width=} {height=} {size=}')
    raw=bytes(p.u.mem_read(p.x(1),size)); payload=_C.create_string_buffer(raw)
    mesa_gl['glBindTexture'](target,texture_id); mesa_gl['glTexSubImage2D'](target,0,0,0,width,height,0x1908,0x1401,_C.cast(payload,_C.c_void_p))
    event={'event':'Update','phase':phase,'object':hex(obj),'target':hex(target),'texture_id':texture_id,'dimensions':[width,height],'bytes':size,'sha256':hashlib.sha256(raw).hexdigest()}
  else:
   event={'event':'SetTag','phase':phase,'object':hex(obj),'tag':_bytes_until_nul(p.x(1)).decode('utf8','replace')}
  texture_adapter_events.append(event); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))

p.u.hook_add(UC_HOOK_CODE,trace_queue_enqueue)
# Intercept only the PenReturnCallback's native queue-enqueue call on the
# existing IContext vslot. Non-message profile queries execute the original
# host getter bytes. For the native Member2 task, synchronously call its native
# vtable+16 Run method and leave the RT target/args untouched.
MESSAGE_CONTEXT_CALL=0x200f2400; MESSAGE_RETURN=0x200f2220; RT_INIT_RETURN=0x200f2240
p.u.mem_write(MESSAGE_CONTEXT_CALL,struct.pack('<II',0xd503201f,0xd65f03c0))
p.put(qvt+16,'Q',MESSAGE_CONTEXT_CALL)
p.u.mem_write(MESSAGE_RETURN,struct.pack('<II',0xd503201f,0xd65f03c0))
native_message_dispatch=[]; message_return_stack=[]; context_enqueue_calls=[]
EXPECTED_MESSAGE_VPTRS={0x10774c8,0x10774f8,0x1077528}

def trace_native_message_dispatch(_uc,address,_size,_data):
 if address==MESSAGE_CONTEXT_CALL:
  message=p.x(1)
  if not message: return
  vptr=struct.unpack('<Q',p.u.mem_read(message,8))[0]
  context_enqueue_calls.append({'phase':phase,'context':hex(p.x(0)),'x1':hex(message),'candidate_vptr':hex(vptr),'vtable_slots_0_48':bytes(p.u.mem_read(vptr,48)).hex() if 0x1000000 <= vptr < 0x1100000 else None,'bytes_0_64':bytes(p.u.mem_read(message,64)).hex(),'bytes_0_144':bytes(p.u.mem_read(message,144)).hex() if vptr in {0x10774f8,0x10774c8,0x1077528} else None})
  if vptr==SET_PEN_DATA_TASK_VPTR:
   slots=struct.unpack('<6Q',p.u.mem_read(vptr,48)); method=slots[2]
   if not method: raise RuntimeError('native PenReturn SetPenData task has null vtable+16')
   target=struct.unpack('<Q',p.u.mem_read(message+16,8))[0]
   caller=p.u.reg_read(R.UC_ARM64_REG_LR)
   before=bytes(p.u.mem_read(target+44,4)).hex()
   set_pen_task_returns.append({'task':message,'target':target,'caller':caller,'vtable':vptr,'run':method,'before':before})
   task_run_seen.append({'phase':phase,'pc':hex(method),'task':hex(message),'target':hex(target),'vtable':hex(vptr),'role':'WorkerTask::Member2<PenDrawableRT, SetPenData(float,int)>::Run via PenReturn queue'})
   p.x(0,message); p.u.reg_write(R.UC_ARM64_REG_LR,SET_PEN_DATA_TASK_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,method)
   return
  if vptr not in EXPECTED_MESSAGE_VPTRS: return
  slots=struct.unpack('<6Q',p.u.mem_read(vptr,48)); method=slots[2]
  if not method: raise RuntimeError(f'native queued message has null vslot+16: {message:#x} {slots!r}')
  caller=p.u.reg_read(R.UC_ARM64_REG_LR)
  record={'phase':phase,'context':hex(p.x(0)),'message':hex(message),'vtable':hex(vptr),'vtable_slots':[hex(v) for v in slots],'native_execute_slot_plus16':hex(method),'message_bytes_0_144':bytes(p.u.mem_read(message,144)).hex(),'caller':hex(caller)}
  executed_task_runs.append({'phase':phase,'task':hex(message),'vtable':hex(vptr),'run_pc':hex(method),'caller':hex(caller)})
  native_message_dispatch.append(record); message_return_stack.append({'caller':caller,'record':record})
  if vptr==0x10774c8:
   rtptr=struct.unpack('<Q',p.u.mem_read(message+32,8))[0]
   init=p.syms['SPen::Pencil3StrokeDrawableRTV1::Init()']
   frame=message_return_stack[-1]; frame.update({'message':message,'method':method,'rt':rtptr,'init':init})
   record['selected_rt']=hex(rtptr); record['host_lifecycle_intervention']='call actual RTV1::Init on this native Drawing-selected receiver before native queued Run; no RT fields patched'
   p.x(0,rtptr); p.u.reg_write(R.UC_ARM64_REG_LR,RT_INIT_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,init)
   return
  p.x(0,message); p.u.reg_write(R.UC_ARM64_REG_LR,MESSAGE_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,method)
 elif address==RT_INIT_RETURN:
  frame=message_return_stack[-1]; rtptr=frame['rt']; frame['record']['native_init_return']={'x0':hex(p.x(0)),'field208':bytes(p.u.mem_read(rtptr+208,8)).hex(),'field216':bytes(p.u.mem_read(rtptr+216,8)).hex()}
  p.x(0,frame['message']); p.u.reg_write(R.UC_ARM64_REG_LR,MESSAGE_RETURN); p.u.reg_write(R.UC_ARM64_REG_PC,frame['method'])
 elif address==MESSAGE_RETURN:
  if not message_return_stack: raise RuntimeError('native message return without enqueue frame')
  frame=message_return_stack.pop(); frame['record']['native_execute_return_x0']=hex(p.x(0))
  p.x(0,1); p.u.reg_write(R.UC_ARM64_REG_PC,frame['caller'])
p.u.hook_add(UC_HOOK_CODE,trace_native_message_dispatch)

'''
inject = "\n".join(" " + line if line else line for line in inject.splitlines())
if source.count(needle) != 1:
    raise RuntimeError("native caller injection point changed")
source = source.replace(needle, needle + inject)
native_setup = r'''p.u.mem_write(0x200ef000,struct.pack('<II',6,2))
 TEXTURE_ACTIVATE_STUB=0x200f2350; TEXTURE_PARAM_STUB=0x200f2360; TEXTURE_UPDATE_STUB=0x200f2370; TEXTURE_TAG_STUB=0x200f2380
 for _stub in (TEXTURE_ACTIVATE_STUB,TEXTURE_PARAM_STUB,TEXTURE_UPDATE_STUB,TEXTURE_TAG_STUB): p.u.mem_write(_stub,struct.pack('<II',0xd503201f,0xd65f03c0))
 # Renderer-aware, source-pinned ShaderManager setup adapted from the existing
 # Calligraphy native Draw probe. EGL/GLES remain host resource boundaries.
 cxa_guard_seen=set()
 def _guard_acquire():
  guard=p.x(0); p.x(0,0 if guard in cxa_guard_seen else 1); cxa_guard_seen.add(guard)
 for _name in ('pthread_mutex_init','pthread_mutex_lock','pthread_mutex_unlock','pthread_mutex_destroy','__cxa_guard_release','__cxa_guard_abort'):
  p.callbacks[_name]=lambda:p.x(0,0)
 p.callbacks['__cxa_guard_acquire']=_guard_acquire
 p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xE611)
 p.callbacks['crc32']=lambda:p.x(0,__import__('zlib').crc32(bytes(p.u.mem_read(p.x(1),p.x(2))) if p.x(1) and p.x(2) else b'') & 0xffffffff)
 p.callbacks['__android_log_print']=lambda:p.x(0,0)
 def _bytes_until_nul(pointer,limit=1<<16):
  out=bytearray()
  for i in range(limit):
   b=p.u.mem_read(pointer+i,1)[0]
   if not b:return bytes(out)
   out.append(b)
  raise RuntimeError('bounded native C string exceeded')
 p.callbacks['strlen']=lambda:p.x(0,len(_bytes_until_nul(p.x(0))))
 p.callbacks['__strlen_chk']=p.callbacks['strlen']
 p.callbacks['strcmp']=lambda:p.x(0,( _bytes_until_nul(p.x(0)) > _bytes_until_nul(p.x(1))) - (_bytes_until_nul(p.x(0)) < _bytes_until_nul(p.x(1))))
 def _strstr():
  hay,needle=p.x(0),p.x(1); h=_bytes_until_nul(hay); n=_bytes_until_nul(needle); i=h.find(n); p.x(0,0 if i<0 else hay+i)
 p.callbacks['strstr']=_strstr
 def _strchr():
  ptr,ch=p.x(0),p.x(1)&0xff; raw=_bytes_until_nul(ptr); i=raw.find(bytes([ch])); p.x(0,ptr+i if i>=0 else 0)
 p.callbacks['strchr']=_strchr
 def _strncpy_chk():
  dst,src,count,capacity=(p.x(i) for i in range(4))
  if count>capacity or count>(1<<20):raise RuntimeError(f'bounded strncpy overflow {count=} {capacity=}')
  data=bytes(p.u.mem_read(src,count)); end=data.find(b'\0'); data=data[:end]+bytes(count-end) if end>=0 else data; p.u.mem_write(dst,data); p.x(0,dst)
 p.callbacks['__strncpy_chk']=_strncpy_chk; p.callbacks['__strncpy_chk2']=_strncpy_chk
 # Reuse the existing bounded shader-source/reflection GLES adapter, with its
 # state hosted over this probe's actual Unicorn memory/register interface.
 glhost=object.__new__(_cd.NativeDrawCalligraphy)
 for _name in ('x','f','get','put','alloc'):
  setattr(glhost,_name,getattr(p,_name))
 def _read_gl_string(pointer):
  data=bytearray()
  for offset in range(1<<16):
   byte=p.u.mem_read(pointer+offset,1)[0]
   if not byte: return data.decode('utf-8','replace')
   data.append(byte)
  raise RuntimeError('GL string exceeded bound')
 def _read_gl_bytes(pointer): return bytes(p.u.mem_read(pointer,1<<16)).split(b'\0',1)[0]
 glhost.read_c_string=_read_gl_string; glhost.read_c_bytes=_read_gl_bytes
 glhost.u=p.u; glhost.gl_calls=[]; glhost.gl_next_id=100; glhost.gl_strings={}; glhost.gl_locations={}; glhost.gl_shader_sources={}; glhost.gl_program_shaders={}
 glhost.program_active_uniforms=_cd.NativeDrawCalligraphy.program_active_uniforms.__get__(glhost)
 for _name in set(p.plt.values()):
  if _name.startswith('gl'):
   def _gl_callback(n=_name):
    if n=='glVertexAttribDivisor':
     glhost.gl_calls.append({'name':n,'args':[p.x(i) for i in range(4)],'host_boundary':'synthetic GLES state adapter'}); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
    else: glhost.gl_host_import(n)
   p.callbacks[_name]=_gl_callback
 # Forward the same native GLES imports to a real surfaceless Mesa EGL/GLES
 # context. Source/texture creation stays an explicit host resource seam.
 import inkpen_paint_native as _ip
 _ip.WIDTH=320; _ip.HEIGHT=180
 mesa=_ip.GLES(); mesa_gl=mesa.f; mesa_calls=[]; mesa_draw_readbacks=[]; mesa_cleared_fbos=set()
 _C=_ip.C
 import pencil_texture_native as _ptn
 _pencil_dso=LIBS/'libSPenPencil3.so'
 _pencil_pattern_bytes=_ptn.read_va(_pencil_dso,0x12c8c,16*120*120*4)
 _pencil_paper_source=_ptn.read_va(_pencil_dso,0xf3c8c,150*150)
 _pencil_paper_bytes=b''.join(bytes((value,0,0,0)) for value in _pencil_paper_source)
 for _name,_ret,_args in (
  ('glGetString',_C.c_void_p,[_C.c_uint]),
  ('glIsEnabled',_C.c_ubyte,[_C.c_uint]),
  ('glGetIntegerv',None,[_C.c_uint,_C.POINTER(_C.c_int)]),
  ('glGetBooleanv',None,[_C.c_uint,_C.POINTER(_C.c_ubyte)]),
  ('glGetVertexAttribiv',None,[_C.c_uint,_C.c_uint,_C.POINTER(_C.c_int)]),
  ('glColorMask',None,[_C.c_ubyte,_C.c_ubyte,_C.c_ubyte,_C.c_ubyte]),
  ('glEnableVertexAttribArray',None,[_C.c_uint]),
  ('glDisableVertexAttribArray',None,[_C.c_uint]),
  ('glInvalidateFramebuffer',None,[_C.c_uint,_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glGetActiveUniform',None,[_C.c_uint,_C.c_uint,_C.c_int,_C.POINTER(_C.c_int),_C.POINTER(_C.c_int),_C.POINTER(_C.c_uint),_C.c_char_p]),
  ('glGetActiveAttrib',None,[_C.c_uint,_C.c_uint,_C.c_int,_C.POINTER(_C.c_int),_C.POINTER(_C.c_int),_C.POINTER(_C.c_uint),_C.c_char_p]),
  ('glGetAttribLocation',_C.c_int,[_C.c_uint,_C.c_char_p]),
  ('glUniform1f',None,[_C.c_int,_C.c_float]),
  ('glUniform2f',None,[_C.c_int,_C.c_float,_C.c_float]),
  ('glUniform3f',None,[_C.c_int,_C.c_float,_C.c_float,_C.c_float]),
  ('glUniform4fv',None,[_C.c_int,_C.c_int,_C.POINTER(_C.c_float)]),
  ('glUniform1fv',None,[_C.c_int,_C.c_int,_C.POINTER(_C.c_float)]),
  ('glUniform2fv',None,[_C.c_int,_C.c_int,_C.POINTER(_C.c_float)]),
  ('glUniform3fv',None,[_C.c_int,_C.c_int,_C.POINTER(_C.c_float)]),
  ('glBufferSubData',None,[_C.c_uint,_C.c_ssize_t,_C.c_ssize_t,_C.c_void_p]),
  ('glTexSubImage2D',None,[_C.c_uint,_C.c_int,_C.c_int,_C.c_int,_C.c_int,_C.c_int,_C.c_uint,_C.c_uint,_C.c_void_p]),
  ('glBindRenderbuffer',None,[_C.c_uint,_C.c_uint]),
  ('glGenRenderbuffers',None,[_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glRenderbufferStorage',None,[_C.c_uint,_C.c_uint,_C.c_int,_C.c_int]),
  ('glFramebufferRenderbuffer',None,[_C.c_uint,_C.c_uint,_C.c_uint,_C.c_uint]),
  ('glDeleteTextures',None,[_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glDeleteBuffers',None,[_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glDeleteFramebuffers',None,[_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glDeleteRenderbuffers',None,[_C.c_int,_C.POINTER(_C.c_uint)]),
  ('glDeleteProgram',None,[_C.c_uint]),
  ('glDeleteShader',None,[_C.c_uint]),
  ('glDetachShader',None,[_C.c_uint,_C.c_uint]),
 ):
  if _name not in mesa_gl: mesa_gl[_name]=_ip.cfunc(mesa.gl,_name,_ret,_args)
 def _mesa_string(ptr,limit=8192): return bytes(p.u.mem_read(ptr,limit)).split(b'\0',1)[0]
 def _mesa_pixel_bytes(w,h,fmt,typ):
  comps={0x1903:1,0x1906:1,0x1909:1,0x8227:2,0x1907:3,0x1908:4}.get(fmt)
  sizes={0x1401:1,0x1403:2,0x1406:4}.get(typ)
  if comps is None or sizes is None or w<0 or h<0 or w*h*comps*sizes>64*1024*1024: raise RuntimeError(f'unsupported/bounded Mesa texture payload {w=} {h=} {fmt=:#x} {typ=:#x}')
  return w*h*comps*sizes
 def _mesa_texture(width,height,data_ptr=0,data_bytes=None):
  tex=_C.c_uint(); mesa_gl['glGenTextures'](1,_C.byref(tex)); mesa_gl['glBindTexture'](0x0de1,tex.value)
  mesa_gl['glTexParameteri'](0x0de1,0x2801,0x2600); mesa_gl['glTexParameteri'](0x0de1,0x2800,0x2600)
  mesa_gl['glTexParameteri'](0x0de1,0x2802,0x812f); mesa_gl['glTexParameteri'](0x0de1,0x2803,0x812f)
  raw=bytes(data_bytes) if data_bytes is not None else bytes(p.u.mem_read(data_ptr,width*height*4)) if data_ptr else b''
  if raw and len(raw)!=width*height*4: raise RuntimeError(f'Mesa RGBA source length mismatch {width=} {height=} {len(raw)=}')
  payload=_C.create_string_buffer(raw) if raw else None
  mesa_gl['glTexImage2D'](0x0de1,0,0x8058,width,height,0,0x1908,0x1401,_C.cast(payload,_C.c_void_p) if payload else None)
  return tex.value,hashlib.sha256(raw).hexdigest() if raw else hashlib.sha256(bytes(width*height*4)).hexdigest()
 _get_attachment=_ip.cfunc(mesa.gl,'glGetFramebufferAttachmentParameteriv',None,[_C.c_uint,_C.c_uint,_C.c_uint,_C.POINTER(_C.c_int)])
 def _attachment(target,pname):
  value=_C.c_int(); _get_attachment(target,0x8ce0,pname,_C.byref(value)); return value.value
 def _mesa_check_status():
  draw_fbo,read_fbo=_C.c_int(),_C.c_int(); vp=(_C.c_int*4)(); mesa_gl['glGetIntegerv'](0x8ca6,_C.byref(draw_fbo)); mesa_gl['glGetIntegerv'](0x8caa,_C.byref(read_fbo)); mesa_gl['glGetIntegerv'](0x0ba2,vp)
  return {'draw_fbo':draw_fbo.value,'read_fbo':read_fbo.value,'viewport':list(vp),'draw_status':int(mesa_gl['glCheckFramebufferStatus'](0x8ca9)),'draw_color_attachment_type':_attachment(0x8ca9,0x8cd0),'draw_color_attachment_name':_attachment(0x8ca9,0x8cd1),'read_color_attachment_type':_attachment(0x8ca8,0x8cd0),'read_color_attachment_name':_attachment(0x8ca8,0x8cd1)}
 def _mesa_dispatch(name):
  x=p.x; s=p.f; rec={'name':name,'x':[x(i) for i in range(9)],'s':[s(i) for i in range(4)]}; mesa_calls.append(rec)
  if name=='glGetString':
   value=mesa_gl[name](x(0)); raw=_C.string_at(value) if value else b''; out=p.alloc(len(raw)+1); p.u.mem_write(out,raw+b'\0'); x(0,out); rec['returned']=raw.decode('utf8','replace')
  elif name=='glGetIntegerv':
   n=4 if x(0)==0x0ba2 else 1; out=(_C.c_int*n)(); mesa_gl[name](x(0),out); p.u.mem_write(x(1),bytes(memoryview(out).cast('B'))); rec['out']=list(out)
  elif name=='glGetBooleanv':
   n=4 if x(0)==0x0c23 else 1; out=(_C.c_ubyte*n)(); mesa_gl[name](x(0),out); p.u.mem_write(x(1),bytes(out)); rec['out']=list(out)
  elif name in ('glGenTextures','glGenFramebuffers','glGenBuffers','glGenVertexArrays','glGenRenderbuffers'):
   n=x(0)
   if n>128: raise RuntimeError(f'unbounded Mesa {name} count {n}')
   ids=(_C.c_uint*n)(); mesa_gl[name](n,ids); p.u.mem_write(x(1),bytes(memoryview(ids).cast('B'))); rec['ids']=list(ids)
  elif name in ('glDeleteTextures','glDeleteFramebuffers','glDeleteBuffers','glDeleteRenderbuffers'):
   n=x(0)
   if n>128: raise RuntimeError(f'unbounded Mesa {name} count {n}')
   vals=struct.unpack('<'+'I'*n,p.u.mem_read(x(1),n*4)) if n else (); ids=(_C.c_uint*n)(*vals); mesa_gl[name](n,ids); rec['ids']=list(vals)
  elif name=='glShaderSource':
   sh,n,arr,lens=x(0),x(1),x(2),x(3); strings=[]; keep=[]; lengths=[]
   if n>64: raise RuntimeError(f'unbounded shader source count {n}')
   for i in range(n):
    ptr=struct.unpack('<Q',p.u.mem_read(arr+8*i,8))[0]; ln=struct.unpack('<i',p.u.mem_read(lens+4*i,4))[0] if lens else -1
    raw=bytes(p.u.mem_read(ptr,ln)) if 0<=ln<=1<<20 else _mesa_string(ptr)
    buf=_C.create_string_buffer(raw); keep.append(buf); strings.append(_C.cast(buf,_C.c_char_p)); lengths.append(len(raw))
   mesa_gl[name](sh,n,(_C.c_char_p*n)(*strings),(_C.c_int*n)(*lengths)); rec['source_sha256']=[hashlib.sha256(bytes(buf.raw).split(b'\0',1)[0]).hexdigest() for buf in keep]; rec['source_text']=[bytes(buf.raw).split(b'\0',1)[0].decode('utf8','replace') for buf in keep]
  elif name in ('glGetShaderiv','glGetProgramiv'):
   out=_C.c_int(); mesa_gl[name](x(0),x(1),_C.byref(out)); p.put(x(2),'i',out.value); rec['out']=out.value
  elif name in ('glGetShaderInfoLog','glGetProgramInfoLog'):
   n=x(1); buf=_C.create_string_buffer(min(max(n,1),65536)); ln=_C.c_int(); mesa_gl[name](x(0),n,_C.byref(ln),buf); p.put(x(2),'i',ln.value) if x(2) else None; p.u.mem_write(x(3),buf.raw[:max(1,min(n,65536))]); rec['length']=ln.value
  elif name in ('glGetActiveUniform','glGetActiveAttrib'):
   prog,idx,bufsize,lp,sp,tp,np=[x(i) for i in range(7)]; ln,sz,kind=_C.c_int(),_C.c_int(),_C.c_uint(); buf=_C.create_string_buffer(min(max(bufsize,1),4096)); mesa_gl[name](prog,idx,bufsize,_C.byref(ln) if lp else None,_C.byref(sz) if sp else None,_C.byref(kind) if tp else None,buf); p.put(lp,'i',ln.value) if lp else None; p.put(sp,'i',sz.value) if sp else None; p.put(tp,'I',kind.value) if tp else None; p.u.mem_write(np,buf.raw[:min(max(bufsize,0),4096)]) if np else None; rec['name_out']=buf.value.decode('utf8','replace')
  elif name in ('glGetUniformLocation','glGetAttribLocation'):
   queried_name=_mesa_string(x(1)).decode('utf8','replace'); val=mesa_gl[name](x(0),queried_name.encode()); x(0,val); rec['queried_name']=queried_name; rec['returned']=val
  elif name=='glBindAttribLocation': mesa_gl[name](x(0),x(1),_mesa_string(x(2)))
  elif name=='glBufferData':
   raw=bytes(p.u.mem_read(x(2),x(1))) if x(2) and x(1) else b''; buf=_C.create_string_buffer(raw) if raw else None; mesa_gl[name](x(0),x(1),_C.cast(buf,_C.c_void_p) if buf else None,x(3)); rec['buffer_bytes']=len(raw); rec['buffer_sha256']=hashlib.sha256(raw).hexdigest(); rec['buffer_prefix_hex']=raw[:160].hex()
  elif name=='glBufferSubData':
   raw=bytes(p.u.mem_read(x(3),x(2))) if x(3) and x(2) else b''; buf=_C.create_string_buffer(raw) if raw else None; mesa_gl[name](x(0),x(1),x(2),_C.cast(buf,_C.c_void_p) if buf else None); rec['buffer_bytes']=len(raw); rec['buffer_sha256']=hashlib.sha256(raw).hexdigest()
  elif name=='glTexImage2D':
   values=[x(i) for i in range(8)]; raw=bytes(p.u.mem_read(x(8),_mesa_pixel_bytes(x(3),x(4),x(6),x(7)))) if x(8) else b''; buf=_C.create_string_buffer(raw) if raw else None; mesa_gl[name](*values,_C.cast(buf,_C.c_void_p) if buf else None); rec['upload_bytes']=len(raw); rec['upload_sha256']=hashlib.sha256(raw).hexdigest() if raw else None
  elif name=='glTexSubImage2D':
   values=[x(i) for i in range(8)]; raw=bytes(p.u.mem_read(x(8),_mesa_pixel_bytes(x(4),x(5),x(6),x(7)))) if x(8) else b''; buf=_C.create_string_buffer(raw) if raw else None; mesa_gl[name](*values,_C.cast(buf,_C.c_void_p) if buf else None); rec['upload_bytes']=len(raw)
  elif name=='glVertexAttribPointer': mesa_gl[name](x(0),x(1),x(2),x(3),x(4),_C.c_void_p(x(5)))
  elif name in ('glUniformMatrix4fv','glUniformMatrix3fv'):
   count=x(1); n=16 if name.endswith('4fv') else 9; vals=struct.unpack('<'+'f'*(n*count),p.u.mem_read(x(3),n*count*4)); mesa_gl[name](x(0),count,x(2),(_C.c_float*(n*count))(*vals)); rec['uniform_values']=list(vals)
  elif name in ('glUniform4fv','glUniform3fv','glUniform2fv','glUniform1fv'):
   n=int(name[9]); count=x(1); vals=struct.unpack('<'+'f'*(n*count),p.u.mem_read(x(2),n*count*4)); mesa_gl[name](x(0),count,(_C.c_float*(n*count))(*vals)); rec['uniform_values']=list(vals)
  elif name=='glReadPixels':
   w,h=x(2),x(3); n=_mesa_pixel_bytes(w,h,x(4),x(5)); buf=_C.create_string_buffer(n); mesa_gl[name](x(0),x(1),w,h,x(4),x(5),buf); p.u.mem_write(x(6),buf.raw); rec['readback_bytes']=n
  elif name in ('glDrawArraysInstanced','glDrawArrays'):
   fbo_state=_mesa_check_status(); args=[x(i) for i in range(4 if name.endswith('Instanced') else 3)]; mesa_gl[name](*args); rec['framebuffer']=fbo_state['draw_fbo']; rec['read_framebuffer']=fbo_state['read_fbo']; rec['draw_color_attachment_type']=fbo_state['draw_color_attachment_type']; rec['draw_color_attachment_name']=fbo_state['draw_color_attachment_name']; rec['read_color_attachment_type']=fbo_state['read_color_attachment_type']; rec['read_color_attachment_name']=fbo_state['read_color_attachment_name']; rec['viewport']=fbo_state['viewport']; rec['framebuffer_status']=hex(fbo_state['draw_status']); err=int(mesa_gl['glGetError']()); rec['gl_error_after']=hex(err)
   state_query={}
   for pname,label in ((0x8b8d,'program'),(0x85b5,'vao'),(0x8894,'array_buffer'),(0x8069,'texture_2d_binding'),(0x84e0,'active_texture')):
    val=_C.c_int(); mesa_gl['glGetIntegerv'](pname,_C.byref(val)); state_query[label]=val.value
   active=state_query['active_texture']; unit_bindings=[]
   for unit in (0x84c0,0x84c1):
    mesa_gl['glActiveTexture'](unit); val=_C.c_int(); mesa_gl['glGetIntegerv'](0x8069,_C.byref(val)); unit_bindings.append({'unit':hex(unit),'texture_2d':val.value})
   mesa_gl['glActiveTexture'](active); state_query['texture_units']=unit_bindings
   attribs=[]
   for idx in range(3):
    row={'index':idx}
    for pname,label in ((0x8622,'enabled'),(0x8623,'size'),(0x8624,'stride'),(0x8625,'type'),(0x889f,'buffer'),(0x88fe,'divisor')):
     val=_C.c_int(); mesa_gl['glGetVertexAttribiv'](idx,pname,_C.byref(val)); row[label]=val.value
    attribs.append(row)
   state_query['attributes']=attribs
   scissor_box=(_C.c_int*4)(); mesa_gl['glGetIntegerv'](0x0c10,scissor_box)
   color_mask=(_C.c_ubyte*4)(); mesa_gl['glGetBooleanv'](0x0c23,color_mask)
   state_query['scissor_enabled']=bool(mesa_gl['glIsEnabled'](0x0c11)); state_query['scissor_box']=list(scissor_box); state_query['color_write_mask']=list(color_mask)
   rec['mesa_state_before_draw']=state_query
   fbo=fbo_state['draw_fbo']; vp=fbo_state['viewport']
   if fbo and vp[2]>0 and vp[3]>0 and vp[2]*vp[3]<=4_000_000:
    _read_state=_mesa_check_status()
    raw=_C.create_string_buffer(vp[2]*vp[3]*4); mesa_gl['glReadPixels'](0,0,vp[2],vp[3],0x1908,0x1401,raw); pix=raw.raw; _read_error=int(mesa_gl['glGetError']())
    rec['readback_framebuffer_state']={'draw_framebuffer':_read_state['draw_fbo'],'read_framebuffer':_read_state['read_fbo'],'draw_attachment_type':_read_state['draw_color_attachment_type'],'draw_attachment_name':_read_state['draw_color_attachment_name'],'read_attachment_type':_read_state['read_color_attachment_type'],'read_attachment_name':_read_state['read_color_attachment_name'],'draw_status':hex(_read_state['draw_status']),'viewport':_read_state['viewport']}
    rec['post_read_gl_error']=hex(_read_error)
    rec['readback']={'width':vp[2],'height':vp[3],'format':'RGBA/UNSIGNED_BYTE','bytes':len(pix),'sha256':hashlib.sha256(pix).hexdigest(),'rgba_zlib_hex':zlib.compress(pix,9).hex(),'nonzero_pixels':sum(1 for j in range(0,len(pix),4) if pix[j:j+4]!=bytes(4)),'channel_max':[max(pix[k::4]) for k in range(4)]}; mesa_draw_readbacks.append(rec['readback'])
   return
  elif name=='glCheckFramebufferStatus':
   val=int(mesa_gl[name](x(0))); x(0,val); rec['returned']=hex(val)
  elif name=='glInvalidateFramebuffer':
   target,n,ptr=x(0),x(1),x(2)
   if n<0 or n>16: raise RuntimeError(f'unbounded glInvalidateFramebuffer attachment count {n}')
   vals=struct.unpack('<'+'I'*n,p.u.mem_read(ptr,n*4)) if n else ()
   arr=(_C.c_uint*n)(*vals); mesa_gl[name](target,n,arr); rec['attachments']=[hex(v) for v in vals]
  elif name=='glGetError':
   val=int(mesa_gl[name]()); x(0,val); rec['returned']=hex(val)
  elif name in ('glCreateShader','glCreateProgram'):
   val=mesa_gl[name](x(0)) if name=='glCreateShader' else mesa_gl[name](); x(0,val); rec['returned']=val
  elif name in ('glUniform1f','glUniform2f','glUniform3f','glUniform4f','glClearColor'):
   args=[x(0)]+[s(i) for i in range({'glUniform1f':1,'glUniform2f':2,'glUniform3f':3,'glUniform4f':4,'glClearColor':4}[name])]; mesa_gl[name](*args); rec['float_args']=args[1:]
  else:
   counts={'glActiveTexture':1,'glAttachShader':2,'glBindBuffer':2,'glBindFramebuffer':2,'glBindRenderbuffer':2,'glBindTexture':2,'glBindVertexArray':1,'glBlendEquationSeparate':2,'glBlendFuncSeparate':4,'glBufferData':4,'glClear':1,'glDisable':1,'glEnable':1,'glEnableVertexAttribArray':1,'glDisableVertexAttribArray':1,'glFramebufferTexture2D':5,'glFramebufferRenderbuffer':4,'glPixelStorei':2,'glRenderbufferStorage':4,'glTexParameteri':3,'glUseProgram':1,'glVertexAttribDivisor':2,'glViewport':4,'glBindAttribLocation':3,'glLinkProgram':1,'glCompileShader':1,'glDeleteProgram':1,'glDeleteShader':1,'glDetachShader':2,'glFinish':0,'glFlush':0,'glScissor':4,'glColorMask':4,'glDepthFunc':1,'glDepthMask':1,'glCullFace':1,'glBlendFunc':2,'glBlendEquation':1,'glDrawArrays':3,'glDrawElements':4,'glUniform1i':2,'glUniform2i':3,'glUniform3i':4,'glUniform4i':5,'glClearStencil':1,'glClearDepthf':1}
   n=counts.get(name)
   if n is None or name not in mesa_gl: raise RuntimeError(f'no pinned Mesa forwarding rule for native GLES import {name}')
   args=[x(i) for i in range(n)]; mesa_gl[name](*args)
   if name=='glBindFramebuffer' and args[1]:
    fbo_state=_mesa_check_status(); fbo=fbo_state['draw_fbo']; rec.update({'bound_framebuffer':fbo,'framebuffer_status':hex(fbo_state['draw_status']),'host_preclear':False})
    if fbo not in mesa_cleared_fbos and fbo_state['draw_status']==0x8cd5:
     mesa_gl['glClearColor'](0.0,0.0,0.0,0.0); mesa_gl['glClear'](0x00004000); error=int(mesa_gl['glGetError']()); mesa_cleared_fbos.add(fbo); rec.update({'host_preclear':True,'preclear_color':[0,0,0,0],'preclear_error':hex(error),'viewport':fbo_state['viewport']})
   if name=='glClear':
    fbo_state=_mesa_check_status(); fbo=fbo_state['draw_fbo']; mesa_cleared_fbos.add(fbo); rec.update({'cleared_framebuffer':fbo,'viewport':fbo_state['viewport'],'framebuffer_status':hex(fbo_state['draw_status'])})
 # Existing imported callback closures call this method dynamically.
 glhost.gl_host_import=_mesa_dispatch
 # The native shader manager first asks its cache for a program binary.
 # Model the explicit cache-miss result used by the joined InkPen harness;
 # shader source compilation and linking still go through real Mesa below.
 def _gl_program_binary_cache_miss():
  rec={'name':'glGetProgramBinary(cache-miss)','program':p.x(0),'buf_size':p.x(1),'length_out':p.x(2),'format_out':p.x(3),'binary_out':p.x(4)}
  if rec['length_out']: p.put(rec['length_out'],'i',0)
  if rec['format_out']: p.put(rec['format_out'],'I',0)
  mesa_calls.append(rec)
 p.callbacks['glGetProgramBinary']=_gl_program_binary_cache_miss
 p.callbacks['glProgramBinary']=lambda:(_ for _ in ()).throw(RuntimeError('unexpected native shader-binary load; only cache miss is adapted'))
 # Native diagnostic formatting is a libc boundary; compilation/linking is
 # still checked through Mesa's status/log APIs. Preserve the bounded
 # destination write expected by the existing pinned host shim.
 def _vsnprintf_empty_diagnostic():
  dest,size=p.x(0),p.x(1)
  if dest and size: p.u.mem_write(dest,b'\0')
  p.x(0,0)
 p.callbacks['vsnprintf']=_vsnprintf_empty_diagnostic
 # Native shader-cache file I/O is a host seam. Return an empty in-memory
 # file for reads and retain bounded writes in the scratch trace; this keeps
 # cache behavior explicit without replacing Mesa shader compilation.
 _shader_cache_files={}
 def _cache_fopen():
  path=_bytes_until_nul(p.x(0)).decode('utf8','replace'); mode=_bytes_until_nul(p.x(1)).decode('ascii','replace')
  token=p.alloc(16); _shader_cache_files[token]={'path':path,'mode':mode,'bytes':bytearray()}; p.x(0,token)
  mesa_calls.append({'name':'fopen(shader-cache)','path':path,'mode':mode,'token':hex(token)})
 def _cache_fread():
  ptr,size,count,token=(p.x(i) for i in range(4)); entry=_shader_cache_files.get(token)
  if entry is None: raise RuntimeError(f'fread on unknown shader-cache FILE {token:#x}')
  p.x(0,0); mesa_calls.append({'name':'fread(shader-cache-miss)','size':size,'count':count,'token':hex(token)});
 def _cache_fwrite():
  ptr,size,count,token=(p.x(i) for i in range(4)); entry=_shader_cache_files.get(token)
  if entry is None or size*count>1<<20: raise RuntimeError(f'bounded fwrite shader-cache failure {token=:#x} {size=} {count=}')
  entry['bytes'].extend(p.u.mem_read(ptr,size*count)); p.x(0,count); mesa_calls.append({'name':'fwrite(shader-cache)','size':size,'count':count,'token':hex(token)})
 def _cache_fclose():
  token=p.x(0); entry=_shader_cache_files.pop(token,None)
  if entry is None: raise RuntimeError(f'fclose on unknown shader-cache FILE {token:#x}')
  p.x(0,0)
 p.callbacks.update({'fopen':_cache_fopen,'fread':_cache_fread,'fwrite':_cache_fwrite,'fclose':_cache_fclose,'fflush':lambda:p.x(0,0),'setbuf':lambda:None,'fileno':lambda:p.x(0,0),'fsync':lambda:p.x(0,0)})
 # Use Mesa for the previously special-cased divisor call as well.
 _old_vertex_divisor=p.callbacks.get('glVertexAttribDivisor')
 def _mesa_divisor_callback(): _mesa_dispatch('glVertexAttribDivisor'); p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
 p.callbacks['glVertexAttribDivisor']=_mesa_divisor_callback
 renderer_base=0x6000000
 # Resolve the real ContextHolder and GLES implementation vtables required
 # by ShaderManager::Key and native Pencil3 RT initialization.
 for off,val in ((0x6d630,0x6a0a0),(0x6a0c0,0x5157c),(0x6d4a8,0x73288),(0x6d4b0,0x73290),(0x6d4a0,0x302c8),(0x6d620,0x6a068),(0x6d650,0x6a210),(0x6d428,0x69ca0),(0x6d6d0,0x6a608),(0x6a618,0x55954),(0x6a620,0x55ca0),(0x6d890,0x6ae90)):
  p.put(renderer_base+off,'Q',renderer_base+val)
 # Apply the actual Renderer ELF relocations for the depth-stencil and
 # rasterizer vtable/GOT entries. This preserves the shipped DSO's bytes and
 # resolves its symbol values rather than constructing a host vtable.
 def _renderer_relocations(wanted):
  import subprocess
  blob=(LIBS/'libSPenRenderer.so').read_bytes()
  shoff=struct.unpack_from('<Q',blob,40)[0]; entsz,nsec,shstr=struct.unpack_from('<HHH',blob,58)
  sections=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*entsz) for i in range(nsec)]
  shnames=blob[sections[shstr][4]:sections[shstr][4]+sections[shstr][5]]
  byname={shnames[s[0]:shnames.find(b'\0',s[0])].decode():s for s in sections}
  dynsym,dynstr=byname['.dynsym'],byname['.dynstr']; strings=blob[dynstr[4]:dynstr[4]+dynstr[5]]
  syms=[]
  for pos in range(dynsym[4],dynsym[4]+dynsym[5],dynsym[9]):
   no,info,other,shndx,value,size=struct.unpack_from('<IBBHQQ',blob,pos)
   syms.append((strings[no:strings.find(b'\0',no)].decode() if no else '',value))
  records=[]
  for section_name in ('.rela.dyn','.rela.plt'):
   section=byname.get(section_name)
   if not section: continue
   for pos in range(section[4],section[4]+section[5],section[9]):
    where,info,addend=struct.unpack_from('<QQq',blob,pos); typ,index=info&0xffffffff,info>>32
    if where not in wanted and not 0x6a508<=where<=0x6a5f8 and where!=0x6d6a8 and not 0x6af50<=where<0x6b078: continue
    name,value=syms[index] if index else ('',0)
    if typ==0x403: resolved=renderer_base+addend
    elif typ in (0x101,0x401):
     resolved=renderer_base+value+addend if value else 0
     if not resolved and name:
      pretty=subprocess.check_output(['c++filt','-n',name],text=True).strip()
      resolved=p.syms.get(pretty,0)
      if resolved: resolved+=addend
    else: raise RuntimeError(f'unexpected Renderer relocation {typ:#x} at {where:#x}')
    if not resolved: raise RuntimeError(f'unresolved Renderer relocation {section_name} {where:#x} {name}')
    p.put(renderer_base+where,'Q',resolved)
    records.append({'section':section_name,'offset':hex(where),'type':hex(typ),'symbol':name,'symbol_value':hex(value),'addend':hex(addend),'resolved':hex(resolved)})
  if not wanted.issubset({int(row['offset'],16) for row in records}):
   raise RuntimeError(f'Renderer relocation coverage mismatch: {records!r}')
  return records
 renderer_relocations=_renderer_relocations({0x6a198,0x6a1a0,0x6a1a8,0x6a1b0,0x6a1b8,0x6d640,0x72870,0x6a648,0x6a650,0x6a658,0x6a660,0x6a668,0x6a670,0x6d6a8,0x6d888})
 for slot,method in enumerate((0x530a0,0x530e8,0x52218,0x52380,0x52a10,0x52824,0x52464,0x5264c,0x53134,0x53140,0x53224)):
  p.put(renderer_base+0x6a220+slot*8,'Q',renderer_base+method)
 for slot,method in enumerate((0x51490,0x51494,0x514b8,0x514cc,0x51570)):
  p.put(renderer_base+0x6a078+slot*8,'Q',renderer_base+method)
 for table,method in ((0x6aea0,0x5b9c8),(0x6aea8,0x5ba20),(0x6aeb0,0x5b824),(0x6aeb8,0x5b9bc),(0x6aec0,0x5b9c4),(0x6aec8,0x5ba6c),(0x6aed0,0x5b2c4),(0x6aed8,0x5c5a8),(0x6aee0,0x5c600),(0x6af08,0x45600),(0x6af10,0x4563c),(0x6af18,0x5c5d4),(0x6af20,0x5c604)):
  p.put(renderer_base+table,'Q',renderer_base+method)
 shader_manager=p.alloc(512); p.put(renderer_base+0x6d480,'Q',shader_manager)
 p.call('SPen::ShaderManager::ShaderManager()',[shader_manager],limit=2000000)
 p.call('SPen::ShaderManager::GetInstance()',[])
 native_resource_calls_shader={'manager':hex(shader_manager),'singleton':hex(p.x(0)),'host_egl_context':0xE611}
 native_resource_calls=[]
 pending_native_texture_factory_returns={}
 native_texture_factory_returns=[]
 native_texture_object_events=[]
 native_offscreen_factory_calls=[]
 _offscreen_entry=p.syms.get('SPen::GLES::RendererFactoryImpl::CreateOffscreenRenderTarget(SPen::ITextureObject*)')
 _offscreen_ctor=p.syms.get('SPen::GLES::OffscreenRenderTarget::OffscreenRenderTarget(SPen::ITextureObject*)')
 native_offscreen_ctor_calls=[]
 native_offscreen_ctor_returns=[]
 def _trace_native_offscreen_factory(_uc,address,_size,_data):
  if address==_offscreen_entry: native_offscreen_factory_calls.append({'entry':hex(address),'input_texture_object':hex(p.x(0)),'route':'native RendererFactory/RendererFactoryImpl/OffscreenRenderTarget; explicit synthetic ITextureObject and host EGL/GLES'})
  if address==_offscreen_ctor: native_offscreen_ctor_calls.append({'entry':hex(address),'this':hex(p.x(0)),'texture':hex(p.x(1)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address in (0x605ae5c,0x605ae68):
   obj=p.x(0); native_offscreen_ctor_returns.append({'pc':hex(address),'object':hex(obj),'bytes':bytes(p.u.mem_read(obj,88)).hex() if obj else None})
 p.u.hook_add(UC_HOOK_CODE,_trace_native_offscreen_factory)
 native_graphics_factory_calls=[]; native_graphics_factory_returns=[]; graphics_init_entries=[]; pending_graphics_init=[]
 _gf_names=('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::GLES::RendererFactoryImpl::GreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::GLES::GraphicsObject::GraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)')
 _gf_entries={p.syms[n]:n for n in _gf_names if n in p.syms}
 _rtinit=p.syms.get('SPen::Pencil3StrokeDrawableRTV1::Init()')
 native_rt_calls=[]
 _rt_points={p.syms[n]:n for n in ('SPen::Pencil3StrokeDrawableRTV1::Update(bool)','SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)','non-virtual thunk to SPen::Pencil3StrokeDrawableRTV1::SetDepthMapBitmap(SPen::ISPBitmap*)') if n in p.syms}
 def _trace_graphics_init(_uc,address,_size,_data):
  if address==0x510c3b0: native_graphics_factory_returns.append({'return_pc':hex(address),'returned_graphics_object':hex(p.x(0)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address in _gf_entries: native_graphics_factory_calls.append({'entry':hex(address),'symbol':_gf_entries[address],'x0_x4':[hex(p.x(i)) for i in range(5)],'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address==_rtinit:
   obj=p.x(0); lr=p.u.reg_read(R.UC_ARM64_REG_LR); graphics_init_entries.append({'event':'init-entry','object':hex(obj),'lr':hex(lr),'field208_before':bytes(p.u.mem_read(obj+208,8)).hex()}); pending_graphics_init.append((lr,obj))
  if address in _rt_points:
   obj=p.x(0); record={'entry':hex(address),'symbol':_rt_points[address],'x0':hex(obj),'x1':hex(p.x(1)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))}
   for off in (0,8,16,24,32,40,48,56,64,72,80,88,96,104,112,120,128,136,144,152,160,168,176,184,192,200,208,216,224,232,240,248,256):
    try: record[f'plus_{off}']=bytes(p.u.mem_read(obj+off,8)).hex()
    except Exception: pass
   native_rt_calls.append(record)
  for lr,obj in list(pending_graphics_init):
   if address==lr:
    graphics_init_entries.append({'event':'init-return','object':hex(obj),'field208_after':bytes(p.u.mem_read(obj+208,8)).hex(),'field216_after':bytes(p.u.mem_read(obj+216,8)).hex()}); pending_graphics_init.remove((lr,obj))
 p.u.hook_add(UC_HOOK_CODE,_trace_graphics_init)
 native_caps=p.alloc(16); native_caps_vtable=p.alloc(32); p.put(native_caps,'Q',native_caps_vtable)
 p.u.mem_write(0x200f2300,struct.pack('<II',0x52820000,0xd65f03c0)); p.put(native_caps_vtable+16,'Q',0x200f2300)
 def _pencil_caps():
  native_resource_calls.append({'call':'RendererFactory::GetRendererDeviceCaps','returned':hex(native_caps),'max_texture_size':4096,'host_adapter':'4096 IRendererDeviceCaps slot+16 contract reused from BrushPen TextureProbe'})
  p.x(0,native_caps)
 pencil_texture_vtable=p.alloc(256); pencil_texture_data_vtable=p.alloc(96)
 for _offset in range(0,256,8): p.put(pencil_texture_vtable+_offset,'Q',0x200f2310)
 for _offset in range(0,96,8): p.put(pencil_texture_data_vtable+_offset,'Q',0x200f2310)
 p.u.mem_write(0x200f2310,bytes.fromhex('e00300aac0035fd6'))
 p.u.mem_write(0x200f2320,struct.pack('<II',0x52802800,0xd65f03c0)) # GetWidth -> 320 synthetic source texture
 p.u.mem_write(0x200f2330,struct.pack('<II',0x52801680,0xd65f03c0)) # GetHeight -> 180 synthetic source texture
 p.put(pencil_texture_vtable+32,'Q',0x200f2320)
 p.put(pencil_texture_vtable+40,'Q',0x200f2330)
 p.put(pencil_texture_vtable+64,'Q',TEXTURE_UPDATE_STUB)
 p.put(pencil_texture_vtable+48,'Q',TEXTURE_ACTIVATE_STUB)
 p.put(pencil_texture_vtable+112,'Q',TEXTURE_PARAM_STUB)
 p.put(pencil_texture_vtable+160,'Q',TEXTURE_TAG_STUB)
 p.put(pencil_texture_data_vtable+48,'Q',TEXTURE_UPDATE_STUB)
 texture_objects_for_mesa=[]
 native_texture_factory_returns=[]
 def _pencil_texture_object():
  descriptor=p.x(0); label=p.x(1); initial=p.x(2); pairs=p.x(3); pair_count=p.x(4); info=p.x(5)
  words=struct.unpack('<8I',p.u.mem_read(descriptor,32)); info_words=struct.unpack('<4I',p.u.mem_read(info,16)); pair_words=struct.unpack('<'+'I'*(pair_count*2),p.u.mem_read(pairs,pair_count*8)) if pair_count else ()
  label_bytes=bytes(p.u.mem_read(label,128)).split(b'\0',1)[0].decode('ascii','replace')
  tex=p.alloc(320); resource_bytes=None; resource_source='host input source'
  if label_bytes.endswith('mPatternsTexture'):
   from pencil_texture_native import read_va as _read_pencil_va
   raw=_read_pencil_va(ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so',0x12c8c,16*120*120*4)
   resource_bytes=b''.join(bytes((raw[i],0,0,0)) for i in range(0,len(raw),4)); resource_source='RGBA expansion of pinned native g_penPtn at VA 0x12c8c'
   if os.environ.get('PENCIL_MASK_CONTROL','native')=='white': resource_bytes=bytes((255,255,255,255))*(120*1920); resource_source='explicit white-mask host control'
  elif label_bytes.endswith('mTextureBG'):
   from pencil_texture_native import read_va as _read_pencil_va
   raw=_read_pencil_va(ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenPencil3.so',0xf3c8c,150*150)
   resource_bytes=b''.join(bytes((v,0,0,0)) for v in raw); resource_source='RGBA expansion of pinned native g_backgroundPtn at VA 0xf3c8c'
  gl_id,surface_sha256=_mesa_texture(words[0],words[1],initial,data_bytes=resource_bytes)
  p.put(tex,'Q',pencil_texture_vtable); p.put(tex+16,'Q',pencil_texture_data_vtable); p.put(tex+40,'II',0x0de1,gl_id); p.put(tex+64,'II',words[0],words[1]); p.put(tex+76,'II',words[3],words[4]); p.put(tex+268,'I',0)
  native_resource_calls.append({'call':'RendererFactory::CreateTextureObject host ITextureObject adapter','descriptor_u32':list(words),'buffer_info_u32':list(info_words),'u32_pairs':list(pair_words),'label':label_bytes,'initial_source_is_null':initial==0,'returned_interface_adapter':hex(tex),'adapter_fields':{'gl_target':0x0de1,'mesa_texture_id':gl_id,'dimensions':[words[0],words[1]]},'surface_sha256':surface_sha256,'resource_source':resource_source,'resource_bytes':len(resource_bytes) if resource_bytes is not None else None})
  native_texture_factory_returns.append({'returned_interface':hex(tex),'object_vptr':hex(struct.unpack('<Q',p.u.mem_read(tex,8))[0]),'mesa_texture_id':gl_id,'label':label_bytes})
  texture_objects_for_mesa.append({'object':tex,'mesa_texture_id':gl_id,'width':words[0],'height':words[1],'sha256':surface_sha256,'label':label_bytes})
  p.x(0,tex)
 # Leave CreateOffscreenRenderTarget unresolved to host callbacks so the
 # pinned RendererFactory wrapper/OffscreenRenderTarget constructor execute.
 # Its ITextureObject and EGL/GLES dependencies remain explicit host adapters.
 p.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=_pencil_caps
 p.callbacks['SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']=_pencil_texture_object
 _native_texture_factory_entry=p.syms['SPen::GLES::RendererFactoryImpl::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)']
 _native_texture_factory_return=0x200f2260
 native_texture_factory_pending=[]
 p.u.mem_write(_native_texture_factory_return,struct.pack('<II',0xd503201f,0xd65f03c0))
 def _trace_texture_factory(_uc,address,_size,_data):
  if address==_native_texture_factory_entry:
   if not native_texture_factory_pending: raise RuntimeError('native Renderer texture factory entered without a pending host request')
   frame=native_texture_factory_pending.pop(); row=frame['record']; caller=frame['caller']
   descriptor=p.x(0); info=p.x(5); count=p.x(4); pairs=p.x(3)
   row.update({'entry':hex(address),'args_x0_x5':[hex(p.x(i)) for i in range(6)],'label':bytes(p.u.mem_read(p.x(1),128)).split(b'\0',1)[0].decode('ascii','replace'),'descriptor_u32':list(struct.unpack('<8I',p.u.mem_read(descriptor,32))),'buffer_info_u32':list(struct.unpack('<4I',p.u.mem_read(info,16))),'u32_pairs':list(struct.unpack('<'+'I'*(count*2),p.u.mem_read(pairs,count*8))) if count else [],'original_caller':hex(caller)})
   pending_native_texture_factory_returns.setdefault(_native_texture_factory_return,[]).append({'row':row,'caller':caller})
   p.u.reg_write(R.UC_ARM64_REG_LR,_native_texture_factory_return)
  elif address==_native_texture_factory_return:
   frames=pending_native_texture_factory_returns.get(address,[])
   if not frames: raise RuntimeError('native Renderer texture factory returned without pending frame')
   frame=frames.pop(); obj=p.x(0); row=frame['row']; row.update({'returned_interface':hex(obj),'object_first_32':bytes(p.u.mem_read(obj,32)).hex() if obj else None,'object_vptrs':[hex(v) for v in struct.unpack('<2Q',p.u.mem_read(obj,16))] if obj else None})
   native_texture_factory_returns.append(row)
   p.u.reg_write(R.UC_ARM64_REG_LR,frame['caller']); p.u.reg_write(R.UC_ARM64_REG_PC,frame['caller'])
   if not frames: pending_native_texture_factory_returns.pop(address,None)
 p.u.hook_add(UC_HOOK_CODE,_trace_texture_factory)
 p.callbacks['strncpy']=lambda:(p.x(0,p.x(0)))
 # Offscreen target creation and RTV1 Init run natively. No framebuffer target
 # fields are patched; the later Draw fault records the remaining resource gap.
 clip_boundary_calls=[]; clip_constructor_entries=[]; clip_ctor=p.syms.get('SPen::SPCanvasClipping::SPCanvasClipping(SPen::ISPBitmap*)'); clip_entry=p.syms.get('SPen::SPCanvasClipping::EnableClipRT(int)'); disable_entry=p.syms.get('SPen::PenDrawableRT::DisableDepthStencil()'); activate_entry=p.syms.get('SPen::SPSubBitmapRT::ActivateFrameBufferRT(bool)')
 def _clip_resource_boundary(_uc,address,_size,_data):
  if address==clip_ctor: clip_constructor_entries.append({'x0':hex(p.x(0)),'x1':hex(p.x(1)),'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
  if address==clip_entry:
   clip_boundary_calls.append({'entry':hex(address),'this':hex(p.x(0)),'enabled':p.x(1),'boundary':'native EnableClipRT diagnostic'})
  elif address==activate_entry:
   obj=p.x(0); fields={}
   for off in (0,16,24,32,40,48,56,64,72,80,88,96,104,112,120):
    try: fields[str(off)]=hex(struct.unpack('<Q',p.u.mem_read(obj+off,8))[0])
    except Exception: fields[str(off)]='unreadable'
   target=int(fields.get('88','0'),16); nested={}
   if target:
    for off in (0,8,16,24,32,40,48,56,64):
     ptr=target+off
     try: nested[str(off)]={'bytes':bytes(p.u.mem_read(ptr,8)).hex(),'value':hex(struct.unpack('<Q',p.u.mem_read(ptr,8))[0])}
     except Exception: nested[str(off)]={'unreadable':True}
    child=int(nested.get('16',{}).get('value','0'),16)
    if child:
     nested['child']=hex(child); nested['child_bytes']=bytes(p.u.mem_read(child,32)).hex()
   clip_boundary_calls.append({'entry':hex(address),'this':hex(obj),'enabled':p.x(1),'fields':fields,'target_object':nested,'x0_x7':[hex(p.x(i)) for i in range(8)],'boundary':'native ActivateFrameBufferRT entry'})
  elif address==disable_entry:
   clip_boundary_calls.append({'entry':hex(address),'this':hex(p.x(0)),'boundary':'host depth/stencil operation adapter'})
   p.u.reg_write(R.UC_ARM64_REG_PC,p.u.reg_read(R.UC_ARM64_REG_LR))
 p.u.hook_add(UC_HOOK_CODE,_clip_resource_boundary)
 paper_mode=os.environ.get('PENCIL_PAPER_MODE','native')
 native_surface=(bytes((255,255,255,255)) if paper_mode=='white' else bytes(4))*(320*180)
 native_surface_ptr=p.alloc(len(native_surface)); p.u.mem_write(native_surface_ptr,native_surface)
 native_surface_tag=p.alloc(32); p.u.mem_write(native_surface_tag,b'pencil-consumer-synthetic\0')
 native_bitmap_factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
 native_bitmap_factory_calls=[]
 _bitmap_factory_entry=p.syms[native_bitmap_factory]
 def _trace_bitmap_factory(_uc,address,_size,_data):
  if address==_bitmap_factory_entry:
   sp=p.u.reg_read(R.UC_ARM64_REG_SP)
   native_bitmap_factory_calls.append({'entry':hex(address),'x0_x7':[hex(p.x(i)) for i in range(8)],'stack_args_8_9_u32':list(struct.unpack('<II',p.u.mem_read(sp,8))),'stack_32_bytes':bytes(p.u.mem_read(sp,32)).hex()})
 p.u.hook_add(UC_HOOK_CODE,_trace_bitmap_factory)
 # CreateBitmap has ten AAPCS64 arguments; enum arguments 8 and 9 are at SP
 # and SP+8. CanvasProbe.call sets SP to 0x200ef000.
 p.u.mem_write(0x200ef000,struct.pack('<II',0,0)+bytes(24))
 p.call(native_bitmap_factory,[queue,320,180,native_surface_tag,native_surface_ptr,0,0,0],limit=5000000)
 native_pencil_bitmap=p.x(0)
 native_pencil_canvas=p.alloc(256)
 p.call('SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)',[native_pencil_canvas,queue,native_pencil_bitmap],limit=5000000)
 _bitmap_vptr=struct.unpack('<Q',p.u.mem_read(native_pencil_bitmap,8))[0]
 native_pencil_canvas_info={'bitmap':hex(native_pencil_bitmap),'bitmap_vptr':hex(_bitmap_vptr),'canvas':hex(native_pencil_canvas),'canvas_vptr':hex(struct.unpack('<Q',p.u.mem_read(native_pencil_canvas,8))[0]),'canvas_bitmap_getter':hex(struct.unpack('<Q',p.u.mem_read(struct.unpack('<Q',p.u.mem_read(native_pencil_canvas,8))[0]+192,8))[0]),'expected_getbitmap':hex(p.syms['SPen::SPPenCanvas::GetBitmap()']),'bitmap_get_writable':hex(struct.unpack('<Q',p.u.mem_read(_bitmap_vptr+152,8))[0]),'expected_writable':hex(p.syms['SPen::SPTextureBitmap::GetBitmapWritable()']),'bitmap_get_texture':hex(struct.unpack('<Q',p.u.mem_read(_bitmap_vptr+168,8))[0]),'expected_get_texture':hex(p.syms['SPen::SPTextureBitmap::GetTextureBitmap()'])}
 print('NATIVE_CANVAS_CHECK',native_pencil_canvas_info)
 if native_pencil_canvas_info['canvas_bitmap_getter'] != native_pencil_canvas_info['expected_getbitmap'] or native_pencil_canvas_info['bitmap_get_writable'] != native_pencil_canvas_info['expected_writable'] or native_pencil_canvas_info['bitmap_get_texture'] != native_pencil_canvas_info['expected_get_texture']:
  raise RuntimeError('native Pencil canvas/bitmap vtable contract mismatch: '+repr(native_pencil_canvas_info))
 p.pen_canvas=native_pencil_canvas
'''
setup_anchor = "p.call('SPen::ObjectDrawing::ObjectDrawing()',[p.drawing]);"
if source.count(setup_anchor) != 1:
    raise RuntimeError("native Drawing setup anchor changed")
source = source.replace(setup_anchor, native_setup + "\n " + setup_anchor)
old = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.'}"
new = "'scope':'Native Drawing caller and direct replay of its selected versioned Pencil3 RedrawPen entry under an explicit synthetic context. Does not claim Samsung device defaults, SDK deserialization, renderer pixels, or export parity.','paper_depthmap_injected':paper_depthmap_injected,'paper_input':paper_input,'paper_setter_trace':paper_setter_trace,'paper_gl_id':paper_gl_id,'paper_surface_sha256':paper_surface_sha256,'background_gl_id':background_gl_id,'background_surface_sha256':background_surface_sha256,'paper_mode':paper_mode,'native_surface_bytes':len(native_surface),'native_surface_sha256':hashlib.sha256(native_surface).hexdigest(),'penreturn_callback_events':penreturn_callback_events,'penreturn_symbols':{n:hex(a) for n,a in penreturn_symbols.items()},'queued_tasks':queued_tasks,'executed_task_runs':executed_task_runs,'task_run_seen':task_run_seen,'scale_after':scale_after,'screen_resolution_calls':screen_resolution_calls,'paint_entries':paint_entries,'native_message_dispatch':native_message_dispatch,'context_enqueue_calls':context_enqueue_calls,'texture_vtable_calls':texture_vtable_calls,'texture_adapter_events':texture_adapter_events,'set_pen_data_calls':set_pen_data_calls,'message_queue_contract':{'context_enqueue_entry':hex(MESSAGE_CONTEXT_CALL),'expected_native_message_vptrs':[hex(v) for v in sorted(EXPECTED_MESSAGE_VPTRS)],'return_stub':hex(MESSAGE_RETURN)},'glv3_depthmap_ptr_after_saved_redraw':hex(struct.unpack('<Q',p.u.mem_read(actual_drawable+488,8))[0]),'rtv1_paper_state_after_saved_redraw':{'depthmap_ptr':hex(struct.unpack('<Q',p.u.mem_read(rt+248,8))[0]),'depthmap_dimensions':list(struct.unpack('<II',p.u.mem_read(rt+256,8))),'background_scale_f32_bits':p.u.mem_read(rt+264,4).hex()}}"
if source.count(old) != 1:
    raise RuntimeError("native caller result assembly changed")
source = source.replace(old,new)
context_width=int(os.environ.get('PENCIL_CONTEXT_WIDTH','1920')); context_height=int(os.environ.get('PENCIL_CONTEXT_HEIGHT','1080'))
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--fixture',type=Path)
mode=parser.add_mutually_exclusive_group()
mode.add_argument('--write-fixture',action='store_true')
mode.add_argument('--check-fixture',action='store_true')
args_cli=parser.parse_args()
FIXTURE=args_cli.fixture or ROOT/'tmp/vector-parity-plan/pencil-paper-consumer-paint-next/output_mesa.json'
if not FIXTURE.is_absolute(): FIXTURE=ROOT/FIXTURE
sys.argv = [str(source_path), "--print-capture", "--advanced-setting", "empty", "--width", str(context_width), "--height", str(context_height)]
output = io.StringIO()
expected_native_fault=None
try:
    with contextlib.redirect_stdout(output):
        state = {"__name__": "__main__", "__file__": str(source_path), "_cd": _cd, "zlib": zlib}
        exec(compile(source, str(source_path), "exec"), state)
except Exception as error:
    if not ('state' in locals() and 'p' in state): raise
    p=state['p']; pc=p.u.reg_read(_cd.R.UC_ARM64_REG_PC); x0=p.x(0)
    try: target=struct.unpack('<Q',p.u.mem_read(x0+88,8))[0]
    except Exception as read_error: target=f'unreadable:{read_error}'
    lr=p.u.reg_read(_cd.R.UC_ARM64_REG_LR)
    if type(error).__name__!='UcError' or pc!=0x408e0d0 or lr!=0x510cf24 or target!=0:
        try: words=[bytes(p.u.mem_read(x0+o,8)).hex() for o in range(0,192,8)]
        except Exception as read_error: words=str(read_error)
        xregs={f'x{i}':hex(p.x(i)) for i in range(31)}
        try: _native_texdump={hex(addr):bytes(p.u.mem_read(addr,32)).hex() for addr in (0x10065bf0,0x10065c00,0x10065c08)}
        except Exception as _dump_error: _native_texdump=repr(_dump_error)
        print('FACTORY_DIAGNOSTIC',repr(state.get('native_texture_factory_returns')),repr(state.get('native_resource_calls')),_native_texdump,file=sys.stderr)
        texdump={hex(addr):bytes(p.u.mem_read(addr,64)).hex() for addr in (0x10065bf0,0x10065c00,0x10065c08) if 0x1000000 <= addr < 0x2000000}
        raise RuntimeError(f'unexpected native failure: {type(error).__name__}: {error}; pc={pc:#x}; lr={lr:#x}; x0={x0:#x}; xregs={xregs}; target={target}; x0_words={words}; texture_dump={texdump}; native_resource_calls={state.get("native_resource_calls")}; texture_factory_returns={state.get("native_texture_factory_returns")}; renderer_relocations={state.get("renderer_relocations")}; clip_calls={state.get("clip_boundary_calls")}; clip_ctor={state.get("clip_constructor_entries")}; offscreen={state.get("native_offscreen_factory_calls")}; offscreen_ctor={state.get("native_offscreen_ctor_calls")}; offscreen_returns={state.get("native_offscreen_ctor_returns")}') from error
    expected_native_fault={'exception':type(error).__name__,'message':str(error),'pc':hex(pc),'lr':hex(p.u.reg_read(_cd.R.UC_ARM64_REG_LR)),'x0':hex(x0),'x1':hex(p.x(1)),'x2':hex(p.x(2)),'subbitmap_target_offset':88,'subbitmap_target':hex(target),'boundary':'native Draw reached SPSubBitmapRT::DeactivateFrameBufferRT and faulted because the host-created subbitmap has no writable framebuffer target'}
if expected_native_fault is None:
    expected_native_fault={'exception':None,'returned_normally':True,'boundary':'native Draw completed without the previously observed null framebuffer target fault'}
p=state['p']
messages=state['native_message_dispatch']; render=next(r for r in messages if r['vtable']=='0x10774c8')
rt=int(render['selected_rt'],16); init=render['native_init_return']
producer_events=[r for r in state['pencil_buffer_events'] if r['phase']=='draw_object_stroke']
producer_redraw_entries=[{k:r[k] for k in ('phase','entry_pc','receiver_address','receiver_role')} for r in state['drawable_entry_snapshots'] if r['phase']=='draw_object_stroke']
producer_stream=bytes.fromhex(producer_events[-1]['after_bytes']) if producer_events else b''
render_bytes=bytes.fromhex(render['message_bytes_0_144'])
render_bitmap=int.from_bytes(render_bytes[16:24],'little')
render_rt=int.from_bytes(render_bytes[32:40],'little')
render_canvas=int.from_bytes(render_bytes[40:48],'little')
setter=next(r for r in state['paper_setter_trace'] if r['pc']=='0x510c7e4')
raw_setter_thunk=next(r for r in state['native_rt_calls'] if r['entry']=='0x510c85c')
setter_thunk={k:raw_setter_thunk[k] for k in ('entry','symbol','x0','x1','lr')}
raw_update_entry=next(r for r in state['native_rt_calls'] if r['entry']=='0x510cc94')
update_entry={k:raw_update_entry[k] for k in ('entry','symbol','x0','x1','lr','plus_208','plus_216','plus_248','plus_256')}
rt_after=bytes(p.u.mem_read(rt+248,8)).hex(); graphics_ptr=int.from_bytes(bytes.fromhex(init['field208']),'little')
gfx_factory_return=next(r for r in state['native_graphics_factory_returns'] if r['returned_graphics_object']==hex(graphics_ptr))
init_entry=next(r for r in state['graphics_init_entries'] if r['event']=='init-entry' and r['object']==hex(rt) and r['lr']=='0x200f2240')
def require(condition,detail):
 if not condition: raise RuntimeError(f'native consumer assertion failed: {detail}')
require(int.from_bytes(bytes.fromhex(render['message_bytes_0_144'])[32:40],'little')==rt,'RenderMsg receiver equals queued message+32')
require(render_bitmap==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'RenderMsg +16 is the Drawing depthmap input')
require(render_canvas==int(state['native_pencil_canvas_info']['canvas'],16),'RenderMsg +40 is the constructed native SPenCanvas')
require(bool(producer_events) and all(int(r['rt'],16)==rt for r in producer_events),'queued RT equals native AddPoint callback producer receivers')
require(bool(producer_redraw_entries),'native selected GLV3 RedrawPen entry occurred before queue submission')
require(bool(producer_stream) and len(producer_stream)%20==0,'native AddPoint producer stream is a complete sequence of 20-byte records')
producer={'redraw_entries':producer_redraw_entries,
 'addpoint_callbacks':[{'rt':r['rt'],'vector_object':r['vector_object'],'before_count_bytes':r['before_count_bytes'],'after_count_bytes':r['after_count_bytes'],'after_record_count':r['after_count_bytes']//20,'float_args':r['float_args'],'return_pc':r['return_pc']} for r in producer_events],
 'rt':producer_events[0]['rt'],'callback_count':len(producer_events),'final_stream':{'record_bytes':20,'record_count':len(producer_stream)//20,'byte_count':len(producer_stream),'sha256':hashlib.sha256(producer_stream).hexdigest(),'bytes_hex':producer_stream.hex()}}
require(init['x0']=='0x0' and int.from_bytes(bytes.fromhex(init['field208']),'little')==graphics_ptr!=0,'native RTV1 Init populated nonnull +208 graphics object')
require(int(setter['x0'],16)==rt and int(setter['x1'],16)==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'native setter body receiver and input bitmap')
require(int(setter_thunk['x0'],16)==rt+0xc8 and int(setter_thunk['x1'],16)==int(setter['x1'],16),'native interface thunk resolves the actual RT receiver')
require(int(update_entry['x0'],16)==rt,'native RenderMsg Update targets the same RT')
require(int.from_bytes(bytes.fromhex(rt_after),'little')==int(setter['x1'],16),'native setter stored bitmap in RT+248')
rt_dimensions=struct.unpack('<II',p.u.mem_read(rt+256,8))
require(rt_dimensions==(state['paper_input']['width'],state['paper_input']['height']),'native setter cached input bitmap dimensions in RT+256/+260')
require(int.from_bytes(bytes.fromhex(update_entry['plus_248']),'little')==0,'RT+248 was null before native setter')
require(int.from_bytes(bytes.fromhex(init_entry['field208_before']),'little')==0,'Init ran on initially uninitialized Drawing-selected RT')
require(int(gfx_factory_return['returned_graphics_object'],16)==graphics_ptr,'native factory return joins RT+208')
require(state['native_bitmap_factory_calls'][0]['stack_args_8_9_u32']==[0,0],'CreateBitmap arguments 8 and 9 were explicitly initialized')
glcalls=state['mesa_calls']
draw_glcalls=[row for row in glcalls if row.get('name') in ('glDrawArrays','glDrawArraysInstanced','glDrawElements','glDrawElementsInstanced')]
require(state['paper_depthmap_injected']['depthmap_and_background_are_distinct'],'sampled depthmap and writable render bitmap are distinct native bitmap/texture objects')
readable_rows=[r for r in state['native_bitmap_interface_trace'] if r['event']=='GetBitmapReadable-entry' and r.get('this')==state['paper_depthmap_injected']['depthmap_bitmap']]
require(bool(readable_rows) and all(int(r['event_return'],16)==int(state['paper_depthmap_injected']['depthmap_bitmap'],16)+0xc0 for r in readable_rows),'native SPTextureBitmap::GetBitmapReadable returns its +0xc0 readable subobject')
activate_rows=[r for r in state['native_bitmap_interface_trace'] if r['event']=='ActivateTextureRT-thunk-entry' and r['this']==hex(int(state['paper_depthmap_injected']['depthmap_bitmap'],16)+0xc0)]
require(any(r['unit']==1 and r['release']==0 and r['vptr']=='0x40d0018' for r in activate_rows),'native readable subobject activates the sampled paper bitmap on texture unit 1: '+repr(activate_rows))
shader_compile=[r for r in glcalls if r.get('name')=='glGetShaderiv' and r.get('x',[0,0])[1]==0x8b81]
program_link=[r for r in glcalls if r.get('name')=='glGetProgramiv' and r.get('x',[0,0])[1]==0x8b82]
require(bool(shader_compile) and all(r.get('out')==1 for r in shader_compile),'all native Draw shaders compile on Mesa')
require(bool(program_link) and all(r.get('out')==1 for r in program_link),'native Draw shader programs link on Mesa')
require(bool(draw_glcalls),'native Draw submits Mesa instanced draws')
for row in draw_glcalls:
 require(row.get('framebuffer_status')=='0x8cd5' and row.get('gl_error_after')=='0x0' and row.get('readback'),'native Draw uses a complete target and returns a Mesa readback without GL errors')
 require(row['framebuffer']==row['read_framebuffer']!=0,'native draw and read framebuffer bindings are the same nonzero FBO')
 require(row['draw_color_attachment_type']==0x1702 and row['read_color_attachment_type']==0x1702 and row['draw_color_attachment_name']!=0 and row['draw_color_attachment_name']==row['read_color_attachment_name'],'draw/read color attachments are the same actual Mesa texture')
 require(row['framebuffer'] in state['mesa_cleared_fbos'],'the actual draw framebuffer was transparently cleared before native drawing')
 if state['paper_input']['mode']=='zero': require(row['readback']['nonzero_pixels']==0,'zero paper input remains transparent after native shader sampling')
 else: require(row['readback']['nonzero_pixels']>0,'native paper source produces nonzero Mesa shader pixels')
libs={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in state['paths'].items()}
libs['Renderer']=hashlib.sha256((ROOT/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so').read_bytes()).hexdigest()
libs['libc++_shared.so']=hashlib.sha256((ROOT/'scratch/apk-analysis-native/arm64-v8a/libc++_shared.so').read_bytes()).hexdigest()
glcalls=state['mesa_calls']
draw_glcalls=[row for row in glcalls if row.get('name') in ('glDrawArrays','glDrawArraysInstanced','glDrawElements','glDrawElementsInstanced')]
gl_interest={'glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D','glGenTextures','glBindTexture','glTexImage2D','glTexSubImage2D','glBufferData','glUseProgram','glClearColor','glClear','glViewport','glScissor','glDrawArrays','glDrawArraysInstanced','glDrawElements','glDrawElementsInstanced','glShaderSource','glCompileShader','glLinkProgram','glGetShaderiv','glGetProgramiv','glGetShaderInfoLog','glGetProgramInfoLog','glGetUniformLocation','glGetAttribLocation','glBindAttribLocation','glVertexAttribPointer','glEnableVertexAttribArray','glDisableVertexAttribArray','glVertexAttribDivisor','glUniformMatrix4fv','glUniformMatrix3fv','glUniform1f','glUniform2f','glUniform3f','glUniform4f','glUniform1i','glUniform2i','glUniform3i','glUniform4i','glUniform1fv','glUniform2fv','glUniform3fv','glUniform4fv','glActiveTexture','glBlendFunc','glBlendFuncSeparate','glBlendEquation','glBlendEquationSeparate'}
gl_trace=[{k:v for k,v in row.items() if k!='buffer_data'} | ({'buffer_data':{key:value for key,value in row['buffer_data'].items() if key!='bytes_hex'} | {'sha256':hashlib.sha256(bytes.fromhex(row['buffer_data']['bytes_hex'])).hexdigest() if row['buffer_data'].get('bytes_hex') else None}} if 'buffer_data' in row else {}) for row in glcalls if row.get('name') in gl_interest]
result={'schema':'pencil3-paper-consumer-native-v1','runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(source_path.read_bytes()).hexdigest(),'pinned_source_hashes':SOURCE_PINS,'runtime_pins':RUNTIME_PINS,'analysis_tool_pins':TOOL_PINS,'host_gles_pins':HOST_GLES_PINS,'libraries':libs,'bitmap_factory_call':state['native_bitmap_factory_calls'],'native_assertions':{'render_message_vtable':'0x10774c8','run_slot':render['native_execute_slot_plus16'],'queued_message_bitmap':hex(render_bitmap),'queued_message_rt':render['selected_rt'],'queued_message_canvas':hex(render_canvas),'init_receiver':init_entry['object'],'init_return_x0':init['x0'],'rt_field_208_graphics_object':init['field208'],'graphics_object_factory_return':gfx_factory_return,'rt_field_216_shader':init['field216'],'rt_field_248_before_update':update_entry['plus_248'],'native_update_entry':update_entry,'native_setter_thunk':setter_thunk,'rt_setter_body_pc':setter['pc'],'rt_setter_this':setter['x0'],'rt_setter_bitmap':setter['x1'],'rt_field_248_after':rt_after,'rt_dimensions_after_setter':list(rt_dimensions),'setter_matches_input_bitmap':int(setter['x1'],16)==int(state['paper_depthmap_injected']['depthmap_bitmap'],16),'setter_matches_rt_field':int.from_bytes(bytes.fromhex(rt_after),'little')==int(setter['x1'],16),'native_renderer_relocations':state['renderer_relocations'],'native_offscreen_factory_calls':state['native_offscreen_factory_calls'],'native_offscreen_ctor_calls':state['native_offscreen_ctor_calls'],'native_offscreen_ctor_returns':state['native_offscreen_ctor_returns'],'gl_call_count':len(glcalls),'draw_gl_calls':draw_glcalls},'explicit_native_init_intervention':render['host_lifecycle_intervention'],'queue_adapter':{'kind':'synthetic IGLMsgQueue','execution':'synchronously invokes the actual queued ITask vtable+16 Run body; async scheduling and ownership lifecycle are not modeled','native_task_vptrs':[r['vtable'] for r in messages]},'inputs':{'context':[context_width,context_height],'synthetic_depthmap_dimensions':[state['paper_input']['width'],state['paper_input']['height']],'native_render_bitmap_dimensions':[320,180],'depthmap_bitmap':state['paper_depthmap_injected']['depthmap_bitmap'],'depthmap_texture':state['paper_depthmap_injected']['depthmap_texture'],'native_pen_canvas':state['native_pencil_canvas_info']},'producer':producer,'host_resources':state['native_resource_calls'],'host_operation_adapters':state['clip_boundary_calls'],'expected_native_draw_boundary':expected_native_fault,'scope':'Native Drawing caller, queued RenderMsg, actual selected RTV1 Init and SetDepthMapBitmap body are executed. Both depthmap and render bitmap use actual SPTextureBitmap wrappers; their texture objects, EGL/GLES interfaces, caps and queue scheduling are explicit host boundaries. Native Draw completes through the injected host GL/resource adapters; no Samsung device or device paint output is claimed.'}
result['native_paint_trace']={'rgba_source':{'scope':'source ObjectStroke getters and native Pen setter callbacks from the synthetic source input','object_stroke_snapshots':state['object_stroke_snapshots'],'pen_color_callbacks':state['paint_color_calls']},'paper':{'depthmap_input':state['paper_input'],'injected_surface_bytes':len(state['native_surface']),'injected_surface_sha256':hashlib.sha256(state['native_surface']).hexdigest(),'state_after_saved_redraw':state['result']['rtv1_paper_state_after_saved_redraw'],'setter_calls':state['paper_setter_trace'],'depthmap_mesa_texture_id':state['paper_gl_id'],'depthmap_texture_sha256':state['paper_surface_sha256'],'background_mesa_texture_id':state['background_gl_id'],'background_texture_sha256':state['background_surface_sha256'],'host_background_bytes':len(state['native_surface']),'host_background_sha256':hashlib.sha256(state['native_surface']).hexdigest()},'renderer':{'relocation_records':state['renderer_relocations'],'offscreen_factory_calls':state['native_offscreen_factory_calls'],'offscreen_constructor_calls':state['native_offscreen_ctor_calls'],'offscreen_constructor_returns':state['native_offscreen_ctor_returns']},'gl':{'dispatch':'native GLES imports forward through ctypes to surfaceless Mesa GLES3; no emulated GL state methods','mesa_context':state['mesa'].identity,'total_calls':len(glcalls),'interesting_calls':gl_trace,'draw_submissions':draw_glcalls,'draw_readbacks':state['mesa_draw_readbacks'],'cleared_framebuffers':sorted(state['mesa_cleared_fbos'])}}
result['native_queue_runtime']={'context_enqueue_calls':state['result']['context_enqueue_calls'],'queued_tasks':state['result']['queued_tasks'],'executed_task_runs':state['result']['executed_task_runs'],'task_run_entries':state['result']['task_run_seen'],'set_pen_data_calls':state['result']['set_pen_data_calls'],'penreturn_callback_events':state['result']['penreturn_callback_events']}
result['native_texture_interface_calls']=state['result']['texture_vtable_calls']
result['native_texture_method_events']=state['result']['texture_adapter_events']
result['native_texture_factory_returns']=state['native_texture_factory_returns']
result['native_bitmap_interface_trace']=state['native_bitmap_interface_trace']
result['native_texture_object_events']=state['native_texture_object_events']
result['native_draw_completion']=expected_native_fault
result['scope']='Native Drawing caller selected the Pencil3 RT, emitted 48 native AddPoint callbacks (960-byte stream), queued and executed native RenderMsg::Run, applied the selected paper bitmap through the actual RTV1 setter, initialized the selected RTV1 under an explicit host lifecycle intervention, and completed native Draw through a host EGL/GLES/resource adapter on Mesa. Shader compile/link and GL calls execute on Mesa; readbacks are host-rendered pixel evidence, not Samsung-device output or full application paint. The native g_backgroundPtn profile is byte-derived from the pinned Pencil3 DSO; white and zero profiles are explicitly synthetic host inputs.'
result.pop('expected_native_draw_boundary',None)
require(bool(state['native_offscreen_ctor_calls']),'native offscreen-render-target constructor executed')
factory_return=next(row for row in state['native_offscreen_ctor_returns'] if row['pc']=='0x605ae68')
require(factory_return['bytes'][:32]=='10a5060600000000'+'88a5060600000000','native constructor wrote relocated OffscreenRenderTarget vtable address points: '+factory_return['bytes'][:64])
require(bool(draw_glcalls),'native Pencil Draw submitted at least one instanced draw through the explicit host GL adapter')
encoded=json.dumps(result,indent=2,sort_keys=True)+'\n'
if args_cli.write_fixture: FIXTURE.write_text(encoded); print(f'wrote {FIXTURE.relative_to(ROOT)}')
else:
    if not FIXTURE.exists(): raise SystemExit('Pencil3 paper consumer fixture missing; review then use --write-fixture')
    if FIXTURE.read_text()!=encoded: raise SystemExit('Pencil3 paper consumer fixture mismatch')
    print(f'verified {FIXTURE.relative_to(ROOT)} (native Draw completed through declared host GLES/state adapters; Mesa readbacks are host-renderer evidence)')
