#!/usr/bin/env python3
"""Join actual AirBrush Model/Drawing vector publication to native RT paint on host Mesa.
Run `PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/airbrush_joined_paint_native.py --check-fixture`.
The fixture is a host GLES oracle, not Android device output.
"""
import argparse, base64, contextlib, hashlib, json, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/'conformance/airbrush-joined-paint-native.json'

def main():
 import os,subprocess
 parser=argparse.ArgumentParser(description=__doc__)
 mode=parser.add_mutually_exclusive_group(required=False)
 mode.add_argument('--write-fixture',action='store_true')
 mode.add_argument('--check-fixture',action='store_true')
 mode.add_argument('--print-capture',action='store_true')
 parser.add_argument('--diagnostics-json',type=Path,help='write per-child stdout/stderr and exit/readback outcomes outside the golden fixture')
 parser.add_argument('--case-run',help=argparse.SUPPRESS)
 args=parser.parse_args()
 if args.case_run is not None:
  color=int(args.case_run,16)
  with contextlib.redirect_stdout(__import__('io').StringIO()):
   case=run_case(color)
  print(json.dumps(case,indent=2,sort_keys=True))
  return
 if sum(bool(x) for x in (args.write_fixture,args.check_fixture,args.print_capture))!=1:
  parser.error('select exactly one of --write-fixture, --check-fixture, or --print-capture')
 diagnostics=[]
 def save_diagnostics():
  if args.diagnostics_json:
   args.diagnostics_json.parent.mkdir(parents=True,exist_ok=True)
   args.diagnostics_json.write_text(json.dumps({'schema':'airbrush-joined-paint-run-diagnostics-v1','attempts':diagnostics},indent=2,sort_keys=True)+'\n')
 def execute(color):
  env=dict(os.environ);env['AIRBRUSH_TRACE_NATIVE']='1'
  env['PYTHONFAULTHANDLER']='1'
  env['PYTHONPATH']=str(ROOT/'scratch/apk-analysis-runtime/python')+':'+str(ROOT/'conformance')+(':'+env['PYTHONPATH'] if env.get('PYTHONPATH') else '')
  proc=subprocess.run([sys.executable,str(Path(__file__).resolve()),'--case-run',f'{color:08x}'],cwd=ROOT,env=env,text=True,capture_output=True)
  attempt={'case_color':f'0x{color:08x}','returncode':proc.returncode,'stdout_bytes':len(proc.stdout.encode()),'stdout_sha256':hashlib.sha256(proc.stdout.encode()).hexdigest(),'stdout':proc.stdout,'stderr':proc.stderr}
  case=None
  if proc.returncode==0:
   case=json.loads(proc.stdout)
   attempt['readback']={key:case['readback_rgba8'].get(key) for key in ('sha256','nonzero_pixels','bytes')}
  diagnostics.append(attempt)
  summary={key:attempt[key] for key in ('case_color','returncode','stdout_bytes','stdout_sha256','stderr')}
  if 'readback' in attempt:summary['readback']=attempt['readback']
  print('AIRBRUSH_CAPTURE_ATTEMPT '+json.dumps(summary,sort_keys=True),file=sys.stderr,flush=True)
  save_diagnostics()
  if proc.returncode!=0:
   raise RuntimeError(f'AirBrush native paint child {color:#010x} failed with {proc.returncode}; full child stdout/stderr are in capture diagnostics')
  if case['readback_rgba8']['nonzero_pixels']==0:
   raise RuntimeError(f'AirBrush paint produced an empty readback for {color:#010x}; full child stdout/stderr and readback summary are in capture diagnostics')
  return case
 cases=[]
 for color in (0xff6cdd8f,0x006cdd8f):
  first=execute(color);repeat=execute(color)
  signature=lambda row: hashlib.sha256(json.dumps({k:row[k] for k in ('source_argb_u32','native_route','render_target','native_draw','readback_rgba8')},sort_keys=True,separators=(',',':')).encode()).hexdigest()
  first_sig,repeat_sig=signature(first),signature(repeat)
  if first_sig!=repeat_sig:
   different={key:{'first':str(first[key])[:500],'repeat':str(repeat[key])[:500]} for key in ('native_route','render_target','native_draw','readback_rgba8') if first[key]!=repeat[key]}
   raise SystemExit(f'AirBrush paint replay was unstable for {color:#010x}: {first_sig} != {repeat_sig}; differences={different}')
  first['repeated_stability']={'successful_runs':2,'semantic_signature_sha256':first_sig}
  cases.append(first)
 fixture={'schema':'airbrush-joined-paint-native-v1','cases':cases}
 save_diagnostics()
 encoded=json.dumps(fixture,indent=2,sort_keys=True)+'\n'
 if args.write_fixture: OUT.write_text(encoded)
 elif args.check_fixture:
  actual=OUT.read_text()
  if actual!=encoded: raise SystemExit('airbrush joined paint fixture differs; review, then use --write-fixture')
 print(encoded,end='')

def run_case(source_argb):
 import ast,contextlib,io,sys,pathlib,runpy,struct,types,hashlib,os,subprocess,re,ctypes as C,zlib
 trace=lambda label: print('AIRBRUSH_PHASE',source_argb,label,file=sys.stderr,flush=True)
 from unicorn import arm64_const as R
 root=pathlib.Path(__file__).resolve().parents[1]; caller=root/'conformance/airbrush_saved_caller_native.py'
 source=caller.read_text()
 src_argb=source_argb
 source=source.replace('[obj,0xff6cdd8f]',f'[obj,0x{src_argb:08x}]')
 source=source.replace("self.x(0, self.context)", "self.x(0, getattr(self, 'native_task_queue', self.context))")
 source=source.replace("manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64);", "manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.native_task_queue=queue;")
 source=source.replace("queue_events=[]", "queue_events=[]\ndeferred_queue_tasks=[]\ndefer_queue_tasks=False")
 needle_func="def taskqueue(u,a,z,d):\n"
 assert source.count(needle_func)==1
 source=source.replace(needle_func, needle_func+" global defer_queue_tasks\n")
 needle_task="  if not method: raise RuntimeError('task has null Run')\n"
 assert source.count(needle_task)==1
 source=source.replace(needle_task, "  if method==0x10506b4 or defer_queue_tasks:\n   deferred_queue_tasks.append({'task':task,'vtable':vt,'run':method,'queue':p.x(0),'caller_lr':caller,'task_bytes':bytes(u.mem_read(task,64)).hex()});defer_queue_tasks=True;p.x(0,1);u.reg_write(R.UC_ARM64_REG_PC,caller);return\n"+needle_task)
 source=source.replace('pending_airbrush_buffer_returns={}', 'transient_airbrush_vectors={}\npending_airbrush_buffer_returns={}')
 needle="  this=u.reg_read(R.UC_ARM64_REG_X0); vector_obj=struct.unpack('<Q',u.mem_read(this+80,8))[0]; bounds="
 assert source.count(needle)==1
 source=source.replace(needle,"  this=u.reg_read(R.UC_ARM64_REG_X0); vector_obj=struct.unpack('<Q',u.mem_read(this+80,8))[0]; transient_airbrush_vectors.setdefault(phase, []).append(vector_obj); bounds=")
 sys.path.insert(0,str(root/'conformance'))
 import oblique_canvas_native as oc
 original_canvas_hook=oc.CanvasProbe.hook_canvas
 def queue_canvas_hook(self,u,address,size,data):
  if address==oc.STUB_CANVAS_DATA and hasattr(self,'native_task_queue'):
   self.x(0,self.native_task_queue);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
  original_canvas_hook(self,u,address,size,data)
 oc.CanvasProbe.hook_canvas=queue_canvas_hook
 sys.argv=[str(caller),'--single-case','--print-capture','--advanced-mode','explicit-empty']
 out=io.StringIO(); m={'__file__':str(caller)}
 try:
  with contextlib.redirect_stdout(out): exec(compile(source,str(caller),'exec'),m)
 except Exception:
  print('CALLER_OUTPUT',out.getvalue()); pp=m.get('p'); print('PARTIAL_QUEUE',m.get('queue_events')); print('PARTIAL_PC',hex(pp.u.reg_read(R.UC_ARM64_REG_PC)) if pp else None); rr=0x10035830; print('PARTIAL_RT_SLOTS',*[hex(x) for x in pp.get(rr+80,'2Q')], 'VECTOR',pp.get(0x10035c70,'3Q')); print('OBJADDRS',{k:hex(v) for k,v in vars(pp).items() if isinstance(v,int) and k in ('context','bitmap','draw_canvas','pen_canvas','drawing','oblique','queue','native_task_queue')}); print('FAILED_TASK',bytes(pp.u.mem_read(0x10036280,64)).hex()); raise
 trace('caller-done')
 p=m['p']; rt=m['rt']; case=__import__('json').loads(out.getvalue()); stream=bytes.fromhex(case['producer']['buffer_hex']); vecobj=m['transient_airbrush_vectors']['draw_object_stroke'][-1]
 assert len(stream)==416 and len(set(m['transient_airbrush_vectors']['draw_object_stroke']))==1
 print('CALLER_OK','source_argb',hex(src_argb),hex(rt),hex(vecobj),len(stream),hashlib.sha256(stream).hexdigest(),'slots',[hex(x) for x in p.get(rt+80,'2Q')], 'deferred',m['deferred_queue_tasks'],'queueevents',len(m['queue_events']),'q',hex(m['queue']))
 # Keep the real queue tasks deferred through RT Init. Execute the genuine
 # SendDataToGPU task after Init, then retain its vector until readback.
 send_task=m['deferred_queue_tasks'][0]
 assert send_task['run']==0x10506b4
 # Exact callback adapter methods from the pinned working Marker3 host-GLES witness.
 sys.path.insert(0,str(root/'conformance'))
 import airbrush_texture_native as at,inkpen_paint_native as ip, fountain_native as fn
 paint_path=root/'conformance/marker3_paint_scale_native.py'; tree=ast.parse(paint_path.read_text()); klass=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='P')
 env={'at':at,'fn':fn,'ip':ip,'C':C,'zlib':zlib,'hashlib':hashlib,'struct':struct,'subprocess':subprocess,'re':re,'json':__import__('json'),'Path':pathlib.Path,'LIB':root/'scratch/apk-analysis-native/arm64-v8a','ARGS':types.SimpleNamespace(width=256,height=256),'SOURCE_HASHES':{},'SOURCE_FILES':{},'sys':sys,'ROOT':root,'base64':__import__('base64')}
 exec(compile(ast.Module(body=[klass],type_ignores=[]),str(paint_path),'exec'),env)
 Paint=env['P']
 for owner in (at.TextureOracle,Paint):
  for name,value in owner.__dict__.items():
   if callable(value) and not name.startswith('__'): setattr(p,name,types.MethodType(value,p))
 p.gles=ip.GLES(); p.emutls={}; p.errno_ptr=p.alloc(4); p.gl_calls=[]; p.shader_sources={}; p.shader_source_calls=[]; p.shader_generations={};p.shader_generation_rows=[];p.program_generation_rows=[];p.next_shader_generation=1;p.next_program_generation=1;p.texture_upload_records=[];p.bound_gl_buffers={};p.native_samplers={};p.native_syncs={};p.native_logs=[];p.next_gl_id=100
 p.callbacks['glGetString']=p.get_string; p.callbacks['glGetIntegerv']=p.get_integer; p.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()']=p.device_caps; p.callbacks['glPixelStorei']=lambda:p.gles.f['glPixelStorei'](p.x(0),p.x(1))
 def texsub():
  a=[p.x(i) for i in range(8)]; ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0]; raw=bytes(p.u.mem_read(ptr,a[4]*a[5]*4)); buf=C.create_string_buffer(raw)
  if not hasattr(p,'airbrush_texsub_fn'):
   p.airbrush_texsub_fn=p.gles.gl.glTexSubImage2D;p.airbrush_texsub_fn.restype=None;p.airbrush_texsub_fn.argtypes=[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p]
  current=C.CFUNCTYPE(C.c_void_p)(p.gles.egl.eglGetCurrentContext)()
  active=C.c_int();binding=C.c_int();getiv=p.gles.gl.glGetIntegerv;getiv.argtypes=[C.c_uint,C.POINTER(C.c_int)];getiv(0x84E0,C.byref(active));getiv(0x8069,C.byref(binding))
  p.texture_upload_records.append({'target':a[0],'level':a[1],'x':a[2],'y':a[3],'width':a[4],'height':a[5],'format':a[6],'type':a[7],'texture':binding.value,'active_texture':active.value,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()})
  print('AIRBRUSH_TEXSUB',json.dumps({'args':a,'guest_data':hex(ptr),'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'current_context':hex(current or 0)},sort_keys=True),file=sys.stderr,flush=True)
  p.airbrush_texsub_fn(*a,C.cast(buf,C.c_void_p))
 p.callbacks['glTexSubImage2D']=texsub; p.callbacks['glFlush']=lambda:p.gles.gl.glFlush(); p.callbacks['glFinish']=lambda:p.gles.gl.glFinish(); p.callbacks['glGetError']=lambda:p.x(0,p.gles.f['glGetError']())
 init=next(n for n in klass.body if isinstance(n,ast.FunctionDef) and n.name=='__init__'); updates=[]
 for n in ast.walk(init):
  if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr=='update' and isinstance(n.func.value,ast.Attribute) and n.func.value.attr=='callbacks':updates.append(n)
 entry=next(n.args[0] for n in updates if isinstance(n.args[0],ast.Dict) and any(isinstance(k,ast.Constant) and k.value=='glCreateShader' for k in n.args[0].keys)); p.callbacks.update(eval(compile(ast.Expression(entry),'paint-callback-map','eval'),env|{'self':p}))
 # Preserve shader and program generations: Mesa reuses shader object names after
 # deletion, so a single ID-keyed source dictionary can overwrite link evidence.
 _create_shader=p.callbacks['glCreateShader'];_shader_source=p.callbacks['glShaderSource'];_compile_shader=p.callbacks['glCompileShader'];_create_program=p.callbacks['glCreateProgram'];_attach_shader=p.callbacks['glAttachShader'];_link_program=p.callbacks['glLinkProgram'];_delete_shader=p.callbacks['glDeleteShader']
 def create_shader_generation():
  shader_type=p.x(0);_create_shader();shader=p.x(0);generation=p.next_shader_generation;p.next_shader_generation+=1
  row={'generation':generation,'shader':shader,'type':shader_type,'source_parts':[],'source_sha256':None,'source_bytes':None,'compiled_source_parts':[],'compiled_source_sha256':None,'compiled_source_bytes':None,'compiled':False,'compile_status':None,'deleted':False};p.shader_generations[shader]=row;p.shader_generation_rows.append(row)
 def capture_shader_source():
  shader=p.x(0);_shader_source();row=p.shader_generations.get(shader)
  if row is None:raise RuntimeError(f'glShaderSource for unknown shader generation {shader}')
  sources=p.shader_source_calls[-1]['source'];parts=[s.encode('utf-8') for s in sources];raw=b''.join(parts);row['source_parts']=[s.decode('utf-8') for s in parts];row['source_bytes']=len(raw);row['source_sha256']=hashlib.sha256(raw).hexdigest()
 def compile_shader_generation():
  shader=p.x(0);_compile_shader();row=p.shader_generations.get(shader)
  if row is None or row['source_sha256'] is None:raise RuntimeError(f'glCompileShader lacks source snapshot for shader {shader}')
  status=C.c_int();p.gles.f['glGetShaderiv'].argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)];p.gles.f['glGetShaderiv'](shader,0x8b81,C.byref(status))
  if status.value!=1:raise RuntimeError(f'GL shader compile failed for generation {row["generation"]}')
  row['compiled_source_parts']=list(row['source_parts']);row['compiled_source_sha256']=row['source_sha256'];row['compiled_source_bytes']=row['source_bytes'];row['compiled']=True;row['compile_status']=status.value
 def create_program_generation():
  _create_program();program=p.x(0);generation=p.next_program_generation;p.next_program_generation+=1
  p.program_generation_rows.append({'generation':generation,'program':program,'attached':[],'linked':False,'linked_sources':[]})
 def current_program_generation(program):
  rows=[row for row in p.program_generation_rows if row['program']==program]
  if not rows:raise RuntimeError(f'GL program {program} has no recorded generation')
  return rows[-1]
 def attach_shader_generation():
  program,shader=p.x(0),p.x(1);row=p.shader_generations.get(shader)
  if row is None or not row['compiled']:raise RuntimeError(f'glAttachShader uses unknown or uncompiled shader generation {shader}')
  _attach_shader();current_program_generation(program)['attached'].append({'shader_generation':row['generation'],'shader_type':row['type'],'source_sha256':row['compiled_source_sha256'],'source_bytes':row['compiled_source_bytes'],'source_parts':list(row['compiled_source_parts'])})
 def link_program_generation():
  program=p.x(0);_link_program();row=current_program_generation(program);status=C.c_int();p.gles.f['glGetProgramiv'].argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)];p.gles.f['glGetProgramiv'](program,0x8b82,C.byref(status));row['link_status']=status.value;row['linked']=status.value==1;row['linked_sources']=list(row['attached'])
  if not row['linked_sources']:raise RuntimeError(f'GL program generation {row["generation"]} linked without shader snapshots')
 p.callbacks.update({'glCreateShader':create_shader_generation,'glShaderSource':capture_shader_source,'glCompileShader':compile_shader_generation,'glCreateProgram':create_program_generation,'glAttachShader':attach_shader_generation,'glLinkProgram':link_program_generation,'glDeleteShader':lambda:(p.shader_generations.get(p.x(0),{}).update({'deleted':True}),_delete_shader())})
 _bind_buffer_callback=p.callbacks['glBindBuffer'];_buffer_data_callback=p.callbacks['glBufferData']
 def bind_buffer_tracked():
  p.bound_gl_buffers[p.x(0)]=p.x(1);_bind_buffer_callback()
 def buffer_data_tracked():
  target=p.x(0);buffer=p.bound_gl_buffers.get(target,0);_buffer_data_callback()
  if p.gl_calls and p.gl_calls[-1].get('call')=='glBufferData':p.gl_calls[-1]['buffer']=buffer
 p.callbacks['glBindBuffer']=bind_buffer_tracked;p.callbacks['glBufferData']=buffer_data_tracked
 _tex_parameter_callback=p.callbacks['glTexParameteri']
 def capture_texture_parameter():
  target,pname,value=p.x(0),p.x(1),p.x(2);active=C.c_int();binding=C.c_int();p.gles.gl.glGetIntegerv(0x84e0,C.byref(active));p.gles.gl.glGetIntegerv(0x8069,C.byref(binding))
  p.gl_calls.append({'call':'glTexParameteri','target':target,'pname':pname,'value':value,'active_texture':active.value,'texture':binding.value});_tex_parameter_callback()
 p.callbacks['glTexParameteri']=capture_texture_parameter
 _tex_image_callback=p.callbacks.get('glTexImage2D')
 if _tex_image_callback:
  def capture_texture_image():
   target,level=[p.x(i) for i in (0,1)];active=C.c_int();binding=C.c_int();p.gles.gl.glGetIntegerv(0x84e0,C.byref(active));p.gles.gl.glGetIntegerv(0x8069,C.byref(binding));_tex_image_callback()
   for row in reversed(p.gl_calls):
    if row.get('call')=='glTexImage2D' and 'level' not in row:
     row.update({'level':level,'texture':binding.value,'active_texture':active.value});break
  p.callbacks['glTexImage2D']=capture_texture_image
 _draw_arrays_callback=p.callbacks.get('glDrawArrays')
 if _draw_arrays_callback:
  def capture_draw_arrays():
   values=[p.x(i) for i in range(3)];_draw_arrays_callback();p.gl_calls.append({'call':'glDrawArrays','values':values,'state':p.gl_state()})
  p.callbacks['glDrawArrays']=capture_draw_arrays
 for name,method in [('glGenSamplers','gl_gen_samplers'),('glSamplerParameteri','gl_sampler_parameter'),('glFenceSync','gl_fence_sync'),('glClientWaitSync','gl_client_wait_sync'),('glWaitSync','gl_wait_sync'),('glDeleteSync','gl_delete_sync')]:p.callbacks[name]=getattr(p,method)
 p.callbacks['glGetActiveUniform']=lambda:(setattr(p,'current_plt_symbol','glGetActiveUniform'),p.gl_get_active());p.callbacks['glGetActiveAttrib']=lambda:(setattr(p,'current_plt_symbol','glGetActiveAttrib'),p.gl_get_active())
 p.callbacks['eglGetCurrentContext']=lambda:p.x(0,0xabc); p.callbacks['__errno']=lambda:p.x(0,p.errno_ptr); p.callbacks['fopen']=p.fopen_miss; p.callbacks['unlink']=p.unlink_miss; p.callbacks['malloc']=p.malloc_log; p.callbacks['posix_memalign']=p.posix_memalign; p.callbacks['__android_log_print']=p.log_print; p.callbacks['memcmp']=p.memcmp_native; p.callbacks['strcmp']=p.strcmp_native; p.callbacks['strncmp']=p.strncmp_native; p.callbacks['memchr']=p.memchr_native; p.callbacks['vsnprintf']=p.vsnprintf; p.callbacks['__strlen_chk']=p.strlen_checked; p.callbacks['__strncpy_chk2']=p.strncpy_checked2; p.callbacks['strstr']=p.strstr; p.callbacks['strncpy']=p.strncpy; p.callbacks['clock_gettime']=p.clock_gettime; p.callbacks['__emutls_get_address']=p.emutls_get_address
 p.callbacks['__strncpy_chk']=lambda:(p.u.mem_write(p.x(0),bytes(p.u.mem_read(p.x(1),p.x(2)))),p.x(0,p.x(0)))
 renderer=root/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so'; blob=renderer.read_bytes(); base=0x9000000
 phoff=struct.unpack_from('<Q',blob,32)[0]; ent,n=struct.unpack_from('<HH',blob,54); extent=0
 for i in range(n):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,phoff+i*ent)
  if typ==1:extent=max(extent,va+ms)
 p.u.mem_map(base,(extent+0xfff)&~0xfff)
 for i in range(n):
  typ,fl,off,va,pa,fs,ms,al=struct.unpack_from('<II6Q',blob,phoff+i*ent)
  if typ==1 and ms:p.u.mem_write(base+va,blob[off:off+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
 ldenv={**os.environ,'LD_LIBRARY_PATH':str(root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}; nm=root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm'; obj=root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
 for line in subprocess.check_output([str(nm),'-D','-C',str(renderer)],text=True,env=ldenv).splitlines():
  q=line.strip().split(' ',2)
  if len(q)==3 and q[1] in ('T','W','V','D','B','R'):p.syms[q[2]]=base+int(q[0],16)
 p.syms['typeinfo for SPen::IGLManagedMsgQueue']=base+0x6b400
 for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',subprocess.check_output([str(obj),'-d','-C','-j','.plt',str(renderer)],text=True,env=ldenv),re.M):p.plt[base+int(a,16)]=name
 # Add Renderer symbol resolution to the loaded Graphics dependency, preserving exact imports.
 p.extra_bases={'Graphics':0x4000000,'Renderer':base};p.relocation_traps={}
 p.resolve_symbol=types.MethodType(__import__('airbrush_saved_native').Oracle.resolve_symbol,p)
 p.relocation_trap=types.MethodType(__import__('airbrush_saved_native').Oracle.relocation_trap,p)
 p.load_extra_relocs=types.MethodType(at.TextureOracle.load_extra_relocs,p);p.load_extra_relocs()
 # Renderer capability globals consumed by the actual factory.
 p.put(base+0x73388,'II',4096,4096);p.u.mem_write(base+0x733c0,b'\x01')
 # Run Renderer constructors exactly as in the existing working adapter.
 shoff=struct.unpack_from('<Q',blob,40)[0]; se,sn,ss=struct.unpack_from('<HHH',blob,58); secs=[struct.unpack_from('<IIQQQQIIQQ',blob,shoff+i*se) for i in range(sn)];names=blob[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs}
 initsec=by['.init_array']
 for off in range(0,initsec[5],8):
  target=struct.unpack('<Q',p.u.mem_read(base+initsec[3]+off,8))[0]
  if target:
   for i in range(4):p.x(i,0)
   p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,m['oc'].STOP);p.u.emu_start(target,m['oc'].STOP,count=3000000)
   if p.u.reg_read(R.UC_ARM64_REG_PC)!=m['oc'].STOP:raise RuntimeError(f'Renderer initializer fault {p.u.reg_read(R.UC_ARM64_REG_PC):#x}')
 trace('renderer-initializers-done')
# Make Renderer factories execute natively, not caller's matrix-only host stubs.
 for name in ('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)','SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'):p.callbacks.pop(name,None)
 try:
  p.call('SPen::AirBrushPenStrokeDrawableRTV1::Init()', [rt], limit=5_000_000)
  gl_stage_errors={'after_init':p.gles.f['glGetError']()}
  assert gl_stage_errors['after_init']==0,gl_stage_errors
  trace('rt-init-done'); print('GLERR_AFTER_INIT',gl_stage_errors['after_init'],flush=True)
  print('INIT_OK',bytes(p.u.mem_read(rt,0x130)).hex())
  rt_slots_after_init=p.get(rt+80,'2Q')
  init_vector_slot=rt_slots_after_init[1]
  p.x(0,send_task['task']);p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,m['oc'].STOP);p.u.emu_start(send_task['run'],m['oc'].STOP,count=5_000_000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m['oc'].STOP:raise RuntimeError('native SendDataToGPU failed')
  published_slots=p.get(rt+80,'2Q')
  assert published_slots[1]==vecobj, ('SendDataToGPU did not publish original vector',published_slots,vecobj)
  assert struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),16)[0]==rt
  assert struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),40)[0]==vecobj
  vector_storage_before_update=p.get(vecobj,'3Q')
  assert vector_storage_before_update[1]-vector_storage_before_update[0]==len(stream),vector_storage_before_update
  gl_stage_errors['after_send_data_task']=p.gles.f['glGetError']()
  assert gl_stage_errors['after_send_data_task']==0,gl_stage_errors
  m['completed_send_task']=send_task['task']
  print('SEND_NATIVE_OK',hex(published_slots[1]),'vector_head',p.get(vecobj,'3Q'))
  vector_bytes_at_update=bytes(p.u.mem_read(vector_storage_before_update[0],len(stream)))
  assert vector_bytes_at_update==stream
  rt_slots_before_update=p.get(rt+80,'2Q')
  assert rt_slots_before_update[1]==vecobj
  p.call('SPen::AirBrushPenStrokeDrawableRTV1::Update(bool)',[rt,0],limit=5_000_000)
  vector_bytes_after_update=bytes(p.u.mem_read(vector_storage_before_update[0],len(stream)))
  rt_slots_after_update=p.get(rt+80,'2Q')
  gl_stage_errors['after_update']=p.gles.f['glGetError']()
  assert gl_stage_errors['after_update']==0,gl_stage_errors
  uploads=[x for x in getattr(p,'gl_calls',[]) if x.get('call')=='glBufferData']
  trace('rt-update-done'); print('GLERR_AFTER_UPDATE',gl_stage_errors['after_update'],flush=True)
  print('UPDATE_OK','uploads',[(x.get('size'),x.get('dataSha256')) for x in uploads])
  # Native SPTextureBitmap target and SPCanvasImpl provide the bitmap subobject to the same selected RTV1.
  m['defer_queue_tasks']=False
  queue=m['queue']; fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0');initial=p.alloc(256*256*4);p.u.mem_write(initial,bytes(256*256*4))
  factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
  def call_stack(name,x,stack):
   for i,v in enumerate(x):p.x(i,v)
   sp=0x200ef000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.mem_write(sp,struct.pack('<'+'Q'*len(stack),*stack));p.u.reg_write(R.UC_ARM64_REG_LR,m['oc'].STOP);p.u.emu_start(p.syms[name],m['oc'].STOP,count=5_000_000)
   if p.u.reg_read(R.UC_ARM64_REG_PC)!=m['oc'].STOP:raise RuntimeError('native bitmap factory failed '+name)
  p.callbacks.pop(factory,None)
  call_stack(factory,[queue,256,256,fmt,initial,1,1024,1],[6,2]); outbmp=p.x(0)
  if not outbmp:raise RuntimeError('native RGBA output bitmap factory returned null')
  canvas=p.alloc(512);p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',[canvas,queue,outbmp,1],limit=5_000_000)
  p.call('SPen::SPCanvasImpl::GetBitmap()',[canvas]);bitmap=p.x(0);p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[bitmap,0]);bitmap=p.x(0)
  gl_stage_errors['after_target']=p.gles.f['glGetError']()
  assert gl_stage_errors['after_target']==0,gl_stage_errors
  trace('target-done'); print('GLERR_AFTER_TARGET',gl_stage_errors['after_target'],flush=True)
  print('TARGET_OK',hex(outbmp),hex(canvas),hex(bitmap),'output_texture_calls',[x for x in p.gl_calls if x.get('call') in ('glTexImage2D','glGenTextures')][-6:],'tail',[(x.get('call'),x) for x in p.gl_calls[-40:]],'bmp_head',bytes(p.u.mem_read(outbmp,0x100)).hex())
  mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)
  def invalid(u,access,address,size,value,user): print('INVALID_MEM',access,hex(address),size,'pc',hex(u.reg_read(R.UC_ARM64_REG_PC)),'lr',hex(u.reg_read(R.UC_ARM64_REG_LR))); return False
  from unicorn import UC_HOOK_MEM_INVALID
  p.u.hook_add(UC_HOOK_MEM_INVALID,invalid)
  p.f(0,3.144);p.call('SPen::AirBrushPenStrokeDrawableRTV1::setRectData(float)',[rt],limit=5_000_000)
  # Trace GL errors at the native Draw call boundary after Init/Update/target were confirmed clean.
  p.native_attribute_calls=[]
  for _name,_count in [('glBindBuffer',2),('glBindVertexArray',1),('glEnableVertexAttribArray',1),('glVertexAttribPointer',6),('glVertexAttribDivisor',2)]:
   _cb=p.callbacks[_name]
   def _capture(_cb=_cb,_nm=_name,_count=_count):
    def _run():
     record={'call':_nm,'args':[p.x(i) for i in range(_count)]}
     if _nm=='glVertexAttribPointer':record['array_buffer']=p.bound_gl_buffers.get(0x8892,0)
     p.native_attribute_calls.append(record);return _cb()
    return _run
   p.callbacks[_name]=_capture()
  p.draw_gl_errors=[]
  for _name,_cb in list(p.callbacks.items()):
   if _name.startswith('gl') and _name!='glGetError' and callable(_cb):
    def _wrap(_cb=_cb,_nm=_name):
     def _run():
      _before=p.gles.f['glGetError']();_result=_cb();_after=p.gles.f['glGetError']()
      if _before or _after:p.draw_gl_errors.append({'callback':_nm,'before':_before,'after':_after})
      return _result
     return _run
    p.callbacks[_name]=_wrap()
  # Snapshot host GLES target and raster state immediately before the real native Draw.
  getiv=p.gles.gl.glGetIntegerv;getiv.argtypes=[C.c_uint,C.POINTER(C.c_int)]
  def gi(k,n=1):
   v=(C.c_int*n)();getiv(k,v);return list(v)
  getbv=p.gles.gl.glGetBooleanv;getbv.argtypes=[C.c_uint,C.POINTER(C.c_ubyte)]
  fn_att=C.CFUNCTYPE(None,C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetFramebufferAttachmentParameteriv)
  def attachment(target):
   v=C.c_int();fn_att(target,0x8ce0,0x8cd1,C.byref(v));return v.value
  cm=(C.c_ubyte*4)();getbv(0x0c23,cm)
  enabled=C.CFUNCTYPE(C.c_ubyte,C.c_uint)(p.gles.gl.glIsEnabled)
  p.pre_draw_state={'draw_fbo':gi(0x8ca6)[0],'read_fbo':gi(0x8caa)[0],'viewport':gi(0x0ba2,4),'color_write_mask':list(cm),'pack_alignment':gi(0x0d05)[0],
   'raster_enables':{name:bool(enabled(cap)) for name,cap in [('blend',0x0be2),('scissor',0x0c11),('depth',0x0b71),('stencil',0x0b90),('cull',0x0b44),('dither',0x0bd0),('sample_alpha_to_coverage',0x809e),('sample_coverage',0x80a0)]},
   'scissor_box':gi(0x0c10,4),'depth_function':gi(0x0b74)[0],'depth_write':bool((lambda v:(p.gles.gl.glGetBooleanv(0x0b72,v),v[0])[1])((C.c_ubyte*1)()))}
  p.pre_draw_state['draw_attachment_object']=attachment(0x8ca9);p.pre_draw_state['read_attachment_object']=attachment(0x8ca8)
  p.pre_draw_state['color_attachment_object']=p.pre_draw_state['draw_attachment_object']
  def query_int(pname):
   value=C.c_int();p.gles.gl.glGetIntegerv.argtypes=[C.c_uint,C.POINTER(C.c_int)];p.gles.gl.glGetIntegerv(pname,C.byref(value));return value.value
  def query_program_interface(program):
   get_program=p.gles.gl.glGetProgramiv;get_program.argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)]
   active_attrs=C.c_int();active_uniforms=C.c_int();get_program(program,0x8b89,C.byref(active_attrs));get_program(program,0x8b86,C.byref(active_uniforms))
   def enumerate_active(function_name,count):
    fn=getattr(p.gles.gl,function_name);fn.argtypes=[C.c_uint,C.c_uint,C.c_int,C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.c_void_p];rows=[]
    for index in range(count):
     length=C.c_int();size=C.c_int();kind=C.c_uint();name=C.create_string_buffer(256);fn(program,index,256,C.byref(length),C.byref(size),C.byref(kind),name);label=name.value.decode('utf-8')
     location_fn=p.gles.gl.glGetAttribLocation if function_name=='glGetActiveAttrib' else p.gles.gl.glGetUniformLocation
     location_fn.argtypes=[C.c_uint,C.c_char_p];location=location_fn(program,label.encode('utf-8'))
     rows.append({'name':label,'size':size.value,'type':kind.value,'location':location})
    return rows
   return {'attributes':enumerate_active('glGetActiveAttrib',active_attrs.value),'uniforms':enumerate_active('glGetActiveUniform',active_uniforms.value)}
  def query_vertex_state(index):
   fn=p.gles.gl.glGetVertexAttribiv;fn.argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)];values={}
   for label,pname in [('enabled',0x8622),('size',0x8623),('type',0x8625),('stride',0x8624),('array_buffer',0x889f),('divisor',0x88fe)]:
    value=C.c_int();fn(index,pname,C.byref(value));values[label]=value.value
   return values
  def capture_draw_contract(program):
   generation=current_program_generation(program)
   if not generation['linked']:raise RuntimeError(f'active Draw program {program} has no successful link snapshot')
   sources=generation['linked_sources'];by_type={row['shader_type']:row for row in sources}
   expected={0x8b31:'4c4b4f5c7d750eb980f61d2a5f6d01dd9abeebcd8e3e7410a10aeb7b6d835ece',0x8b30:'2c38ca9bf6a0cc8cd72300c6d3aa25246d74cd55232c2b333de28f0746364ad6'}
   observed={kind:row['source_sha256'] for kind,row in by_type.items()}
   if observed!=expected:raise RuntimeError(f'AirBrush Draw program source mismatch: {observed}')
   interface=query_program_interface(program);attrs={row['name']:row for row in interface['attributes']};uniforms={row['name']:row for row in interface['uniforms']}
   if {name:row['type'] for name,row in attrs.items()}!={'vertex':0x8b52,'transform':0x8b52}:raise RuntimeError(f'AirBrush active attributes mismatch: {attrs}')
   if {name:row['type'] for name,row in uniforms.items()}!={'ProjectionMatrix':0x8b5c,'sampler0':0x8b5e,'inputColor':0x8b52}:raise RuntimeError(f'AirBrush active uniforms mismatch: {uniforms}')
   if {name:row['location'] for name,row in attrs.items()}!={'vertex':0,'transform':1}:raise RuntimeError(f'AirBrush attribute locations mismatch: {attrs}')
   if {name:row['location'] for name,row in uniforms.items()}!={'ProjectionMatrix':0,'sampler0':1,'inputColor':2}:raise RuntimeError(f'AirBrush uniform locations mismatch: {uniforms}')
   sampler_uniform=C.c_int();p.gles.gl.glGetUniformiv.argtypes=[C.c_uint,C.c_int,C.POINTER(C.c_int)];p.gles.gl.glGetUniformiv(program,uniforms['sampler0']['location'],C.byref(sampler_uniform))
   active_texture=query_int(0x84e0)
   if active_texture!=0x84c0 or sampler_uniform.value!=0:raise RuntimeError(f'AirBrush sampler does not select texture unit zero: active={active_texture:#x}, sampler0={sampler_uniform.value}')
   pattern_texture=query_int(0x8069)
   query_error_before=p.gles.f['glGetError']()
   if query_error_before:raise RuntimeError(f'GL error before AirBrush texture-state queries: {query_error_before:#x}')
   def query_texture_parameters():
    fn=p.gles.gl.glGetTexParameteriv;fn.argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)];values={}
    for label,pname in [('min_filter',0x2801),('mag_filter',0x2800),('wrap_s',0x2802),('wrap_t',0x2803),('base_level',0x813c),('max_level',0x813d)]:
     value=C.c_int();fn(0x0de1,pname,C.byref(value));values[label]=value.value
    return values
   texture_parameters=query_texture_parameters()
   query_error_after_texture=p.gles.f['glGetError']()
   sampler_binding=C.c_int();p.gles.gl.glGetIntegerv(0x8919,C.byref(sampler_binding))
   query_error_after_sampler_binding=p.gles.f['glGetError']()
   sampler_parameters=None;native_sampler=None
   if sampler_binding.value:
    native_sampler=next((key for key,value in p.native_samplers.items() if value==sampler_binding.value),None)
    if native_sampler is None:raise RuntimeError(f'bound GLES sampler {sampler_binding.value} has no captured native sampler identity')
    fn=p.gles.gl.glGetSamplerParameteriv;fn.argtypes=[C.c_uint,C.c_uint,C.POINTER(C.c_int)];sampler_parameters={}
    for label,pname in [('min_filter',0x2801),('mag_filter',0x2800),('wrap_s',0x2802),('wrap_t',0x2803)]:
     value=C.c_int();fn(sampler_binding.value,pname,C.byref(value));sampler_parameters[label]=value.value
    query_error_after_sampler_parameters=p.gles.f['glGetError']()
   else:query_error_after_sampler_parameters=0
   effective_parameters=sampler_parameters or texture_parameters
   expected_parameters={'min_filter':0x2600,'mag_filter':0x2600,'wrap_s':0x812f,'wrap_t':0x812f}
   query_errors=[query_error_before,query_error_after_texture,query_error_after_sampler_binding,query_error_after_sampler_parameters]
   if any(query_errors):raise RuntimeError(f'GL error during AirBrush texture-state queries: {query_errors}')
   pattern=[row for row in p.texture_upload_records if (row['width'],row['height'],row['format'],row['type'],row['bytes'],row['sha256'])==(96,96,0x1908,0x1401,36864,'8c9fc495d6fd01b3e9b04d10d5e1f95d7b2969c7bbc0c7bfc66d94e325f18873')]
   if len(pattern)!=1 or pattern[0]['texture']!=pattern_texture:raise RuntimeError(f'AirBrush draw does not bind the exact uploaded pattern texture: bound={pattern_texture}, uploads={pattern}')
   vertex=query_vertex_state(attrs['vertex']['location']);transform=query_vertex_state(attrs['transform']['location'])
   uploads={}
   for row in p.gl_calls:
    if row.get('call')=='glBufferData' and row.get('buffer'):
     uploads[row['buffer']]={'bytes':row['size'],'sha256':row.get('dataSha256'),'data_hex':row.get('dataHex')}
   if not (vertex['enabled']==1 and vertex['size']==4 and vertex['type']==0x1406 and vertex['stride']==16 and vertex['divisor']==0 and vertex['array_buffer']):raise RuntimeError(f'AirBrush vertex buffer/layout mismatch: {vertex}')
   if not (transform['enabled']==1 and transform['size']==4 and transform['type']==0x1406 and transform['stride']==16 and transform['divisor']==1 and transform['array_buffer'] and transform['array_buffer']!=vertex['array_buffer']):raise RuntimeError(f'AirBrush transform buffer/layout mismatch: {transform}')
   vertex_upload=uploads.get(vertex['array_buffer']);transform_upload=uploads.get(transform['array_buffer'])
   if not vertex_upload or vertex_upload['bytes']!=64:raise RuntimeError(f'AirBrush vertex attribute has no native 64-byte quad upload: {vertex_upload}')
   if not transform_upload or transform_upload['bytes']!=len(stream) or transform_upload['sha256']!=hashlib.sha256(stream).hexdigest():raise RuntimeError(f'AirBrush transform attribute has no exact Drawing-stream upload: {transform_upload}')
   expected_sampling={'min_filter':0x2600,'mag_filter':0x2600,'wrap_s':0x812f,'wrap_t':0x812f}
   sampling_calls=[row for row in p.gl_calls if row.get('call') in ('glTexParameteri','glSamplerParameteri','glBindSampler','glGenSamplers')]
   texture_level_calls=[row for row in p.gl_calls if row.get('call') in ('glTexImage2D','glTexStorage2D','glGenerateMipmap') and row.get('texture')==pattern_texture]
   mip_filters={0x2700,0x2701,0x2702,0x2703};requires_mipmaps=effective_parameters['min_filter'] in mip_filters
   level0_defined=any(row.get('call') in ('glTexImage2D','glTexStorage2D') and row.get('level',0)==0 for row in texture_level_calls)
   mipmap_calls=[row for row in p.gl_calls if row.get('call')=='glGenerateMipmap' and row.get('texture')==pattern_texture]
   return {'program':program,'generation':generation['generation'],'source_hashes':{str(kind):row['source_sha256'] for kind,row in by_type.items()},'source_bytes':{str(kind):row['source_bytes'] for kind,row in by_type.items()},'active_interface':interface,'sampler0_unit':sampler_uniform.value,'active_texture':active_texture,'pattern_texture':pattern_texture,'pattern_upload':{key:pattern[0][key] for key in ('width','height','format','type','texture','bytes','sha256')},'texture_parameters':texture_parameters,'bound_sampler_object':{'mesa_id':sampler_binding.value,'native_id':native_sampler,'parameters':sampler_parameters},'effective_sampling_parameters':effective_parameters,'nearest_clamp_contract_matches_live_state':effective_parameters==expected_sampling,'sampling_setup_calls':sampling_calls,'texture_level_setup_calls':texture_level_calls,'mipmap_generation_calls':mipmap_calls,'effective_min_filter_requires_mipmaps':requires_mipmaps,'level0_defined_for_pattern_texture':level0_defined,'sampling_completeness_under_captured_effective_min_filter':level0_defined and (not requires_mipmaps or bool(mipmap_calls)),'sampling_query_gl_errors':query_errors,'vertex_buffer':{'state':vertex,'upload':{key:vertex_upload[key] for key in ('bytes','sha256')}},'transform_buffer':{'state':transform,'upload':{key:transform_upload[key] for key in ('bytes','sha256')}}}
  # Replace only the inherited host adapter's unsupported GLES2 GL_RED diagnostic readback; preserve the native draw call.
  def native_draw_adapter(self):
   vals=[self.x(i) for i in range(4)];state=self.gl_state();program=state['program'];contract=capture_draw_contract(program)
   gi=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(self.gles.gl.glGetIntegerv);target={}
   for label,pname,n in [('draw_fbo',0x8ca6,1),('read_fbo',0x8caa,1),('viewport',0x0ba2,4),('pack_alignment',0x0d05,1)]:
    out=(C.c_int*n)();gi(pname,out);target[label]=list(out)
   value=C.c_int();gi(0x0c02,C.byref(value));target['read_buffer']=value.value
   if target['draw_fbo']!=[1] or target['read_fbo']!=[1] or target['viewport']!=[0,0,256,256]:raise RuntimeError(f'unexpected target before native draw: {target}')
   if not hasattr(self,'airbrush_read_pixels_fn'):
    self.airbrush_read_pixels_fn=self.gles.gl.glReadPixels;self.airbrush_read_pixels_fn.restype=None;self.airbrush_read_pixels_fn.argtypes=[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p]
   self.gles.gl.glFinish();before=C.create_string_buffer(256*256*4);self.airbrush_read_pixels_fn(0,0,256,256,0x1908,0x1401,C.cast(before,C.c_void_p))
   before_error=self.gles.f['glGetError']()
   if before_error:raise RuntimeError(f'GL error on pre-draw full RGBA readback: {before_error:#x}')
   self.before_draw_readback={'state':target,'bytes':len(before.raw),'sha256':hashlib.sha256(before.raw).hexdigest(),'nonzero_pixels':sum(before.raw[i:i+4]!=bytes(4) for i in range(0,len(before.raw),4)),'data':bytes(before.raw)}
   self.draw_contract=contract;self.gl_calls.append({'call':'glDrawArraysInstanced','values':vals,'state':state,'active_program_contract':contract});self.gles.f['glDrawArraysInstanced'](*vals)
  p.gl_draw_arrays_instanced=types.MethodType(native_draw_adapter,p);p.callbacks['glDrawArraysInstanced']=p.gl_draw_arrays_instanced
  trace('before-draw')
  p.call('SPen::AirBrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt,bitmap,mat],limit=5_000_000)
  gl_stage_errors['after_draw']=p.gles.f['glGetError']()
  assert gl_stage_errors['after_draw']==0 and not p.draw_gl_errors,(gl_stage_errors,p.draw_gl_errors)
  get_i=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetIntegerv)
  def gi(k,n=1):
   v=(C.c_int*n)();get_i(k,v);return list(v)
  draw_calls=[x for x in p.gl_calls if x.get('call') in ('glDrawArraysInstanced','glDrawArrays')]
  enabled=C.CFUNCTYPE(C.c_ubyte,C.c_uint)(p.gles.gl.glIsEnabled)
  post_draw_state={'draw_fbo':gi(0x8ca6)[0],'read_fbo':gi(0x8caa)[0],'read_buffer':gi(0x0c02)[0],'draw_attachment_object':attachment(0x8ca9),'read_attachment_object':attachment(0x8ca8),'viewport':gi(0x0ba2,4),'pack_alignment':gi(0x0d05)[0],
   'raster_enables':{name:bool(enabled(cap)) for name,cap in [('blend',0x0be2),('scissor',0x0c11),('depth',0x0b71),('stencil',0x0b90),('cull',0x0b44),('dither',0x0bd0),('sample_alpha_to_coverage',0x809e),('sample_coverage',0x80a0)]}}
  p.canvas_reference=outbmp
  task_runs=[{'run':hex(send_task['run']),'task':hex(m['completed_send_task']),'phase':'host-scheduled_native_task_body_after_rt_init_before_update','task_vtable':hex(send_task['vtable']),'task_bytes_hex':send_task['task_bytes'],'task_receiver_matches_rt':struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),16)[0]==rt,'task_vector_matches_source_object':struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),40)[0]==vecobj,'native_rt_slots_after_init':[hex(x) for x in rt_slots_after_init],'native_rt_slots_after_send':[hex(x) for x in published_slots],'native_rt_slots_before_update':[hex(x) for x in rt_slots_before_update],'native_rt_slots_after_update':[hex(x) for x in rt_slots_after_update],'vector_object_descriptor_before_update':[hex(x) for x in vector_storage_before_update],'native_rt_plus88':hex(p.get(rt+88,'Q')[0]),'vector_same_object':p.get(rt+88,'Q')[0]==vecobj}]
  # Do not execute the deferred redraw callback: its synthetic PenCanvas reaches an unimplemented native bitmap interface.
  # The same published vector is consumed directly by native Update and Draw above.
  readback_pre_state={'draw_fbo':gi(0x8ca6)[0],'read_fbo':gi(0x8caa)[0],'read_buffer':gi(0x0c02)[0],'draw_attachment_object':attachment(0x8ca9),'read_attachment_object':attachment(0x8ca8),'viewport':gi(0x0ba2,4),'pack_alignment':gi(0x0d05)[0]}
  gl_stage_errors['before_readback']=p.gles.f['glGetError']()
  assert gl_stage_errors['before_readback']==0,gl_stage_errors
  if not hasattr(p,'airbrush_read_pixels_fn'):
   p.airbrush_read_pixels_fn=p.gles.gl.glReadPixels;p.airbrush_read_pixels_fn.restype=None;p.airbrush_read_pixels_fn.argtypes=[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p]
  p.gles.gl.glFinish();pixels=C.create_string_buffer(256*256*4);p.airbrush_read_pixels_fn(0,0,256,256,0x1908,0x1401,C.cast(pixels,C.c_void_p))
  gl_stage_errors['after_readback']=p.gles.f['glGetError']()
  assert gl_stage_errors['after_readback']==0,gl_stage_errors
  post_read_state={'draw_fbo':gi(0x8ca6)[0],'read_fbo':gi(0x8caa)[0],'read_buffer':gi(0x0c02)[0],'draw_attachment_object':attachment(0x8ca9),'read_attachment_object':attachment(0x8ca8),'viewport':gi(0x0ba2,4),'pack_alignment':gi(0x0d05)[0]}
  item=next(x for x in m['deferred_queue_tasks'] if x['run']==0x1045e6c)
  delete_task_bytes=bytes.fromhex(item['task_bytes'])
  assert struct.unpack_from('<Q',delete_task_bytes,16)[0]==vecobj
  vector_storage_before_delete=p.get(vecobj,'3Q')
  assert vector_storage_before_delete==vector_storage_before_update
  slots_before_delete=p.get(rt+80,'2Q')
  p.x(0,item['task']);p.u.reg_write(R.UC_ARM64_REG_SP,0x200ef000);p.u.reg_write(R.UC_ARM64_REG_LR,m['oc'].STOP);p.u.emu_start(item['run'],m['oc'].STOP,count=5_000_000)
  if p.u.reg_read(R.UC_ARM64_REG_PC)!=m['oc'].STOP:raise RuntimeError('deferred vector Delete task failed')
  trace('readback-and-delete-done')
  task_runs.append({'run':hex(item['run']),'task':hex(item['task']),'phase':'after_gl_finish_and_full_rgba_readback','task_vtable':hex(item['vtable']),'task_bytes_hex':item['task_bytes'],'task_vector_matches_source_object':struct.unpack_from('<Q',delete_task_bytes,16)[0]==vecobj,'rt_slots_before_delete':[hex(x) for x in slots_before_delete],'rt_slots_after_delete':[hex(x) for x in p.get(rt+80,'2Q')],'vector_pointer':hex(vecobj)})
  get_i=C.CFUNCTYPE(None,C.c_uint,C.POINTER(C.c_int))(p.gles.gl.glGetIntegerv);fb=C.c_int();vp=(C.c_int*4)();get_i(0x8ca6,C.byref(fb));get_i(0x0ba2,vp)
  nonzero_indices=[i for i in range(256*256) if pixels.raw[i*4:i*4+4]!=bytes(4)]
  before_pixels=p.before_draw_readback['data']
  changed_pixels=[i for i in range(256*256) if before_pixels[i*4:i*4+4]!=pixels.raw[i*4:i*4+4]]
  blend=[x for x in p.gl_calls if 'Blend' in x.get('call','') or x.get('call') in ('glEnable','glDisable')][-12:]
  draw_calls=[x for x in p.gl_calls if x.get('call') in ('glDrawArraysInstanced','glDrawArrays')]
  texture_allocations=[x for x in p.gl_calls if x.get('call')=='glTexImage2D']
  attachments=[x for x in p.gl_calls if x.get('call')=='glFramebufferTexture2D']
  buffer_uploads=[x for x in p.gl_calls if x.get('call')=='glBufferData' and x.get('size')]
  instance_uploads=[x for x in buffer_uploads if x.get('size')==416]
  assert len(instance_uploads)==1 and instance_uploads[0]['dataSha256']==hashlib.sha256(stream).hexdigest()
  assert bytes.fromhex(instance_uploads[0]['dataHex'])==stream
  assert len(attachments)==1 and attachments[0]['framebuffer']==1 and attachments[0]['args'][3]==2
  output_allocations=[x for x in texture_allocations if x['width']==256 and x['height']==256]
  assert len(output_allocations)==1 and (output_allocations[0]['internalFormat'],output_allocations[0]['format'],output_allocations[0]['type'])==(32856,6408,5121)
  complete=[x for x in p.gl_calls if x.get('call')=='glCheckFramebufferStatus' and x.get('framebuffer')==1]
  assert complete and complete[-1]['status']==36053
  assert p.pre_draw_state['draw_fbo']==p.pre_draw_state['read_fbo']==1
  assert p.pre_draw_state['draw_attachment_object']==p.pre_draw_state['read_attachment_object']==2
  assert post_draw_state['draw_fbo']==post_draw_state['read_fbo']==1 and post_draw_state['draw_attachment_object']==post_draw_state['read_attachment_object']==2
  assert readback_pre_state['draw_fbo']==readback_pre_state['read_fbo']==1 and readback_pre_state['draw_attachment_object']==readback_pre_state['read_attachment_object']==2
  assert post_read_state['draw_fbo']==post_read_state['read_fbo']==1 and post_read_state['draw_attachment_object']==post_read_state['read_attachment_object']==2
  assert fb.value==1 and list(vp)==[0,0,256,256]
  assert all(value==0 for value in gl_stage_errors.values()) and not p.draw_gl_errors
  native_libs=dict(case['native_libraries_sha256'])
  native_libs['Renderer']=hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so').read_bytes()).hexdigest()
  native_libs['libc++_shared.so']=hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a/libc++_shared.so').read_bytes()).hexdigest()
  helper_names=['airbrush_saved_caller_native.py','airbrush-saved-caller-native.json','airbrush_saved_native.py','airbrush_texture_native.py','inkpen_paint_native.py','oblique_canvas_native.py','fountain_native.py','elf_plt.py','marker3_paint_scale_native.py']
  helper_hashes={name:hashlib.sha256((root/'conformance'/name).read_bytes()).hexdigest() for name in helper_names}
  color_calls=[x for x in case['native_selection']['paint_color_calls'] if x.get('phase')=='draw_object_stroke']
  requested_hex=f'0x{src_argb:08x}'
  assert any(int(x['argb_u32'],16)==src_argb for x in color_calls), (requested_hex,color_calls)
  case['input']['color_argb_u32']=requested_hex
  color_call_summary=[{key:row[key] for key in ('method','phase','argb_u32','size_f32_bits') if key in row} for row in color_calls]
  caller_summary={'case_id':case['case_id'],'pen_name':case['input']['pen_name'],'advanced_setting_mode':case['input']['advanced_setting_mode'],'advanced_setting_ascii':case['input']['advanced_setting_ascii'],'profile':case['profile'],'factory_symbol':case['native_selection']['factory_symbol'],'selected_drawable_constructor_default_version':case['native_selection']['manual_constructor_default_version'],'source_color_calls':color_call_summary}
  report={'case_id':f"airbrush-v2-drawing-to-mesa-source-alpha-{(src_argb>>24)&255:02x}",
   'source_argb_u32':requested_hex,'caller_case':caller_summary,
   'native_route':{'selected_drawable_default_profile_version':case['profile']['native_drawable_profile_version'],'selected_drawable_version_constructor_default':case['native_selection']['manual_constructor_default_version'],'advanced_setting_mode':case['input']['advanced_setting_mode'],'pen_name':case['input']['pen_name'],'source_color_call':color_call_summary,
    'native_send_data_task_run':'0x10506b4','send_data_task_ran_before_paint':True,'send_data_execution_phase':'host-scheduled native task body after native RT Init and before Update; natural queue dispatch is not claimed','native_send_task_vtable':hex(send_task['vtable']),'native_send_task_bytes_hex':send_task['task_bytes'],'task_receiver_matches_selected_rt':struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),16)[0]==rt,'task_vector_matches_source_object':struct.unpack_from('<Q',bytes.fromhex(send_task['task_bytes']),40)[0]==vecobj,'rt_slots_after_init':[hex(x) for x in rt_slots_after_init],'rt_slots_after_send_data':[hex(x) for x in published_slots],'rt_slots_before_update':[hex(x) for x in rt_slots_before_update],'rt_slots_after_update':[hex(x) for x in rt_slots_after_update],'rt_slots_before_delete':[hex(x) for x in slots_before_delete],'rt_slots_after_delete':[hex(x) for x in p.get(rt+80,'2Q')],'vector_object_descriptor_before_update':[hex(x) for x in vector_storage_before_update],'vector_object_descriptor_before_delete':[hex(x) for x in vector_storage_before_delete],'same_vector_object_published_to_rt_plus88':p.get(rt+88,'Q')[0]==vecobj,'host_wrote_rt_plus88':False,'vector_bytes':len(stream),'vector_hex':stream.hex(),'vector_sha256':hashlib.sha256(stream).hexdigest(),'vector_bytes_sha256_at_update':hashlib.sha256(vector_bytes_at_update).hexdigest(),'vector_bytes_sha256_after_update':hashlib.sha256(vector_bytes_after_update).hexdigest(),'vector_upload_sha256':instance_uploads[0]['dataSha256'],
    'pending_pen_cache_redraw_task_run':'0x104676c','pending_redraw_reason':'Synthetic PenCanvas bitmap virtual interface is incomplete; the native Redraw callback remains deferred. The same published vector object is consumed by the RT native Update and Draw calls directly.',
    'native_delete_task_run':'0x1045e6c','native_delete_executed_after_gl_finish_and_readback':True,'vector_free_host_boundary':'caller allocator free is a host no-op; native vector destruction task executes, but allocator release behavior is not claimed.'},
   'render_target':{'size_px':[256,256],'format':'RGBA8','gl_tex_image_call':output_allocations[0],'framebuffer_complete_status':complete[-1]['status'],'framebuffer':1,'color_attachment':attachments[-1]['args'][3],'color_attachment_query_object':p.pre_draw_state['color_attachment_object'],'read_fbo':p.pre_draw_state['read_fbo'],'draw_fbo':p.pre_draw_state['draw_fbo'],'pre_draw_viewport':p.pre_draw_state['viewport'],'pre_draw_raster_state':p.pre_draw_state['raster_enables'],'pre_draw_scissor_box':p.pre_draw_state['scissor_box'],'pre_draw_depth_function':p.pre_draw_state['depth_function'],'pre_draw_depth_write':p.pre_draw_state['depth_write'],'pre_draw_state':p.pre_draw_state,'before_draw_readback':{key:p.before_draw_readback[key] for key in ('state','bytes','sha256','nonzero_pixels')},'post_draw_viewport':list(vp),'post_draw_state':post_draw_state,'readback_pre_state':readback_pre_state,'post_read_state':post_read_state,'color_write_mask':p.pre_draw_state['color_write_mask'],'pack_alignment':p.pre_draw_state['pack_alignment'],'initial_rgba_sha256':hashlib.sha256(bytes(256*256*4)).hexdigest()},
   'native_draw':{'calls':draw_calls,'buffer_uploads':[{'buffer':x.get('buffer'),'size':x['size'],'sha256':x['dataSha256'],'data_hex':x['dataHex']} for x in buffer_uploads],'attributes':p.native_attribute_calls,'blend_gl_calls':blend,'uniform_calls':[x for x in p.gl_calls if x.get('call','').startswith('glUniform')],'rt_rgba_f32':p.get(rt+44,'4f'),'shader_compilations':[{'generation':x['generation'],'shader_type':x['type'],'source_parts':x['compiled_source_parts'],'source_bytes':x['compiled_source_bytes'],'source_sha256':x['compiled_source_sha256'],'compile_status':x['compile_status']} for x in p.shader_generation_rows],'program_link_generations':[{'generation':x['generation'],'link_status':x['link_status'],'linked':x['linked'],'attached_shader_generations':[{'generation':a['shader_generation'],'shader_type':a['shader_type'],'source_bytes':a['source_bytes'],'source_sha256':a['source_sha256']} for a in x['linked_sources']]} for x in p.program_generation_rows],'active_draw_program_contract':p.draw_contract,'texture_uploads':p.texture_upload_records,'texture_sampled_by_captured_fragment_shader':True,'gl_errors_after_init_update_target_draw_read':gl_stage_errors},
   'readback_rgba8':{'width':256,'height':256,'bytes':len(pixels.raw),'sha256':hashlib.sha256(pixels.raw).hexdigest(),'nonzero_pixels':len(nonzero_indices),'changed_pixels_from_pre_draw':len(changed_pixels),'changed_pixels_sha256':hashlib.sha256(b''.join(struct.pack('<I',i) for i in changed_pixels)).hexdigest(),'format':'GL_RGBA','format_enum':0x1908,'type':'GL_UNSIGNED_BYTE','type_enum':0x1401,'read_fbo':readback_pre_state['read_fbo'],'read_buffer':readback_pre_state['read_buffer'],'pack_alignment':readback_pre_state['pack_alignment'],'gl_error_before':gl_stage_errors['before_readback'],'gl_error_after':gl_stage_errors['after_readback'],'first_nonzero_pixels':[(i%256,i//256,list(pixels.raw[i*4:i*4+4])) for i in nonzero_indices[:12]],'zlib_base64':base64.b64encode(zlib.compress(pixels.raw,9)).decode('ascii')},
   'message_tasks':task_runs,
   'host_boundaries':['Synthetic Drawing input arrays, IContext and task queue; native Model/Drawing selection and AddPoint-generated vector execute in pinned DSOs.','The genuine SendDataToGPU WorkerTask body publishes the actual source vector object at selected RT+88 after native RT Init and before Update, but the host schedules that Run body directly; natural queue dispatch is not claimed. No host write changes RT+88. The native PenCacheRedrawCallback is deferred because its synthetic PenCanvas lacks the native bitmap virtual interface.','The same selected RT runs native Init/Update/Draw. Native graphics bitmap factory and canvas create the RGBA8 target; Mesa GLES host callbacks execute GL. A host adapter bypasses inherited GLES2-incompatible GL_RED debug readback while forwarding glDrawArraysInstanced unchanged.','The actual native vector Delete WorkerTask body runs only after glFinish and the full RGBA readback have copied the pixels. Host allocator free is a no-op. No Android-device GPU or full Renderer message queue lifecycle is claimed.'],
   'host_renderer_setup':{'renderer_base':hex(base),'capability_pair_address':hex(base+0x73388),'host_seeded_capability_pair':[4096,4096],'host_seeded_flag_address':hex(base+0x733c0),'host_seeded_flag':1,'capability_values_are_mesa_queried':False,'guest_egl_current_context_stub':'0xabc','scope':'Host-initialized Renderer capability globals and EGL identity; native .init_array and factories execute, complete Android Renderer initialization is not claimed.'},
   'source_hashes':{'runner':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_libraries':native_libs,'helpers':helper_hashes},
   'host_gles':p.gles.identity}
  return report
 except Exception as e:
  pc=p.u.reg_read(R.UC_ARM64_REG_PC); print('INIT_FAIL',repr(e),'pc',hex(pc),'plt',p.plt.get(pc),'lr',hex(p.u.reg_read(R.UC_ARM64_REG_LR)),'regs',[hex(p.x(i)) for i in range(8)])
  raise

if __name__=='__main__': main()
