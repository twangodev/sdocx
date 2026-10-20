#!/usr/bin/env python3
"""Replay the pinned HF02 FountainPen V16 caller through captured queue tasks.

Run with the project's pinned Unicorn and AArch64 toolchain:
  python3 conformance/fountain_saved_render_native.py

No arguments check the fixture; --write-fixture explicitly refreshes it.
The queue adapter synchronously dispatches the original native task vtables in
submission order. Canvas resources are native SPPenCanvas/Bitmap objects backed
by the host GLES adapter; this remains controlled scheduling, not Android UI.
"""
import os,sys,struct,json,hashlib,argparse,ctypes as C
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]; os.chdir(ROOT)
FIXTURE=ROOT/'conformance/fountain-saved-render-native.json'
SOURCE_FIXTURE=ROOT/'conformance/object-stroke-channels-native.json'
SOURCE_FIXTURE_SHA256='794c365ed0999439c289a45713ba2a33a23d19e58324acfb8ee3956564bf1c37'
SOURCE_ARCHIVE_SHA256='dd6580a0531cf1712ef6526f1b0d51c42adaeb0d11feacca5d23ba562239da82'
PYTHON_SHA256='889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9'
RUNTIME_DIR=ROOT/'scratch/apk-analysis-runtime/python'
UNICORN_FILE_SHA256={'unicorn/__init__.py':'9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b','unicorn/unicorn.py':'e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7','unicorn/lib/libunicorn.so.2':'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c'}
TOOL_SHA256={'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump':'b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb','/usr/bin/nm':'ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1','/usr/bin/c++filt':'7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm':'507950b76ed983334636dee2c9eba697d159e47918fdd0068a21c1b87f8f7116'}
HOST_GL_SHA256={'/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6','/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812','/usr/lib/x86_64-linux-gnu/libGLdispatch.so.0.0.0':'890baeef03fd73560a2fde85ebe9fb49ca3e90686923c5ee22101b28ec8ee7aa','/usr/lib/x86_64-linux-gnu/libEGL_mesa.so.0.0.0':'af1229604d80d73422ff3767f572849dce074a0ecdba8254e93a06a88cfec2e6','/usr/lib/x86_64-linux-gnu/libLLVM.so.19.1':'3523f50f635d2a1ea47518a392451f2854b92823ceaf1345fb806099dd1a3b8a','/usr/lib/x86_64-linux-gnu/libEGL_nvidia.so.615.71.09':'55bc036537d02dd4e45d1aa117f29355d8b4f992067a8eacad18702bef18f52d','/usr/lib/x86_64-linux-gnu/libnvidia-eglcore.so.615.71.09':'d314812db5967416334e578e0e557310b9b9aaadfcb130526e29ea9f701e12c0','/usr/lib/x86_64-linux-gnu/libnvidia-egl-gbm.so.1.1.4':'c0eaa8e9c6f0167f8136a31978f46b6d001a16a1996604361d1c9cdc12ed4836','/usr/lib/x86_64-linux-gnu/libnvidia-egl-wayland2.so.1.0.2':'ed9a02f6ec01db2c55568ac6abb4ec19cd107d7ebe8db025c3b1557311f47779','/usr/lib/x86_64-linux-gnu/libnvidia-egl-xcb.so.1.0.6':'2ff3c4ac0eeadfe41da855ab7851b3e8350fb4cda5733608785d508c2aca6686','/usr/lib/x86_64-linux-gnu/libnvidia-egl-xlib.so.1.0.6':'3fea35ad3a04425f49fb0d43743fc933bfeb10ae660a9c1a728958cb19c49ce4','/usr/lib/x86_64-linux-gnu/libxcb-dri3.so.0.1.0':'ff8c8ca1388a0f73121c9d182b0241c687725df0f8ad8385d1c408f79b0a8810'}
if sys.version_info[:3]!=(3,13,5) or hashlib.sha256(Path(sys.executable).resolve().read_bytes()).hexdigest()!=PYTHON_SHA256: raise SystemExit('pinned Python 3.13.5 executable changed')
sys.path.insert(0,str(RUNTIME_DIR))
for _rel,_expected in UNICORN_FILE_SHA256.items():
 if hashlib.sha256((RUNTIME_DIR/_rel).read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned Unicorn runtime changed: {_rel}')
import unicorn
if unicorn.__version__!='2.1.4': raise SystemExit(f'unexpected Unicorn version {unicorn.__version__}')
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R
MODE_PARSER=argparse.ArgumentParser(description='Hash-pinned FountainPen ObjectStroke -> Drawing saved-caller native oracle')
MODE=MODE_PARSER.add_mutually_exclusive_group()
MODE.add_argument('--write-fixture',action='store_true',help='explicitly write or refresh the native capture')
MODE.add_argument('--check-fixture',action='store_true',help='replay and compare the native capture')
MODE_ARGS=MODE_PARSER.parse_args()
SELF_SHA256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
if not MODE_ARGS.write_fixture:
 if not FIXTURE.exists(): raise SystemExit('fixture missing; use --write-fixture explicitly')
 _preflight=json.loads(FIXTURE.read_text())
 if _preflight.get('provenance',{}).get('runner_sha256')!=SELF_SHA256: raise SystemExit('runner self-pin mismatch; regenerate fixture explicitly')
if hashlib.sha256(SOURCE_FIXTURE.read_bytes()).hexdigest()!=SOURCE_FIXTURE_SHA256: raise SystemExit('pinned source fixture changed')
SOURCE_RECORD=json.loads(SOURCE_FIXTURE.read_text())['source']
SOURCE_PAYLOAD=bytes.fromhex(SOURCE_RECORD['payload_hex'])
if len(SOURCE_PAYLOAD)!=SOURCE_RECORD['payload_length'] or hashlib.sha256(SOURCE_PAYLOAD).hexdigest()!=SOURCE_RECORD['payload_sha256']: raise SystemExit('source fixture payload length/hash mismatch')
SOURCE_FRAME=SOURCE_PAYLOAD[SOURCE_RECORD['frame_offset']:SOURCE_RECORD['frame_offset']+SOURCE_RECORD['frame_length']]
if len(SOURCE_FRAME)!=SOURCE_RECORD['frame_length'] or hashlib.sha256(SOURCE_FRAME).hexdigest()!=SOURCE_RECORD['frame_sha256']: raise SystemExit('source fixture frame length/hash mismatch')
for _tool,_expected in TOOL_SHA256.items():
 if hashlib.sha256((ROOT/_tool if not _tool.startswith('/') else Path(_tool)).read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned native analysis tool changed: {_tool}')
for _path,_expected in HOST_GL_SHA256.items():
 if hashlib.sha256(Path(_path).read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned host EGL/GLES/Mesa library changed: {_path}')
HELPER_SHA256={'marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8','airbrush_texture_native.py':'76bb130daef795be53296cff4f778be6e7be5f487c8b1d4c4b6d159bb688e470','airbrush_saved_native.py':'cad2bd9ec88a393f0b0b4a7dbe58c7b469f9b00b283d4c544f491a8ab190999a','inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae','native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91','oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2','fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30','elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'}
LIBRARY_SHA256={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','Marker3':'d9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5','AirBrushPen':'92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}
for _name,_expected in HELPER_SHA256.items():
 _path=ROOT/'conformance'/_name
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned helper changed: {_name}')
for _name,_expected in LIBRARY_SHA256.items():
 _path=ROOT/'scratch/apk-analysis-native/arm64-v8a'/('libc++_shared.so' if _name=='libc++_shared' else f'libSPen{_name}.so')
 if hashlib.sha256(_path.read_bytes()).hexdigest()!=_expected: raise SystemExit(f'pinned library changed: {_name}')
sys.path[:0]=[str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
import native_drawing_runtime
import ast
import airbrush_texture_native as at, inkpen_paint_native as ip, fountain_native as fn
rt=native_drawing_runtime.create_runtime();p=rt.p; p.dots=[]; import subprocess,re
_renderer_path=Path('scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so')
_renderer_raw=_renderer_path.read_bytes()
_renderer_base=0xa000000
_phoff=struct.unpack_from('<Q',_renderer_raw,32)[0];_phent,_phnum=struct.unpack_from('<HH',_renderer_raw,54);_extent=0
for _i in range(_phnum):
 _typ,_flags,_off,_va,_pa,_fs,_ms,_al=struct.unpack_from('<II6Q',_renderer_raw,_phoff+_i*_phent)
 if _typ==1:_extent=max(_extent,_va+_ms)
p.u.mem_map(_renderer_base,(_extent+0xfff)&~0xfff)
for _i in range(_phnum):
 _typ,_flags,_off,_va,_pa,_fs,_ms,_al=struct.unpack_from('<II6Q',_renderer_raw,_phoff+_i*_phent)
 if _typ==1 and _ms:
  p.u.mem_write(_renderer_base+_va,_renderer_raw[_off:_off+_fs])
  if _ms>_fs:p.u.mem_write(_renderer_base+_va+_fs,bytes(_ms-_fs))
for _line in subprocess.check_output(['nm','-D','-C',str(_renderer_path)],text=True,env=rt.env).splitlines():
 _parts=_line.strip().split(' ',2)
 if len(_parts)==3 and _parts[1] in ('T','W'):p.syms[_parts[2]]=_renderer_base+int(_parts[0],16)
_shoff=struct.unpack_from('<Q',_renderer_raw,40)[0];_se,_sn,_ss=struct.unpack_from('<HHH',_renderer_raw,58);_secs=[struct.unpack_from('<IIQQQQIIQQ',_renderer_raw,_shoff+_i*_se) for _i in range(_sn)];_strtab=_renderer_raw[_secs[_ss][4]:_secs[_ss][4]+_secs[_ss][5]];_by={_strtab[_s[0]:_strtab.find(b'\0',_s[0])].decode():_s for _s in _secs};_ds,_st=_by['.dynsym'],_by['.dynstr'];_strings=_renderer_raw[_st[4]:_st[4]+_st[5]];_sy=[]
for _off in range(_ds[4],_ds[4]+_ds[5],_ds[9]):
 _no,_info,_other,_ndx,_val,_size=struct.unpack_from('<IBBHQQ',_renderer_raw,_off);_sy.append((_strings[_no:_strings.find(b'\0',_no)].decode() if _no else '',_val))
for _secname in ('.rela.dyn','.rela.plt'):
 _sec=_by.get(_secname)
 if not _sec:continue
 for _off in range(_sec[4],_sec[4]+_sec[5],_sec[9]):
  _where,_info,_add=struct.unpack_from('<QQq',_renderer_raw,_off);_typ,_idx=_info&0xffffffff,_info>>32
  if _typ==0x403:p.u.mem_write(_renderer_base+_where,struct.pack('<Q',_renderer_base+_add))
  elif _typ in (0x401,0x402,0x403,0x101,0x1025,0x1026):
   _name,_value=_sy[_idx];_target=(_renderer_base+_value) if _value else p.syms.get(_name,0)
   if not _target:_target=rt.rawsyms.get(_name,0)
   if _target:p.u.mem_write(_renderer_base+_where,struct.pack('<Q',_target+(_add if _typ in (0x401,0x101) else 0)))
_objdump=Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump')
_dis=subprocess.check_output([str(_objdump),'-d','-C','-j','.plt',str(_renderer_path)],text=True,env=rt.env)
for _off,_name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',_dis,re.M):p.plt[_renderer_base+int(_off,16)]=_name
rt.bases['Renderer']=_renderer_base; p.callbacks.pop('SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)',None); LIBS=rt.LIBS; env=rt.env
# Reuse the reviewed host GLES adapter class while binding it to this exact
# HF02 Unicorn runtime. All host allocations must go through p.alloc to avoid
# the separate adapter object's heap colliding with the actual Drawing heap.
_ns={'at':at,'ip':ip,'fn':fn,'ROOT':ROOT,'LIB':LIBS,'sys':sys,'os':os,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':re,'zlib':__import__('zlib'),'C':__import__('ctypes'),'json':json,'base64':__import__('base64'),'argparse':argparse,'Path':Path,'ARGS':argparse.Namespace(width=256,height=256,point_limit=0)}
_tree=ast.parse((ROOT/'conformance/marker3_paint_scale_native.py').read_text()); _pnode=next(x for x in _tree.body if isinstance(x,ast.ClassDef) and x.name=='P')
exec(compile(ast.Module(body=[_pnode],type_ignores=[]),'<extract Renderer GLES adapter>','exec'),_ns)
_adapter=_ns['P'](); _adapter.u=p.u; _adapter.alloc=p.alloc
HOST_GLES_INFO={name:(_adapter.gles.gl_string(enum) or b'').decode('ascii','replace') for name,enum in [('version',0x1F02),('vendor',0x1F00),('renderer',0x1F01)]}
_adapter.syms.clear(); _adapter.syms.update(p.syms); _adapter.plt.clear(); _adapter.plt.update(p.plt); _adapter.errno_ptr=p.alloc(4); _adapter.locale_ptr=p.alloc(64); _adapter.locale_name=p.alloc(2); p.u.mem_write(_adapter.locale_name,b'C\0'); _adapter.current_locale=0x7009200
for _key,_callback in _adapter.callbacks.items():
 if _key.startswith('gl') or _key.startswith('egl') or _key in ('strstr','strcmp','strncmp','memcmp','memchr','strlen','__strlen_chk','__strncpy_chk','__strncpy_chk2','crc32','vsnprintf','fopen','unlink','__android_log_print','__errno','getauxval','__system_property_get','setlocale','newlocale','uselocale','freelocale','mbtowc'):
  p.callbacks[_key]=_callback
_renderer_vertex_events=[]
_orig_vertex_pointer=p.callbacks['glVertexAttribPointer']
def _capture_vertex_pointer():
 _values=[p.x(i) for i in range(6)];_binding=C.c_int();_adapter.gles.gl.glGetIntegerv(0x8894,C.byref(_binding))
 _renderer_vertex_events.append({'call':'glVertexAttribPointer','index':_values[0],'size':_values[1],'type':_values[2],'normalized':_values[3],'stride':_values[4],'pointer_offset':_values[5],'array_buffer_binding':_binding.value})
 _orig_vertex_pointer()
p.callbacks['glVertexAttribPointer']=_capture_vertex_pointer
_orig_vertex_divisor=p.callbacks['glVertexAttribDivisor']
def _capture_vertex_divisor():
 _event={'call':'glVertexAttribDivisor','index':p.x(0),'divisor':p.x(1)};_renderer_vertex_events.append(_event);_orig_vertex_divisor()
p.callbacks['glVertexAttribDivisor']=_capture_vertex_divisor
_orig_enable_vertex=p.callbacks['glEnableVertexAttribArray']
def _capture_enable_vertex():
 _renderer_vertex_events.append({'call':'glEnableVertexAttribArray','index':p.x(0)});_orig_enable_vertex()
p.callbacks['glEnableVertexAttribArray']=_capture_enable_vertex
_renderer_shader_link_events=[]
_orig_create_program=p.callbacks['glCreateProgram']
def _capture_create_program():
 _orig_create_program();_renderer_shader_link_events.append({'call':'glCreateProgram','program':p.x(0)})
p.callbacks['glCreateProgram']=_capture_create_program
_orig_attach_shader=p.callbacks['glAttachShader']
def _capture_attach_shader():
 _renderer_shader_link_events.append({'call':'glAttachShader','program':p.x(0),'shader':p.x(1)});_orig_attach_shader()
p.callbacks['glAttachShader']=_capture_attach_shader
_orig_link_program=p.callbacks['glLinkProgram']
def _capture_link_program():
 _renderer_shader_link_events.append({'call':'glLinkProgram','program':p.x(0)});_orig_link_program()
p.callbacks['glLinkProgram']=_capture_link_program
_orig_get_uniform=p.callbacks['glGetUniformLocation']
def _capture_get_uniform():
 _program,_name=p.x(0),_adapter.cstr(p.x(1)).decode('utf-8');_orig_get_uniform();_renderer_shader_link_events.append({'call':'glGetUniformLocation','program':_program,'name':_name,'location':p.x(0)})
p.callbacks['glGetUniformLocation']=_capture_get_uniform
def _draw_instanced_full_frame_only():
 _values=[p.x(i) for i in range(4)]
 _adapter.gl_calls.append({'call':'glDrawArraysInstanced','state':_adapter.gl_state(),'values':_values})
 _adapter.gles.f['glDrawArraysInstanced'](*_values)
p.callbacks['glDrawArraysInstanced']=_draw_instanced_full_frame_only
def _renderer_strncpy():
 _dst,_src,_n=p.x(0),p.x(1),p.x(2); _raw=bytes(p.u.mem_read(_src,_n)).split(b'\0',1)[0]; p.u.mem_write(_dst,_raw+b'\0'*max(0,_n-len(_raw))); p.x(0,_dst)
p.callbacks['strncpy']=_renderer_strncpy
def _renderer_delete_textures():
 _n,_ptr=p.x(0),p.x(1); _ids=list(p.get(_ptr,f'{_n}i')) if 0<_n<256 else []
 if _ids:_adapter.gles.gl.glDeleteTextures(_n,(__import__('ctypes').c_uint*_n)(*_ids))
p.callbacks['glDeleteTextures']=_renderer_delete_textures
def _renderer_delete_samplers():
 _n,_ptr=p.x(0),p.x(1); _ids=list(p.get(_ptr,f'{_n}i')) if 0<_n<256 else []
 if _ids:_adapter.gles.gl.glDeleteSamplers(_n,(__import__('ctypes').c_uint*_n)(*_ids))
p.callbacks['glDeleteSamplers']=_renderer_delete_samplers
rt.run_init(_renderer_path,_renderer_base)
# Explicitly initialize the mapped FountainPen plugin and its static factory data.
FP=LIBS/'libSPenFountainPen.so'; fpbase=rt.bases['FountainPen']; assert hashlib.sha256(FP.read_bytes()).hexdigest()=='1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
rt.run_init(FP,fpbase)
# Drawing's PenManager constructs its built-in DefaultPen before resolving the
# requested FountainPen. Map that pinned plugin into a free scratch region so
# the real selection path can continue.
DP=LIBS/'libSPenDefaultPen.so'; dpbase=0x9000000
assert hashlib.sha256(DP.read_bytes()).hexdigest()=='1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f'
b=DP.read_bytes(); phoff=struct.unpack_from('<Q',b,32)[0]; ent,num=struct.unpack_from('<HH',b,54); extent=0
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1:extent=max(extent,va+ms)
p.u.mem_map(dpbase,(extent+0xfff)&~0xfff)
for i in range(num):
 typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,phoff+i*ent)
 if typ==1 and ms:
  p.u.mem_write(dpbase+va,b[off:off+fs])
  if ms>fs:p.u.mem_write(dpbase+va+fs,bytes(ms-fs))
for line in __import__('subprocess').check_output(['nm','-D','-C',str(DP)],text=True,env=env).splitlines():
 parts=line.strip().split(' ',2)
 if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=dpbase+int(parts[0],16)
for line in __import__('subprocess').check_output(['nm','-D',str(DP)],text=True,env=env).splitlines():
 parts=line.strip().split()
 if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(parts[2],dpbase+int(parts[0],16))
shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58); secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=b[st[4]:st[4]+st[5]]; sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off); sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',b,off); typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(dpbase+where,struct.pack('<Q',dpbase+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx]; target=dpbase+val if val else rt.rawsyms.get(name,0) or p.syms.get(name,0)
   if target:p.u.mem_write(dpbase+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
import elf_plt
for off,sym in elf_plt.resolve(DP).items():
 import subprocess
 pretty=subprocess.check_output(['c++filt','-n',sym],text=True).strip() if sym.startswith('_Z') else sym
 p.plt[dpbase+off]=pretty
rt.run_init(DP,dpbase)
rt.run_init(rt.MODEL,0x7000000)
queue=p.alloc(64);qvt=p.alloc(64);p.put(queue,'Q',qvt);p.put(qvt+16,'Q',0x20001000);p._runtime_queue=queue;p._task_returns=[]
p.u.mem_write(0x20001000,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001020,bytes.fromhex('1f2003d5c0035fd6'))
p.u.mem_write(0x20001040,bytes.fromhex('1f2003d5c0035fd6'))
p.put(p.context_vtable+16,'Q',0x20001040)
queue_events=[]
p._queue_task_pending=[]
def taskqueue(u,a,z,d):
 if a==0x20001040:p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==0x20001000:
  task=p.x(1);vt=p.get(task,'Q')[0];method=p.get(vt+16,'Q')[0];_event={'task':hex(task),'vtable':hex(vt),'slot16_method':hex(method),'pre_bytes_144':bytes(u.mem_read(task,144)).hex()}
  if method==0x63804:
   _receiver=p.get(task+16,'Q')[0];_slot_byte_offset=p.get(task+24,'Q')[0];_tag=p.get(task+32,'Q')[0];_subobject=_receiver+(_tag>>1);_sub_vtable=p.get(_subobject,'Q')[0];_target=p.get(_sub_vtable+_slot_byte_offset,'Q')[0]
   _event['indirect_member_call']={'receiver':hex(_receiver),'slot_byte_offset':hex(_slot_byte_offset),'tag':_tag,'subobject':hex(_subobject),'subobject_vtable':hex(_sub_vtable),'target':hex(_target),'matching_exported_symbols':[name for name,address in p.syms.items() if address==_target]}
  _receiver=p.get(task+16,'Q')[0]
  try:_event['receiver_vptr']=hex(p.get(_receiver,'Q')[0])
  except Exception as _e:_event['receiver_vptr_error']=str(_e)
  queue_events.append(_event)
  if not method:raise RuntimeError('null queued task')
  caller=u.reg_read(R.UC_ARM64_REG_LR);p._task_returns.append(caller);p._queue_task_pending.append((task,_event));p.x(0,task);u.reg_write(R.UC_ARM64_REG_LR,0x20001020);u.reg_write(R.UC_ARM64_REG_PC,method)
 elif a==0x20001020:
  _task,_event=p._queue_task_pending.pop();_event['post_bytes_144']=bytes(u.mem_read(_task,144)).hex();_event['returned']=True;p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,p._task_returns.pop())
p.u.hook_add(UC_HOOK_CODE,taskqueue)
# Match the existing native Drawing caller's explicit 1920x1080 IContext adapter.
ctx=p.alloc(32);vtable=p.alloc(256);p.put(ctx,'Q',vtable)
ctx_child=p.alloc(64);child_vt=p.alloc(128);p.put(ctx_child,'Q',child_vt)
for addr in (0x200f2300,0x200f2340,0x200f2380,0x200f23c0,0x200f23e0,0x200f2200,0x200f2400,0x200f2420,0x200f2440,0x200f2460):p.u.mem_write(addr,bytes.fromhex('c0035fd6'))
p.put(vtable+16,'Q',0x200f2300);p.put(vtable+24,'Q',0x200f23e0);p.put(vtable+80,'Q',0x200f2340);p.put(vtable+144,'Q',0x200f2200);p.put(vtable+152,'Q',0x200f2200);p.put(child_vt+32,'Q',0x200f2380);p.put(child_vt+40,'Q',0x200f23c0)
def ctxhooks(u,a,z,d):
 if a==0x200f2300:p.x(0,ctx_child)
 elif a==0x200f23e0:p.x(0,queue)
 elif a==0x200f2340:p.x(0,p.x(1))
 elif a==0x200f2380:p.x(0,1920)
 elif a==0x200f23c0:p.x(0,1080)
 elif a in (0x200f2400,0x200f2420):p.x(0,64)
 elif a==0x200f2440:p.x(0,0)
 elif a==0x200f2460:p.x(0,0)
 else:return
 u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,ctxhooks)
# Module-loader host adapter resolves only the already-mapped, pinned FountainPen plugin.
def cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\0',1)[0].decode('ascii','replace')
loader=[]
FP_HANDLE=0x50000001; DP_HANDLE=0x90000001
def dlopen():
 name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if Path(name).name=='libSPenFountainPen.so':p.x(0,FP_HANDLE)
 elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)
 else:raise RuntimeError('unexpected DSO '+name)
def dlsym():
 handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if name!='createPenInst' or handle not in (FP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)
 plugin='libSPenFountainPen.so' if handle==FP_HANDLE else 'libSPenDefaultPen.so'
 raw=FP.read_bytes() if handle==FP_HANDLE else DP.read_bytes()
 nm=__import__('subprocess').check_output(['nm','-D',str(FP if handle==FP_HANDLE else DP)],text=True,env=env)
 offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]
 if not offsets:raise RuntimeError(plugin+' lacks createPenInst')
 p.x(0,(fpbase if handle==FP_HANDLE else dpbase)+offsets[0])
p.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)
# Capture errors for every native GLES callback after the renderer bridge is
# installed. This drains and attributes a preexisting error to its last call.
renderer_gl_error_checks=[]
_resource_phase='renderer_setup'
renderer_texture_generations=[]
_original_gen_textures=p.callbacks['glGenTextures']
def _capture_gen_textures():
 _count,_out=p.x(0),p.x(1);_original_gen_textures()
 renderer_texture_generations.append({'phase':_resource_phase,'ids':list(p.get(_out,f'{_count}I')) if 0<_count<128 else [],'count':_count})
p.callbacks['glGenTextures']=_capture_gen_textures
for _gl_name,_gl_fn in list(p.callbacks.items()):
 if _gl_name.startswith('gl') and _gl_name!='glGetError':
  def _guard_factory(_name,_fn):
   def _guarded():
    _before=_adapter.gles.f['glGetError']();_fn();_after=_adapter.gles.f['glGetError']()
    renderer_gl_error_checks.append({'call':_name,'before':_before,'after':_after})
   return _guarded
  p.callbacks[_gl_name]=_guard_factory(_gl_name,_gl_fn)
# Native ObjectStroke and per-case actual Drawing caller probes.

def call(name,args=(),floats=(),limit=5_000_000):
 for i,value in enumerate(args):p.x(i,value)
 for i,value in enumerate(floats):p.f(i,value)
 stop=0x200f1000
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,stop)
 try:p.u.emu_start(p.syms[name],stop,count=limit)
 except Exception as e:raise RuntimeError(f'{name} failed PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x} X0-X5={[hex(p.x(i)) for i in range(6)]}') from e
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=stop:raise RuntimeError(f'{name} did not return at {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
 return p.x(0)

def string(value):
 o=p.alloc(32); call('SPen::String::String()',[o]); call('SPen::String::Construct()',[o]); b=p.alloc(len(value.encode())+1);p.u.mem_write(b,value.encode()+b'\0');call('SPen::String::Set(char const*)',[o,b]);return o

drawing=call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1_000_000)
if not drawing:raise RuntimeError('DrawingFactory::CreateDrawing returned null')
# The captured PenGLRenderMsg asks its ISPPenCanvas for GetBitmap at slot 192.
# The CanvasProbe canvas has no such implementation, so create a genuine native
# bitmap/canvas pair before replay and pass that object through Drawing.
p.syms['typeinfo for SPen::IGLManagedMsgQueue']=0xB000000+0x6b400
p.put(qvt-16,'qQ',0,p.syms['typeinfo for SPen::IGLManagedMsgQueue'])
_queue_managed=p.alloc(16);_managed_queue=p.alloc(32);_managed_vt=p.alloc(64)
_managed_register=p.alloc(16);_managed_unregister=p.alloc(16);_managed_tokens={};_managed_events=[]
p.put(_managed_queue,'Q',_managed_vt);p.put(_managed_vt+16,'Q',_managed_register);p.put(_managed_vt+24,'Q',_managed_unregister);p.put(qvt+56,'Q',_queue_managed)
def _queue_resource_hooks(u,a,size,user):
 if a==_queue_managed:p.x(0,_managed_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==_managed_register:
  _manager,_obj=p.x(0),p.x(1);_token=p.alloc(8);p.put(_token,'Q',_obj);_managed_tokens[_token]=_obj;_managed_events.append({'op':'Register','manager':hex(_manager),'object':hex(_obj),'token':hex(_token)});p.x(0,_token);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==_managed_unregister:
  _manager,_token=p.x(0),p.x(1);_obj=_managed_tokens.pop(_token,None);_managed_events.append({'op':'Unregister','manager':hex(_manager),'object':hex(_obj or 0),'token':hex(_token)});p.x(0,0);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(UC_HOOK_CODE,_queue_resource_hooks)
_input_w,_input_h=1920,1080;_input_fmt=p.alloc(8);p.u.mem_write(_input_fmt,b'RGBA\0')
_input_pixels=p.alloc(_input_w*_input_h*4);p.u.mem_write(_input_pixels,bytes(_input_w*_input_h*4))
_bitmap_factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
_resource_phase='CreateBitmap used as native callback canvas'
for _i,_value in enumerate((queue,_input_w,_input_h,_input_fmt,_input_pixels,1,1024,1)):p.x(_i,_value)
_stack=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,_stack);p.u.mem_write(_stack,struct.pack('<QQ',6,2));p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000);p.u.emu_start(p.syms[_bitmap_factory],0x200f1000,count=5_000_000)
if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError(f'{_bitmap_factory} callback-canvas call failed at {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
_callback_bitmap=p.x(0);_native_pen_canvas=p.alloc(256)
call('SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)',[_native_pen_canvas,queue,_callback_bitmap])
_native_canvas_vptr=p.get(_native_pen_canvas,'Q')[0]
_native_impl=p.get(_native_pen_canvas+32,'Q')[0]
_native_canvas_bitmap=p.get(_native_impl+8,'Q')[0]
if p.get(_native_canvas_vptr+184,'Q')[0]!=p.syms['SPen::SPPenCanvas::GetMsgQueue()'] or p.get(_native_canvas_vptr+192,'Q')[0]!=p.syms['SPen::SPPenCanvas::GetBitmap()'] or _native_canvas_bitmap!=_callback_bitmap:
 raise RuntimeError('native SPPenCanvas queue/bitmap vtable or ownership join mismatch')
p.pen_canvas=_native_pen_canvas
_resource_phase='native HF02 Drawing and callback dispatch'
# Context returns its native profile object for device resolution.
records=[]
for label,mode,size,advanced_value in [('hf02_saved_v16','set',9.116800308227539,'18;0;100;')]:
 loader.clear()
 drawing=call('SPen::DrawingFactory::CreateDrawing(SPen::IContext const*)',[ctx],limit=1_000_000)
 p.callbacks['uuid_create']=lambda:p.x(0,0);p.callbacks['uuid_make']=lambda:p.x(0,0);p.callbacks['uuid_export']=lambda:p.x(0,0);p.callbacks['uuid_destroy']=lambda:None
 obj=p.alloc(256);call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStroke::Construct()',[obj]);impl=p.get(obj+32,'Q')[0]
 _source_record=SOURCE_RECORD; _payload=SOURCE_PAYLOAD; _frame=SOURCE_FRAME
 # The committed channel fixture preserves base+ObjectStroke payload bytes. Its
 # separate HF02 StringIDManager table is the only reconstructed small input.
 _table=bytes.fromhex('0200010000000900310038003b0030003b003100300030003b0000000000330063006f006d002e00730061006d00730075006e0067002e0061006e00640072006f00690064002e00730064006b002e00700065006e002e00700065006e002e007000720065006c006f00610064002e0046006f0075006e007400610069006e00500065006e00'); assert hashlib.sha256(_table).hexdigest()=='f35b044485b8a91fd825ac682c734b23c565e52fbfb869fc3d4d3e89b414035e'; _sm=p.alloc(8);p.u.mem_write(_sm,bytes(8));call('SPen::StringIDManager::StringIDManager()',[_sm]);call('SPen::StringIDManager::Construct()',[_sm]);_tb=p.alloc(len(_table));p.u.mem_write(_tb,_table);_table_rc=call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[_sm,_tb,len(_table)])
 _ctx=p.alloc(0x800);p.u.mem_write(_ctx,bytes(0x800));call('SPen::ModelContext::ModelContext()',[_ctx]);call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[_ctx,_sm]);call('SPen::ModelContext::GetStringIDManager()',[_ctx]);assert p.x(0)==_sm
 _base=_payload[:_source_record['frame_offset']];_bp=p.alloc(len(_base));p.u.mem_write(_bp,_base);_base_rc=call('SPen::ObjectBase::NewApplyBinary(unsigned char*, int, SPen::Orientation, int)',[obj,_bp,len(_base),0,0])
 _fp=p.alloc(len(_frame));p.u.mem_write(_fp,_frame);_handler=p.alloc(0x100);p.u.mem_write(_handler,bytes(0x100));call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)',[_handler,impl]);p.f(0,1.0);_frame_rc=call('SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)',[_handler,_fp,len(_frame),0])
 _attach_rc=call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,_ctx,0])
 def _native_utf8(_ptr):
  if not _ptr:return None
  call('SPen::String::GetUTF8Size() const',[_ptr]);_n=p.x(0);_out=p.alloc(_n+1);call('SPen::String::GetUTF8(char*, int) const',[_ptr,_out,_n+1]);return bytes(p.u.mem_read(_out,_n+1)).split(bytes([0]),1)[0].decode('utf-8')
 call('SPen::ObjectStroke::GetPenName() const',[obj]);_native_name=_native_utf8(p.x(0));call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]);_native_advanced=_native_utf8(p.x(0));call('SPen::ObjectStroke::GetPenSize() const',[obj]);_native_size=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex();call('SPen::ObjectStroke::GetColor() const',[obj]);_native_color=p.x(0);call('SPen::ObjectStroke::GetToolType() const',[obj]);_native_tool=p.x(0);call('SPen::ObjectStroke::GetStrokeType() const',[obj]);_native_stroke_type=p.x(0);call('SPen::ObjectStroke::IsCurveEnabled() const',[obj]);_native_curve=p.x(0)
 _serialized_native_state={'table_apply_return':_table_rc,'base_apply_return':_base_rc,'type1_apply_return':_frame_rc,'on_attach_raw_x0':_attach_rc,'pen_name':_native_name,'advanced_setting':_native_advanced,'size_f32_bits':_native_size,'color_argb':hex(_native_color),'tool_type':_native_tool,'stroke_type':_native_stroke_type,'curve_enabled':_native_curve,'model_context':hex(_ctx),'string_id_manager':hex(_sm),'object':hex(obj),'implementation':hex(impl),'type1_frame_sha256':hashlib.sha256(_frame).hexdigest(),'base_frame_sha256':hashlib.sha256(_base).hexdigest(),'table_sha256':hashlib.sha256(_table).hexdigest()}
 model_arrays={}
 for _field,_getter in [('points','SPen::ObjectStroke::GetPoint() const'),('pressure','SPen::ObjectStroke::GetPressure() const'),('timestamps','SPen::ObjectStroke::GetTimeStamp() const'),('tilt','SPen::ObjectStroke::GetTilt() const'),('orientation','SPen::ObjectStroke::GetOrientation() const')]:
  call(_getter,[obj]); model_arrays[_field]=p.x(0)
 call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)
 call('SPen::ObjectBase::IsVisible() const',[obj]); _native_visible=p.x(0)
 size_s0=None
 call('SPen::ObjectStroke::GetPenSize() const',[obj]);size_s0=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()
 pen_set_calls=[]; add_calls=[]; redraws=[]; rt_addpoint_states=[]; addpoint_pending={}; addpoint_return_states=[]; rt_dtor_entries=[]
 set_size_addr=p.syms['SPen::Pen::SetSize(float)']; get_size_addr=p.syms['SPen::Pen::GetSize()']
 redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']
 redraw_addrs={p.syms[name]:name for name in redraw_names}
 add_names=['SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)','SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)','SPen::FountainPenStrokeDrawableRTV6::AddPoint(SPen::Vector3<float>, float, SPen::Vector3<float>)'];add_addrs={p.syms[name]:name for name in add_names}
 rt_dtor_addr=p.syms['SPen::FountainPenStrokeDrawableRT::~FountainPenStrokeDrawableRT()']
 def trace(u,a,z,d):
  if a==set_size_addr:pen_set_calls.append({'this':hex(p.x(0)),'input_s0':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'return_w0':None})
  elif a==rt_dtor_addr and add_calls and p.x(0)==int(add_calls[0]['receiver'],16):rt_dtor_entries.append({'receiver':hex(p.x(0)),'vptr_at_entry':hex(p.get(p.x(0),'Q')[0])})
  elif a==set_size_addr+0x2c and pen_set_calls:pen_set_calls[-1]['return_w0']=p.x(0)
  elif a==get_size_addr: pass
  elif a in redraw_addrs:redraws.append({'entry':redraw_addrs[a],'drawable':hex(p.x(0)),'stroke':hex(p.x(1))})
  elif a in add_addrs:
   _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(6)]
   _receiver=p.x(0)
   add_calls.append({'entry':add_addrs[a],'receiver':hex(_receiver),'receiver_vptr':hex(p.get(_receiver,'Q')[0]),'float_words_s0_s5':_words,'native_addpoint_channels':{'position_xyz_f32_bits':_words[0:3],'direction_xy_f32_bits':_words[3:5],'transparency_f32_bits':_words[5]}})
   addpoint_pending[u.reg_read(R.UC_ARM64_REG_LR)]=p.x(0)
   _rt=p.x(0)
   try: _v80=p.get(_rt+80,'Q')[0]; _vec80=[hex(v) for v in p.get(_v80,'3Q')] if _v80 else None
   except Exception as _e: _v80=0; _vec80=str(_e)
   rt_addpoint_states.append({'receiver':hex(_rt),'v80':hex(_v80) if _v80 else None,'vector80':_vec80,'raw_80_112':bytes(u.mem_read(_rt+80,32)).hex()})
 trace_hook=p.u.hook_add(UC_HOOK_CODE,trace)
 def _addpoint_return_trace(u,a,z,d):
  if a in addpoint_pending:
   _rt=addpoint_pending.pop(a); _v=p.get(_rt+80,'Q')[0]
   try:
    _b,_e,_c=p.get(_v,'3Q'); _data=bytes(u.mem_read(_b,_e-_b)) if _e>_b else b''
    addpoint_return_states.append({'receiver':hex(_rt),'vector_object':hex(_v),'bytes':_e-_b,'sha256':hashlib.sha256(_data).hexdigest(),'tail_f32_bits':[f'{x:08x}' for x in struct.unpack('<7I',_data[-28:])] if len(_data)>=28 else []})
   except Exception as _e: addpoint_return_states.append({'receiver':hex(_rt),'vector_object':hex(_v) if _v else None,'error':str(_e)})
 addpoint_return_hook=p.u.hook_add(UC_HOOK_CODE,_addpoint_return_trace)
 set_buffer_calls=[]
 set_buffer_addr=p.syms['SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)']
 def _set_buffer_trace(u,a,z,d):
  if a==set_buffer_addr:
   _vec=lambda _ptr: {'ptr':hex(_ptr) if _ptr else None,'slots':[hex(v) for v in p.get(_ptr,'3Q')] if _ptr else None}
   set_buffer_calls.append({'receiver':hex(p.x(0)),'vector_one':_vec(p.x(1)),'vector_two':_vec(p.x(2))})
 set_buffer_hook=p.u.hook_add(UC_HOOK_CODE,_set_buffer_trace)
 send_data_to_gpu_addr=p.syms['SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)']
 set_buffer_pending={}
 redraw_return_states=[]
 set_buffer_return_states=[]
 send_data_calls=[]
 def _lifecycle_trace(u,a,z,d):
  if a==set_buffer_addr:
   set_buffer_pending[u.reg_read(R.UC_ARM64_REG_LR)]=p.x(0)
  elif a in set_buffer_pending:
   _rt=set_buffer_pending.pop(a)
   _slot=lambda _off: p.get(_rt+_off,'Q')[0]
   def _vec(_ptr):
    if not _ptr: return {'ptr':None,'slots':None}
    try:
     _begin,_end,_cap=p.get(_ptr,'3Q')
     return {'ptr':hex(_ptr),'slots':[hex(_begin),hex(_end),hex(_cap)],'bytes':_end-_begin}
    except Exception as _e: return {'ptr':hex(_ptr),'slots':None,'read_error':str(_e)}
   set_buffer_return_states.append({'receiver':hex(_rt),'vector_80':_vec(_slot(80)),'vector_88':_vec(_slot(88)),'vector_96':_vec(_slot(96)),'fields_264_280':bytes(u.mem_read(_rt+264,16)).hex()})
  if a==send_data_to_gpu_addr:
   send_data_calls.append({'receiver':hex(p.x(0)),'arg1':hex(p.x(1)) if p.x(1) else None,'arg2':hex(p.x(2)) if p.x(2) else None})
 lifecycle_hook=p.u.hook_add(UC_HOOK_CODE,_lifecycle_trace)
 def _addpoint_return_trace(u,a,z,d):
  if a in addpoint_pending:
   _rt=addpoint_pending.pop(a); _v=p.get(_rt+80,'Q')[0]
   try:
    _b,_e,_c=p.get(_v,'3Q'); _data=bytes(u.mem_read(_b,_e-_b)) if _e>_b else b''
    addpoint_return_states.append({'receiver':hex(_rt),'vector_object':hex(_v),'bytes':_e-_b,'sha256':hashlib.sha256(_data).hexdigest(),'tail_f32_bits':[f'{x:08x}' for x in struct.unpack('<7I',_data[-28:])] if len(_data)>=28 else []})
   except Exception as _e: addpoint_return_states.append({'receiver':hex(_rt),'vector_object':hex(_v) if _v else None,'error':str(_e)})
 addpoint_return_hook=p.u.hook_add(UC_HOOK_CODE,_addpoint_return_trace)
 _upload_checkpoint_addr=rt.bases['PenCommon']+0x50480
 _upload_checkpoint={'hit':False,'calls':[]}
 def _preclear_snapshot(u,a,z,d):
  if a==_upload_checkpoint_addr and not _upload_checkpoint['hit']:
   _upload_checkpoint['hit']=True
   _rt=p.x(23);_vec=p.get(_rt+80,'Q')[0];_begin,_end,_cap=p.get(_vec,'3Q');_raw=bytes(u.mem_read(_begin,_end-_begin))
   _upload_checkpoint.update({'receiver':hex(_rt),'receiver_vptr':hex(p.get(_rt,'Q')[0]),'vector_object':hex(_vec),'vector_bytes':len(_raw),'vector_sha256':hashlib.sha256(_raw).hexdigest()})
 _upload_hook=p.u.hook_add(UC_HOOK_CODE,_preclear_snapshot)
 _draw_name='SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)'
 for _i,_value in enumerate((drawing,p.draw_canvas,p.pen_canvas,obj,1)):p.x(_i,_value)
 p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
 p.u.emu_start(p.syms[_draw_name],0x200f1000,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError(f'{_draw_name} failed PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x}')
 rc=p.x(0);p.u.hook_del(_upload_hook)
 # Capture the native queued render while its native SPPenCanvas bitmap/FBO
 # remains bound. The task uses the selected callback canvas, not an RT-slot
 # patch; record both framebuffer bindings and attachment identity.
 _adapter.gles.gl.glFinish()
 _draw_fbo=C.c_int();_read_fbo=C.c_int();_attachment_type=C.c_int(-1);_attachment_name=C.c_int(-1)
 _getint=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(_adapter.gles.gl.glGetIntegerv)
 _getatt=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int))(_adapter.gles.gl.glGetFramebufferAttachmentParameteriv)
 _getint(0x8CA6,C.byref(_draw_fbo));_getint(0x8CAA,C.byref(_read_fbo))
 _getatt(0x8D40,0x8CE0,0x8CD0,C.byref(_attachment_type));_getatt(0x8D40,0x8CE0,0x8CD1,C.byref(_attachment_name))
 _rgba=C.create_string_buffer(_input_w*_input_h*4)
 _readpixels=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(_adapter.gles.gl.glReadPixels)
 _gl_error_before=_adapter.gles.f['glGetError']();_readpixels(0,0,_input_w,_input_h,0x1908,0x1401,_rgba);_gl_error_after=_adapter.gles.f['glGetError']()
 _rgba_bytes=_rgba.raw;_nonzero_alpha=[(i%_input_w,i//_input_w) for i in range(_input_w*_input_h) if _rgba_bytes[i*4+3]]
 call('SPen::SPPenCanvas::GetBitmap()' ,[_native_pen_canvas]);_bitmap_after_queue=p.x(0)
 _hf02_readback={'width':_input_w,'height':_input_h,'bytes':len(_rgba_bytes),'rgba_sha256':hashlib.sha256(_rgba_bytes).hexdigest(),'rgba8_zlib_base64':__import__('base64').b64encode(__import__('zlib').compress(_rgba_bytes,9)).decode('ascii'),'alpha_nonzero_pixels':len(_nonzero_alpha),'alpha_nonzero_bbox':[min(x for x,y in _nonzero_alpha),min(y for x,y in _nonzero_alpha),max(x for x,y in _nonzero_alpha),max(y for x,y in _nonzero_alpha)] if _nonzero_alpha else None,'draw_framebuffer_binding':_draw_fbo.value,'read_framebuffer_binding':_read_fbo.value,'read_pname':0x8CAA,'draw_pname':0x8CA6,'read_attachment_type':_attachment_type.value,'read_attachment_name':_attachment_name.value,'native_callback_canvas':hex(_native_pen_canvas),'native_callback_bitmap':hex(_callback_bitmap),'native_getbitmap_after_queue':hex(_bitmap_after_queue),'canvas_bitmap_matches':_bitmap_after_queue==_callback_bitmap,'queue_managed_events':_managed_events,'gl_error_before_readpixels':_gl_error_before,'gl_error_after_readpixels':_gl_error_after}
 if _bitmap_after_queue!=_callback_bitmap:raise RuntimeError('native SPPenCanvas::GetBitmap no longer identifies the callback bitmap')
 _canvas_texture_ids=[_texture_id for _generation in renderer_texture_generations if _generation['phase']=='CreateBitmap used as native callback canvas' for _texture_id in _generation['ids']]
 if _attachment_type.value!=0x1702 or _attachment_name.value not in _canvas_texture_ids:
  raise RuntimeError(f'native callback canvas framebuffer does not attach its generated texture: type={_attachment_type.value:#x}, attachment={_attachment_name.value}, generated={_canvas_texture_ids}')
 if _draw_fbo.value!=_read_fbo.value or _draw_fbo.value==0 or _gl_error_before or _gl_error_after or not _nonzero_alpha:
  raise RuntimeError(f'native queued render readback failed framebuffer/error/pixel checks: draw={_draw_fbo.value}, read={_read_fbo.value}, errors={_gl_error_before}/{_gl_error_after}, alpha={len(_nonzero_alpha)}')
 if len(queue_events)<17 or any(not _event.get('returned') for _event in queue_events):
  raise RuntimeError(f'native queue adapter did not return every captured task: {len(queue_events)} events')
 _callback_tasks=queue_events[-4:]
 if [int(_event['slot16_method'],16) for _event in _callback_tasks[:3]]!=[0x10506b4,0x104676c,0x1045e6c]:
  raise RuntimeError(f'callback closure execution order changed: {_callback_tasks}')
 _send_task=_callback_tasks[0];_render_task=_callback_tasks[1];_delete_task=_callback_tasks[2]
 _send_raw=bytes.fromhex(_send_task['pre_bytes_144']);_render_raw=bytes.fromhex(_render_task['pre_bytes_144']);_delete_raw=bytes.fromhex(_delete_task['pre_bytes_144'])
 if struct.unpack_from('<Q',_send_raw,16)[0]!=int(_upload_checkpoint['receiver'],16) or struct.unpack_from('<Q',_send_raw,24)[0]!=p.syms['SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)']:
  raise RuntimeError('captured native upload task does not target the actual RTV5 SendDataToGPU method')
 if struct.unpack_from('<Q',_send_raw,40)[0]!=int(_upload_checkpoint['vector_object'],16) or struct.unpack_from('<Q',_render_raw,32)[0]!=int(_upload_checkpoint['receiver'],16) or struct.unpack_from('<Q',_render_raw,40)[0]!=_native_pen_canvas or struct.unpack_from('<Q',_delete_raw,16)[0]!=int(_upload_checkpoint['vector_object'],16):
  raise RuntimeError('native callback task receiver/vector/canvas ownership differs from captured source objects')
 _callback_execution_order=[{'position':i+1,'task':_event['task'],'vtable':_event['vtable'],'slot16_method':_event['slot16_method'],'returned':_event['returned']} for i,_event in enumerate(_callback_tasks)]
 _hf02_rt=int(set_buffer_calls[0]['receiver'],16) if set_buffer_calls else 0
 _rt_vptr_checkpoints=[{'stage':'after_natural_queue_return','receiver':hex(_hf02_rt),'vptr':hex(p.get(_hf02_rt,'Q')[0]) if _hf02_rt else None}]
 _hf02_init_result=None;_hf02_init_error='Init/Draw are not separate host calls; the captured native PenGLRenderMsg task performs the paint path';_hf02_init_fail_pc=None;_hf02_init_import=None
 _hf02_canvas_result=None;_hf02_canvas_error=None;_hf02_draw_result=None;_hf02_draw_error=None
 # Capture returned PenData/current Pen and its actual generic size.
 manager=p.get(drawing+16,'Q')[0]
 # Find any native Pen::SetSize target from calls; caller object is recovered from logged receiver.
 selected_pen=int(pen_set_calls[-1]['this'],16) if pen_set_calls else None
 size_bits=None
 if selected_pen:
  call('SPen::Pen::GetSize()',[selected_pen]);size_bits=struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex()

 try: drawable=call('SPen::FountainPen::GetStrokeDrawableGL()',[selected_pen]) if selected_pen else 0
 except Exception as e: drawable_error=str(e); drawable=0
 else: drawable_error=None
 vptr=p.get(drawable,'Q')[0] if drawable else 0
 p.u.hook_del(trace_hook);p.u.hook_del(set_buffer_hook);p.u.hook_del(lifecycle_hook)
 records.append({'case':label,'advanced_setting':advanced_value,'style_pen_size_setter':mode,'source_model_get_pen_size_f32_bits':size_s0,'source_model_get_point_count':model_count,'drawing_return':rc,'native_pen_setsize_calls':pen_set_calls,'selected_pen':hex(selected_pen) if selected_pen else None,'native_pen_getsize_f32_bits':size_bits,'selected_drawable':hex(drawable) if drawable else None,'drawable_error':drawable_error,'drawable_vtable':hex(vptr) if vptr else None,'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,'queue_task_effects':queue_events,'callback_execution_order':_callback_execution_order,'send_data_to_gpu_calls':send_data_calls,'native_upload_checkpoint':_upload_checkpoint,'renderer_texture_generations':renderer_texture_generations,'native_callback_canvas_setup':{'canvas':hex(_native_pen_canvas),'vptr':hex(_native_canvas_vptr),'impl':hex(_native_impl),'bitmap':hex(_callback_bitmap),'bitmap_from_impl':hex(_native_canvas_bitmap),'get_msg_queue_vslot_184':hex(p.get(_native_canvas_vptr+184,'Q')[0]),'get_bitmap_vslot_192':hex(p.get(_native_canvas_vptr+192,'Q')[0])},'renderer_rt_vptr_checkpoints':_rt_vptr_checkpoints,'renderer_rt_destructor_entries':rt_dtor_entries,'renderer_readback':_hf02_readback,'renderer_vertex_events':_renderer_vertex_events,'renderer_shader_source_calls':_adapter.shader_source_calls,'loader_calls':list(loader),'drawing_manager':hex(manager) if manager else None,'source_arrays':{'points_f32_bits':[f'{v:08x}' for v in struct.unpack('<'+str(model_count*2)+'I',bytes(p.u.mem_read(model_arrays['points'],model_count*8)))],'pressure_f32_bits':[f'{v:08x}' for v in struct.unpack('<'+str(model_count)+'I',bytes(p.u.mem_read(model_arrays['pressure'],model_count*4)))],'timestamps_i32':list(struct.unpack('<'+str(model_count)+'i',bytes(p.u.mem_read(model_arrays['timestamps'],model_count*4)))),'tilt_f32_bits':[f'{v:08x}' for v in struct.unpack('<'+str(model_count)+'I',bytes(p.u.mem_read(model_arrays['tilt'],model_count*4)))],'orientation_f32_bits':[f'{v:08x}' for v in struct.unpack('<'+str(model_count)+'I',bytes(p.u.mem_read(model_arrays['orientation'],model_count*4))) ]}})
for _row in records:
 _row['selected_drawable_class']='GLV16'
 _row['hf02_native_state']=_serialized_native_state
 _row['hf02_visible']=_native_visible
 _row['renderer_init_result']=_hf02_init_result
 _row['renderer_init_error']=_hf02_init_error
 _row['renderer_init_fail_pc']=_hf02_init_fail_pc
 _row['renderer_init_unresolved_plt']=_hf02_init_import
 _row['renderer_canvas_result']=_hf02_canvas_result
 _row['renderer_canvas_error']=_hf02_canvas_error
 _row['renderer_draw_result']=_hf02_draw_result
 _row['renderer_draw_error']=_hf02_draw_error
 _row['renderer_rt_slots_after_draw']={str(o):hex(p.get(int(_row['rt_receiver'],16)+o,'Q')[0]) for o in (80,88,96,200,208,272)} if _row.get('rt_receiver') else None
 _rt=int(_row['rtv_addpoint_entries'][0]['receiver'],16)
 _row['renderer_receiver']=hex(_rt)
 _row['renderer_rt_slots_after_draw']={str(o):hex(p.get(_rt+o,'Q')[0]) for o in (80,88,96,200,208)}
 _row['renderer_instance_count_u32_at_plus272']=p.get(_rt+272,'I')[0]
 _row['renderer_gl_call_counts']={name:sum(1 for event in _adapter.gl_calls if event.get('call')==name) for name in sorted({event.get('call','<unnamed>') for event in _adapter.gl_calls})}
 _row['renderer_gl_draw_events']=[event for event in _adapter.gl_calls if event.get('call') in ('glDrawArrays','glDrawArraysInstanced')]
 _row['renderer_gl_calls']=[event for event in _adapter.gl_calls if event.get('call') in ('glBufferData','glUseProgram','glUniformMatrix4fv','glUniform4f','glUniform4fv','glUniform1i','glUniform1f','glBindBuffer','glVertexAttribPointer','glVertexAttribDivisor','glBindTexture','glActiveTexture','glBindSampler','glBindFramebuffer','glFramebufferTexture2D','glCheckFramebufferStatus','glTexImage2D','glViewport','glDrawArrays','glDrawArraysInstanced','glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask','maskReadback')]
 _row['renderer_vertex_events']=_renderer_vertex_events
 _row['renderer_shader_source_calls']=_adapter.shader_source_calls
 _row['renderer_shader_link_events']=_renderer_shader_link_events
 _row['renderer_gl_error_checks']=renderer_gl_error_checks
 _instance_uploads=[_event for _event in _row['renderer_gl_calls'] if _event.get('call')=='glBufferData' and _event.get('size')==2128]
 if len(_instance_uploads)!=1 or _instance_uploads[0].get('dataSha256')!='3829df249606e8a8c360780d91929ef21dda075a0c379a6c38b09f8caa9fa56a' or _instance_uploads[0].get('dataSha256')!=_upload_checkpoint['vector_sha256']:
  raise RuntimeError(f'native Renderer upload is not the captured 76-record source vector: {_instance_uploads}')
 if _row['renderer_readback']['bytes']!=1920*1080*4 or _row['renderer_readback']['read_framebuffer_binding']!=_row['renderer_readback']['draw_framebuffer_binding'] or _row['renderer_readback']['gl_error_before_readpixels'] or _row['renderer_readback']['gl_error_after_readpixels']:
  raise RuntimeError('full native callback-canvas RGBA readback/FBO/error checks failed')
 _row['evidence_scope']='Native HF02 ObjectStroke apply/Drawing selects GLV16 and records 76 native RTV5 AddPoint outputs. A synchronous host queue adapter dispatches the captured native task vtables in submission order: native SendDataToGPU receives the live vector, then native PenGLRenderMsg paints to the native SPPenCanvas bitmap, followed by native vector deletion. Native resource construction and GLES callbacks are host adapters; the 1920x1080 RGBA readback is host Mesa output, not Android scheduling, device pixels, or public Notes playback.'
 if _row['drawing_return']!=1 or len(_row['rtv_addpoint_entries'])!=76: raise RuntimeError('HF02 native Drawing/V16 AddPoint capture mismatch')
# Source and library pins were checked before runtime setup; include the stable
# input vectors and the selected native class plus every AddPoint FP lane.
import unicorn
value={'schema':'sdocx-fountain-saved-render-native-v1','provenance':{'runner_sha256':SELF_SHA256,'source_fixture_sha256':SOURCE_FIXTURE_SHA256,'source_archive_sha256':SOURCE_ARCHIVE_SHA256,'helpers':HELPER_SHA256,'libraries':LIBRARY_SHA256,'analysis_tools':{_tool:hashlib.sha256((ROOT/_tool if not _tool.startswith('/') else Path(_tool)).read_bytes()).hexdigest() for _tool in TOOL_SHA256},'host_gl_library_sha256':HOST_GL_SHA256,'runtime_versions':{'python':sys.version.split()[0],'python_executable':str(Path(sys.executable).resolve()),'python_executable_sha256':hashlib.sha256(Path(sys.executable).resolve().read_bytes()).hexdigest(),'unicorn':getattr(unicorn,'__version__','unknown'),'unicorn_files':UNICORN_FILE_SHA256},'renderer_dso_sha256':hashlib.sha256(_renderer_raw).hexdigest(),'host_adapter_source':'conformance/marker3_paint_scale_native.py class P, re-bound to the same HF02 Unicorn Uc and using caller p.alloc','runner_boundary':'Actual HF02 ObjectStroke Model apply and Drawing route, selected GLV16, 76 native RTV5 AddPoint outputs. A synchronous host queue adapter dispatches the captured native task vtables in submission order. The native SendDataToGPU task receives the live vector, then the native PenGLRenderMsg task paints to a natively constructed SPPenCanvas/Bitmap. Renderer GLES callbacks, native resource construction, and queue scheduling are host adapters.'},'host_gles':HOST_GLES_INFO,'profile':[1920,1080],'source_input':{'kind':'HF02 actual persisted FountainPen frame','archive_sha256':'dd6580a0531cf1712ef6526f1b0d51c42adaeb0d11feacca5d23ba562239da82','frame_sha256':'490fa32ab9d69416e4c48dec88b26ad14e362d7b08eb3e1b6a9ada47ce2980c2','frame_bytes':2514,'payload_sha256':'c01022129e9a3a09ee95dc5935d5e177bd507e03d6d9dbd357a5278a64ab72b4','table_sha256':'f35b044485b8a91fd825ac682c734b23c565e52fbfb869fc3d4d3e89b414035e','string_table_values':['18;0;100;','com.samsung.android.sdk.pen.pen.preload.FountainPen'],'point_count':204,'page_orientation':0},'cases':records}
value=json.loads(json.dumps(value))
if MODE_ARGS.write_fixture:
 FIXTURE.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
 print(f'wrote {FIXTURE.relative_to(ROOT)} ({len(records)} cases)')
else:
 _existing=json.loads(FIXTURE.read_text())
 if _existing!=value:
  def _first_diff(a,b,path=''):
   if type(a)!=type(b):return (path,type(a).__name__,type(b).__name__)
   if isinstance(a,dict):
    for _key in sorted(a.keys()|b.keys()):
     if _key not in a or _key not in b:return (path+'/'+_key,'missing','present')
     if a[_key]!=b[_key]:
      _diff=_first_diff(a[_key],b[_key],path+'/'+_key)
      if _diff:return _diff
   elif isinstance(a,list):
    if len(a)!=len(b):return (path,'length',len(a),len(b))
    for _i,(_left,_right) in enumerate(zip(a,b)):
     if _left!=_right:
      _diff=_first_diff(_left,_right,path+'/'+str(_i))
      if _diff:return _diff
   else:return (path,repr(a)[:120],repr(b)[:120])
  raise SystemExit(f'native Fountain Drawing fixture mismatch: {_first_diff(_existing,value)}')
 print(f'verified {len(records)} Fountain Drawing cases')
