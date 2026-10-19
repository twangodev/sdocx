#!/usr/bin/env python3
"""Bounded native Crayon2 saved-object playback-to-Mesa paint runner.

Model inputs, queue acceptance, EGL/GLES, allocation, and host graphics resources
are adapters. Crayon2/Renderer methods execute from hash-pinned native DSOs.
"""
import argparse, base64, contextlib, ctypes as C, hashlib, json, os, runpy, struct, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'conformance/crayon2-saved-playback-paint-native.json'
SELF_PIN='45f1f47aae60a6891bcdd631707138f9cb59e122ae123f224b802823497e230d'
EXPECTED_FIXTURE_SHA256='df256ab0b5ba08fc4447d7e08f22b35570e2731b20a3f741f81c4e2ac56738f0'
HELPER_PINS={
 'conformance/fountain_saved_caller_native.py':'b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19',
 'conformance/native_drawing_runtime.py':'65dd0a4294332b68410d9436762c4f0ad84a0ad4b2a21a742e7b098e089fba91',
 'conformance/fountain_native.py':'40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
 'conformance/oblique_canvas_native.py':'1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
 'conformance/inkpen_paint_native.py':'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae',
 'conformance/elf_plt.py':'816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
 'conformance/fountain-saved-caller-native.json':'552dbabd5cf1b760cf63c8fba967c7a6fe14ca124155ccc929d0587a7ffa4695',
 'scratch/apk-analysis-runtime/python/unicorn/__init__.py':'9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b',
 'scratch/apk-analysis-runtime/python/unicorn/unicorn_py3/unicorn.py':'62ffe84341f4288e309040e06555b81798031817d32fd19a945e250813f872f0',
}
DSO_PINS={
 'libSPenCrayon2.so':'4e9de0e699eb3781e7cdb17d6816370ce3e2f75cf6fe2087fada50cfa816739e',
 'libSPenRenderer.so':'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenDefaultPen.so':'1b37056c81646c407e9e0697c78e31b50ffab0bdb896966d1e89feb02385544f',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
TOOL_PINS={
 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump':'b9ae08c9c02c91ced02b80a140c937ba119fe073872fff701f67477bf597a5eb',
 '/usr/bin/nm':'ddb55c0685721bf494aa2d090056b8b32a40614ad4464b305896f527d882c3c1',
 '/usr/bin/c++filt':'7d54ea6797a5e8c69255f711b79577efd9934c3502c6edee4bc25b7dc7d97bd2',
 '/usr/lib/x86_64-linux-gnu/libEGL.so.1.1.0':'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6',
 '/usr/lib/x86_64-linux-gnu/libGLESv2.so.2.1.0':'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812',
}
def sha(raw): return hashlib.sha256(raw).hexdigest()
def normalized_self_hash():
 text=Path(__file__).read_text()
 text=text.replace(f"SELF_PIN='{SELF_PIN}'", "SELF_PIN='<SELF_PIN>'")
 text=text.replace(f"EXPECTED_FIXTURE_SHA256='{EXPECTED_FIXTURE_SHA256}'", "EXPECTED_FIXTURE_SHA256='<FIXTURE_SHA256>'")
 return sha(text.encode())
def preflight():
 if normalized_self_hash()!=SELF_PIN:raise SystemExit(f'runner self-pin mismatch: {normalized_self_hash()}')
 for rel,expected in {**HELPER_PINS,**TOOL_PINS}.items():
  path=Path(rel) if rel.startswith('/') else ROOT/rel
  got=sha(path.read_bytes())
  if got!=expected:raise SystemExit(f'pre-import pin mismatch {rel}: {got} != {expected}')
 libdir=ROOT/'scratch/apk-analysis-native/arm64-v8a'
 for name,expected in DSO_PINS.items():
  got=sha((libdir/name).read_bytes())
  if got!=expected:raise SystemExit(f'pre-import DSO pin mismatch {name}: {got} != {expected}')

def getter_capture(report, source):
 entries=report['source_getter_events'];returns=report['source_getter_returns']
 if len(entries)!=len(returns):raise RuntimeError('native source getter entry/return counts differ')
 n=len(source['timestamps'])
 expected={
  'GetPoint() const':struct.pack('<'+'f'*(2*n),*(v for point in source['points'] for v in point)),
  'GetPressure() const':struct.pack('<'+'f'*n,*source['pressures']),
  'GetTimeStamp() const':struct.pack('<'+'i'*n,*source['timestamps']),
  'GetTilt() const':struct.pack('<'+'f'*n,*source['tilts']),
  'GetOrientation() const':struct.pack('<'+'f'*n,*source['orientations']),
 }
 rows=[]
 for entry,result in zip(entries,returns):
  name=entry['getter'].split('::')[-1]
  value=int(result['x0'],16)
  row={'getter':name,'return_x0':value if 'Count' in name or 'ToolType' in name else ('non_null_pointer' if value else 'null_pointer'),'s0_f32_bits':result['s0_f32_bits']}
  if name in expected:
   raw=bytes.fromhex(result['array_bytes_hex'])
   if raw!=expected[name]:raise RuntimeError(f'{name}: native returned array differs from declared source values')
   row.update(array_count=n,array_bytes_hex=raw.hex(),matches_declared_source=True)
  rows.append(row)
 names={r['getter'] for r in rows}
 if set(expected)-names:raise RuntimeError(f'missing native array getters: {sorted(set(expected)-names)}')
 for row in rows:
  if row['getter']=='GetPointCount() const' and row['return_x0']!=n:raise RuntimeError('native point count differs from declared source arrays')
 return rows

def compact_capture(name,report):
 draw=report['crayon2_native_draw']; read=draw['framebuffer_readback']; packed=report['packed_boundary']; update=report['crayon2_post_playback_update']
 raw=base64.b64decode(read['rgba8_base64'])
 if len(raw)!=256*256*4 or sha(raw)!=read['rgba8_sha256']:raise RuntimeError(f'{name}: full RGBA readback length/hash mismatch')
 if read['gl_error_before_readpixels'] or read['gl_error_after_readpixels']:raise RuntimeError(f'{name}: GL error around readback')
 if read['draw_framebuffer_binding_0x8ca6']!=read['read_framebuffer_binding_0x8caa']:raise RuntimeError(f'{name}: draw/read framebuffer bindings differ')
 if read['read_attachment_object_name']!=read['draw_attachment_object_name'] or not read['read_attachment_object_name']:raise RuntimeError(f'{name}: read/draw attachment identity differs')
 vector=bytes.fromhex(packed['final_packed_stream_hex'])
 if len(vector)!=packed['final_packed_record_count']*24:raise RuntimeError(f'{name}: native packed stream length mismatch')
 upload=next((x for x in update['mesa_gl_events'] if x['name']=='glBufferData' and x['size']==len(vector)),None)
 if not upload or upload['data_sha256']!=sha(vector):raise RuntimeError(f'{name}: native renderer upload does not match producer vector')
 draws=[x for x in draw['actual_gl_events'] if x['name']=='glDrawArraysInstanced']
 if len(draws)!=1 or draws[0]['instances']!=len(vector)//24:raise RuntimeError(f'{name}: expected one instanced draw per native packed record')
 if not draw['error'] is None:raise RuntimeError(f'{name}: native Draw fault: {draw["error"]}')
 attrs=[x for x in draw['actual_gl_events'] if x['name'] in ('glVertexAttribPointer','glVertexAttribDivisor','glBindBuffer')]
 return {
  'name':name,
  'host_view_translation':draw['host_view_translation'],
  'background_rgba':read['background_rgba'],
  'draw_result_raw':draw['result'],
  'output_target':{'read_framebuffer_binding':read['read_framebuffer_binding_0x8caa'],'draw_framebuffer_binding':read['draw_framebuffer_binding_0x8ca6'],'read_color_attachment_texture':read['read_attachment_object_name'],'draw_color_attachment_texture':read['draw_attachment_object_name']},
  'renderer_bindings_and_attributes':attrs,
  'renderer_uniform_calls':[x for x in draw['actual_gl_events'] if x['name'].startswith('glUniform')],
  'instanced_draw':draws[0],
  'framebuffer':read,
 }

def aggregate(captures):
 first=captures[0][1]
 source={
  'source_object':first['source_object'],
  'host_injected_renderer_setting':first['host_setting_write'],
  'native_getters':getter_capture(first,first['source_object']),
  'native_geometry_counts':first['native_route']['geometry_counts'],
  'drawpoint_stream_sha256':first['native_route']['drawpoint_stream_sha256'],
  'native_packed_record_count':first['packed_boundary']['final_packed_record_count'],
  'native_vector_sha256':first['packed_boundary']['final_packed_stream_sha256'],
  'native_vector_hex':first['packed_boundary']['final_packed_stream_hex'],
  'queued_worker':{'task_vtable':captures[0][1]['canvas_queue_posts'][0]['task_vtable'],'run_method':captures[0][1]['canvas_queue_posts'][0]['run_method'],'member_target_word':captures[0][1]['canvas_queue_posts'][0]['member_target_word'],'slots_before':captures[0][1]['canvas_worker_runs'][0]['slots_before'],'slots_after':captures[0][1]['canvas_worker_runs'][0]['slots_after']},
  'update':{'result_raw':first['crayon2_post_playback_update']['result'],'gl_buffer_data':next(x for x in first['crayon2_post_playback_update']['mesa_gl_events'] if x['name']=='glBufferData')},
  'native_init':{'result':first['rt_init']['result'],'shader_manager_setup':first['shader_manager_setup'],'renderer_parameters_init':first['renderer_parameters_init'],'shader_source_hashes':[e['sources_sha256'] for e in first['init_gl_events'] if e['name']=='glShaderSource'],'shader_status':[{'name':e['name'],'pname':e.get('pname'),'value':e.get('value')} for e in first['init_gl_events'] if e['name'] in ('glGetShaderiv','glGetProgramiv') and e.get('pname') in (0x8b81,0x8b82)],'texture_uploads':[{'args':e['args'],'bytes':e['bytes'],'data_sha256':e['data_sha256']} for e in first['init_gl_events'] if e['name']=='glTexSubImage2D'],'host_gl_limits':[e for e in first['init_gl_events'] if e['name']=='glGetIntegerv' and e['enum'] in (0x0d33,0x0d3a)]},
 }
 # The input, native getter results, texture/shader setup, vector, queue handoff,
 # and GL upload must be identical for all target/background controls.
 for name,report in captures[1:]:
  if report['source_object']!=source['source_object'] or getter_capture(report,report['source_object'])!=source['native_getters'] or report['host_setting_write']!=source['host_injected_renderer_setting'] or report['packed_boundary']['final_packed_stream_hex']!=source['native_vector_hex'] or report['crayon2_post_playback_update']['mesa_gl_events']!=first['crayon2_post_playback_update']['mesa_gl_events']:
   raise RuntimeError(f'{name}: controls changed source/getter/vector/upload contract')
  init_uploads=[{'args':e['args'],'bytes':e['bytes'],'data_sha256':e['data_sha256']} for e in report['init_gl_events'] if e['name']=='glTexSubImage2D']
  if init_uploads!=source['native_init']['texture_uploads']:raise RuntimeError(f'{name}: controls changed native texture uploads')
 cases=[compact_capture(name,report) for name,report in captures]
 by={x['name']:x for x in cases};origin=by['origin-transparent'];translated=by['translated-transparent'];opaque=by['translated-opaque']
 if origin['framebuffer']['bbox_vs_background'] is None or origin['framebuffer']['bbox_vs_background'][0]!=0 or origin['framebuffer']['bbox_vs_background'][1]!=0:raise RuntimeError('origin control no longer demonstrates edge clipping')
 box=translated['framebuffer']['bbox_vs_background']
 if not box or box[0]<=0 or box[1]<=0 or box[2]>=255 or box[3]>=255:raise RuntimeError(f'translated support is clipped: {box}')
 def mask(case):
  raw=base64.b64decode(case['framebuffer']['rgba8_base64']);bg=bytes(case['background_rgba'])
  return bytes(raw[i:i+4]!=bg for i in range(0,len(raw),4))
 if mask(translated)!=mask(opaque):raise RuntimeError('opaque and transparent translated controls have different stroke support')
 return {'schema':'crayon2-native-saved-playback-paint-v1','normalized_runner_source_sha256':SELF_PIN,'pins':{'helpers':HELPER_PINS,'dsos':DSO_PINS,'tools_and_host_gles':TOOL_PINS},'scope':'Pinned native Crayon2/Renderer Init, direct saved-ObjectStroke RedrawPen playback, queued worker, Update, and Draw under Unicorn with actual host Mesa GLES; this route does not enter Drawing. Model setters create the source stroke, while the runner separately writes the factory-created Crayon2 setting record to supply renderer pen size. Saved ObjectStroke ARGB is recorded as Model input but forwarding it into the Crayon2 shader color is not established; the captured GL uniforms and RGBA are renderer observations. Queue acceptance, modeled bionic RNG, EGL/GL resource setup, and host bitmap/canvas are adapters. This is not Samsung-device or actual-note pixel parity.','host_mesa':first['host_mesa'],'source_capture':source,'paint_controls':cases}

def main():
 parser=argparse.ArgumentParser(description='Replay pinned Crayon2 saved-object playback-to-Mesa paint controls')
 group=parser.add_mutually_exclusive_group();group.add_argument('--write-fixture',action='store_true',help='explicitly capture and write the fixture');group.add_argument('--check-fixture',action='store_true',help='replay and verify the fixture (default)')
 parser.add_argument('--_capture',choices=('origin-transparent','translated-transparent','translated-opaque'),help=argparse.SUPPRESS)
 args=parser.parse_args();preflight()
 if args._capture:
  translation='0,0' if args._capture=='origin-transparent' else '64,64'
  bg='0,0,0,0' if args._capture!='translated-opaque' else '24,48,72,255'
  os.environ['CRAYON_TRANSLATION']=translation;os.environ['CRAYON_BG_RGBA']=bg
  return args._capture
 if not args.write_fixture:
  actual=sha(FIXTURE.read_bytes()) if FIXTURE.exists() else None
  if actual!=EXPECTED_FIXTURE_SHA256:raise SystemExit(f'fixture pin mismatch: {actual} != {EXPECTED_FIXTURE_SHA256}; use --write-fixture explicitly to refresh')
 captures=[]
 env=os.environ.copy();env['PYTHONPATH']=os.pathsep.join([str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python'),env.get('PYTHONPATH','')]).rstrip(os.pathsep)
 env['LD_LIBRARY_PATH']=os.pathsep.join([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'),env.get('LD_LIBRARY_PATH','')]).rstrip(os.pathsep)
 for case in ('origin-transparent','translated-transparent','translated-opaque'):
  proc=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--_capture',case],cwd=ROOT,env=env,text=True,capture_output=True)
  if proc.returncode:raise SystemExit(f'{case} native capture failed ({proc.returncode}):\n{proc.stdout}\n{proc.stderr}')
  try:captures.append((case,json.loads(proc.stdout)))
  except Exception as exc:raise SystemExit(f'{case} did not emit one JSON report: {exc}\n{proc.stdout[:2000]}')
 fixture=(json.dumps(aggregate(captures),indent=2,sort_keys=True)+'\n').encode()
 if args.write_fixture:
  FIXTURE.write_bytes(fixture);print(f'wrote {FIXTURE.relative_to(ROOT)} sha256={sha(fixture)}')
 else:
  if FIXTURE.read_bytes()!=fixture:raise SystemExit(f'fresh Crayon2 native paint capture differs from {FIXTURE.relative_to(ROOT)}; use --write-fixture explicitly to refresh')
  print(f'verified {FIXTURE.relative_to(ROOT)} sha256={sha(fixture)}')
# The capture branch returns to the native probe body below; wrapper branch exits.

_capture_case=main()
if _capture_case is None:raise SystemExit(0)
os.environ['CRAYON_CASE']='base'
sys.path[:0]=[str(ROOT),str(ROOT/'conformance'),str(ROOT/'scratch/apk-analysis-runtime/python')]
os.chdir(ROOT)
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_INVALID
from unicorn import arm64_const as R
from conformance.elf_plt import resolve as resolve_plt

# Reuse the established pinned Model/Drawing/CanvasProbe runtime and its
# real MotionEvent ABI. The additional Crayon2 DSO is mapped and relocated below.
sys.argv=['fountain_saved_caller_native.py','--check-fixture']
with contextlib.redirect_stdout(open(os.devnull,'w')):
    c=runpy.run_path(str(ROOT/'conformance/fountain_saved_caller_native.py'))
p=c['p']; rt=c['rt']; env=c['env']; LIBS=rt.LIBS
lib=LIBS/'libSPenCrayon2.so'; raw=lib.read_bytes(); digest=hashlib.sha256(raw).hexdigest()
assert digest=='4e9de0e699eb3781e7cdb17d6816370ce3e2f75cf6fe2087fada50cfa816739e'
base=0xa000000
phoff=struct.unpack_from('<Q',raw,32)[0]; phent,phnum=struct.unpack_from('<HH',raw,54); extent=0
ph=[]
for i in range(phnum):
    seg=struct.unpack_from('<II6Q',raw,phoff+i*phent);ph.append(seg)
    if seg[0]==1:extent=max(extent,seg[3]+seg[6])
p.u.mem_map(base,(extent+0xfff)&~0xfff)
for typ,flags,off,va,pa,filesz,memsz,align in ph:
    if typ==1 and memsz:
        p.u.mem_write(base+va,raw[off:off+filesz])
        if memsz>filesz:p.u.mem_write(base+va+filesz,bytes(memsz-filesz))
for line in subprocess.check_output(['nm','-D','-C',str(lib)],text=True,env=env).splitlines():
    fields=line.strip().split(' ',2)
    if len(fields)==3 and fields[1].upper() in ('T','W','D','B','R','V'):
        p.syms[fields[2]]=base+int(fields[0],16)
for line in subprocess.check_output(['nm','-D',str(lib)],text=True,env=env).splitlines():
    fields=line.strip().split()
    if len(fields)==3 and fields[1].upper() in ('T','W','D','B','R','V'):
        rt.rawsyms.setdefault(fields[2],base+int(fields[0],16))
shoff=struct.unpack_from('<Q',raw,40)[0]; shent,shnum,shstr=struct.unpack_from('<HHH',raw,58)
sections=[struct.unpack_from('<IIQQQQIIQQ',raw,shoff+i*shent) for i in range(shnum)]
shnames=raw[sections[shstr][4]:sections[shstr][4]+sections[shstr][5]]
byname={shnames[q[0]:shnames.find(b'\0',q[0])].decode():q for q in sections}
ds,st=byname['.dynsym'],byname['.dynstr']; strings=raw[st[4]:st[4]+st[5]]; symbols=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
    no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',raw,off)
    symbols.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
    sec=byname.get(secname)
    if not sec:continue
    for off in range(sec[4],sec[4]+sec[5],sec[9]):
        where,info,add=struct.unpack_from('<QQq',raw,off); typ,idx=info&0xffffffff,info>>32
        if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
        elif typ in (0x401,0x402,0x101,0x1025,0x1026):
            name,val=symbols[idx];target=base+val if val else p.syms.get(name,0) or rt.rawsyms.get(name,0)
            if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
for off,name in resolve_plt(lib).items():
    pretty=subprocess.check_output(['c++filt','-n',name],text=True).strip() if name.startswith('_Z') else name
    p.plt[base+off]=pretty

STOP=0x200f1000
def call(name,args=(),floats=(),limit=8000000):
    for i,v in enumerate(args):p.x(i,v)
    for i,v in enumerate(floats):p.f(i,v)
    p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,STOP)
    try:p.u.emu_start(p.syms[name],STOP,count=limit)
    except Exception as e:raise RuntimeError(f'{name} fault PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} LR={p.u.reg_read(R.UC_ARM64_REG_LR):#x} x0-x5={[hex(p.x(i)) for i in range(6)]}') from e
    if p.u.reg_read(R.UC_ARM64_REG_PC)!=STOP:raise RuntimeError(f'{name} did not return: {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
    return p.x(0)

# Create Crayon2 and its Crayon2Data through the pinned plugin factory. This
# executes actual Pen::Pen/Crayon2::Crayon2 and uses their default pattern/config
# state; only the saved stroke's size is then set natively through the backing
# setting-data slot before constructing the V1 drawable.
factory_trace=[]
trace_addrs={}
for n in ('SPen::Crayon2::Crayon2()','SPen::Pen::Pen()'):
    if n in p.syms: trace_addrs[p.syms[n]]=n
factory_hook=p.u.hook_add(UC_HOOK_CODE,lambda u,a,z,d: factory_trace.append({'symbol':trace_addrs[a],'address':hex(a)}) if a in trace_addrs else None)
pen=call('createPenInst',[])
p.u.hook_del(factory_hook)
if not pen: raise RuntimeError('createPenInst returned null')
data=p.get(pen+120,'Q')[0]
settings=p.get(data,'Q')[0]
factory_setting_initial=bytes(p.u.mem_read(settings,32)).hex()
if not data or not settings: raise RuntimeError(f'factory produced invalid data/settings {data:#x}/{settings:#x}')
manager=p.alloc(0x100); drawable=p.alloc(0x400)
call('SPen::PenGLDataManager::PenGLDataManager()',[manager])
queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt); queue_post=0x20001000; queue_task_return=0x20001020; p.put(qvt+16,'Q',queue_post)
p.u.mem_write(queue_post,bytes.fromhex('1f2003d5c0035fd6')); p.u.mem_write(queue_task_return,bytes.fromhex('1f2003d5c0035fd6')); p._task_returns=[]; queue_posts=[]
def taskqueue(u,a,z,d):
    if a==queue_post:
        task=p.x(1); vt=p.get(task,'Q')[0]; method=p.get(vt+16,'Q')[0]
        if not method: raise RuntimeError('null queued task')
        p._task_returns.append(u.reg_read(R.UC_ARM64_REG_LR)); queue_posts.append({'task':hex(task),'run_method':hex(method),'post_caller_lr':hex(u.reg_read(R.UC_ARM64_REG_LR))}); p.x(0,task); u.reg_write(R.UC_ARM64_REG_LR,queue_task_return); u.reg_write(R.UC_ARM64_REG_PC,method)
    elif a==queue_task_return:
        p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,p._task_returns.pop())
p.u.hook_add(UC_HOOK_CODE,taskqueue)
call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue])
p._runtime_queue=queue
call('SPen::Crayon2StrokeDrawableGLV1::Crayon2StrokeDrawableGLV1(SPen::Crayon2Data*, SPen::PenGLDataManager*)',[drawable,data,manager])
# Load the pinned Renderer DSO so VertexDescriptor's real addAttribute
# implementation can execute during Crayon2 RTV1::Init instead of a host stub.
renderer=LIBS/'libSPenRenderer.so'; rraw=renderer.read_bytes(); renderer_hash=hashlib.sha256(rraw).hexdigest()
rbase=0xb000000
rphoff=struct.unpack_from('<Q',rraw,32)[0]; rphent,rphnum=struct.unpack_from('<HH',rraw,54); rextent=0; rph=[]
for i in range(rphnum):
    seg=struct.unpack_from('<II6Q',rraw,rphoff+i*rphent);rph.append(seg)
    if seg[0]==1:rextent=max(rextent,seg[3]+seg[6])
p.u.mem_map(rbase,(rextent+0xfff)&~0xfff)
for typ,flags,off,va,pa,filesz,memsz,align in rph:
    if typ==1 and memsz:
        p.u.mem_write(rbase+va,rraw[off:off+filesz])
        if memsz>filesz:p.u.mem_write(rbase+va+filesz,bytes(memsz-filesz))
rshoff=struct.unpack_from('<Q',rraw,40)[0]; rshent,rshnum,rshstr=struct.unpack_from('<HHH',rraw,58)
rsecs=[struct.unpack_from('<IIQQQQIIQQ',rraw,rshoff+i*rshent) for i in range(rshnum)]
rnames=rraw[rsecs[rshstr][4]:rsecs[rshstr][4]+rsecs[rshstr][5]]
rby={rnames[q[0]:rnames.find(b'\0',q[0])].decode():q for q in rsecs}
rds,rst=rby['.dynsym'],rby['.dynstr'];rstrings=rraw[rst[4]:rst[4]+rst[5]];rsyms=[]
for off in range(rds[4],rds[4]+rds[5],rds[9]):
    no,info,other,ndx,val,size=struct.unpack_from('<IBBHQQ',rraw,off);rsyms.append((rstrings[no:rstrings.find(b'\0',no)].decode() if no else '',val))
for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=env).splitlines():
    fields=line.strip().split(' ',2)
    if len(fields)==3 and fields[1].upper() in ('T','W','D','B','R','V'):p.syms[fields[2]]=rbase+int(fields[0],16)
for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=env).splitlines():
    fields=line.strip().split()
    if len(fields)==3 and fields[1].upper() in ('T','W','D','B','R','V'):rt.rawsyms.setdefault(fields[2],rbase+int(fields[0],16))
for secname in ('.rela.dyn','.rela.plt'):
    sec=rby.get(secname)
    if not sec:continue
    for off in range(sec[4],sec[4]+sec[5],sec[9]):
        where,info,add=struct.unpack_from('<QQq',rraw,off);typ,idx=info&0xffffffff,info>>32
        if typ==0x403:p.u.mem_write(rbase+where,struct.pack('<Q',rbase+add))
        elif typ in (0x401,0x402,0x101,0x1025,0x1026):
            name,val=rsyms[idx];target=rbase+val if val else p.syms.get(name,0) or rt.rawsyms.get(name,0)
            if target:p.u.mem_write(rbase+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
for off,name in resolve_plt(renderer).items():
    pretty=subprocess.check_output(['c++filt','-n',name],text=True).strip() if name.startswith('_Z') else name
    p.plt[rbase+off]=pretty
# Renderer normally installs this process-wide object from its Android-loader
# initializer. Reproduce only that lifecycle prerequisite: run the pinned
# native constructor and store its pointer in the statically verified GOT/data
# global used by the real GetInstance() body (Renderer RVA 0x6d480).
shader_manager=p.alloc(512)
shader_manager_ctor='SPen::ShaderManager::ShaderManager()'
shader_manager_global=rbase+0x6d480
call(shader_manager_ctor,[shader_manager])
p.put(shader_manager_global,'Q',shader_manager)
shader_manager_singleton=call('SPen::ShaderManager::GetInstance()',[])
if shader_manager_singleton!=shader_manager:raise RuntimeError(f'native ShaderManager singleton mismatch: {shader_manager_singleton:#x} != {shader_manager:#x}')
add_sym='SPen::VertexDescriptor::addAttribute(SPen::VertexDescriptor::AttributeType, int, int, int)'
assert add_sym in p.syms, 'Renderer lacks pinned VertexDescriptor::addAttribute'
# Retarget the Crayon2 import to the mapped native Renderer function and
# remove the generic import trap for this one PLT slot.
add_target=p.syms[add_sym]
for secname in ('.rela.dyn','.rela.plt'):
    sec=byname.get(secname)
    if not sec:continue
    for off in range(sec[4],sec[4]+sec[5],sec[9]):
        where,info,add=struct.unpack_from('<QQq',raw,off);typ,idx=info&0xffffffff,info>>32
        if symbols[idx][0]=='_ZN4SPen16VertexDescriptor11addAttributeENS0_13AttributeTypeEiii':
            p.u.mem_write(base+where,struct.pack('<Q',add_target+(add if typ in (0x401,0x101) else 0)))
for off,name in resolve_plt(lib).items():
    if name=='_ZN4SPen16VertexDescriptor11addAttributeENS0_13AttributeTypeEiii':p.plt.pop(base+off,None)
gl_trace=[]
# The platform-created IRendererDeviceCaps normally reports this value. Keep
# this probe's size gate explicit and metadata-only; this 2048 result is not a
# measured host/Mesa or Samsung GPU limit.
caps_object=p.alloc(16);caps_vtable=p.alloc(32);caps_method=0x200f4500
p.put(caps_object,'Q',caps_vtable);p.put(caps_vtable+16,'Q',caps_method);p.u.mem_write(caps_method,struct.pack('<II',0x52810000,0xd65f03c0))
caps_events=[]
def get_renderer_caps():
    caps_events.append({'returned':hex(caps_object),'virtual_slot':'+16','synthetic_max_dimension':2048});p.x(0,caps_object)
p.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=get_renderer_caps
aligned_allocations=[]
def host_posix_memalign():
    # POSIX ABI: int posix_memalign(void **memptr, size_t alignment,
    #                               size_t size). The guest native texture
    # allocator owns this aligned region; host memory merely backs its bytes.
    slot,alignment,size=p.x(0),p.x(1),p.x(2)
    if alignment<struct.calcsize('P') or alignment&(alignment-1) or size>0x10000000:
        raise RuntimeError(f'posix_memalign adapter rejected alignment/size {alignment}/{size}')
    rawptr=p.alloc(size+alignment-1);ptr=(rawptr+alignment-1)&-alignment
    p.put(slot,'Q',ptr);aligned_allocations.append({'slot':hex(slot),'raw':hex(rawptr),'aligned':hex(ptr),'alignment':alignment,'size':size});p.x(0,0)
p.callbacks['posix_memalign']=host_posix_memalign
allocation_sizes={};original_alloc=p.alloc
def tracked_alloc(size):
    ptr=original_alloc(size);allocation_sizes[ptr]=size;return ptr
p.alloc=tracked_alloc
realloc_events=[]
def host_realloc():
    old,new_size=p.x(0),p.x(1)
    if new_size>0x1000000:raise RuntimeError(f'realloc exceeds bounded size: {new_size}')
    if not old:ptr=p.alloc(max(new_size,1));old_size=0
    else:
        if old not in allocation_sizes:raise RuntimeError(f'realloc of untracked allocation {old:#x}')
        old_size=allocation_sizes.pop(old);ptr=p.alloc(max(new_size,1));copied=min(old_size,new_size)
        if copied:p.u.mem_write(ptr,bytes(p.u.mem_read(old,copied)))
    realloc_events.append({'old':hex(old),'old_size':old_size,'new':hex(ptr),'new_size':new_size,'copied':min(old_size,new_size) if old else 0})
    p.x(0,ptr)
p.callbacks['realloc']=host_realloc
stdio_events=[]
virtual_files={};virtual_file_events=[]
def host_fopen():
    pathptr,modeptr=p.x(0),p.x(1);path=bytes(p.u.mem_read(pathptr,4096)).split(b'\0',1)[0].decode(errors='replace');mode=bytes(p.u.mem_read(modeptr,128)).split(b'\0',1)[0].decode(errors='replace');handle=p.alloc(32)
    data=bytearray() if any(c in mode for c in 'wa+') else virtual_files.get(path,{}).get('data',bytearray())
    virtual_files[path]={'path':path,'mode':mode,'data':data,'offset':0,'handle':handle,'closed':False};virtual_files[handle]=virtual_files[path];virtual_file_events.append({'op':'fopen','path':path,'mode':mode,'handle':hex(handle)});p.x(0,handle)
def host_fclose():
    handle=p.x(0);f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'fclose for unknown virtual file {handle:#x}')
    f['closed']=True;virtual_file_events.append({'op':'fclose','path':f['path'],'bytes':len(f['data'])});p.x(0,0)
def host_fread():
    ptr,size,count,handle=[p.x(i) for i in range(4)];f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'fread for unknown virtual file {handle:#x}')
    n=min(size*count,len(f['data'])-f['offset']);n=max(0,n);payload=bytes(f['data'][f['offset']:f['offset']+n]);p.u.mem_write(ptr,payload);f['offset']+=n;got=n//size if size else 0;virtual_file_events.append({'op':'fread','path':f['path'],'bytes':n,'offset':f['offset']});p.x(0,got)
def host_fseek():
    handle,off,whence=p.x(0),p.x(1),p.x(2);f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'fseek for unknown virtual file {handle:#x}')
    signed=struct.unpack('<q',struct.pack('<Q',off))[0]
    f['offset']=signed if whence==0 else f['offset']+signed if whence==1 else len(f['data'])+signed if whence==2 else -1
    if f['offset']<0:raise RuntimeError(f'invalid virtual file seek {f["offset"]}')
    p.x(0,0)
def host_ftell():
    handle=p.x(0);f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'ftell for unknown virtual file {handle:#x}')
    p.x(0,f['offset'])
def host_setbuf():
    handle,buffer=p.x(0),p.x(1);f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'setbuf for unknown virtual file {handle:#x}')
    f['buffer']=buffer;virtual_file_events.append({'op':'setbuf','path':f['path'],'buffer':hex(buffer)})
def host_setvbuf():
    handle,buffer,mode,size=[p.x(i) for i in range(4)];f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'setvbuf for unknown virtual file {handle:#x}')
    f['buffer']=buffer;virtual_file_events.append({'op':'setvbuf','path':f['path'],'buffer':hex(buffer),'mode':mode,'size':size});p.x(0,0)
def host_fflush():
    handle=p.x(0);f=virtual_files.get(handle)
    if handle and f is None:raise RuntimeError(f'fflush for unknown virtual file {handle:#x}')
    virtual_file_events.append({'op':'fflush','path':f['path'] if f else None});p.x(0,0)
def host_fileno():
    handle=p.x(0);f=virtual_files.get(handle)
    if f is None:raise RuntimeError(f'fileno for unknown virtual file {handle:#x}')
    f['fd']=3;virtual_file_events.append({'op':'fileno','path':f['path'],'fd':3});p.x(0,3)
def host_fsync():
    fd=p.x(0);f=next((v for k,v in virtual_files.items() if isinstance(k,str) and v.get('fd')==fd),None)
    if f is None:raise RuntimeError(f'fsync for unknown virtual fd {fd}')
    virtual_file_events.append({'op':'fsync','path':f['path'],'fd':fd,'bytes':len(f['data'])});p.x(0,0)
p.callbacks.update({'fopen':host_fopen,'fopen64':host_fopen,'fclose':host_fclose,'fread':host_fread,'fseek':host_fseek,'fseek64':host_fseek,'ftell':host_ftell,'ftell64':host_ftell,'setbuf':host_setbuf,'setvbuf':host_setvbuf,'fflush':host_fflush,'fileno':host_fileno,'fsync':host_fsync,'fdatasync':host_fsync,'ferror':lambda:p.x(0,0),'feof':lambda:p.x(0,0)})
def host_fwrite():
    ptr,size,count,stream=[p.x(i) for i in range(4)];n=min(size*count,0x10000);payload=bytes(p.u.mem_read(ptr,n)) if ptr and n else b''
    f=virtual_files.get(stream)
    if f is not None:
        end=f['offset']+n
        if end>0x10000000:raise RuntimeError(f'virtual file write too large: {end}')
        if len(f['data'])<f['offset']:f['data'].extend(bytes(f['offset']-len(f['data'])))
        if len(f['data'])<end:f['data'].extend(bytes(end-len(f['data'])))
        f['data'][f['offset']:end]=payload;f['offset']=end;virtual_file_events.append({'op':'fwrite','path':f['path'],'size':size,'count':count,'bytes':n,'offset':f['offset'],'sha256':hashlib.sha256(payload).hexdigest()})
    else:stdio_events.append({'call':'fwrite','size':size,'count':count,'stream':hex(stream),'payload_hex':payload.hex(),'payload_text':payload.decode(errors='replace')})
    p.x(0,count)
p.callbacks['fwrite']=host_fwrite
def host_vfprintf():
    stream,fmt,va=p.x(0),p.x(1),p.x(2);fmt_raw=bytes(p.u.mem_read(fmt,4096)).split(b'\0',1)[0]
    va_raw=bytes(p.u.mem_read(va,32)) if va else b'';args=[]
    if len(va_raw)==32:
        stack,gr_top,vr_top,gr_offs,vr_offs=struct.unpack('<QQQii',va_raw);cursor=gr_top+gr_offs
        for i in range(3):
            ptr=p.get(cursor+8*i,'Q')[0];raw=bytes(p.u.mem_read(ptr,4096)).split(b'\0',1)[0] if ptr else b'';args.append({'ptr':hex(ptr),'text':raw.decode(errors='replace')})
    stdio_events.append({'call':'vfprintf','stream':hex(stream),'format_ptr':hex(fmt),'format':fmt_raw.decode(errors='replace'),'va_list_ptr':hex(va),'va_list_raw_hex':va_raw.hex(),'string_args':args});p.x(0,0)
p.callbacks['vfprintf']=host_vfprintf
def host_fputc():
    ch,stream=p.x(0),p.x(1);stdio_events.append({'call':'fputc','char':ch&0xff,'stream':hex(stream)});p.x(0,ch&0xff)
p.callbacks['fputc']=host_fputc
def host_vsnprintf():
    dst,capacity,fmtptr,va=p.x(0),p.x(1),p.x(2),p.x(3);fmt=bytes(p.u.mem_read(fmtptr,0x1000)).split(b'\0',1)[0].decode(errors='replace');rawva=bytes(p.u.mem_read(va,32));stack,gr_top,vr_top,gr_offs,vr_offs=struct.unpack('<QQQii',rawva);cursor=gr_top+gr_offs
    import re
    args=[];out=[];i=0
    while i<len(fmt):
        if fmt[i]!='%':out.append(fmt[i]);i+=1;continue
        if i+1<len(fmt) and fmt[i+1]=='%':out.append('%');i+=2;continue
        m=re.match(r'%([#0\- +]*)(\d*)(?:\.(\d+))?(?:ll|l|z)?([sdixXuup])',fmt[i:])
        if not m:raise RuntimeError(f'unsupported vsnprintf format at {fmt[i:]!r}')
        flags,width,precision,kind=m.groups();value=p.get(cursor,'Q')[0];cursor+=8
        if kind=='s':
            text=bytes(p.u.mem_read(value,0x10000)).split(b'\0',1)[0].decode(errors='replace') if value else '(null)'
        elif kind=='p':text=f'0x{value:x}'
        elif kind in 'xXu':text=(f'{value:x}' if kind=='x' else f'{value:X}' if kind=='X' else str(value))
        elif kind in 'di':text=str(struct.unpack('<q',struct.pack('<Q',value))[0])
        else:raise RuntimeError(f'unsupported vsnprintf specifier {kind}')
        if precision and kind=='s':text=text[:int(precision)]
        if width and len(text)<int(width):text=text.rjust(int(width),'0' if '0' in flags else ' ')
        out.append(text);args.append({'kind':kind,'raw':hex(value),'text':text});i+=m.end()
    text=''.join(out);encoded=text.encode();written=encoded[:max(0,capacity-1)] if capacity else b''
    if capacity:p.u.mem_write(dst,written+b'\0')
    stdio_events.append({'call':'vsnprintf','format':fmt,'args':args,'result':text,'capacity':capacity});p.x(0,len(encoded))
p.callbacks['vsnprintf']=host_vsnprintf
# Keep the native TextureObject loader path honest when its implementation
# asks the platform loader to enumerate mapped modules. Feed the exact ELF
# program headers for this probe's mapped DSO set and execute each guest
# callback natively; do not synthesize a module-match result.
dl_iterate_rows=[];dl_modules=[*[(name,path,rt.bases[name]) for name,path in rt.paths.items()],('libSPenCrayon2.so',lib,base),('libSPenRenderer.so',renderer,rbase)]
next_tls_id=1
for soname,path,module_base in dl_modules:
    blob=path.read_bytes();phoff=struct.unpack_from('<Q',blob,32)[0];phent,phnum=struct.unpack_from('<HH',blob,54)
    headers=[struct.unpack_from('<II6Q',blob,phoff+i*phent) for i in range(phnum)]
    tls=next((h for h in headers if h[0]==7),None);tls_data=0;tls_modid=0
    if tls:
        tls_modid=next_tls_id;next_tls_id+=1;tls_data=p.alloc(tls[6]) if tls[6] else 0
        if tls_data and tls[5]:p.u.mem_write(tls_data,blob[tls[2]:tls[2]+tls[5]])
    dl_iterate_rows.append({'name':soname.encode(),'base':module_base,'phdrs':b''.join(blob[phoff+i*phent:phoff+(i+1)*phent] for i in range(phnum)),'count':phnum,'tls_modid':tls_modid,'tls_data':tls_data})
dl_iterate_events=[];dl_state={};dl_return=0x200f30f0;p.u.mem_write(dl_return,bytes.fromhex('1f2003d5c0035fd6'))
def dl_iterate_next():
    idx=dl_state['index']
    if idx>=len(dl_iterate_rows):
        p.x(0,0);p.u.reg_write(R.UC_ARM64_REG_SP,dl_state['outer_sp']);p.u.reg_write(R.UC_ARM64_REG_PC,dl_state['outer_lr']);dl_state.clear();return
    row=dl_iterate_rows[idx];nameptr=p.alloc(len(row['name'])+1);p.u.mem_write(nameptr,row['name']+b'\0')
    phdrptr=p.alloc(len(row['phdrs']));p.u.mem_write(phdrptr,row['phdrs']);info=p.alloc(64)
    p.put(info,'8Q',row['base'],nameptr,phdrptr,row['count'],1,0,row['tls_modid'],row['tls_data'])
    dl_state.update(info=info,name=row['name'].decode());p.x(0,info);p.x(1,64);p.x(2,dl_state['data']);p.u.reg_write(R.UC_ARM64_REG_SP,(dl_state['outer_sp']-16)&~15);p.u.reg_write(R.UC_ARM64_REG_LR,dl_return);p.u.reg_write(R.UC_ARM64_REG_PC,dl_state['callback'])
def dl_iterate_phdr():
    cb,data=p.x(0),p.x(1)
    if not cb or not dl_iterate_rows:raise RuntimeError('dl_iterate_phdr missing guest callback or mapped ELF rows')
    dl_state.update(callback=cb,data=data,outer_lr=p.u.reg_read(R.UC_ARM64_REG_LR),outer_sp=p.u.reg_read(R.UC_ARM64_REG_SP),index=0);dl_iterate_next()
def dl_iterate_return(u,a,z,d):
    if a!=dl_return:return
    if not dl_state:raise RuntimeError('unexpected dl_iterate_phdr callback return')
    result=p.x(0);dl_iterate_events.append({'module':dl_state['name'],'callback':hex(dl_state['callback']),'result':result})
    if result:p.u.reg_write(R.UC_ARM64_REG_SP,dl_state['outer_sp']);p.u.reg_write(R.UC_ARM64_REG_PC,dl_state['outer_lr']);dl_state.clear()
    else:dl_state['index']+=1;dl_iterate_next()
p.u.hook_add(UC_HOOK_CODE,dl_iterate_return);p.callbacks['dl_iterate_phdr']=dl_iterate_phdr
rwlock_events=[];rwlocks={}
def rwlock_read():
    addr=p.x(0);state=rwlocks.setdefault(addr,{'readers':0,'writer':False})
    if state['writer']:raise RuntimeError(f'pthread_rwlock_rdlock conflicts with writer at {addr:#x}')
    state['readers']+=1;rwlock_events.append({'operation':'rdlock','lock':hex(addr),'reader_count':state['readers']});p.x(0,0)
def rwlock_write():
    addr=p.x(0);state=rwlocks.setdefault(addr,{'readers':0,'writer':False})
    if state['writer'] or state['readers']:raise RuntimeError(f'pthread_rwlock_wrlock conflicts at {addr:#x}: {state}')
    state['writer']=True;rwlock_events.append({'operation':'wrlock','lock':hex(addr)});p.x(0,0)
def rwlock_unlock():
    addr=p.x(0);state=rwlocks.get(addr)
    if state is None:raise RuntimeError(f'pthread_rwlock_unlock on unseen lock {addr:#x}')
    if state['writer']:state['writer']=False;op='unlock-writer'
    elif state['readers']:state['readers']-=1;op='unlock-reader'
    else:raise RuntimeError(f'pthread_rwlock_unlock on unlocked lock {addr:#x}')
    rwlock_events.append({'operation':op,'lock':hex(addr),'reader_count':state['readers']});p.x(0,0)
p.callbacks.update({'pthread_rwlock_rdlock':rwlock_read,'pthread_rwlock_wrlock':rwlock_write,'pthread_rwlock_unlock':rwlock_unlock})
getpid_sentinel=0x1337;p.callbacks['getpid']=lambda:(gl_trace.append({'name':'getpid','modeled_self_pid':getpid_sentinel}),p.x(0,getpid_sentinel))
syscall_events=[]
def host_syscall():
    nr=p.x(0)
    if nr!=270:raise RuntimeError(f'unsupported native syscall {nr}')
    pid,local_vec,local_count,remote_vec,remote_count,flags=[p.x(i) for i in range(1,7)]
    if pid!=getpid_sentinel or local_count!=1 or remote_count!=1 or flags!=0:raise RuntimeError(f'process_vm_readv outside single-thread self-copy boundary: {(pid,local_count,remote_count,flags)}')
    local_ptr,local_len=p.get(local_vec,'2Q');remote_ptr,remote_len=p.get(remote_vec,'2Q');n=min(local_len,remote_len)
    if n>0x100000:raise RuntimeError(f'process_vm_readv copy exceeds bound: {n}')
    payload=bytes(p.u.mem_read(remote_ptr,n));p.u.mem_write(local_ptr,payload)
    syscall_events.append({'syscall_nr':nr,'name':'process_vm_readv','pid':pid,'local_ptr':hex(local_ptr),'local_len':local_len,'remote_ptr':hex(remote_ptr),'remote_len':remote_len,'copied':n,'payload_sha256':hashlib.sha256(payload).hexdigest()});p.x(0,n)
p.callbacks['syscall']=host_syscall
gl_strings={0x1f00:b'Codex metadata adapter\0',0x1f01:b'virtual GLES\0',0x1f02:b'OpenGL ES 3.2\0',0x1f03:b'\0'}
gl_string_ptrs={k:p.alloc(len(v)) for k,v in gl_strings.items()}
for k,v in gl_strings.items():p.u.mem_write(gl_string_ptrs[k],v)
def gl_get_string():
    enum=p.x(0); ptr=gl_string_ptrs.get(enum,gl_string_ptrs[0x1f03]);gl_trace.append({'name':'glGetString','enum':enum,'result':hex(ptr)});p.x(0,ptr)
p.callbacks['glGetString']=gl_get_string
libc_calls=[]
def host_strstr():
    hay=bytes(p.u.mem_read(p.x(0),4096)).split(b'\0',1)[0]; needle=bytes(p.u.mem_read(p.x(1),256)).split(b'\0',1)[0]
    pos=hay.find(needle); result=p.x(0)+pos if pos>=0 else 0
    libc_calls.append({'name':'strstr','haystack':hay.decode(errors='replace'),'needle':needle.decode(errors='replace'),'result':hex(result)})
    p.x(0,result)
p.callbacks['strstr']=host_strstr
def host_strchr():
    ptr,ch=p.x(0),p.x(1)&0xff;raw=bytes(p.u.mem_read(ptr,4096)).split(b'\0',1)[0];index=raw.find(bytes([ch]));result=ptr+index if index>=0 else 0;libc_calls.append({'name':'strchr','text':raw.decode(errors='replace'),'char':ch,'result':hex(result)});p.x(0,result)
p.callbacks['strchr']=host_strchr
def host_strtof():
    import ctypes
    ptr,endptr=p.x(0),p.x(1);raw=bytes(p.u.mem_read(ptr,256)).split(b'\0',1)[0].decode(errors='replace').strip()
    token=raw.split()[0] if raw else '0'
    try:value=ctypes.c_float(float(token)).value
    except ValueError:value=0.0
    if endptr:p.put(endptr,'Q',ptr+len(token))
    p.f(0,value);libc_calls.append({'name':'strtof','text':raw,'value':value})
p.callbacks['strtof']=host_strtof
gl_next_id={'value':100}
def gl_gen_buffers():
    n=p.x(0);ptr=p.x(1);ids=list(range(gl_next_id['value'],gl_next_id['value']+n));gl_next_id['value']+=n
    if n:p.u.mem_write(ptr,struct.pack('<'+'I'*n,*ids))
    gl_trace.append({'name':'glGenBuffers','count':n,'ids':ids})
p.callbacks['glGenBuffers']=gl_gen_buffers
def gl_bind_buffer():gl_trace.append({'name':'glBindBuffer','target':p.x(0),'buffer':p.x(1)})
def gl_buffer_data():
    target,size,ptr,usage=[p.x(i) for i in range(4)];rawbytes=bytes(p.u.mem_read(ptr,min(size,0x100000))) if ptr and size else b''
    gl_trace.append({'name':'glBufferData','target':target,'size':size,'usage':usage,'data_sha256':hashlib.sha256(rawbytes).hexdigest() if rawbytes else None,'data_head_hex':rawbytes[:64].hex()})
p.callbacks['glBindBuffer']=gl_bind_buffer;p.callbacks['glBufferData']=gl_buffer_data
def gl_get_integer():
    enum,ptr=p.x(0),p.x(1)
    if enum==0x0d3a:
        values=[2048,2048];p.put(ptr,'2i',*values);gl_trace.append({'name':'glGetIntegerv','enum':enum,'values':values,'synthetic_metadata':True})
    else:
        value={0x8869:16,0x8872:16,0x821b:3,0x821c:2,0x0d33:2048}.get(enum,0);p.put(ptr,'I',value);gl_trace.append({'name':'glGetIntegerv','enum':enum,'value':value,'synthetic_metadata':enum in (0x0d33,)})
p.callbacks['glGetIntegerv']=gl_get_integer
p.callbacks['eglGetCurrentContext']=lambda:(gl_trace.append({'name':'eglGetCurrentContext','result':'0x200f4000'}),p.x(0,0x200f4000))
shader_source_rows=[]; shader_id={'value':200}
def gl_create_shader():
    kind=p.x(0);shader_id['value']+=1;gl_trace.append({'name':'glCreateShader','kind':kind,'id':shader_id['value']});p.x(0,shader_id['value'])
def gl_shader_source():
    sh,count,ptrarr,plen=[p.x(i) for i in range(4)];ptrs=struct.unpack('<'+'Q'*count,p.u.mem_read(ptrarr,8*count)) if count else ();sources=[]
    for ptr in ptrs:sources.append(bytes(p.u.mem_read(ptr,0x10000)).split(b'\0',1)[0].decode(errors='replace'))
    shader_source_rows.append({'shader':sh,'sources':sources});gl_trace.append({'name':'glShaderSource','shader':sh,'count':count,'sources':sources})
def gl_noop1(name):
    def f():gl_trace.append({'name':name,'args':[p.x(i) for i in range(4)]})
    return f
def gl_status(name):
    def f():
        obj,pname,out=p.x(0),p.x(1),p.x(2);status=0 if pname==0x8741 else (8 if pname==0x8b86 else (64 if pname==0x8b87 else 1));p.put(out,'I',status);gl_trace.append({'name':name,'object':obj,'pname':pname,'status':status})
    return f
def gl_create_program():
    shader_id['value']+=1;gl_trace.append({'name':'glCreateProgram','id':shader_id['value']});p.x(0,shader_id['value'])
import zlib
def host_crc32():
    crc,ptr,n=p.x(0),p.x(1),p.x(2);raw=bytes(p.u.mem_read(ptr,n)) if ptr and n else b'';p.x(0,zlib.crc32(raw,crc)&0xffffffff)
p.callbacks['crc32']=host_crc32
uniforms=[('uProjectionMatrix',0x8b5c),('pointPattern',0x8b5e),('paperTexture',0x8b5e),('bgTexture',0x8b5e),('inputColor',0x8b52),('uPaperScale',0x8b50),('uBgTextureScale',0x8b50),('isEnablePaper',0x8b56)]
def gl_get_active_uniform():
    program,index,bufsize,length_ptr,size_ptr,type_ptr,name_ptr=[p.x(i) for i in range(7)]
    nm,ty=uniforms[index] if 0<=index<len(uniforms) else ('',0);raw=nm.encode()+b'\0';p.u.mem_write(name_ptr,raw[:bufsize]);
    if length_ptr:p.put(length_ptr,'I',len(nm))
    if size_ptr:p.put(size_ptr,'I',1)
    if type_ptr:p.put(type_ptr,'I',ty)
    gl_trace.append({'name':'glGetActiveUniform','program':program,'index':index,'uniform':nm,'type':ty})
def gl_get_uniform_location():
    program,nameptr=p.x(0),p.x(1);nm=bytes(p.u.mem_read(nameptr,256)).split(b'\0',1)[0].decode(errors='replace');loc=uniforms.index(next((u for u in uniforms if u[0]==nm),('',0))) if nm in [u[0] for u in uniforms] else -1
    gl_trace.append({'name':'glGetUniformLocation','program':program,'uniform':nm,'location':loc});p.x(0,loc)
def gl_get_attrib_location():
    program,nameptr=p.x(0),p.x(1);nm=bytes(p.u.mem_read(nameptr,256)).split(b'\0',1)[0].decode(errors='replace');loc={'aRectPoint':0,'aPointInfo':1,'aBGVertex':2}.get(nm,-1)
    gl_trace.append({'name':'glGetAttribLocation','program':program,'attribute':nm,'location':loc});p.x(0,loc)
def host_strncpy_chk():
    dst,src,n,cap=p.x(0),p.x(1),min(p.x(2),p.x(3)),p.x(3);raw=bytes(p.u.mem_read(src,n));p.u.mem_write(dst,raw+b'\0'*max(0,n-len(raw)));p.x(0,dst)
p.callbacks['__strncpy_chk']=host_strncpy_chk
p.callbacks['__strncpy_chk2']=host_strncpy_chk
p.callbacks['__strlen_chk']=lambda:p.x(0,len(bytes(p.u.mem_read(p.x(0),4096)).split(b'\0',1)[0]))
def host_strcmp():
    a=bytes(p.u.mem_read(p.x(0),4096)).split(b'\0',1)[0];b=bytes(p.u.mem_read(p.x(1),4096)).split(b'\0',1)[0];p.x(0,(a>b)-(a<b))
p.callbacks['strcmp']=host_strcmp
def host_strncpy():
    dst,src,n=p.x(0),p.x(1),p.x(2); raw=bytes(p.u.mem_read(src,n)).split(b'\0',1)[0]; payload=raw+b'\0'*max(0,n-len(raw)); p.u.mem_write(dst,payload); p.x(0,dst)
p.callbacks['strncpy']=host_strncpy
clock_events=[]
def host_clock_gettime():
    clk,out=p.x(0),p.x(1); tick=len(clock_events); p.put(out,'qq',1700000000,tick*1000000); clock_events.append({'clock_id':clk,'timespec':hex(out),'tick':tick}); p.x(0,0)
p.callbacks['clock_gettime']=host_clock_gettime
p.callbacks.update({'glCreateShader':gl_create_shader,'glShaderSource':gl_shader_source,'glCompileShader':gl_noop1('glCompileShader'),'glGetShaderiv':gl_status('glGetShaderiv'),'glCreateProgram':gl_create_program,'glAttachShader':gl_noop1('glAttachShader'),'glLinkProgram':gl_noop1('glLinkProgram'),'glGetProgramiv':gl_status('glGetProgramiv'),'glGetActiveUniform':gl_get_active_uniform,'glGetUniformLocation':gl_get_uniform_location,'glGetAttribLocation':gl_get_attrib_location,'glDetachShader':gl_noop1('glDetachShader'),'glDeleteShader':gl_noop1('glDeleteShader'),'glDeleteProgram':gl_noop1('glDeleteProgram')})
# Forward GLES imports to a real surfaceless Mesa GLES3 context. The guest
# methods remain native; this context substitutes only host platform GL/EGL.
import inkpen_paint_native as ip
mesa=ip.GLES();gf=mesa.f;gl=mesa.gl;actual_gl_events=[]
def glfunc(name,ret,args):return ip.cfunc(gl,name,ret,args)
def gl_gen_names(name):
    count,out=p.x(0),p.x(1);ids=(C.c_uint*count)();glfunc(name,None,[C.c_int,C.POINTER(C.c_uint)])(count,ids)
    for i,value in enumerate(ids):p.put(out+4*i,'I',value)
    actual_gl_events.append({'name':name,'count':count,'ids':list(ids)})
def gl_gen_textures_actual():gl_gen_names('glGenTextures')
def gl_gen_buffers_actual():gl_gen_names('glGenBuffers')
def gl_gen_vaos_actual():gl_gen_names('glGenVertexArrays')
def gl_gen_samplers_actual():gl_gen_names('glGenSamplers')
def gl_gen_framebuffers_actual():gl_gen_names('glGenFramebuffers')
def gl_bind_framebuffer_actual():
    target,fbo=p.x(0),p.x(1);glfunc('glBindFramebuffer',None,[C.c_uint,C.c_uint])(target,fbo);actual_gl_events.append({'name':'glBindFramebuffer','target':target,'framebuffer':fbo})
def gl_framebuffer_texture_actual():
    vals=[p.x(i) for i in range(5)];glfunc('glFramebufferTexture2D',None,[C.c_uint,C.c_uint,C.c_uint,C.c_uint,C.c_int])(*vals);actual_gl_events.append({'name':'glFramebufferTexture2D','args':vals})
def gl_check_framebuffer_actual():
    target=p.x(0);status=glfunc('glCheckFramebufferStatus',C.c_uint,[C.c_uint])(target);p.x(0,status);actual_gl_events.append({'name':'glCheckFramebufferStatus','target':target,'status':status})
def gl_sampler_parameter_actual():
    sampler,pname,param=p.x(0),p.x(1),p.x(2)
    glfunc('glSamplerParameteri',None,[C.c_uint,C.c_uint,C.c_int])(sampler,pname,param)
    actual_gl_events.append({'name':'glSamplerParameteri','sampler':sampler,'pname':pname,'value':param})
def gl_bind_sampler_actual():
    unit,sampler=p.x(0),p.x(1)
    glfunc('glBindSampler',None,[C.c_uint,C.c_uint])(unit,sampler)
    actual_gl_events.append({'name':'glBindSampler','unit':unit,'sampler':sampler})
def gl_fence_sync_actual():
    condition,flags=p.x(0),p.x(1);sync=glfunc('glFenceSync',C.c_void_p,[C.c_uint,C.c_uint])(condition,flags);p.x(0,sync or 0);actual_gl_events.append({'name':'glFenceSync','condition':condition,'flags':flags,'sync':hex(sync or 0)})
def gl_client_wait_sync_actual():
    sync,flags,timeout=p.x(0),p.x(1),p.x(2);status=glfunc('glClientWaitSync',C.c_uint,[C.c_void_p,C.c_uint,C.c_uint64])(sync,flags,timeout);p.x(0,status);actual_gl_events.append({'name':'glClientWaitSync','sync':hex(sync),'flags':flags,'timeout':timeout,'status':status})
def gl_wait_sync_actual():
    sync,flags,timeout=p.x(0),p.x(1),p.x(2);glfunc('glWaitSync',None,[C.c_void_p,C.c_uint,C.c_uint64])(sync,flags,timeout);actual_gl_events.append({'name':'glWaitSync','sync':hex(sync),'flags':flags,'timeout':timeout})
def gl_delete_sync_actual():
    sync=p.x(0);glfunc('glDeleteSync',None,[C.c_void_p])(sync);actual_gl_events.append({'name':'glDeleteSync','sync':hex(sync)})
def gl_get_string_actual():
    enum=p.x(0);raw=mesa.gl_string(enum) or b'';buf=p.alloc(len(raw)+1);p.u.mem_write(buf,raw+b'\0');p.x(0,buf);actual_gl_events.append({'name':'glGetString','enum':enum,'value_sha256':hashlib.sha256(raw).hexdigest(),'value_bytes':len(raw),'value':raw.decode(errors='replace') if enum in (0x1f00,0x1f01,0x1f02,0x1f03) and len(raw)<256 else None})
def gl_get_integer_actual():
    enum,out=p.x(0),p.x(1);count=2 if enum==0x0d3a else 1;values=(C.c_int*count)();glfunc('glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(enum,values)
    p.put(out,('2i' if count==2 else 'i'),*list(values));actual_gl_events.append({'name':'glGetIntegerv','enum':enum,'values':list(values)})
def gl_shader_source_actual():
    sh,count,arr,lengths=[p.x(i) for i in range(4)];ptrs=struct.unpack('<'+'Q'*count,p.u.mem_read(arr,count*8)) if count else ();sources=[];bufs=[]
    for i,ptr in enumerate(ptrs):
        raw=bytes(p.u.mem_read(ptr,0x10000)).split(b'\0',1)[0];sources.append(raw);bufs.append(C.c_char_p(raw))
    src_array=(C.c_char_p*count)(*bufs);lens=None
    if lengths:lens=(C.c_int*count)(*p.get(lengths,'i'*count))
    gf['glShaderSource'](sh,count,src_array,lens);actual_gl_events.append({'name':'glShaderSource','shader':sh,'sources_sha256':[hashlib.sha256(x).hexdigest() for x in sources]})
def gl_get_active_actual(kind):
    program,index,bufsize,lengthp,sizep,typep,namep=[p.x(i) for i in range(7)];namebuf=C.create_string_buffer(max(1,bufsize));length=C.c_int();size=C.c_int();typ=C.c_uint()
    glfunc(kind,None,[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_char_p])(program,index,bufsize,C.byref(length),C.byref(size),C.byref(typ),namebuf)
    raw=namebuf.value;p.u.mem_write(namep,raw[:bufsize]+(b'\0' if len(raw)<bufsize else b''));p.put(lengthp,'I',length.value);p.put(sizep,'I',size.value);p.put(typep,'I',typ.value);actual_gl_events.append({'name':kind,'program':program,'index':index,'uniform_or_attribute':raw.decode(errors='replace'),'type':typ.value})
def gl_get_program_binary_actual():
    program,capacity,lengthp,formatp,binaryp=[p.x(i) for i in range(5)];length=C.c_int();fmt=C.c_uint();data=C.create_string_buffer(capacity);glfunc('glGetProgramBinary',None,[C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p])(program,capacity,C.byref(length),C.byref(fmt),data)
    if lengthp:p.put(lengthp,'i',length.value)
    if formatp:p.put(formatp,'I',fmt.value)
    if binaryp and length.value:p.u.mem_write(binaryp,data.raw[:length.value])
    actual_gl_events.append({'name':'glGetProgramBinary','program':program,'capacity':capacity,'length':length.value,'format':fmt.value,'sha256':hashlib.sha256(data.raw[:length.value]).hexdigest() if length.value else None})
def gl_program_binary_actual():
    program,fmt,ptr,length=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,length)) if ptr and 0<=length<=0x1000000 else b'';buf=C.create_string_buffer(raw);glfunc('glProgramBinary',None,[C.c_uint,C.c_uint,C.c_void_p,C.c_int])(program,fmt,C.cast(buf,C.c_void_p),length);actual_gl_events.append({'name':'glProgramBinary','program':program,'format':fmt,'length':length,'sha256':hashlib.sha256(raw).hexdigest()})
def gl_uniform_matrix_actual():
    loc,count,transpose,ptr=[p.x(i) for i in range(4)];vals=p.get(ptr,'f'*(16*count));gf['glUniformMatrix4fv'](loc,count,transpose,(C.c_float*len(vals))(*vals));actual_gl_events.append({'name':'glUniformMatrix4fv','location':loc,'count':count})
def gl_uniform1i_actual():gf['glUniform1i'](p.x(0),p.x(1));actual_gl_events.append({'name':'glUniform1i','location':p.x(0),'value':p.x(1)})
def gl_uniform1f_actual():glfunc('glUniform1f',None,[C.c_int,C.c_float])(p.x(0),p.f(0));actual_gl_events.append({'name':'glUniform1f','location':p.x(0),'value':p.f(0)})
def gl_uniform2f_actual():glfunc('glUniform2f',None,[C.c_int,C.c_float,C.c_float])(p.x(0),p.f(0),p.f(1));actual_gl_events.append({'name':'glUniform2f','location':p.x(0),'values':[p.f(0),p.f(1)]})
def gl_uniform4f_actual():gf['glUniform4f'](p.x(0),*[p.f(i) for i in range(4)]);actual_gl_events.append({'name':'glUniform4f','location':p.x(0),'values':[p.f(i) for i in range(4)]})
def gl_uniform2fv_actual():
    loc,count,ptr=p.x(0),p.x(1),p.x(2);vals=p.get(ptr,'f'*(2*count));glfunc('glUniform2fv',None,[C.c_int,C.c_int,C.POINTER(C.c_float)])(loc,count,(C.c_float*len(vals))(*vals));actual_gl_events.append({'name':'glUniform2fv','location':loc,'count':count,'values':vals})
def gl_uniform4fv_actual():
    loc,count,ptr=p.x(0),p.x(1),p.x(2);vals=p.get(ptr,'f'*(4*count));glfunc('glUniform4fv',None,[C.c_int,C.c_int,C.POINTER(C.c_float)])(loc,count,(C.c_float*len(vals))(*vals));actual_gl_events.append({'name':'glUniform4fv','location':loc,'count':count,'values':vals})
def gl_tex_image_actual():
    vals=[p.x(i) for i in range(8)];sp=p.u.reg_read(R.UC_ARM64_REG_SP);ptr=p.get(sp,'Q')[0];channels={0x1903:1,0x1908:4,0x1906:1,0x8227:2}.get(vals[6],4);n=vals[3]*vals[4]*channels
    if n>0x10000000:raise RuntimeError(f'glTexImage2D exceeds adapter bound: {n}')
    raw=bytes(p.u.mem_read(ptr,n)) if ptr and n else None;data=C.create_string_buffer(raw) if raw else None
    gf['glTexImage2D'](*vals,C.cast(data,C.c_void_p) if data else None);actual_gl_events.append({'name':'glTexImage2D','args':vals,'pixels':hex(ptr),'bytes':n if raw else 0})
def gl_tex_sub_actual():
    vals=[p.x(i) for i in range(8)];sp=p.u.reg_read(R.UC_ARM64_REG_SP);ptr=p.get(sp,'Q')[0];n=vals[4]*vals[5]*{0x1903:1,0x1908:4,0x1906:1,0x8227:2}.get(vals[6],4);raw=bytes(p.u.mem_read(ptr,n)) if ptr and n else None;data=C.create_string_buffer(raw) if raw else None
    glfunc('glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(*vals,C.cast(data,C.c_void_p) if data else None);actual_gl_events.append({'name':'glTexSubImage2D','args':vals,'bytes':n if raw else 0,'data_sha256':hashlib.sha256(raw).hexdigest() if raw else None})
def gl_draw_arrays_actual():
    mode,first,count=p.x(0),p.x(1),p.x(2);gf['glDrawArrays'](mode,first,count);actual_gl_events.append({'name':'glDrawArrays','mode':mode,'first':first,'count':count})
def gl_draw_arrays_instanced_actual():
    mode,first,count,instances=[p.x(i) for i in range(4)];gf['glDrawArraysInstanced'](mode,first,count,instances);actual_gl_events.append({'name':'glDrawArraysInstanced','mode':mode,'first':first,'count':count,'instances':instances})
def gl_draw_elements_actual():
    mode,count,typ,ptr=[p.x(i) for i in range(4)];glfunc('glDrawElements',None,[C.c_uint,C.c_int,C.c_uint,C.c_void_p])(mode,count,typ,ptr);actual_gl_events.append({'name':'glDrawElements','mode':mode,'count':count,'type':typ,'indices':hex(ptr)})
def gl_draw_elements_instanced_actual():
    mode,count,typ,ptr,instances=[p.x(i) for i in range(5)];glfunc('glDrawElementsInstanced',None,[C.c_uint,C.c_int,C.c_uint,C.c_void_p,C.c_int])(mode,count,typ,ptr,instances);actual_gl_events.append({'name':'glDrawElementsInstanced','mode':mode,'count':count,'type':typ,'indices':hex(ptr),'instances':instances})
def gl_bind_buffer_actual():
    target,buffer=p.x(0),p.x(1);gf['glBindBuffer'](target,buffer);actual_gl_events.append({'name':'glBindBuffer','target':target,'buffer':buffer})
def gl_vertex_attrib_divisor_actual():
    index,divisor=p.x(0),p.x(1);glfunc('glVertexAttribDivisor',None,[C.c_uint,C.c_uint])(index,divisor);actual_gl_events.append({'name':'glVertexAttribDivisor','index':index,'divisor':divisor})
def gl_buffer_data_actual():
    target,size,ptr,usage=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size)) if ptr and 0<=size<=0x10000000 else None;data=C.create_string_buffer(raw) if raw else None;gf['glBufferData'](target,size,C.cast(data,C.c_void_p) if data else None,usage);actual_gl_events.append({'name':'glBufferData','target':target,'size':size,'usage':usage,'data_sha256':hashlib.sha256(raw).hexdigest() if raw else None})
def gl_vertex_attrib_actual():
    idx,sz,ty,norm,stride,ptr=[p.x(i) for i in range(6)];gf['glVertexAttribPointer'](idx,sz,ty,norm,stride,C.c_void_p(ptr));actual_gl_events.append({'name':'glVertexAttribPointer','index':idx,'size':sz,'type':ty,'normalized':norm,'stride':stride,'pointer':hex(ptr)})
egl_current=ip.cfunc(mesa.egl,'eglGetCurrentContext',C.c_void_p,[])
actual_callbacks={'glGetString':gl_get_string_actual,'glGetIntegerv':gl_get_integer_actual,'glGetError':lambda:p.x(0,gf['glGetError']()),'eglGetCurrentContext':lambda:p.x(0,egl_current() or 0),'glGenTextures':gl_gen_textures_actual,'glGenBuffers':gl_gen_buffers_actual,'glGenVertexArrays':gl_gen_vaos_actual,'glGenSamplers':gl_gen_samplers_actual,'glGenFramebuffers':gl_gen_framebuffers_actual,'glBindFramebuffer':gl_bind_framebuffer_actual,'glFramebufferTexture2D':gl_framebuffer_texture_actual,'glCheckFramebufferStatus':gl_check_framebuffer_actual,'glSamplerParameteri':gl_sampler_parameter_actual,'glBindSampler':gl_bind_sampler_actual,'glFlush':lambda:(glfunc('glFlush',None,[])(),actual_gl_events.append({'name':'glFlush'})),'glFinish':lambda:(glfunc('glFinish',None,[])(),actual_gl_events.append({'name':'glFinish'})),'glFenceSync':gl_fence_sync_actual,'glClientWaitSync':gl_client_wait_sync_actual,'glWaitSync':gl_wait_sync_actual,'glDeleteSync':gl_delete_sync_actual,'glCreateShader':lambda:p.x(0,gf['glCreateShader'](p.x(0))),'glShaderSource':gl_shader_source_actual,'glCompileShader':lambda:gf['glCompileShader'](p.x(0)),'glGetShaderiv':lambda:(lambda v:(glfunc('glGetShaderiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)])(p.x(0),p.x(1),C.byref(v)),p.put(p.x(2),'I',v.value),actual_gl_events.append({'name':'glGetShaderiv','shader':p.x(0),'pname':p.x(1),'value':v.value})))(C.c_int()),'glGetProgramiv':lambda:(lambda v:(glfunc('glGetProgramiv',None,[C.c_uint,C.c_uint,C.POINTER(C.c_int)])(p.x(0),p.x(1),C.byref(v)),p.put(p.x(2),'I',v.value),actual_gl_events.append({'name':'glGetProgramiv','program':p.x(0),'pname':p.x(1),'value':v.value})))(C.c_int()),'glCreateProgram':lambda:p.x(0,gf['glCreateProgram']()),'glBindAttribLocation':lambda:(lambda prog,idx,nm:(glfunc('glBindAttribLocation',None,[C.c_uint,C.c_uint,C.c_char_p])(prog,idx,nm),actual_gl_events.append({'name':'glBindAttribLocation','program':prog,'index':idx,'name_value':nm.decode(errors='replace')})))(p.x(0),p.x(1),bytes(p.u.mem_read(p.x(2),256)).split(b'\0',1)[0]),'glAttachShader':lambda:gf['glAttachShader'](p.x(0),p.x(1)),'glLinkProgram':lambda:gf['glLinkProgram'](p.x(0)),'glGetActiveUniform':lambda:gl_get_active_actual('glGetActiveUniform'),'glGetActiveAttrib':lambda:gl_get_active_actual('glGetActiveAttrib'),'glGetProgramBinary':gl_get_program_binary_actual,'glProgramBinary':gl_program_binary_actual,'glGetUniformLocation':lambda:(lambda name:(lambda loc:(p.x(0,loc),actual_gl_events.append({'name':'glGetUniformLocation','uniform':name,'location':loc})))(gf['glGetUniformLocation'](p.x(0),name.encode())))(bytes(p.u.mem_read(p.x(1),1024)).split(b'\0',1)[0].decode(errors='replace')),'glGetAttribLocation':lambda:(lambda name:p.x(0,glfunc('glGetAttribLocation',C.c_int,[C.c_uint,C.c_char_p])(p.x(0),name.encode())))(bytes(p.u.mem_read(p.x(1),1024)).split(b'\0',1)[0].decode(errors='replace')),'glUseProgram':lambda:gf['glUseProgram'](p.x(0)),'glUniformMatrix4fv':gl_uniform_matrix_actual,'glUniform1i':gl_uniform1i_actual,'glUniform1f':gl_uniform1f_actual,'glUniform2f':gl_uniform2f_actual,'glUniform2fv':gl_uniform2fv_actual,'glUniform4f':gl_uniform4f_actual,'glUniform4fv':gl_uniform4fv_actual,'glBindTexture':lambda:gf['glBindTexture'](p.x(0),p.x(1)),'glTexParameteri':lambda:gf['glTexParameteri'](p.x(0),p.x(1),p.x(2)),'glTexImage2D':gl_tex_image_actual,'glTexSubImage2D':gl_tex_sub_actual,'glActiveTexture':lambda:gf['glActiveTexture'](p.x(0)),'glBindBuffer':gl_bind_buffer_actual,'glBufferData':gl_buffer_data_actual,'glEnableVertexAttribArray':lambda:gf['glEnableVertexAttribArray'](p.x(0)),'glDisableVertexAttribArray':lambda:glfunc('glDisableVertexAttribArray',None,[C.c_uint])(p.x(0)),'glVertexAttribPointer':gl_vertex_attrib_actual,'glVertexAttribDivisor':gl_vertex_attrib_divisor_actual,'glDrawArrays':gl_draw_arrays_actual,'glDrawArraysInstanced':gl_draw_arrays_instanced_actual,'glDrawElements':gl_draw_elements_actual,'glDrawElementsInstanced':gl_draw_elements_instanced_actual,'glViewport':lambda:gf['glViewport'](*[p.x(i) for i in range(4)]),'glEnable':lambda:gf['glEnable'](p.x(0)),'glDisable':lambda:gf['glDisable'](p.x(0)),'glBlendEquationSeparate':lambda:gf['glBlendEquationSeparate'](p.x(0),p.x(1)),'glBlendFuncSeparate':lambda:gf['glBlendFuncSeparate'](*[p.x(i) for i in range(4)]),'glClearColor':lambda:gf['glClearColor'](*[p.f(i) for i in range(4)]),'glClear':lambda:gf['glClear'](p.x(0)),'glInvalidateFramebuffer':lambda:(lambda target,count,ptr,atts:(glfunc('glInvalidateFramebuffer',None,[C.c_uint,C.c_int,C.POINTER(C.c_uint)])(target,count,(C.c_uint*count)(*atts)),actual_gl_events.append({'name':'glInvalidateFramebuffer','target':target,'attachments':atts})))(p.x(0),p.x(1),p.x(2),p.get(p.x(2),'I'*p.x(1))), 'glPixelStorei':lambda:gf['glPixelStorei'](p.x(0),p.x(1)),'glColorMask':lambda:(glfunc('glColorMask',None,[C.c_ubyte,C.c_ubyte,C.c_ubyte,C.c_ubyte])(*[p.x(i) for i in range(4)]),actual_gl_events.append({'name':'glColorMask','values':[p.x(i) for i in range(4)]}))}
p.callbacks.update(actual_callbacks)
p.callbacks.update(actual_callbacks)
rt_init_trace=[]
rt_methods=('SPen::Crayon2StrokeDrawableRTV1::Init()','SPen::Crayon2StrokeDrawableRTV1::buildTextures(int, int)','SPen::Crayon2StrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)','SPen::Crayon2StrokeDrawableRTV1::setTextures(unsigned int, unsigned int)','SPen::Crayon2StrokeDrawableRTV1::Update(bool)','SPen::Crayon2StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)')
rt_method_addrs={p.syms[n]:n for n in rt_methods if n in p.syms}
rt_method_addrs[p.syms[add_sym]]=add_sym
rt_import_addrs={a:n for a,n in p.plt.items()}
def trace_rt_init(u,a,z,d):
    if a in rt_method_addrs:
        row={'type':'native_method','name':rt_method_addrs[a],'address':hex(a),'rva':hex(a-base) if a>=base else None,'x0_x4':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(5)]}
        if a==p.syms[add_sym]:row['attribute_raw_args']={'w1':u.reg_read(R.UC_ARM64_REG_W1),'w2':u.reg_read(R.UC_ARM64_REG_W2),'w3':u.reg_read(R.UC_ARM64_REG_W3),'w4':u.reg_read(R.UC_ARM64_REG_W4)}
        rt_init_trace.append(row)
    if a in rt_import_addrs:
        name=rt_import_addrs[a];row={'type':'plt','name':name,'address':hex(a)}
        if any(token in name for token in ('CreateBitmap','SPSubBitmapRT','GetRendererDeviceCaps','CreateTextureObject')):
            sp=u.reg_read(R.UC_ARM64_REG_SP);row['args_x0_x7']=[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8)];row['stack_args_8_9_hex']=bytes(u.mem_read(sp,16)).hex()
        rt_init_trace.append(row)
invalid_accesses=[]
def invalid_mem(u,access,address,size,value,d):
    invalid_accesses.append({'access':access,'address':hex(address),'size':size,'value':hex(value),'pc':hex(u.reg_read(R.UC_ARM64_REG_PC))});return False
invalid_hook=p.u.hook_add(UC_HOOK_MEM_INVALID,invalid_mem)
rt_init_hook=p.u.hook_add(UC_HOOK_CODE,trace_rt_init)
parameter_event_start=len(gl_trace)
parameter_actual_event_start=len(actual_gl_events)
renderer_parameters_result=call('SPen::OpenGLRenderer::InitParameters()',[],limit=2000000)
renderer_parameter_gl_calls=gl_trace[parameter_event_start:]
renderer_parameter_actual_gl_calls=actual_gl_events[parameter_actual_event_start:]
rt_before_init=bytes(p.u.mem_read(p.get(drawable+16,'Q')[0],0x200)).hex()
try:
    init_result=call('SPen::Crayon2StrokeDrawableRTV1::Init()', [p.get(drawable+16,'Q')[0]],limit=2000000); init_error=None; init_cause=None
except Exception as e:
    init_result=None; init_error=str(e); init_cause=repr(e.__cause__)
p.u.hook_del(rt_init_hook);p.u.hook_del(invalid_hook)
rt_after_init=bytes(p.u.mem_read(p.get(drawable+16,'Q')[0],0x200)).hex()
case=os.environ.get('CRAYON_CASE','base'); stroke_size=8.0 if case=='size8' else 4.0
setting_before_override=bytes(p.u.mem_read(settings,16)).hex()
p.put(settings,'f',stroke_size)
host_setting_write={'target':'factory-created Crayon2Data setting record','offset':0,'field_type':'f32','value':stroke_size,'value_f32_bits':struct.pack('<f',stroke_size).hex(),'record_before_prefix_hex':setting_before_override,'record_after_prefix_hex':bytes(p.u.mem_read(settings,16)).hex(),'purpose':'runner host write supplying the renderer setting; not a Drawing-side setting update'}
p.put(drawable+32,'Q',p.pen_canvas)
class BionicRandom:
    def seed(self,value):
        self.state=[value & 0xffffffff]
        for _ in range(1,31):
            prev=self.state[-1]; nxt=16807*(prev%127773)-2836*(prev//127773)
            if nxt<=0:nxt+=0x7fffffff
            self.state.append(nxt)
        self.front,self.rear=3,0
        for _ in range(310):self.next()
    def next(self):
        total=(self.state[self.front]+self.state[self.rear])&0xffffffff
        self.state[self.front]=total; value=(total>>1)&0x7fffffff
        self.front=(self.front+1)%31; self.rear=(self.rear+1)%31
        return value
rng=BionicRandom();random_calls=[];rng_mode=os.environ.get('CRAYON_RNG','bionic-type3')
def do_srand():
    seed=p.x(0);rng.seed(seed);random_calls.append({'fn':'srand','seed':seed})
def do_rand():
    value=0 if rng_mode=='zero' else rng.next();random_calls.append({'fn':'rand','value':value});p.x(0,value)
p.callbacks['srand']=do_srand;p.callbacks['rand']=do_rand
scalar=[]
for tool in (0,1,2,3,4):
    for pressure in (0.0,0.25,0.75,1.0,1.25):
        call('SPen::Crayon2DrawableGLBase::getAdjustedPressure(SPen::MotionEvent::Tooltype, float)',[drawable,tool],[pressure])
        scalar.append({'helper':'getAdjustedPressure','tool_type':tool,'input_f32_bits':struct.pack('<f',pressure).hex(),'output_f32_bits':struct.pack('<I',p.u.reg_read(R.UC_ARM64_REG_S0)).hex(),'output':p.f(0)})
for length,size,pressure in ((0.0,4.0,.5),(1.0,4.0,.5),(16.0,4.0,.5),(16.0,8.0,1.0),(100.0,4.0,.75)):
    call('SPen::Crayon2DrawableGLBase::getRepeat(float, float, float)',[drawable],[length,size,pressure])
    scalar.append({'helper':'getRepeat','length_f32_bits':struct.pack('<f',length).hex(),'size_f32_bits':struct.pack('<f',size).hex(),'pressure_f32_bits':struct.pack('<f',pressure).hex(),'output_w0':p.x(0)})

# Build one Model ObjectStroke via the pinned setters, then run Crayon2's real
# saved-object RedrawPen caller through its native draw callbacks. This is a
# direct saved-object playback route, not the Drawing entry.
obj=p.alloc(256);impl=p.alloc(512)
call('SPen::ObjectStroke::ObjectStroke()',[obj]);call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl,obj]);p.put(obj+32,'Q',impl)
name=p.alloc(32);call('SPen::String::String()',[name]);call('SPen::String::Construct()',[name])
buf=p.alloc(128);pen_name=b'com.samsung.android.sdk.pen.pen.preload.Crayon2\0';p.u.mem_write(buf,pen_name);call('SPen::String::Set(char const*)',[name,buf]);call('SPen::ObjectStroke::SetPenName(SPen::String const*)',[obj,name])
saved_color_argb=0xff2468ac
call('SPen::ObjectStroke::SetPenSize(float)',[obj],[stroke_size]);call('SPen::ObjectStroke::SetColor(unsigned int)',[obj,saved_color_argb]);call('SPen::ObjectStroke::SetToolType(SPen::ToolType)',[obj,2]);call('SPen::ObjectStroke::SetStrokeType(SPen::StrokeType)',[obj,0])
points=[(1.,-2.),(3.,1.),(6.,4.)];pressures=[.3,.6,.85 if case!='pressure' else .35];times=[300,315,340];tilts=[.12,.32,.55];orientations=[.2,.35,.55];n=len(points)
ptrs=[p.alloc(8*n),p.alloc(4*n),p.alloc(4*n),p.alloc(4*n),p.alloc(4*n)]
p.u.mem_write(ptrs[0],struct.pack('<'+'f'*(2*n),*(x for pt in points for x in pt)));p.u.mem_write(ptrs[1],struct.pack('<'+'f'*n,*pressures));p.u.mem_write(ptrs[2],struct.pack('<'+'i'*n,*times));p.u.mem_write(ptrs[3],struct.pack('<'+'f'*n,*tilts));p.u.mem_write(ptrs[4],struct.pack('<'+'f'*n,*orientations))
call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,*ptrs,n])
if call('SPen::ObjectStroke::GetPointCount() const',[obj])!=n:raise RuntimeError('Crayon2 test ObjectStroke point setter did not retain count')
rect=p.alloc(16);p.u.mem_write(rect,struct.pack('<4f',0.,0.,100.,100.))
draw_name='SPen::Crayon2StrokeDrawableGLV1::Draw(SPen::MotionEvent const*, SPen::RectF*)'
redraw_name='SPen::Crayon2StrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)'
handoff=[];dispatch_targets=[];draw_addr=p.syms[draw_name];draw_thunk='non-virtual thunk to '+draw_name;draw_thunk_addr=p.syms[draw_thunk]
line_name='SPen::Crayon2StrokeDrawableGL::drawLine(float, float, float, SPen::RectF*)'; point_name='SPen::Crayon2StrokeDrawableGL::drawPoint(SPen::SmPoint*, float, SPen::RectF*)'; endline_name='SPen::Crayon2StrokeDrawableGL::drawEndLine(float, float, float, SPen::RectF*)'; geometry=[]; flow=[]; geom_addrs={p.syms[line_name]:line_name,p.syms[point_name]:point_name,p.syms[endline_name]:endline_name}
getter_names=('SPen::ObjectStroke::GetPointCount() const','SPen::ObjectStroke::GetPoint() const','SPen::ObjectStroke::GetPressure() const','SPen::ObjectStroke::GetTimeStamp() const','SPen::ObjectStroke::GetTilt() const','SPen::ObjectStroke::GetOrientation() const','SPen::ObjectStroke::GetPenSize() const','SPen::ObjectStroke::GetToolType() const','SPen::ObjectStroke::GetPenName() const')
getter_addrs={p.syms[n]:n for n in getter_names if n in p.syms};getter_events=[];getter_returns=[];pending_getters={}
playback_name='SPen::Crayon2StrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'
playback_addr=p.syms[playback_name];playback_thunk='non-virtual thunk to '+playback_name;playback_thunk_addr=p.syms[playback_thunk]
def stop_at_draw(u,a,sz,d):
    if a in getter_addrs:
        row={'getter':getter_addrs[a],'entry':hex(a),'receiver':hex(p.x(0)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'args_x0_x3':[hex(p.x(i)) for i in range(4)]}
        getter_events.append(row);pending_getters.setdefault(u.reg_read(R.UC_ARM64_REG_LR),[]).append(row)
    if a in pending_getters and pending_getters[a]:
        row=pending_getters[a].pop(0)
        ret={'getter':row['getter'],'return_pc':hex(a),'x0':hex(p.x(0)),'s0_f32_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex()}
        array_sizes={'SPen::ObjectStroke::GetPoint() const':8*n,'SPen::ObjectStroke::GetPressure() const':4*n,'SPen::ObjectStroke::GetTimeStamp() const':4*n,'SPen::ObjectStroke::GetTilt() const':4*n,'SPen::ObjectStroke::GetOrientation() const':4*n}
        if row['getter'] in array_sizes:
            if not p.x(0):raise RuntimeError(f"{row['getter']} returned a null source array")
            ret['array_bytes_hex']=bytes(u.mem_read(p.x(0),array_sizes[row['getter']])).hex()
        getter_returns.append(ret)
    if base+0x2571c <= a < base+0x25d2c: flow.append(hex(a-base))
    if a==base+0x25df0:dispatch_targets.append({'callsite':hex(a),'target':hex(p.x(8)),'vptr':hex(p.get(drawable,'Q')[0])})
    if a in (playback_addr,playback_thunk_addr):
        handoff.append({'entry':playback_name if a==playback_addr else playback_thunk,'allocated_drawable':hex(drawable),'drawable':hex(p.x(0)),'motion_event':hex(p.x(1)),'rect':hex(p.x(2)),'rt_slot':hex(p.get(p.x(0)+16,'Q')[0]),'canvas_slot':hex(p.get(p.x(0)+32,'Q')[0]),'x1_nonnull':bool(p.x(1)),'x2_nonnull':bool(p.x(2))})
    if a in geom_addrs:
        row={'entry':geom_addrs[a],'receiver':hex(p.x(0)),'x1':hex(p.x(1)),'s0_s2_f32_bits':[struct.pack('<I',u.reg_read(getattr(R,f'UC_ARM64_REG_S{i}'))).hex() for i in range(3)],'s0_s2':[p.f(i) for i in range(3)],'x8_x9':[hex(p.x(i)) for i in (8,9)]}
        if a==p.syms[point_name]: row['point_xy_f32_bits']=bytes(u.mem_read(p.x(1),8)).hex()
        row['x2_rect']=hex(p.x(2))
        geometry.append(row)
# Capture the causal native GL drawPoint -> RT AddPoint handoff and native
# RT object bytes around the complete saved redraw. AddPoint's ABI is from the
# exported symbol; retain raw coordinate/scalar/integer arguments before any
# renderer boundary.
rt_obj=p.get(drawable+16,'Q')[0]
rt_before=bytes(p.u.mem_read(rt_obj,0x200))
allocations=[]; alloc_active={'on':False}; previous_tracked_alloc=p.alloc
def tracked_alloc(n):
    a=previous_tracked_alloc(n)
    if alloc_active['on']: allocations.append({'address':a,'size':n})
    return a
p.alloc=tracked_alloc
add_returns=[]; pending_add_returns=[]; vector_last={'end':0,'begin':0,'bytes':b''}
add_name='SPen::Crayon2StrokeDrawableRTV1::AddPoint(SPen::Vector2<float>*, float, int)'
add_addr=p.syms[add_name]
add_events=[]
def capture_packed_entry(u,a,sz,d):
    if a==add_addr:
        vp=p.x(0); vec=p.x(1)
        pending_add_returns.append(u.reg_read(R.UC_ARM64_REG_LR))
        parent=[e for e in geometry if e['entry']==point_name][-1]
        add_events.append({'receiver':hex(vp),'vptr':hex(p.get(vp,'Q')[0]),'vector_ptr':hex(vec),'xy_f32_bits':bytes(u.mem_read(vec,8)).hex(),'xy':[struct.unpack('<2f',bytes(u.mem_read(vec,8)))][0],'float_s0_bits':struct.pack('<I',u.reg_read(R.UC_ARM64_REG_S0)).hex(),'float_s0':p.f(0),'x2_integer':p.x(2),'drawpoint_ordinal':sum(e['entry']==point_name for e in geometry),'drawpoint_input_xy_f32_bits':parent['point_xy_f32_bits'],'drawpoint_input_floats_f32_bits':parent['s0_s2_f32_bits'],'lr':hex(u.reg_read(R.UC_ARM64_REG_LR))})
def capture_add_return(u,a,sz,d):
    if pending_add_returns and a==pending_add_returns[0]:
        pending_add_returns.pop(0)
        vector_obj=p.get(rt_obj+80,'Q')[0]
        desc=p.get(vector_obj,'3Q') if vector_obj else (0,0,0)
        begin,end,capacity=desc
        if not (begin<=end<=capacity) or end-begin>1_000_000: raise RuntimeError(f'Crayon2 packed vector descriptor invalid: {desc!r}')
        payload=bytes(u.mem_read(begin,end-begin)) if begin and end>begin else b''
        old_bytes=vector_last['bytes']
        if len(payload)!=len(old_bytes)+24: raise RuntimeError(f'Crayon2 packed vector grew by {len(payload)-len(old_bytes)} bytes, expected one 24-byte record')
        delta=payload[-24:]
        vector_last.update({'begin':begin,'end':end,'bytes':payload})
        add_returns.append({'return_pc':hex(a),'vector_object':hex(vector_obj),'descriptor':[hex(v) for v in desc],'buffer_byte_count':len(payload),'buffer_sha256':hashlib.sha256(payload).hexdigest(),'record_hex':delta.hex(),'record_f32':[struct.unpack('<6f',delta)] [0]})
global_reads=[]
tracked_globals=[('g_Crayon2PointPtn',base+0x11698,300),('g_Crayon2BgPtn',base+0x117c4,22500),('g_Crayon2RandomX',base+0x3c278,200),('g_Crayon2RandomY',base+0x3c340,200)]
def capture_global_read(u,access,address,size,value,d):
    for n,start,length in tracked_globals:
        if start<=address<start+length:
            global_reads.append({'symbol':n,'address':hex(address),'offset':address-start,'size':size,'pc':hex(u.reg_read(R.UC_ARM64_REG_PC)-base)})
            break
read_hook=p.u.hook_add(UC_HOOK_MEM_READ,capture_global_read)
hook=p.u.hook_add(UC_HOOK_CODE,stop_at_draw)
packed_hook=p.u.hook_add(UC_HOOK_CODE,capture_packed_entry)
return_hook=p.u.hook_add(UC_HOOK_CODE,capture_add_return)
# Preserve the native CanvasProbe task acceptance boundary so the Crayon2
# PenReturnCallback worker remains deferred until we explicitly run its real
# worker method. This is the queued handoff that can move the vector into RT+88.
common_base=rt.bases['PenCommon']; canvas_queue_posts=[]
def capture_canvas_queue(u,a,z,d):
    if a!=0x20001040:return
    task=p.x(1);vt=p.get(task,'Q')[0];method=p.get(vt+16,'Q')[0]
    canvas_queue_posts.append({'task':hex(task),'task_vtable':hex(vt),'run_method':hex(method),'member_receiver':hex(p.get(task+16,'Q')[0]),'member_target_word':hex(p.get(task+24,'Q')[0]),'task_bytes_64':bytes(u.mem_read(task,64)).hex()})
canvas_queue_hook=p.u.hook_add(UC_HOOK_CODE,capture_canvas_queue)
alloc_active['on']=True
p.x(0,drawable);p.x(1,obj);p.x(2,rect);p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,STOP)
p.u.emu_start(p.syms[redraw_name],STOP,count=20_000_000)
if len(handoff)!=1:raise RuntimeError(f'Crayon2 saved redraw did not reach MotionEvent playback exactly once: {handoff} dispatch={dispatch_targets} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x} result={p.x(0)}')
p.u.hook_del(hook);p.u.hook_del(packed_hook);p.u.hook_del(return_hook);p.u.hook_del(read_hook);p.u.hook_del(canvas_queue_hook);alloc_active['on']=False
# Execute only the queued native SendDataToGPU worker selected by its member
# target; retain all other captured tasks. Never write RT+88 from the host.
canvas_worker_runs=[]
for qrow in canvas_queue_posts:
    if int(qrow['member_target_word'],16)!=common_base+0x4a5c4:continue
    task=int(qrow['task'],16);method=int(qrow['run_method'],16)
    before={'rt80':hex(p.get(rt_obj+80,'Q')[0]),'rt88':hex(p.get(rt_obj+88,'Q')[0])}
    p.x(0,task);p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,STOP)
    p.u.emu_start(method,STOP,count=2000000)
    if p.u.reg_read(R.UC_ARM64_REG_PC)!=STOP:raise RuntimeError(f'native Canvas worker failed to return: {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
    after={'rt80':hex(p.get(rt_obj+80,'Q')[0]),'rt88':hex(p.get(rt_obj+88,'Q')[0])}
    canvas_worker_runs.append({'task':hex(task),'run_method':hex(method),'target':qrow['member_target_word'],'slots_before':before,'slots_after':after})
rt_after=bytes(p.u.mem_read(rt_obj,0x200))
rt_changed=[{'offset':i,'before':rt_before[i:i+16].hex(),'after':rt_after[i:i+16].hex()} for i in range(0,len(rt_before),16) if rt_before[i:i+16]!=rt_after[i:i+16]]
if p.u.reg_read(R.UC_ARM64_REG_PC)!=STOP: raise RuntimeError(f'full saved redraw did not return: PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
if not queue_posts: raise RuntimeError('no queued native worker ran')
# Zero-rand control is expected to suppress emission; nonzero native drawPoint entry remains separately asserted.
if len(handoff)!=1 or dispatch_targets!=[{'callsite':hex(base+0x25df0),'target':hex(base+0x2571c),'vptr':hex(p.get(drawable,'Q')[0])}]: raise RuntimeError(f'unexpected virtual dispatch: {handoff} {dispatch_targets}')
expected_points={'base':463,'size8':211,'pressure':383}[case]
if not random_calls: raise RuntimeError('no modeled/native RNG calls observed')
if rng_mode=='bionic-type3' and random_calls[:6]!=[{'fn':'srand','seed':1},{'fn':'srand','seed':1},{'fn':'rand','value':1804289383},{'fn':'rand','value':846930886},{'fn':'rand','value':1681692777},{'fn':'rand','value':1714636915}]: raise RuntimeError(f'bionic TYPE_3 modeled RNG trace changed: {random_calls[:6]}')
if len([e for e in geometry if e['entry']==point_name])!=expected_points: raise RuntimeError(f'Crayon2 point count changed for {case}: {len(geometry)}')
def fbits(v): return struct.pack('<f',v).hex()
line_events=[e for e in geometry if e['entry']==line_name]; end_events=[e for e in geometry if e['entry']==endline_name]; point_events=[e for e in geometry if e['entry']==point_name]
if len(line_events)!=1 or line_events[0]['s0_s2_f32_bits']!=[fbits(points[1][0]),fbits(points[1][1]),fbits(pressures[1])]: raise RuntimeError(f'drawLine args mismatch: {line_events}')
if len(end_events)!=1 or end_events[0]['s0_s2_f32_bits']!=[fbits(points[-1][0]),fbits(points[-1][1]),fbits(pressures[-1])]: raise RuntimeError(f'drawEndLine args mismatch: {end_events}')
if point_events[0]['point_xy_f32_bits']!=fbits(points[0][0])+fbits(points[0][1]) or point_events[0]['s0_s2_f32_bits'][0]!=fbits(stroke_size): raise RuntimeError(f'first native drawPoint args mismatch: {point_events[0]}')
if len(add_events)!=len(add_returns): raise RuntimeError(f'AddPoint entry/return mismatch: {len(add_events)}/{len(add_returns)}')
if rng_mode=='bionic-type3' and len({e['drawpoint_ordinal'] for e in add_events})!=len(add_events): raise RuntimeError('expected one packed AddPoint event per associated drawPoint ordinal')
if len(vector_last['bytes'])!=24*len(add_events): raise RuntimeError(f'packed vector length mismatch: {len(vector_last["bytes"])} vs {len(add_events)} records')
for e,r in zip(add_events,add_returns):
    f=r['record_f32']
    if struct.pack('<2f',*f[:2])!=bytes.fromhex(e['xy_f32_bits']): raise RuntimeError(f'packed record XY differs from AddPoint Vector2*: {e} {f}')
    if struct.pack('<f',f[2])!=struct.pack('<f',e['float_s0']*.5): raise RuntimeError(f'packed record field 2 is not half AddPoint S0: {e} {f}')
    if f[3]!=float(e['x2_integer']): raise RuntimeError(f'packed record field 3 differs from AddPoint integer argument: {e} {f}')
event=int(handoff[0]['motion_event'],16); history_count=n-1; motion=[]; current={}; current_event_time=times[-1]-times[0]
# Bounded Crayon2 RT consumer join: use the naturally queued +88 vector and
# a same-Unicorn native ISPSubBitmap output target, then forward native Update/Draw
# GLES operations to Mesa. This is a host Mesa execution, not device parity.
paint_gl_start=len(actual_gl_events)
crayon_post_update={'rt_88_before':hex(p.get(rt_obj+88,'Q')[0]),'result':None,'error':None}
try:
    crayon_post_update['result']=call('SPen::Crayon2StrokeDrawableRTV1::Update(bool)',[rt_obj,0],limit=4000000)
except Exception as exc:
    crayon_post_update.update(error=str(exc),cause=repr(exc.__cause__),pc=hex(p.u.reg_read(R.UC_ARM64_REG_PC)))
    p.u.reg_write(R.UC_ARM64_REG_PC,STOP)
crayon_post_update['rt_88_after']=hex(p.get(rt_obj+88,'Q')[0])
crayon_post_update['mesa_gl_events']=actual_gl_events[paint_gl_start:]
crayon_draw={'result':None,'error':None,'actual_gl_events':[],'output_target_gl_events':[],'output_bitmap':None,'canvas':None,'subbitmap':None}
try:
    target_setup_start=len(actual_gl_events)
    fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0');width=256;height=256
    bg=bytes(int(x) for x in os.environ.get('CRAYON_BG_RGBA','0,0,0,0').split(','))
    if len(bg)!=4 or any(x<0 or x>255 for x in bg):raise ValueError(f'invalid RGBA background: {list(bg)}')
    pixels=p.alloc(width*height*4);p.u.mem_write(pixels,bg*(width*height))
    bitmap_factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
    def call_stack(name,x,stack):
        for i,v in enumerate(x):p.x(i,v)
        sp=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack),*stack));p.u.reg_write(R.UC_ARM64_REG_LR,STOP)
        p.u.emu_start(p.syms[name],STOP,count=5000000)
        if p.u.reg_read(R.UC_ARM64_REG_PC)!=STOP:raise RuntimeError(f'stack call failed: {name} PC={p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
        return p.x(0)
    outbmp=call_stack(bitmap_factory,[queue,width,height,fmt,pixels,1,1024,1],[6,2])
    if not outbmp:raise RuntimeError('native CreateBitmap returned null')
    canvas=p.alloc(512)
    call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1])
    bitmap=call('SPen::SPCanvasImpl::GetBitmap()',[canvas])
    subbitmap=call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0])
    crayon_draw.update(output_bitmap=hex(outbmp),canvas=hex(canvas),bitmap=hex(bitmap),subbitmap=hex(subbitmap),subbitmap_vptr=hex(p.get(subbitmap,'Q')[0]))
    crayon_draw['output_target_gl_events']=actual_gl_events[target_setup_start:]
    tx,ty=(int(v) for v in os.environ.get('CRAYON_TRANSLATION','0,0').split(','))
    mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1)
    crayon_draw['host_view_translation']=[tx,ty]
    draw_start=len(actual_gl_events)
    crayon_draw['result']=call('SPen::Crayon2StrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt_obj,subbitmap,mat],limit=5000000)
    crayon_draw['actual_gl_events']=actual_gl_events[draw_start:]
    glfunc('glFinish',None,[])()
    getint=glfunc('glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])
    draw_fbo=C.c_int();read_fbo=C.c_int();getint(0x8ca6,C.byref(draw_fbo));getint(0x8caa,C.byref(read_fbo))
    attachment=C.c_int();draw_attachment=C.c_int()
    attachment_query=glfunc('glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])
    attachment_query(0x8ca8,0x8ce0,0x8cd1,C.byref(attachment));attachment_query(0x8ca9,0x8ce0,0x8cd1,C.byref(draw_attachment))
    pre_read_error=gf['glGetError']()
    target_pixels=C.create_string_buffer(width*height*4)
    glfunc('glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(0,0,width,height,0x1908,0x1401,target_pixels)
    read_error=gf['glGetError']()
    raw=target_pixels.raw;changed=[i for i in range(width*height) if raw[i*4:i*4+4]!=bg]
    bbox=[min(i%width for i in changed),min(i//width for i in changed),max(i%width for i in changed),max(i//width for i in changed)] if changed else None
    crayon_draw['framebuffer_readback']={'draw_framebuffer_binding_0x8ca6':draw_fbo.value,'read_framebuffer_binding_0x8caa':read_fbo.value,'read_attachment_enum':0x8ce0,'read_attachment_object_name':attachment.value,'draw_attachment_object_name':draw_attachment.value,'rgba8_sha256':hashlib.sha256(raw).hexdigest(),'rgba8_base64':base64.b64encode(raw).decode(),'background_rgba':list(bg),'changed_pixel_count_vs_background':len(changed),'bbox_vs_background':bbox,'sample_128_128':list(raw[(128*width+128)*4:(128*width+128)*4+4]),'gl_error_before_readpixels':pre_read_error,'gl_error_after_readpixels':read_error}
except Exception as exc:
    crayon_draw.update(error=str(exc),cause=repr(exc.__cause__),pc=hex(p.u.reg_read(R.UC_ARM64_REG_PC)),actual_gl_events=actual_gl_events[paint_gl_start:])
    p.u.reg_write(R.UC_ARM64_REG_PC,STOP)
print(json.dumps({'library_sha256':digest,'base':hex(base),'drawable_constructor':'Crayon2StrokeDrawableGLV1(Crayon2Data*,PenGLDataManager*)','factory':'createPenInst','factory_trace':factory_trace,'factory_pen_vptr':hex(p.get(pen,'Q')[0]),'factory_data':hex(data),'factory_settings':hex(settings),'factory_settings_before_size_override':factory_setting_initial,'canvas_queue_posts':canvas_queue_posts,'canvas_worker_runs':canvas_worker_runs,'host_mesa':mesa.identity,'shader_manager_setup':{'constructor':shader_manager_ctor,'singleton_global_rva':'0x6d480','get_instance_matches_constructed_object':shader_manager_singleton==shader_manager},'renderer_parameters_init':{'result':renderer_parameters_result,'actual_host_gl_calls':renderer_parameter_actual_gl_calls},'init_gl_events':actual_gl_events[:paint_gl_start],'source_getter_events':getter_events,'source_getter_returns':getter_returns,'crayon2_post_playback_update':crayon_post_update,'crayon2_native_draw':crayon_draw,'rng_profile':'AOSP bionic TYPE_3 model, reseeded by each native srand call; not Samsung runtime execution','scope':'pinned native createPenInst/Crayon2 constructor and factory-created Crayon2Data/setting record; synthetic Model ObjectStroke setters; native MotionEvent conversion, direct saved-object RedrawPen replay, and drawLine/drawEndLine/drawPoint callback bodies run to return. The renderer setting is separately host-written into Crayon2Data; this is not a Drawing-side update. Queue is a synchronous host adapter; rand/srand are modeled with AOSP bionic TYPE_3, not Android runtime execution. Renderer Init, Update, Draw, shader, and Mesa GLES output are captured; no PDF or Samsung-device runtime claim.','scalar_helpers':scalar,'host_setting_write':host_setting_write,'source_object':{'case':case,'points':points,'pressures':pressures,'timestamps':times,'tilts':tilts,'orientations':orientations,'tool_type':2,'pen_size':stroke_size,'color_argb_set_by_Model_setter':f'0x{saved_color_argb:08x}','pen_name':'com.samsung.android.sdk.pen.pen.preload.Crayon2'},'renderer_sha256':renderer_hash,'rt_init':{'result':init_result,'trace':rt_init_trace,'before':rt_before_init,'after':rt_after_init},'rng_mode':rng_mode,'global_reads':{'count':len(global_reads),'by_symbol':{n:sum(e['symbol']==n for e in global_reads) for n,_,_ in tracked_globals},'first':global_reads[:20]},'packed_boundary':{'symbol':add_name,'rva':hex(add_addr-base),'receiver':hex(rt_obj),'vptr':hex(p.get(rt_obj,'Q')[0]),'events':add_events,'count':len(add_events),'return_count':len(add_returns),'pending_returns':len(pending_add_returns),'return_snapshots':add_returns,'final_packed_stream_hex':vector_last['bytes'].hex(),'final_packed_stream_sha256':hashlib.sha256(vector_last['bytes']).hexdigest(),'final_packed_record_count':len(vector_last['bytes'])//24,'packed_record_contract':'native 24-byte/six-f32 record appended per AddPoint: fields 0..1 equal Vector2*; field 2 equals S0/2 as f32; field 3 equals integer x2 converted to f32; fields 4..5 retained raw, not semantically labeled.','rt_changed_16byte_chunks':rt_changed,'native_allocations':[{**x,'sha256_first4k':hashlib.sha256(bytes(p.u.mem_read(x['address'],min(x['size'],4096)))).hexdigest()} for x in allocations]},'native_route':{'redraw_symbol':redraw_name,'draw_symbol':draw_name,'motion_event_playback_entry':handoff,'geometry_events':geometry,'geometry_counts':{name:sum(e['entry']==name for e in geometry) for name in geom_addrs.values()},'drawpoint_stream_sha256':hashlib.sha256(b''.join(bytes.fromhex(e['point_xy_f32_bits'])+bytes.fromhex(e['s0_s2_f32_bits'][0]) for e in geometry if e['entry']==point_name)).hexdigest(),'random_calls':random_calls,'queue_posts':queue_posts,'flow_rvas':flow,'end_pc':hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'end_result':p.x(0),'drawline_plt':next((hex(a) for a,nm in p.plt.items() if 'Crayon2StrokeDrawableGL::drawLine' in nm),None),'history_size':history_count,'historical_channels':motion,'current_channels':current,'event_time':current_event_time,'channel_match':'captured in separate probe.py run; playback run continues until GL-call path returns'}},indent=2))
