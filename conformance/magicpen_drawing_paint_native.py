#!/usr/bin/env python3
"""Hash-gated MagicPen adaptation of the canonical saved-Drawing caller.

This runner does not carry a copied Drawing/Model harness. It verifies the
canonical Fountain caller's exact base hash, applies asserted source patches
in memory, and executes that adapted caller in the same pinned runtime.
"""
import argparse, base64, hashlib, json, os, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/fountain_saved_caller_native.py"
FIXTURE = ROOT / "conformance/magicpen-drawing-paint-native.json"
LIB = ROOT / "scratch/apk-analysis-native/arm64-v8a"
HELPER_HASHES = {
    'conformance/fountain_saved_caller_native.py':'b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19',
    'conformance/marker3_paint_scale_native.py':'9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
    'conformance/airbrush_texture_native.py':'76bb130daef795be53296cff4f778be6e7be5f487c8b1d4c4b6d159bb688e470',
    'conformance/airbrush_saved_native.py':'cad2bd9ec88a393f0b0b4a7dbe58c7b469f9b00b283d4c544f491a8ab190999a',
    'conformance/inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
    'conformance/fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
    'conformance/oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
    'conformance/native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91',
    'conformance/elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
}
DSO_HASHES = {
    'libSPenMagicPen.so':'e4927da583e96142d072bb3edd977334aa34a58bb723ab1f9aaf531f3bd670a2',
    'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
    'libSPenObliquePen.so':'9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee',
    'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'libSPenDefaultPen.so':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f',
    'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
    'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
    'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
    'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
    'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
    'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
    'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
TOOL_HASHES = {
    'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump':'b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb',
    '/usr/bin/nm':'ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1',
    '/usr/bin/c++filt':'7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2',
    '/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6',
    '/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812',
}
def sha256(raw): return hashlib.sha256(raw).hexdigest()
def check_hash(path, expected):
    actual = sha256(Path(path).read_bytes())
    if actual != expected: raise SystemExit(f"pinned input changed: {path}: {actual}")

# Verify every source, DSO, native tool and host graphics library before
# importing Unicorn, paint helpers, or creating the EGL context.
for rel, expected in HELPER_HASHES.items(): check_hash(ROOT / rel, expected)
for rel, expected in DSO_HASHES.items(): check_hash(LIB / rel, expected)
for path, expected in TOOL_HASHES.items(): check_hash(ROOT / path if not path.startswith('/') else path, expected)

cli = argparse.ArgumentParser(description="Capture/check a bounded native MagicPen Drawing-to-Mesa paint join")
mode = cli.add_mutually_exclusive_group()
mode.add_argument('--write-fixture', action='store_true', help='explicitly write the captured native/paint fixture')
mode.add_argument('--check-fixture', action='store_true', help='replay and compare the frozen fixture')
cli.add_argument('--_capture-background', choices=('transparent','opaque'), help=argparse.SUPPRESS)
CLI_ARGS = cli.parse_args()
EXPECTED_FIXTURE_SHA256='874f659069c8ab206c049f4e7c7161524146f55a93c44fd4ac378bba6e5c9bc2'
if CLI_ARGS._capture_background is None and not CLI_ARGS.write_fixture:
    check_hash(FIXTURE,EXPECTED_FIXTURE_SHA256)

if CLI_ARGS._capture_background is None:
    child_env=os.environ.copy()
    py_paths=[str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
    child_env['PYTHONPATH']=os.pathsep.join(py_paths+[child_env.get('PYTHONPATH','')]).rstrip(os.pathsep)
    child_env['LD_LIBRARY_PATH']=os.pathsep.join([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'),child_env.get('LD_LIBRARY_PATH','')]).rstrip(os.pathsep)
    child_runs=[]
    for bg in ('transparent','opaque'):
        proc=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--_capture-background',bg],cwd=ROOT,env=child_env,text=True,capture_output=True)
        if proc.returncode:
            raise SystemExit(f'{bg} MagicPen paint replay failed ({proc.returncode}):\n{proc.stdout}{proc.stderr}')
        try: child_runs.append(json.loads(proc.stdout))
        except Exception as exc: raise SystemExit(f'{bg} paint child did not emit a single JSON report: {exc}\n{proc.stdout}')
    if child_runs[0]['source_capture']!=child_runs[1]['source_capture']:
        raise SystemExit('transparent and opaque paint controls did not share identical native Drawing captures')
    fixture_value={'schema':'magicpen-drawing-paint-native-v1','source_capture':child_runs[0]['source_capture'],'paint_witnesses':[x['paint'] for x in child_runs]}
    encoded=(json.dumps(fixture_value,indent=2,sort_keys=True)+'\n').encode()
    if CLI_ARGS.write_fixture:
        FIXTURE.write_bytes(encoded)
        print(f'wrote {FIXTURE.relative_to(ROOT)} sha256={sha256(encoded)}')
    else:
        if not FIXTURE.exists() or FIXTURE.read_bytes()!=encoded:
            raise SystemExit(f'fresh MagicPen Drawing/paint capture differs from {FIXTURE.relative_to(ROOT)}; use --write-fixture explicitly to refresh')
        print(f'verified {FIXTURE.relative_to(ROOT)} sha256={sha256(encoded)}')
    raise SystemExit(0)

EXPECTED_BASE_SHA256 = "b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19"
BASE_SHA256 = hashlib.sha256(BASE.read_bytes()).hexdigest()
if BASE_SHA256 != EXPECTED_BASE_SHA256:
    raise SystemExit(f"canonical Fountain caller changed: {BASE_SHA256}")

source = BASE.read_text()

def replace_once(old: str, new: str, label: str) -> None:
    global source
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"patch {label!r}: expected one exact anchor, found {count}")
    source = source.replace(old, new, 1)

def replace_region(start: str, end: str, replacement: str, label: str) -> None:
    global source
    if source.count(start) != 1 or source.count(end) != 1:
        raise SystemExit(f"patch region {label!r}: anchor count changed")
    begin = source.index(start)
    finish = source.index(end)
    if begin >= finish:
        raise SystemExit(f"patch region {label!r}: anchors reversed")
    source = source[:begin] + replacement + source[finish:]

replace_once(
    "FIXTURE=ROOT/'conformance/fountain-saved-caller-native.json'",
    "FIXTURE=ROOT/'conformance/magicpen-drawing-paint-native.json'",
    "durable fixture path",
)
replace_once(
    "LIBRARY_SHA256={'FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}",
    "LIBRARY_SHA256={'MagicPen':'e4927da583e96142d072bb3edd977334aa34a58bb723ab1f9aaf531f3bd670a2','FountainPen':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec','ObliquePen':'9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee','PenCommon':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d','DefaultPen':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f','Drawing':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd','Model':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a','Base':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb','Graphics':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621','Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed','Composer':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f','libc++_shared':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'}",
    "MagicPen DSO and Renderer preflight",
)

replace_once(
    "# Explicitly initialize the mapped FountainPen plugin and its static factory data.\nFP=LIBS/'libSPenFountainPen.so'; fpbase=rt.bases['FountainPen']; assert hashlib.sha256(FP.read_bytes()).hexdigest()=='1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'\nrt.run_init(FP,fpbase)",
    """# Map/init the pinned MagicPen DSO at a fresh address, using the runtime's real libc++ and symbol tables.
MP=LIBS/'libSPenMagicPen.so'; mpbase=0xA000000
def map_plugin(path,base):
 raw=path.read_bytes(); phoff=struct.unpack_from('<Q',raw,32)[0]; ent,num=struct.unpack_from('<HH',raw,54); extent=0
 for i in range(num):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
  if typ==1:extent=max(extent,va+ms)
 p.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(num):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',raw,phoff+i*ent)
  if typ==1 and ms:
   p.u.mem_write(base+va,raw[off:off+fs])
   if ms>fs:p.u.mem_write(base+va+fs,bytes(ms-fs))
 for line in __import__('subprocess').check_output(['nm','-D','-C',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split(' ',2)
  if len(parts)==3 and parts[1] in ('T','W'):p.syms[parts[2]]=base+int(parts[0],16)
 for line in __import__('subprocess').check_output(['nm','-D',str(path)],text=True,env=env).splitlines():
  parts=line.strip().split()
  if len(parts)==3 and parts[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(parts[2],base+int(parts[0],16))
 shoff=struct.unpack_from('<Q',raw,40)[0]; se,sn,ss=struct.unpack_from('<HHH',raw,58); secs=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*se) for i in range(sn)]; names=raw[secs[ss][4]:secs[ss][4]+secs[ss][5]]; by={names[s[0]:names.find(b'\\0',s[0])].decode():s for s in secs}; ds,st=by['.dynsym'],by['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; sy=[]
 for off in range(ds[4],ds[4]+ds[5],ds[9]):
  no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',raw,off); sy.append((strings[no:strings.find(b'\\0',no)].decode() if no else '',val))
 for secname in ('.rela.dyn','.rela.plt'):
  sec=by.get(secname)
  if not sec:continue
  for off in range(sec[4],sec[4]+sec[5],sec[9]):
   where,info,add=struct.unpack_from('<QQq',raw,off); typ,idx=info&0xffffffff,info>>32
   if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
   elif typ in (0x401,0x402,0x101,0x1025,0x1026):
    name,val=sy[idx]; target=base+val if val else rt.rawsyms.get(name,0) or p.syms.get(name,0)
    if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
 for off,sym in __import__('elf_plt').resolve(path).items():
  pretty=__import__('subprocess').check_output(['c++filt','-n',sym],text=True).strip() if sym.startswith('_Z') else sym
  p.plt[base+off]=pretty
 rt.run_init(path,base)
 return extent
map_plugin(MP,mpbase)
# This is a direct NEEDED dependency and is hash-pinned above, but the bounded
# ObjectStroke geometry route does not map/execute Renderer.
""",
    "MagicPen loader/init block",
)

replace_once(
    "pen_set_calls=[]; add_calls=[]; redraws=[]",
    "pen_set_calls=[]; add_calls=[]; redraws=[]; rt_buffer_snapshots=[]; pending_add_receivers=[]",
    "RTV1 return snapshots",
)
replace_once(
    " def trace(u,a,z,d):\n  if a==set_size_addr:",
    """ def trace(u,a,z,d):
  if a==mpbase+0x2f530:
   if not pending_add_receivers:raise RuntimeError('RTV1 AddPoint return without entry receiver')
   receiver=pending_add_receivers.pop(); vecptr=p.get(receiver+80,'Q')[0]
   if not vecptr:raise RuntimeError(f'RTV1 vector pointer null at AddPoint return for {receiver:#x}')
   begin,end,capacity=p.get(vecptr,'QQQ')
   if not(begin<=end<=capacity) or (end-begin)%4:raise RuntimeError(f'invalid native MagicPen RTV1 vector {begin:#x}/{end:#x}/{capacity:#x}')
   raw=bytes(u.mem_read(begin,end-begin)) if end>begin else b''
   rt_buffer_snapshots.append({'receiver':hex(receiver),'vector_object':hex(vecptr),'begin_end_capacity':[hex(begin),hex(end),hex(capacity)],'float_count':(end-begin)//4,'bytes_le_hex':raw.hex(),'sha256':hashlib.sha256(raw).hexdigest()})
  elif a==set_size_addr:""",
    "AddPoint epilogue snapshot hook",
)
replace_once(
    "  elif a in add_addrs:\n   _words=",
    "  elif a in add_addrs:\n   pending_add_receivers.append(p.x(0))\n   _words=",
    "AddPoint entry receiver stack",
)
replace_once(
    " p.u.hook_del(trace_hook)\n records.append(",
    " if pending_add_receivers:raise RuntimeError(f'unmatched native AddPoint entries: {len(pending_add_receivers)}')\n p.u.hook_del(trace_hook)\n native_rtv1_vector=rt_buffer_snapshots[-1] if rt_buffer_snapshots else None\n records.append(",
    "require complete AddPoint returns",
)
replace_once(
    "'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,",
    "'redraw_entries':redraws,'rtv_addpoint_entries':add_calls,'native_rtv1_vector':native_rtv1_vector,'native_rtv1_vector_snapshots':rt_buffer_snapshots,'native_model_metadata':{'tool_type':model_tool,'stroke_type':model_stroke,'curve_enabled':model_curve,'color_argb':model_color},",
    "native post-return buffer fields",
)

replace_once(
    "# Module-loader host adapter resolves only the already-mapped, pinned FountainPen plugin.\ndef cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\\0',1)[0].decode('ascii','replace')\nloader=[]\nFP_HANDLE=0x50000001; DP_HANDLE=0x90000001\ndef dlopen():\n name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})\n if Path(name).name=='libSPenFountainPen.so':p.x(0,FP_HANDLE)\n elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)\n else:raise RuntimeError('unexpected DSO '+name)\ndef dlsym():\n handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})\n if name!='createPenInst' or handle not in (FP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)\n plugin='libSPenFountainPen.so' if handle==FP_HANDLE else 'libSPenDefaultPen.so'\n raw=FP.read_bytes() if handle==FP_HANDLE else DP.read_bytes()\n nm=__import__('subprocess').check_output(['nm','-D',str(FP if handle==FP_HANDLE else DP)],text=True,env=env)\n offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]\n if not offsets:raise RuntimeError(plugin+' lacks createPenInst')\n p.x(0,(fpbase if handle==FP_HANDLE else dpbase)+offsets[0])\np.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)",
    """# Loader addresses only the already mapped, hash-pinned plugins.
def cstr(a):return bytes(p.u.mem_read(a,128)).split(b'\\0',1)[0].decode('ascii','replace')
loader=[]
MP_HANDLE=0x50000001; DP_HANDLE=0x90000001
def dlopen():
 name=cstr(p.x(0));loader.append({'fn':'dlopen','name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if Path(name).name=='libSPenMagicPen.so':p.x(0,MP_HANDLE)
 elif Path(name).name=='libSPenDefaultPen.so':p.x(0,DP_HANDLE)
 else:raise RuntimeError('unexpected DSO '+name)
def dlsym():
 handle=p.x(0); name=cstr(p.x(1));loader.append({'fn':'dlsym','handle':hex(handle),'name':name,'lr':hex(p.u.reg_read(R.UC_ARM64_REG_LR))})
 if name!='createPenInst' or handle not in (MP_HANDLE,DP_HANDLE):raise RuntimeError('unexpected dlsym '+hex(handle)+' '+name)
 path=MP if handle==MP_HANDLE else DP
 base=mpbase if handle==MP_HANDLE else dpbase
 nm=__import__('subprocess').check_output(['nm','-D',str(path)],text=True,env=env)
 offsets=[int(line.split()[0],16) for line in nm.splitlines() if line.split()[-1:] == ['createPenInst']]
 if len(offsets)!=1:raise RuntimeError(str(path)+' createPenInst symbol count '+str(len(offsets)))
 p.x(0,base+offsets[0])
p.callbacks['dlopen']=dlopen;p.callbacks['dlsym']=dlsym;p.callbacks['dlerror']=lambda:p.x(0,0)""",
    "MagicPen dynamic loader",
)

replace_once(
    "for label,mode,size,advanced_value in [(f'{v14}_{case}', mode, size, '14;') for v14 in ('v14',) for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]] + [(f'v16_{case}',mode,size,'18;0;100;') for case,mode,size in [('omitted','omit',None),('zero','set',0.),('small','set',.1),('normal','set',10.)]]:\n loader.clear()",
    "for label,advanced_value,advanced_mode in [('unset',None,'omitted'),('null-arg',None,'null'),('empty','','value'),('v1','1;','value'),('v2','2;','value')]:\n mode='set'; size=10.0\n loader.clear()",
    "five advanced-setting controls",
)
replace_once(
    "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.FountainPen')])",
    "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.MagicPen')])",
    "MagicPen ObjectStroke name",
)
replace_once(
    "call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)",
    "call('SPen::ObjectStroke::GetPointCount() const',[obj]); model_count=p.x(0)\n call('SPen::ObjectStroke::GetToolType() const',[obj]); model_tool=p.x(0)\n call('SPen::ObjectStroke::GetStrokeType() const',[obj]); model_stroke=p.x(0)\n call('SPen::ObjectStroke::IsCurveEnabled() const',[obj]); model_curve=bool(p.x(0))\n call('SPen::ObjectStroke::GetColor() const',[obj]); model_color=f'0x{p.x(0)&0xffffffff:08x}'",
    "MagicPen source metadata getters",
)
replace_once(
    "# Native selection contract V14 is exercised with the actual saved advanced setting.\n call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])",
    """# Exercise setter omitted, null pointer, empty String, and explicit version values.
 if advanced_mode=='null':call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,0])
 elif advanced_mode=='value':call('SPen::ObjectStroke::SetAdvancedPenSetting(SPen::String const*)',[obj,string(advanced_value)])
 def read_native_string(ptr):
  if not ptr:return None
  call('SPen::String::GetUTF8Size() const',[ptr]); n=p.x(0)
  if n<0 or n>4096:raise RuntimeError(f'invalid native string size {n}')
  buf=p.alloc(n+1);call('SPen::String::GetUTF8(char*, int) const',[ptr,buf,n+1])
  return bytes(p.u.mem_read(buf,n+1)).split(b'\\0',1)[0].decode('ascii','replace')
 call('SPen::ObjectStroke::GetAdvancedPenSetting() const',[obj]); advanced_ptr=p.x(0); advanced_native=read_native_string(advanced_ptr)""",
    "MagicPen advanced-setting controls",
)

replace_once(
    "redraw_names=['SPen::FountainPenStrokeDrawableGLV14::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)','SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)','SPen::FountainPenStrokeDrawableRTV6::AddPoint(SPen::Vector3<float>, float, SPen::Vector3<float>)'];add_addrs={p.syms[name]:name for name in add_names}",
    "redraw_names=['SPen::MagicPenStrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)','SPen::MagicPenStrokeDrawableGLV2::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)']\n redraw_addrs={p.syms[name]:name for name in redraw_names}\n add_names=['SPen::MagicPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'];add_addrs={p.syms[name]:name for name in add_names}",
    "MagicPen trace symbols",
)
replace_once(
    "  _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(6)]\n   add_calls.append({'entry':add_addrs[a],'receiver':hex(p.x(0)),'float_words_s0_s5':_words,'native_addpoint_channels':{'position_xyz_f32_bits':_words[0:3],'direction_xy_f32_bits':_words[3:5],'transparency_f32_bits':_words[5]}})",
    "  _words=[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)]\n   add_calls.append({'entry':add_addrs[a],'entry_address':hex(a),'receiver':hex(p.x(0)),'float_words_s0_s2':_words,'native_addpoint_channels':{'position_xy_f32_bits':_words[0:2],'radius_f32_bits':_words[2]}})",
    "RTV1 float arguments",
)
replace_once(
    " try: drawable=call('SPen::FountainPen::GetStrokeDrawableGL()',[selected_pen]) if selected_pen else 0\n except Exception as e: drawable_error=str(e); drawable=0\n else: drawable_error=None",
    " if not selected_pen:raise RuntimeError('native ObjectDrawing did not select a MagicPen')\n drawable=call('SPen::MagicPen::GetStrokeDrawableGL()',[selected_pen])\n drawable_error=None",
    "MagicPen factory getter",
)
replace_once(
    "records.append({'case':label,'advanced_setting':advanced_value,",
    "records.append({'case':label,'advanced_setting':advanced_value,'advanced_mode':advanced_mode,'advanced_setting_getter':{'is_null':advanced_ptr==0,'utf8':advanced_native},",
    "capture settings metadata",
)

replace_region(
    "for _row in records:\n _versions=",
    "# Source and library pins were checked before runtime setup; include the stable",
    """for _row in records:
 _versions=[f'GLV{v}' for v in (1,2) if any(f'GLV{v}::RedrawPen' in call['entry'] for call in _row['redraw_entries'])]
 if len(_versions)!=1:raise RuntimeError(f'expected one MagicPen drawable class for {_row["case"]}: {_versions}')
 _row['selected_drawable_class']=_versions[0]
 _expected={'unset':'GLV1','null-arg':'GLV1','empty':'GLV2','v1':'GLV1','v2':'GLV2'}[_row['case']]
 if _versions[0]!=_expected:raise RuntimeError(f'wrong selected class for {_row["case"]}: {_versions[0]}')
 _expected_setting={'unset':(True,None),'null-arg':(True,None),'empty':(False,''),'v1':(False,'1;'),'v2':(False,'2;')}[_row['case']]
 if (_row['advanced_setting_getter']['is_null'],_row['advanced_setting_getter']['utf8'])!=_expected_setting:raise RuntimeError(f'advanced getter changed for {_row["case"]}')
 if _row['drawing_return']!=1 or len(_row['redraw_entries'])!=1:raise RuntimeError(f'MagicPen Drawing route did not run once for {_row["case"]}')
 if not any(item['fn']=='dlsym' and item['name']=='createPenInst' and item['handle']=='0x50000001' for item in _row['loader_calls']):raise RuntimeError('MagicPen factory not selected')
 if _row['source_model_get_point_count']!=3:raise RuntimeError('source ObjectStroke point count changed')
 if _row['source_arrays']!={'points_f32_bits':['00000000','00000000','41200000','00000000','41a00000','40000000'],'pressure_f32_bits':['3e99999a','3f19999a','3f800000'],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')
 if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':True,'color_argb':'0xff000000'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')
 if not _row['rtv_addpoint_entries'] or len({item['receiver'] for item in _row['rtv_addpoint_entries']})!=1:raise RuntimeError('RTV1 stream is empty or changed receiver')
 _words=''.join(word for row in _row['rtv_addpoint_entries'] for word in row['float_words_s0_s2'])
 _row['rtv_addpoint_argument_stream_hex_le']=_words
 _row['rtv_addpoint_argument_stream_sha256']=hashlib.sha256(bytes.fromhex(_words)).hexdigest()
 _row['rtv_addpoint_record_count']=len(_row['rtv_addpoint_entries'])
 if len(_row['native_rtv1_vector_snapshots'])!=len(_row['rtv_addpoint_entries']):raise RuntimeError('missing native AddPoint epilogue vector snapshots')
 if _row['native_rtv1_vector'] is None or _row['native_rtv1_vector']['float_count']!=3*_row['rtv_addpoint_record_count']:raise RuntimeError('native post-return RTV1 vector count mismatch')
 expected=bytearray()
 for row in _row['rtv_addpoint_entries']:
  words=row['float_words_s0_s2']; expected.extend(bytes.fromhex(words[0])); expected.extend(bytes.fromhex(words[1]))
  expected.extend(struct.pack('<f',struct.unpack('<f',bytes.fromhex(words[2]))[0]+0.5))
 if bytes(expected)!=bytes.fromhex(_row['native_rtv1_vector']['bytes_le_hex']):raise RuntimeError('native vector is not the AddPoint argument stream with native radius +0.5')
 _row['evidence_scope']='synthetic native ObjectStroke setter input; native Drawing selection and RedrawPen/AddPoint; no SDK deserialization or paint output'
""",
    "MagicPen bounded assertions",
)

replace_once(
    "'sdocx-fountain-saved-caller-v1'",
    "'scratch-magicpen-saved-caller-v1'",
    "scratch output schema",
)
replace_once(
    "'pen_name':'com.samsung.android.sdk.pen.pen.preload.FountainPen'",
    "'pen_name':'com.samsung.android.sdk.pen.pen.preload.MagicPen'",
    "MagicPen source metadata",
)
replace_once(
    "'runner_boundary':'Native Model ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, PenManager lookup, and selected FountainPen RedrawPen execute. V14 enters native RTV4 AddPoint; V16 evidence is limited to selected GLV16 class and size handoff, with no V16 geometry capture. IContext dimensions, queue dispatch, allocator, CanvasProbe drawing callbacks, and non-null synthetic dlopen/dlsym handles are host adapters; no raster output is claimed.'",
    "'runner_boundary':'Native Model ObjectStroke setters/getters, DrawingFactory, ObjectDrawing::drawObjectStroke, MagicPen factory, selected GLV1/GLV2 RedrawPen, and RTV1 AddPoint execute. IContext dimensions, queue dispatch, allocator, CanvasProbe callbacks, and dlopen/dlsym handles are host adapters. Renderer is a pinned direct dependency but is not mapped/executed. No serialized Note reader, texture binding, GPU draw, or raster output is claimed.'",
    "bounded evidence boundary",
)

# Distinguish actually mapped libraries from the ELF NEEDED inventory.
needle = "if MODE_ARGS.write_fixture:\n"
if source.count(needle) != 1:
    raise SystemExit("output-mode anchor count changed")
source = source.replace(
    needle,
    "value['provenance']['base_caller_path']='conformance/fountain_saved_caller_native.py'\n"
    "value['provenance']['base_caller_sha256']='b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19'\n"
    "value['provenance']['adapter_patch_labels']=['workspace root','scratch fixture path','MagicPen DSO and Renderer preflight','MagicPen loader/init block','RTV1 return snapshots','AddPoint epilogue snapshot hook','AddPoint entry receiver stack','require complete AddPoint returns','native post-return buffer fields','MagicPen dynamic loader','five advanced-setting controls','MagicPen ObjectStroke name','MagicPen source metadata getters','MagicPen advanced-setting controls','MagicPen trace symbols','RTV1 float arguments','MagicPen factory getter','capture settings metadata','MagicPen bounded assertions','scratch output schema','MagicPen source metadata','bounded evidence boundary']\n"
    "value['provenance']['mapped_dso_names']=['MagicPen','FountainPen','ObliquePen','PenCommon','Base','Drawing','Graphics','DefaultPen','Model','Composer','libc++_shared']\n"
    "value['provenance']['direct_needed_unmapped']={'Renderer':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'}\n"
    "value['provenance']['direct_needed_system']=['liblog.so','libm.so','libdl.so','libc.so']\n"
    + needle,
    1,
)

replace_once(
    "print(f'verified {len(records)} Fountain Drawing cases')",
    "print(f'verified {len(records)} MagicPen Drawing cases')",
    "MagicPen check-fixture label",
)

# Keep the canonical script's normal fixture write/check behavior but run its
# transformed bytes with this wrapper's __file__. Update is intentionally
# deferred until after native Init has created its own GraphicsObject.
replace_once(
    "rc=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])\n # Capture returned PenData/current Pen and its actual generic size.",
    """rc=call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[drawing,p.draw_canvas,p.pen_canvas,obj,1])
 update_capture=None
 if add_calls:
  rt_receiver=int(add_calls[-1]['receiver'],16)
  update_capture={'receiver':hex(rt_receiver),'field_80':hex(p.get(rt_receiver+80,'Q')[0]),'field_88':hex(p.get(rt_receiver+88,'Q')[0]),'field_96':hex(p.get(rt_receiver+96,'Q')[0]),'native_rt_rgba_f32':list(p.get(rt_receiver+44,'4f')),'native_rt_color_bytes':bytes(p.u.mem_read(rt_receiver+44,16)).hex()}
 # Capture returned PenData/current Pen and its actual generic size.""",
    "capture source-created RT state without Update resource adapter",
)
replace_once(
 "'native_model_metadata':{'tool_type':model_tool,'stroke_type':model_stroke,'curve_enabled':model_curve,'color_argb':model_color},",
 "'native_model_metadata':{'tool_type':model_tool,'stroke_type':model_stroke,'curve_enabled':model_curve,'color_argb':model_color},'native_rt_before_init':update_capture,",
 "capture native RT fields before lifecycle Init",
)

# Wire the real queue through the same Canvas::GetDataQueue boundary used by
# the selected-renderer lifecycle. This permits native SendDataToGPU WorkerTask
# execution and defers only renderer-message and vector-delete task types.
replace_once(
    "import native_drawing_runtime",
    """import oblique_canvas_native as _oc
_old_canvas_hook=_oc.CanvasProbe.hook_canvas
def _magicpen_queue_canvas_hook(self,uc,address,size,data):
 if address==0x200f2040 and getattr(self,'native_task_queue',0):
  self.x(0,self.native_task_queue);uc.reg_write(R.UC_ARM64_REG_PC,uc.reg_read(R.UC_ARM64_REG_LR));return
 return _old_canvas_hook(self,uc,address,size,data)
_oc.CanvasProbe.hook_canvas=_magicpen_queue_canvas_hook
import native_drawing_runtime""",
    "route selected native queue through CanvasProbe",
)
replace_once(
    "queue=p.alloc(64);qvt=p.alloc(64);p.put(queue,'Q',qvt);p.put(qvt+16,'Q',0x20001000);p._runtime_queue=queue;p._task_returns=[]",
    "queue=p.alloc(64);qvt=p.alloc(64);p.put(queue,'Q',qvt);p.put(qvt+16,'Q',0x20001000);p._runtime_queue=queue;p._task_returns=[];p.native_task_queue=queue;p.natural_queue_events=[];p.deferred_native_tasks=[]",
    "enable actual native WorkerTask handoff",
)
replace_once(
    "  if not method:raise RuntimeError('null queued task')",
    """  p.natural_queue_events.append({'phase':label,'task':hex(task),'vtable':hex(vt),'run':hex(method)})
  if vt in (0x10774c8,0x1077528):
   p.deferred_native_tasks.append({'task':hex(task),'vtable':hex(vt),'run':hex(method)})
   p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  if not method:raise RuntimeError('null queued task')""",
    "defer only renderer and vector-delete tasks",
)
replace_once(
    "loader.clear()",
    "loader.clear(); case_queue_start=len(p.natural_queue_events); case_deferred_start=len(p.deferred_native_tasks)",
    "capture per-case native queue event offsets",
)
replace_once(
 "'native_rt_before_init':update_capture,",
 "'native_rt_before_init':update_capture,'native_queue_tasks':p.natural_queue_events[case_queue_start:],'deferred_native_tasks':p.deferred_native_tasks[case_deferred_start:],",
    "record actual queue handoff boundaries",
)

replace_once(
    "map_plugin(MP,mpbase)\n# This is a direct NEEDED dependency and is hash-pinned above, but the bounded\n# ObjectStroke geometry route does not map/execute Renderer.",
    "map_plugin(MP,mpbase)\nRP=LIBS/'libSPenRenderer.so';rpbase=0xB000000\nassert hashlib.sha256(RP.read_bytes()).hexdigest()=='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'\nmap_plugin(RP,rpbase)",
    "map pinned Renderer for direct descriptor helper calls",
)
argb = 0x80336699
pen_name_line = "call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,string('com.samsung.android.sdk.pen.pen.preload.MagicPen')])"
if source.count(pen_name_line) != 1:
    raise SystemExit('source ARGB setter anchor changed')
source = source.replace(pen_name_line, pen_name_line + f"\n call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,{argb}])", 1)
black_assert = "if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':True,'color_argb':'0xff000000'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')"
color_assert = "if _row['native_model_metadata']!={'tool_type':2,'stroke_type':0,'curve_enabled':True,'color_argb':'0x80336699'}:raise RuntimeError('native ObjectStroke metadata getters differ from configured input')"
if source.count(black_assert) != 1:
    raise SystemExit('source ARGB validator anchor changed')
source = source.replace(black_assert, color_assert, 1)
# Strip only the base caller's output tail; this wrapper owns fixture IO.
tail_anchor = "\nif MODE_ARGS.write_fixture:\n FIXTURE.write_text("
if source.count(tail_anchor) != 1:
    raise SystemExit('base caller output tail anchor changed')
source = source[:source.rindex(tail_anchor)] + "\n"

paint_namespace={'__name__':'__main__','__file__':str(Path(__file__).resolve())}
old_argv = sys.argv[:]
sys.argv = [str(BASE)]
try:
    exec(compile(source, str(BASE), "exec"), paint_namespace)
finally:
    sys.argv = old_argv
globals().update(paint_namespace)

import ast, ctypes as C, importlib.util, re, subprocess, types, zlib
from unicorn import arm64_const as R
import airbrush_texture_native as at, inkpen_paint_native as ip, fountain_native as fn
LIB=ROOT/'scratch/apk-analysis-native/arm64-v8a'
SOURCE=ROOT/'conformance/marker3_paint_scale_native.py'
EXPECTED_INPUTS=HELPER_HASHES
EXPECTED_DSOS=DSO_HASHES
def digest_path(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
HOST_GL_PATHS={name:str((Path('/lib/x86_64-linux-gnu')/name).resolve()) for name in ('libEGL.so.1','libGLESv2.so.2')}
HOST_GL_HASHES={name:digest_path(path) for name,path in HOST_GL_PATHS.items()}
if HOST_GL_HASHES!={'libEGL.so.1':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6','libGLESv2.so.2':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812'}: raise SystemExit(f'host EGL/GLES library changed: {HOST_GL_HASHES}')
# The source caller has already run exactly once; only its last explicit V2
# MagicPen case is joined to the host GLES paint route.
runtime_obj=globals()['rt']; p=runtime_obj.p; queue=globals()['queue']; qvt=globals()['qvt']; record=records[-1]
rt=int(record['rtv_addpoint_entries'][-1]['receiver'],16)
run={'p':p,'rt':runtime_obj,'queue':queue,'qvt':qvt,'records':records,'oc':__import__('oblique_canvas_native').CanvasProbe}
vecobj=int(record['native_rtv1_vector']['vector_object'],16)
vecbegin,vecend,veccap=p.get(vecobj,'3Q')
vector_data=bytes(p.u.mem_read(vecbegin,vecend-vecbegin))
assert hashlib.sha256(vector_data).hexdigest()==record['native_rtv1_vector']['sha256']
assert record['native_model_metadata']['color_argb']=='0x80336699'
assert len(vector_data)==96 and hashlib.sha256(vector_data).hexdigest()=='48fab498fb7b870d2fb0ebcf3d725efbcf1352324592286aa9082df635b1377d'
native_color=bytes(p.u.mem_read(rt+44,16))
assert native_color.hex()==record['native_rt_before_init']['native_rt_color_bytes']
control_argb=0x80336699; translation=(64.0,64.0)
# Install the pinned host Mesa GLES adapters used by the joined native paint witness.
background_kind=CLI_ARGS._capture_background
background_rgba=(0,0,0,0) if background_kind=='transparent' else (64,128,192,255)
if background_kind not in ('transparent','opaque'):
 raise SystemExit('MAGICPEN_BG must be transparent or opaque')
ARGS=types.SimpleNamespace(width=256,height=256)
tree=ast.parse(SOURCE.read_text())
klass=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='P')
env={'at':at,'fn':fn,'ip':ip,'C':C,'zlib':zlib,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':re,'json':json,'Path':Path,'LIB':LIB,'ARGS':ARGS,'SOURCE_HASHES':{},'SOURCE_FILES':{},'sys':sys,'ROOT':ROOT,'base64':__import__('base64')}
exec(compile(ast.Module(body=[klass],type_ignores=[]),str(SOURCE),'exec'),env)
Paint=env['P']
for owner in (at.TextureOracle,Paint):
 for name,value in owner.__dict__.items():
  if callable(value) and not name.startswith('__'): setattr(p,name,types.MethodType(value,p))
p.gles=ip.GLES();p.errno_ptr=p.alloc(4);p.gl_calls=[];p.shader_sources={};p.shader_source_calls=[];p.native_samplers={};p.native_syncs={};p.native_logs=[];p.next_gl_id=100
p.callbacks['glGetString']=p.get_string;p.callbacks['glGetIntegerv']=p.get_integer
p.callbacks['glGetError']=lambda:p.x(0,p.gles.f['glGetError']())
p.callbacks['glPixelStorei']=lambda:(p.gl_calls.append({'call':'glPixelStorei','pname':p.x(0),'value':p.x(1)}),p.gles.f['glPixelStorei'](p.x(0),p.x(1)))
p.callbacks['glDeleteTextures']=lambda:(lambda n,ptr:(lambda ids:(ip.cfunc(p.gles.gl,'glDeleteTextures',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*ids)),p.gl_calls.append({'call':'glDeleteTextures','ids':ids})))(list(p.get(ptr,'I'*n)) ))(p.x(0),p.x(1))
p.callbacks['glDeleteSamplers']=lambda:(lambda n,ptr:(lambda ids:(p._sync_proc('glDeleteSamplers',None,[C.c_int,C.POINTER(C.c_uint)])(n,(C.c_uint*n)(*[p.native_samplers.pop(i) for i in ids])),p.gl_calls.append({'call':'glDeleteSamplers','ids':ids})))(list(p.get(ptr,'I'*n))))(p.x(0),p.x(1))
p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xabc);p.callbacks['strstr']=p.strstr;p.callbacks['strncpy']=p.strncpy
p.callbacks['__strncpy_chk']=lambda:(p.u.mem_write(p.x(0),bytes(p.u.mem_read(p.x(1),p.x(2)))),p.x(0,p.x(0)))
init=next(n for n in klass.body if isinstance(n,ast.FunctionDef) and n.name=='__init__')
updates=[n for n in ast.walk(init) if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr=='update' and isinstance(n.func.value,ast.Attribute) and n.func.value.attr=='callbacks']
entry=next(n.args[0] for n in updates if isinstance(n.args[0],ast.Dict) and any(isinstance(k,ast.Constant) and k.value=='glCreateShader' for k in n.args[0].keys))
p.callbacks.update(eval(compile(ast.Expression(entry),'paint-callback-map','eval'),env|{'self':p}))
for n in ast.walk(init):
 if isinstance(n,ast.Assign) and any(isinstance(t,ast.Subscript) and isinstance(t.value,ast.Attribute) and t.value.attr=='callbacks' for t in n.targets):
  key=n.targets[0].slice
  if isinstance(key,ast.Constant) and isinstance(key.value,str):p.callbacks[key.value]=eval(compile(ast.Expression(n.value),'paint-callback-assignment','eval'),env|{'self':p})
for name,method in [('glGenSamplers','gl_gen_samplers'),('glSamplerParameteri','gl_sampler_parameter'),('glFenceSync','gl_fence_sync'),('glClientWaitSync','gl_client_wait_sync'),('glWaitSync','gl_wait_sync'),('glDeleteSync','gl_delete_sync')]:p.callbacks[name]=getattr(p,method)
# Split the native helper's instanced draw from its diagnostic mask readback so
# GL errors can be attributed to the actual draw. The standalone host
# glReadPixels path from the prior helper is not part of native RTV1 paint.
def split_instanced_draw():
 vals=[p.x(i) for i in range(4)];p.gl_calls.append({'call':'glDrawArraysInstanced','values':vals,'state':p.gl_state()})
 p.gles.f['glDrawArraysInstanced'](*vals);draw_error=p.gles.f['glGetError']()
 p.gl_calls.append({'call':'instanced_draw_error','error':draw_error})
p.callbacks['glDrawArraysInstanced']=split_instanced_draw
p.callbacks['glGetActiveUniform']=lambda:(setattr(p,'current_plt_symbol','glGetActiveUniform'),p.gl_get_active())
p.callbacks['glGetActiveAttrib']=lambda:(setattr(p,'current_plt_symbol','glGetActiveAttrib'),p.gl_get_active())
# Texture contents are undefined after a null-data glTexImage2D. Seed only
# those renderer-owned RGBA8 allocations with transparent black so the host
# Mesa witness has a deterministic scratch target; retain native dimensions,
# formats, filtering, draw calls, and all RT data unchanged.
native_tex_image=p.callbacks['glTexImage2D']
host_zero_texture_seeds=[]
def deterministic_tex_image():
 width,height=p.x(3),p.x(4)
 stack=p.u.reg_read(R.UC_ARM64_REG_SP);pixels_ptr=p.get(stack,'Q')[0]
 bound_texture=p.gl_state()['texture2d']
 if pixels_ptr==0 and width>0 and height>0 and p.x(2)==0x8058 and p.x(6)==0x1908 and p.x(7)==0x1401:
  zeros=p.alloc(width*height*4);p.u.mem_write(zeros,bytes(width*height*4));p.put(stack,'Q',zeros)
  host_zero_texture_seeds.append({'texture':bound_texture,'width':width,'height':height,'internal_format':p.x(2),'format':p.x(6),'type':p.x(7),'seed_rgba8':[0,0,0,0]})
 native_tex_image()
 if p.gl_calls and p.gl_calls[-1]['call']=='glTexImage2D':p.gl_calls[-1]['boundTexture']=bound_texture
host_tex_image_callback=deterministic_tex_image
p.callbacks['glTexImage2D']=host_tex_image_callback
_basic_gl_state=p.gl_state
def _paint_gl_state(self):
 state=_basic_gl_state()
 gi=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(self.gles.gl.glGetIntegerv)
 gb=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_ubyte))(self.gles.gl.glGetBooleanv)
 enabled=C.CFUNCTYPE(C.c_ubyte,C.c_uint)(self.gles.gl.glIsEnabled)
 viewport=(C.c_int*4)();gi(0x0BA2,viewport);state['viewport']=list(viewport);state['blend_enabled']=bool(enabled(0x0BE2))
 for label,pname in [('blend_equation_rgb',0x8009),('blend_equation_alpha',0x883D),('blend_src_rgb',0x80C9),('blend_dst_rgb',0x80C8),('blend_src_alpha',0x80CB),('blend_dst_alpha',0x80CA)]:
  value=C.c_int();gi(pname,C.byref(value));state[label]=value.value
 mask=(C.c_ubyte*4)();gb(0x0C23,mask);state['color_mask']=[bool(x) for x in mask]
 return state
p.gl_state=types.MethodType(_paint_gl_state,p)

# Renderer was mapped and initialized in the hash-gated caller wrapper. Remove
# only its factory boundary shims so MagicPen Init calls native factories.
p.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',None)
p.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)',None)

gl_error_checks=[]
for glname,glfn in list(p.callbacks.items()):
 if glname.startswith('gl') and glname!='glGetError':
  def guard_factory(name,fn):
   def guarded():
    before=p.gles.f['glGetError']()
    fn();after=p.gles.f['glGetError']();gl_error_checks.append({'call':name,'before':before,'after':after})
   return guarded
  p.callbacks[glname]=guard_factory(glname,glfn)

# A small queue-managed-object facade is needed by native canvas ownership.
p.queue=queue;p.syms['typeinfo for SPen::IGLManagedMsgQueue']=0xB000000+0x6b400;p.put(qvt-16,'qQ',0,p.syms['typeinfo for SPen::IGLManagedMsgQueue'])
queue_managed=p.alloc(16);managed_queue=p.alloc(32);managed_vt=p.alloc(64);managed_register=p.alloc(16);managed_unregister=p.alloc(16)
managed_tokens={};managed_events=[]
p.put(managed_queue,'Q',managed_vt);p.put(managed_vt+16,'Q',managed_register);p.put(managed_vt+24,'Q',managed_unregister);p.put(qvt+56,'Q',queue_managed)
def queue_hooks(u,a,size,user):
 if a==queue_managed:p.x(0,managed_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_register:
  manager,obj=p.x(0),p.x(1);token=p.alloc(8);p.put(token,'Q',obj);managed_tokens[token]=obj;managed_events.append({'op':'Register','manager':hex(manager),'object':hex(obj),'token':hex(token)});p.x(0,token);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 elif a==managed_unregister:
  manager,token=p.x(0),p.x(1);obj=managed_tokens.pop(token,None);managed_events.append({'op':'Unregister','manager':hex(manager),'object':hex(obj or 0),'token':hex(token)});p.x(0,0);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
p.u.hook_add(4,queue_hooks)

call=lambda name,args=(),limit=5_000_000:p.call(name,args,limit=limit)
magic_sym=lambda suffix: f'SPen::MagicPenStrokeDrawableRTV1::{suffix}'
# Replace the caller's matrix-only CanvasProbe boundary with the native canvas
# manager/factory route, matching the joined Marker3 paint adapter.
p.callbacks.pop('SPen::PenGLDataManager::GetPenCanvas(SPen::IGLMsgQueue*, SPen::String*, int, int, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',None)
p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None)
# The fresh Drawing caller did not install a GraphicsObject at RT+272. Do not
# write the RT field: native Init must create and own that resource itself.
pre_init_graphics_object=p.get(rt+272,'Q')[0]
assert pre_init_graphics_object==0, f'unexpected preexisting RT+272 object: {pre_init_graphics_object:#x}'
call(magic_sym('Init()'),[rt])
native_graphics_object=p.get(rt+272,'Q')[0]
assert native_graphics_object
blend_objects={off:p.get(rt+off,'Q')[0] for off in (328,336,344)}
activate_addr=p.syms['SPen::GLES::BlendStateObject::Activate()']
blend_activate_events=[]
def capture_activate(u,a,size,user):
 if a==activate_addr:
  receiver=u.reg_read(R.UC_ARM64_REG_X0)
  blend_activate_events.append({'receiver':hex(receiver),'rt_field_offset':next((off for off,obj in blend_objects.items() if obj==receiver),None),'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))})
p.u.hook_add(4,capture_activate)
call(magic_sym('CreatePenCanvas(int, int, SPen::IGLMsgQueue*)'),[rt,256,256,queue])

# Create a real native RGBA8 output target over a host-owned zeroed texture.
fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0');zero=p.alloc(256*256*4);p.u.mem_write(zero,bytes(background_rgba)*(256*256))
factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
def call_stack(name,x,stack):
 for i,v in enumerate(x):p.x(i,v)
 sp=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack),*stack));stop=run['rt'].oc.STOP;p.u.reg_write(R.UC_ARM64_REG_LR,stop);p.u.emu_start(p.syms[name],stop,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=stop:raise RuntimeError(f'stack call failed: {name}')
call_stack(factory,[queue,256,256,fmt,zero,1,1024,1],[6,2]);outbmp=p.x(0)
assert outbmp
canvas=p.alloc(512);call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1])
call('SPen::SPCanvasImpl::GetBitmap()',[canvas]);bitmap=p.x(0);call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);bitmap=p.x(0)
assert p.get(rt+80,'Q')[0]==0 and p.get(rt+88,'Q')[0]==vecobj
call(magic_sym('Update(bool)'),[rt,0])
mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,translation[0],translation[1],0,1)
recent=[]
def trace_pc(u,a,z,user):
 recent.append(a)
 if len(recent)>32:del recent[0]
p.u.hook_add(4,trace_pc)
try:call(magic_sym('Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'),[rt,bitmap,mat])
except Exception as e:
 pc=p.u.reg_read(R.UC_ARM64_REG_PC);lr=p.u.reg_read(R.UC_ARM64_REG_LR)
 drawobj=p.get(rt+272,'Q')[0];drawvt=p.get(drawobj,'Q')[0] if drawobj else 0
 print('DRAW_FAULT',repr(e),'pc',hex(pc),'lr',hex(lr),'plt',p.plt.get(pc),'x0_x8',[hex(p.x(i)) for i in range(9)],'sp',hex(p.u.reg_read(R.UC_ARM64_REG_SP)),'recent',[hex(x) for x in recent],'draw_object_vtable',hex(drawvt),'slot56',hex(p.get(drawvt+56,'Q')[0]) if drawvt else None,'rt_fields',[hex(p.get(rt+off,'Q')[0]) for off in (256,264,272,288,296,320,328,336,344)])
 raise
p.gles.gl.glFinish();pixels=C.create_string_buffer(256*256*4)
draw_fbo=C.c_int();read_fbo=C.c_int();get_integer=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetIntegerv)
get_integer(0x8CA6,C.byref(draw_fbo)) # GL_DRAW_FRAMEBUFFER_BINDING / GL_FRAMEBUFFER_BINDING
get_integer(0x8CAA,C.byref(read_fbo)) # GL_READ_FRAMEBUFFER_BINDING
read=C.CFUNCTYPE(None,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p)(p.gles.gl.glReadPixels)
read(0,0,256,256,0x1908,0x1401,pixels)
errors=p.gles.f['glGetError']()
background_bytes=bytes(background_rgba)
affected=[(i%256,i//256) for i in range(256*256) if pixels.raw[i*4:i*4+4]!=background_bytes]
bbox=[min(x for x,y in affected),min(y for x,y in affected),max(x for x,y in affected),max(y for x,y in affected)] if affected else None
def rgba_at(x,y):return list(pixels.raw[(y*256+x)*4:(y*256+x+1)*4])
assert bbox and bbox[0]>0 and bbox[1]>0 and bbox[2]<255 and bbox[3]<255, f'translated control still touches target edge: {bbox}'
assert rgba_at(128,128)==list(background_rgba), 'outside-stroke pixel did not preserve the initialized target background'
expected_stroke_sample=([26,51,77,128] if background_kind=='transparent' else [58,115,173,255])
assert rgba_at(64,64)==expected_stroke_sample, f'partial-alpha source-over pixel changed: {rgba_at(64,64)}'
matching_uploads=[x for x in p.gl_calls if x['call']=='glBufferData' and x['size']==len(vector_data) and x['dataSha256']==hashlib.sha256(vector_data).hexdigest()]
assert matching_uploads, 'native GraphicsObject did not upload exact Drawing-produced RTV1 bytes'
capture_calls={'glBufferData','glUseProgram','glUniformMatrix4fv','glUniform4f','glUniform4fv','glUniform1i','glUniform1f','glBindBuffer','glBindTexture','glActiveTexture','glBindSampler','glBindFramebuffer','glFramebufferTexture2D','glCheckFramebufferStatus','glTexImage2D','glViewport','glDrawArrays','glDrawArraysInstanced','instanced_draw_error','glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask','glClearColor','glClear'}
paint_gl_calls=[x for x in p.gl_calls if x['call'] in capture_calls]
framebuffer_checks=[x for x in p.gl_calls if x['call']=='glCheckFramebufferStatus']
assert len(framebuffer_checks)>=2 and all(x['status']==0x8cd5 for x in framebuffer_checks), 'native draw used an incomplete framebuffer'
assert all(x['after']==0 for x in gl_error_checks), f'wrapped GLES call errors: {[x for x in gl_error_checks if x["after"]!=0]}'
assert errors==0, f'final GLES error {errors:#x}'
clear_calls=[x for x in p.gl_calls if x['call'] in ('glClearColor','glClear')]
draw_framebuffers=[x['state']['framebuffer'] for x in p.gl_calls if x['call'] in ('glDrawArrays','glDrawArraysInstanced')]
assert draw_framebuffers and draw_fbo.value==draw_framebuffers[-1] and read_fbo.value==draw_framebuffers[-1], f'draw/read framebuffer bindings {draw_fbo.value}/{read_fbo.value} are not final draw target {draw_framebuffers[-1] if draw_framebuffers else None}'
framebuffer_texture_joins=[{'framebuffer':x['framebuffer'],'texture':x['args'][3],'args':x['args']} for x in p.gl_calls if x['call']=='glFramebufferTexture2D']
assert any(x['framebuffer']==read_fbo.value for x in framebuffer_texture_joins), 'readback framebuffer has no captured texture attachment'
gl_identity=p.gles.identity
operation_errors=[{'call':'glDrawArraysInstanced','error':x['error']} for x in p.gl_calls if x['call']=='instanced_draw_error' and x['error']]
report={'schema':'magicpen-selected-drawing-rtv1-host-mesa-color-translation-control-v1',
 'scope':'Actual MagicPen Drawing caller generated the RTV1 and native SendDataToGPU WorkerTask placed its vector in RT+88; the fresh RT had no GraphicsObject at RT+272 and no host write cleared or seeded that field. Native Init created the GraphicsObject, followed by native CreatePenCanvas, Update(false), Draw and native Renderer code. Queue managed objects, output bitmap pixels, and EGL/GLES are host adapters. Mesa output is not Android-device parity.',
 'source_case':record['case'],'native_rtv1_receiver':hex(rt),'native_vector_object':hex(vecobj),
 'native_vector_sha256':hashlib.sha256(vector_data).hexdigest(),'native_vector_bytes':len(vector_data),'native_vector_hex':vector_data.hex(),
 'source_argb_input':f'0x{control_argb:08x}','view_translation':[translation[0],translation[1]],
 'graphics_object_lifecycle':{'rt_plus_272_before_native_init':hex(pre_init_graphics_object),'rt_plus_272_after_native_init':hex(native_graphics_object),'created_by_native_init':True},
 'exact_native_gles_buffer_uploads':matching_uploads,'native_rt_rgba_f32':list(struct.unpack('<4f',native_color)),
 'native_rt_rgba_bytes':native_color.hex(),'target_initial_background':{'kind':background_kind,'rgba8':list(background_rgba)},
 'target_size':[256,256],'native_queue_events':p.natural_queue_events,'managed_queue_events':managed_events,
 'native_blend_calls':[x for x in p.gl_calls if x['call'] in ('glBlendEquationSeparate','glBlendFuncSeparate','glEnable','glDisable','glColorMask')],
 'native_blend_activate_events':blend_activate_events,
 'host_gl_identity':gl_identity,
 'execution_closure':{'helper_sha256':{rel:expected for rel,expected in EXPECTED_INPUTS.items()},'native_dso_sha256':EXPECTED_DSOS,'host_gl_paths':HOST_GL_PATHS,'host_gl_sha256':HOST_GL_HASHES,'runner_path':'conformance/magicpen_drawing_paint_native.py'},
 'framebuffer_completeness':{'checks':framebuffer_checks,'all_complete':True},'clear_operations':clear_calls,'operation_errors':operation_errors,
 'framebuffer_texture_joins':framebuffer_texture_joins,'framebuffer_bindings_before_readpixels':{'draw_pname_GL_DRAW_FRAMEBUFFER_BINDING':0x8CA6,'draw_value':draw_fbo.value,'read_pname_GL_READ_FRAMEBUFFER_BINDING':0x8CAA,'read_value':read_fbo.value},'readback_target':{'draw_framebuffer':draw_fbo.value,'read_framebuffer':read_fbo.value,'width':256,'height':256,'output_bitmap_object':hex(outbmp),'native_pen_canvas_subbitmap':hex(bitmap)},
 'host_zero_initialized_texture_allocations':host_zero_texture_seeds,
 'draw_calls':[x for x in p.gl_calls if x['call'] in ('glDrawArrays','glDrawArraysInstanced')],
 'shader_source_calls':p.shader_source_calls,'texture_allocations':[x for x in p.gl_calls if x['call']=='glTexImage2D'],
 'viewport_calls':[x for x in p.gl_calls if x['call']=='glViewport'],'paint_gl_calls':paint_gl_calls,
 'readback':{'bytes':len(pixels.raw),'sha256':hashlib.sha256(pixels.raw).hexdigest(),'rgba8_zlib_base64':base64.b64encode(zlib.compress(pixels.raw,9)).decode('ascii'),'storage_origin':'OpenGL lower-left row first','pixel_format':'RGBA8','width':256,'height':256,'changed_from_background_pixels':len(affected),'changed_from_background_bbox':bbox,
  'samples':[{'xy':[x,y],'rgba':list(pixels.raw[(y*256+x)*4:(y*256+x+1)*4])} for x,y in ((0,0),(128,128),(20,20),(64,64),(128,64))]},
 'gl_errors_final':errors,'gl_error_checks':gl_error_checks,
 'library_hashes':{name:hashlib.sha256((LIB/name).read_bytes()).hexdigest() for name in ('libSPenMagicPen.so','libSPenRenderer.so','libSPenPenCommon.so','libSPenGraphics.so','libSPenDrawing.so','libSPenModel.so','libc++_shared.so')},
 'boundaries':['MagicPen five-setting caller run; last explicit-empty case selected','native queue SendDataToGPU task already run by caller harness; render and delete queue tasks deferred','host queue managed-object facade','host-native RGBA8 output allocation seed; native factories wrap target','Mesa GLES host and shader bindings','selected transparent/opaque background is a host initial target control']}
paint_report=report
source_capture={'schema':'magicpen-saved-caller-native-v1','provenance':{'base_caller_path':'conformance/fountain_saved_caller_native.py','base_caller_sha256':BASE_SHA256,'adapter_patch_labels':['MagicPen DSO mapping and factory','ObjectStroke MagicPen name and ARGB','advanced setting null/empty/V1/V2 controls','RTV1 AddPoint entry and native vector return capture','native SendDataToGPU queue dispatch','host IContext and CanvasProbe callbacks'],'mapped_dso_sha256':{rel:expected for rel,expected in DSO_HASHES.items()},'helper_sha256':HELPER_HASHES},'profile':[1920,1080],'source_input':{'pen_name':'com.samsung.android.sdk.pen.pen.preload.MagicPen','points':[[0,0],[10,0],[20,2]],'pressure':[0.3,0.6,1.0],'timestamps':[100,110,120],'tilt':[0,0,0],'orientation':[0,0,0],'tool_type':2,'stroke_type':0,'curve_enabled':True,'visible':True,'color_argb':'0x80336699'},'cases':records}
print(json.dumps({'source_capture':source_capture,'paint':paint_report},sort_keys=True))
