import argparse,importlib.util,sys,contextlib,io,hashlib,struct,os,subprocess,re,json,base64,zlib
from pathlib import Path
from unicorn import arm64_const as R
from unicorn import UC_HOOK_MEM_INVALID
CLI=argparse.ArgumentParser(description='Replay saved InkPen V4 Draw through native Renderer and Mesa')
group=CLI.add_mutually_exclusive_group(required=True);group.add_argument('--check-fixture',action='store_true');group.add_argument('--write-fixture',action='store_true');group.add_argument('--print-capture',action='store_true');CLI_ARGS=CLI.parse_args()
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'conformance'))
sys.argv=['inkpen_saved_caller_native.py','--print-capture','--advanced-setting','empty','--width','1920','--height','1080']
spec=importlib.util.spec_from_file_location('caller',ROOT/'conformance/inkpen_saved_caller_native.py');m=importlib.util.module_from_spec(spec)
# Resolve the inherited Canvas::GetDataQueue stub to the same native queue
# installed in this caller's PenGLDataManager before ObjectDrawing dispatch.
import oblique_canvas_native as _oc
_old_canvas_hook=_oc.CanvasProbe.hook_canvas
def _inkpen_real_queue_hook(self,uc,address,size,data):
 if address==0x200f2040 and getattr(self,'native_task_queue',0):
  queue=self.native_task_queue; vtable=struct.unpack('<Q',self.u.mem_read(queue,8))[0]; post=struct.unpack('<Q',self.u.mem_read(vtable+16,8))[0]
  self.native_canvas_queue_events.append({'queue':hex(queue),'vtable':hex(vtable),'post':hex(post)})
  self.x(0,queue); uc.reg_write(R.UC_ARM64_REG_PC,uc.reg_read(R.UC_ARM64_REG_LR)); return
 return _old_canvas_hook(self,uc,address,size,data)
_oc.CanvasProbe.hook_canvas=_inkpen_real_queue_hook
caller_source=(ROOT/'conformance/inkpen_saved_caller_native.py').read_text()
caller_source='queue_task_events=[]\nqueue_task_active=[]\n'+caller_source
queue_setup="manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.put(queue,'Q',qvt);"
queue_setup_real="manager=p.alloc(256); queue=p.alloc(64); qvt=p.alloc(64); p.native_task_queue=queue; p.native_canvas_queue_events=[]; p.deferred_native_tasks=[]; p.put(queue,'Q',qvt);"
if caller_source.count(queue_setup)!=1: raise RuntimeError('InkPen caller queue setup anchor changed')
caller_source=caller_source.replace(queue_setup,queue_setup_real)
task_anchor="  if not method: raise RuntimeError('task has null Run')"
task_defer="  _words=bytes(u.mem_read(task,64)); _rt=int(inkpen_buffer_events[-1]['rt'],16) if inkpen_buffer_events else 0; _member=struct.unpack_from('<Q',_words,24)[0]; _symbol=next((name for name,address in p.syms.items() if address==_member and 'PenDrawableRT::' in name),None); _event={'queue':hex(p.x(0)),'task':hex(task),'vtable':hex(vt),'run':hex(method),'phase':phase,'words':_words.hex(),'member_callback':hex(_member),'member_symbol':_symbol,'vector_object':hex(struct.unpack_from('<Q',_words,40)[0]),'rt':hex(_rt),'slots_before':{str(o):hex(p.get(_rt+o,'Q')[0]) for o in (80,88,96)} if _rt else {}}; queue_task_events.append(_event); queue_task_active.append(_event)\n  if vt in (0x10774c8,0x1077528): _event['deferred']=True; _event['slots_after']=dict(_event['slots_before']); p.deferred_native_tasks.append({'task':hex(task),'vtable':hex(vt),'run':hex(method)}); queue_task_active.pop(); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR)); return\n  if not method: raise RuntimeError('task has null Run')"
if caller_source.count(task_anchor)!=1: raise RuntimeError('InkPen caller task dispatch anchor changed')
caller_source=caller_source.replace(task_anchor,task_defer)
return_anchor=" elif a==0x20001020:\n  caller=p._task_returns.pop(); p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)"
return_body=" elif a==0x20001020:\n  caller=p._task_returns.pop(); _event=queue_task_active.pop(); _rt=int(_event['rt'],16); _event['slots_after']={str(o):hex(p.get(_rt+o,'Q')[0]) for o in (80,88,96)} if _rt else {}; p.x(0,1); u.reg_write(R.UC_ARM64_REG_PC,caller)"
if caller_source.count(return_anchor)!=1: raise RuntimeError('InkPen task-return anchor changed')
caller_source=caller_source.replace(return_anchor,return_body)
with contextlib.redirect_stdout(io.StringIO()):exec(compile(caller_source,str(ROOT/'conformance/inkpen_saved_caller_native.py'),'exec'),m.__dict__)
p=m.p; LIB=m.LIBS; base=0x9000000; renderer=LIB/'libSPenRenderer.so'; b=renderer.read_bytes(); assert hashlib.sha256(b).hexdigest()=='f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
# Map Renderer at a disjoint emulator base.
ph=struct.unpack_from('<Q',b,32)[0]; ent,n=struct.unpack_from('<HH',b,54); extent=0
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,ph+i*ent)
 if t==1:extent=max(extent,va+ms)
p.u.mem_map(base,(extent+0xfff)&~0xfff)
for i in range(n):
 t,fl,o,va,pa,fs,ms,al=struct.unpack_from('<II6Q',b,ph+i*ent)
 if t==1 and ms:p.u.mem_write(base+va,b[o:o+fs]);p.u.mem_write(base+va+fs,bytes(ms-fs) if ms>fs else b'')
# install exports and PLT
env=dict(os.environ);env['LD_LIBRARY_PATH']=str((ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu').resolve())
for line in subprocess.check_output(['nm','-D','-C',str(renderer)],text=True,env=env).splitlines():
 q=line.strip().split(' ',2)
 if len(q)==3 and q[1] in ('T','W'):p.syms[q[2]]=base+int(q[0],16)
dis=subprocess.check_output([str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'),'-d','-C','-j','.plt',str(renderer)],text=True,env=env)
for a,name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:',dis,re.M):p.plt[base+int(a,16)]=name
# apply Renderer dynamic relocs with caller's resolved symbols
# dynamic relocs relevant to internal/static and imported symbols
rawsyms={}
for line in subprocess.check_output(['nm','-D',str(renderer)],text=True,env=env).splitlines():
 q=line.split()
 if len(q)==3 and q[1].upper() in ('T','W','D','B','R','V'):rawsyms[q[2]]=base+int(q[0],16)
shoff=struct.unpack_from('<Q',b,40)[0]; se,sn,ss=struct.unpack_from('<HHH',b,58);secs=[struct.unpack_from('<IIQQQQIIQQ',b,shoff+i*se) for i in range(sn)]; names=b[secs[ss][4]:secs[ss][4]+secs[ss][5]];by={names[x[0]:names.find(b'\0',x[0])].decode():x for x in secs};ds,st=by['.dynsym'],by['.dynstr'];strings=b[st[4]:st[4]+st[5]];sy=[]
for off in range(ds[4],ds[4]+ds[5],ds[9]):
 no,inf,oth,ndx,val,size=struct.unpack_from('<IBBHQQ',b,off);sy.append((strings[no:strings.find(b'\0',no)].decode() if no else '',val))
for secname in ('.rela.dyn','.rela.plt'):
 sec=by.get(secname)
 if not sec:continue
 for off in range(sec[4],sec[4]+sec[5],sec[9]):
  where,info,add=struct.unpack_from('<QQq',b,off);typ,idx=info&0xffffffff,info>>32
  if typ==0x403:p.u.mem_write(base+where,struct.pack('<Q',base+add))
  elif typ in (0x401,0x402,0x101,0x1025,0x1026):
   name,val=sy[idx]; target=base+val if val else rawsyms.get(name,0) or p.syms.get(name,0)
   if target:p.u.mem_write(base+where,struct.pack('<Q',target+(add if typ in (0x401,0x101) else 0)))
rt=m.selected_rt
def inv(u,access,address,size,value,data):
 print('INVALID',access,hex(address),size,'pc',hex(u.reg_read(R.UC_ARM64_REG_PC)),'near',sorted([(hex(v),k) for k,v in p.syms.items() if abs(v-u.reg_read(R.UC_ARM64_REG_PC))<0x100])[:12],'lastpcs',[hex(v) for v in drawpcs],'x0x8',[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(9)],'x19x26',[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(19,27)])
 return False
p.u.hook_add(UC_HOOK_MEM_INVALID,inv)
drawpcs=[]
def trace_draw(u,a,z,d):
 if 0x5032fa0<=a<0x5034000:drawpcs.append(a);drawpcs[:]=drawpcs[-30:]
p.u.hook_add(4,trace_draw)
# Install native Renderer exports at distinct base and bounded factory products.
from pathlib import Path
import types
# DSO relocations/init for Renderer handled separately in this staged harness.
source=m.LIBS/'libSPenRenderer.so'
# factory callback implementations
state={'descriptor':None,'descriptors':[],'blend_objects':[],'graphics_uploads':[],'draws':[],'calls':[],'gl_target_events':[]}
def cap_blend():
 src=p.x(0);state['descriptor']=bytes(p.u.mem_read(src,40));state['descriptors'].append(state['descriptor']);out=p.alloc(128);state['blend_objects'].append(out);p.x(0,out)
def fake(name,fn):
 at=0x200f3000+len(state['calls'])*16;p.u.mem_write(at,bytes.fromhex('1f2003d5c0035fd6'));state['calls'].append({'name':name,'fn':fn,'address':at});return at
def fake_hook(u,a,z,d):
 for row in state['calls']:
  if row.get('address')==a:
   row['fn']();u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR));return
p.u.hook_add(4,fake_hook)
# callback native synthetic RT interfaces
def depth_create():
 o=p.alloc(16);v=p.alloc(32);p.put(o,'Q',v);p.put(v+24,'Q',fake('depth.activate',lambda:None));p.x(0,o)
def graphics_create():
 o=p.alloc(4096);object_id=len(state.setdefault('graphics_objects',[]));args=[p.x(i) for i in range(5)];row={'object':o,'buffers':[],'create_args':args,'descriptor_hex':bytes(p.u.mem_read(args[2],512)).hex(),'updates':[]};state['graphics_objects'].append(row)
 def update():
  idx,ptr,count=p.x(1),p.x(2),p.x(3);raw=bytes(p.u.mem_read(ptr,count*12)) if idx==1 else bytes(p.u.mem_read(ptr,64)); row['updates'].append({'index':idx,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()});state['graphics_uploads'].append({'buffer_index':idx,'byte_count':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'data':raw.hex()})
  payload=ip.C.create_string_buffer(raw);mesa.f['glBindBuffer'](0x8892,row['buffers'][idx]);mesa.f['glBufferData'](0x8892,len(raw),payload,0x88E4)
 def draw():
  args=[p.x(i) for i in range(6)];bound=ip.C.c_int();vp=(ip.C.c_int*4)();getin(0x8CA6,ip.C.byref(bound));getin(0x0BA2,vp);state['draws'].append({'object_id':object_id,'args':args,'buffers':row['buffers'],'framebuffer':bound.value,'viewport':list(vp)})
  if object_id==0:
   mesa.f['glBindBuffer'](0x8892,row['buffers'][0]);mesa.f['glEnableVertexAttribArray'](0);mesa.f['glVertexAttribPointer'](0,4,0x1406,0,16,ip.C.c_void_p(0));mesa.f['glVertexAttribDivisor'](0,0)
   mesa.f['glBindBuffer'](0x8892,row['buffers'][1]);mesa.f['glEnableVertexAttribArray'](1);mesa.f['glVertexAttribPointer'](1,3,0x1406,0,12,ip.C.c_void_p(0));mesa.f['glVertexAttribDivisor'](1,1)
   mesa.f['glDrawArraysInstanced'](5,args[1],args[2],args[3])
  else:
   mesa.f['glBindBuffer'](0x8892,row['buffers'][0]);mesa.f['glEnableVertexAttribArray'](0);mesa.f['glVertexAttribPointer'](0,2,0x1406,0,8,ip.C.c_void_p(0));mesa.f['glVertexAttribDivisor'](0,0)
   mesa.f['glDrawArrays'](5,args[1],args[2])
 ctor='SPen::GLES::GraphicsObject::GraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)'
 try:p.call(ctor,x=(o,*args),limit=5_000_000)
 except Exception as e:print('FACTORY_NATIVE_CTOR_FAIL',repr(e));raise
 state['graphics']=o;p.x(0,o)
import inkpen_paint_native as ip
mesa=ip.GLES(); gf=mesa.f; gltrace=[]
def _early_ids(kind):
 n,out=p.x(0),p.x(1);ids=(ip.C.c_uint*n)();ip.cfunc(mesa.gl,'glGen'+kind,None,[ip.C.c_int,ip.C.POINTER(ip.C.c_uint)])(n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)]
for _kind in ('Buffers','VertexArrays'):
 p.callbacks['glGen'+_kind]=lambda kind=_kind:_early_ids(kind)
def _early2(name):
 fn=ip.cfunc(mesa.gl,name,None,[ip.C.c_uint,ip.C.c_uint]);p.callbacks[name]=lambda fn=fn:fn(p.x(0),p.x(1))
def _early1(name):
 fn=ip.cfunc(mesa.gl,name,None,[ip.C.c_uint]);p.callbacks[name]=lambda fn=fn:fn(p.x(0))
for _name in ('glBindBuffer','glBindVertexArray','glVertexAttribDivisor'):_early2(_name)
for _name in ('glEnableVertexAttribArray','glDisableVertexAttribArray'):_early1(_name)
def _early_buffer_data():
 target,size,ptr,usage=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size)) if ptr and size else b'';buf=ip.C.create_string_buffer(raw) if raw else None;bound=ip.C.c_int();ip.cfunc(mesa.gl,'glGetIntegerv',None,[ip.C.c_uint,ip.C.POINTER(ip.C.c_int)])(0x8894,ip.C.byref(bound));ip.cfunc(mesa.gl,'glBufferData',None,[ip.C.c_uint,ip.C.c_ssize_t,ip.C.c_void_p,ip.C.c_uint])(target,size,buf,usage);state.setdefault('early_gl_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest()})
p.callbacks['glBufferData']=_early_buffer_data
def _early_attrib_pointer():
 index,size,typ,norm,stride,ptr=[p.x(i) for i in range(6)];ip.cfunc(mesa.gl,'glVertexAttribPointer',None,[ip.C.c_uint,ip.C.c_int,ip.C.c_uint,ip.C.c_ubyte,ip.C.c_int,ip.C.c_void_p])(index,size,typ,norm,stride,ip.C.c_void_p(ptr))
p.callbacks['glVertexAttribPointer']=_early_attrib_pointer
def _early_get_string():
 enum=p.x(0);ptr=ip.cfunc(mesa.gl,'glGetString',ip.C.c_void_p,[ip.C.c_uint])(enum);raw=ip.C.string_at(ptr) if ptr else b'';out=p.alloc(len(raw)+1);p.u.mem_write(out,raw+b'\0');p.x(0,out)
p.callbacks['glGetString']=_early_get_string
def _early_get_integer():
 pname,out=p.x(0),p.x(1);n=4 if pname==0x0BA2 else 1;v=(ip.C.c_int*n)();ip.cfunc(mesa.gl,'glGetIntegerv',None,[ip.C.c_uint,ip.C.POINTER(ip.C.c_int)])(pname,v);p.u.mem_write(out,bytes(memoryview(v).cast('B')))
p.callbacks['glGetIntegerv']=_early_get_integer
def gl_create_shader():
 kind=p.x(0);v=gf['glCreateShader'](kind);gltrace.append(['glCreateShader',kind,v]);p.x(0,v)
def gl_shader_source():
 sh,count,arr,lens=[p.x(i) for i in range(4)]; strings=[]; keep=[]; lengths=[]
 for i in range(count):
  ptr=struct.unpack('<Q',p.u.mem_read(arr+8*i,8))[0]; n=struct.unpack('<i',p.u.mem_read(lens+4*i,4))[0] if lens else -1
  raw=bytes(p.u.mem_read(ptr,n)) if n>=0 else bytes(p.u.mem_read(ptr,8192)).split(b'\0',1)[0]
  buf=ip.C.create_string_buffer(raw);keep.append(buf);strings.append(ip.C.cast(buf,ip.C.c_char_p));lengths.append(len(raw))
 gf['glShaderSource'](sh,count,(ip.C.c_char_p*count)(*strings),(ip.C.c_int*count)(*lengths));gltrace.append(['glShaderSource',sh,count,[x.value.decode('utf8','replace') for x in strings]])
def gl_getshaderiv():
 sh,pname,out=p.x(0),p.x(1),p.x(2);v=ip.C.c_int();gf['glGetShaderiv'](sh,pname,ip.C.byref(v));p.put(out,'I',v.value);gltrace.append(['glGetShaderiv',sh,pname,v.value])
def gl_getprogramiv():
 prog,pname,out=p.x(0),p.x(1),p.x(2);v=ip.C.c_int();gf['glGetProgramiv'](prog,pname,ip.C.byref(v));p.put(out,'I',v.value);gltrace.append(['glGetProgramiv',prog,pname,v.value])
def gl_info(which):
 sh,n,out=p.x(0),p.x(1),p.x(3);buf=ip.C.create_string_buffer(min(max(n,1),4096)); f=gf[which];f(sh,n,None,buf);p.u.mem_write(out,buf.raw)
def gl_link():
 prog=p.x(0);gf['glLinkProgram'](prog);gltrace.append(['glLinkProgram',prog])
def gl_loc():
 prog,name=p.x(0),bytes(p.u.mem_read(p.x(1),256)).split(b'\0',1)[0];v=gf['glGetUniformLocation'](prog,name);gltrace.append(['glGetUniformLocation',prog,name.decode(),v]);p.x(0,v)
def gl_u1f():
 loc,value=p.x(0),p.f(0);gf['glUniform1f'](loc,value);gltrace.append(['glUniform1f',loc,value.hex()])
def gl_u1i():
 loc,value=p.x(0),p.x(1);gf['glUniform1i'](loc,value);gltrace.append(['glUniform1i',loc,value])
def gl_u4f():
 loc=p.x(0);vals=[p.f(i) for i in range(4)];gf['glUniform4f'](loc,*vals);gltrace.append(['glUniform4f',loc,[v.hex() for v in vals]])
def gl_create_program():
 value=gf['glCreateProgram']();gltrace.append(['glCreateProgram',value]);p.x(0,value)
def gl_attach_shader():
 prog,shader=p.x(0),p.x(1);gf['glAttachShader'](prog,shader);gltrace.append(['glAttachShader',prog,shader])
def gl_um4():
 loc,n,tr,ptr=[p.x(i) for i in range(4)];vals=p.get(ptr,'f'*(16*n));gf['glUniformMatrix4fv'](loc,n,tr,(ip.C.c_float*(16*n))(*vals));gltrace.append(['glUniformMatrix4fv',loc,n,tr,[v.hex() for v in vals]])
getin=ip.cfunc(mesa.gl,'glGetIntegerv',None,[ip.C.c_uint,ip.C.POINTER(ip.C.c_int)])
def gl_program_binary_miss():
 if p.x(2):p.put(p.x(2),'i',0)
 if p.x(3):p.put(p.x(3),'I',0)
p.callbacks['glGetProgramBinary']=gl_program_binary_miss
p.callbacks['glProgramBinary']=lambda:(_ for _ in ()).throw(RuntimeError('unexpected host shader binary load'))
def host_vsnprintf():
 dest,size=p.x(0),p.x(1)
 if dest and size:p.u.mem_write(dest,b'\0')
 p.x(0,0)
p.callbacks['vsnprintf']=host_vsnprintf
file_objs={}
def _fopen():
 token=p.alloc(16);file_objs[token]=bytearray();p.x(0,token)
def _fread():
 ptr,size,count,token=[p.x(i) for i in range(4)];p.x(0,0)
def _fwrite():
 ptr,size,count,token=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size*count));file_objs.setdefault(token,bytearray()).extend(raw);p.x(0,count)
def _fclose():p.x(0,0)
def _strchr():
 ptr,ch=p.x(0),p.x(1)&255;raw=bytes(p.u.mem_read(ptr,4096));i=raw.find(bytes([ch]));p.x(0,ptr+i if i>=0 else 0)
p.callbacks['strchr']=_strchr
def _strncpy_chk():
 dst,src,n=p.x(0),p.x(1),p.x(2);
 if n>65536:raise RuntimeError('strncpy bound')
 raw=bytes(p.u.mem_read(src,n));z=raw.find(b'\0');raw=raw[:z]+bytes(n-z) if z>=0 else raw;p.u.mem_write(dst,raw);p.x(0,dst)
def _strlen_chk():
 ptr=p.x(0);raw=_cstring(ptr);p.x(0,len(raw))
def _strncpy_chk2():
 dst,src,n,dsize,ssize=[p.x(i) for i in range(5)]
 if n>65536 or n>dsize or n>ssize:raise RuntimeError('strncpy_chk2 out of bounds')
 raw=bytes(p.u.mem_read(src,n));z=raw.find(b'\0');raw=raw[:z]+bytes(n-z) if z>=0 else raw;p.u.mem_write(dst,raw);p.x(0,dst)
p.callbacks['__strncpy_chk2']=_strncpy_chk2
p.callbacks.update({'__strncpy_chk':_strncpy_chk,'__strlen_chk':_strlen_chk})
p.callbacks.update({'fopen':_fopen,'fread':_fread,'fwrite':_fwrite,'fclose':_fclose,'fflush':lambda:p.x(0,0),'setbuf':lambda:None,'fileno':lambda:p.x(0,0),'fsync':lambda:p.x(0,0)})
def gl_get_integer():
 pname,out=p.x(0),p.x(1);n=4 if pname==0x0BA2 else 1;v=(ip.C.c_int*n)();getin(pname,v);p.u.mem_write(out,bytes(memoryview(v).cast('B')));gltrace.append(['glGetIntegerv',pname,list(v)])
detach=ip.cfunc(mesa.gl,'glDetachShader',None,[ip.C.c_uint,ip.C.c_uint])
delete_shader=ip.cfunc(mesa.gl,'glDeleteShader',None,[ip.C.c_uint])
bind_attr=ip.cfunc(mesa.gl,'glBindAttribLocation',None,[ip.C.c_uint,ip.C.c_uint,ip.C.c_char_p])
def gl_bind_attrib():
 prog,index,name=p.x(0),p.x(1),_cstring(p.x(2));bind_attr(prog,index,name)
active_uniform=ip.cfunc(mesa.gl,'glGetActiveUniform',None,[ip.C.c_uint,ip.C.c_uint,ip.C.c_int,ip.C.POINTER(ip.C.c_int),ip.C.POINTER(ip.C.c_int),ip.C.POINTER(ip.C.c_uint),ip.C.c_char_p])
active_attrib=ip.cfunc(mesa.gl,'glGetActiveAttrib',None,[ip.C.c_uint,ip.C.c_uint,ip.C.c_int,ip.C.POINTER(ip.C.c_int),ip.C.POINTER(ip.C.c_int),ip.C.POINTER(ip.C.c_uint),ip.C.c_char_p])
def active(which):
 prog,idx,bufsize,lengthp,sizep,typep,namep=[p.x(i) for i in range(7)];ln,sz,kind=ip.C.c_int(),ip.C.c_int(),ip.C.c_uint();buf=ip.C.create_string_buffer(min(max(bufsize,1),4096));fn=active_uniform if which=='uniform' else active_attrib;fn(prog,idx,bufsize,ip.C.byref(ln) if lengthp else None,ip.C.byref(sz) if sizep else None,ip.C.byref(kind) if typep else None,buf)
 if lengthp:p.put(lengthp,'i',ln.value)
 if sizep:p.put(sizep,'i',sz.value)
 if typep:p.put(typep,'I',kind.value)
 if namep:p.u.mem_write(namep,buf.raw[:min(max(bufsize,0),4096)])
 gltrace.append(['glGetActive'+which.title(),prog,idx,buf.value.decode('utf8','replace')])
p.callbacks.update({'glDeleteShader':lambda:delete_shader(p.x(0)),'glDetachShader':lambda:detach(p.x(0),p.x(1)),'glBindAttribLocation':gl_bind_attrib,'glGetActiveUniform':lambda:active('uniform'),'glGetActiveAttrib':lambda:active('attrib'),'glGetIntegerv':gl_get_integer,'glCreateShader':gl_create_shader,'glShaderSource':gl_shader_source,'glCompileShader':lambda:(gf['glCompileShader'](p.x(0)),gltrace.append(['glCompileShader',p.x(0)])),'glGetShaderiv':gl_getshaderiv,'glGetShaderInfoLog':lambda:gl_info('glGetShaderInfoLog'),'glCreateProgram':gl_create_program,'glAttachShader':gl_attach_shader,'glLinkProgram':gl_link,'glGetProgramiv':gl_getprogramiv,'glGetProgramInfoLog':lambda:gl_info('glGetProgramInfoLog'),'glUseProgram':lambda:gf['glUseProgram'](p.x(0)),'glGetUniformLocation':gl_loc,'glUniform1f':gl_u1f,'glUniform1i':gl_u1i,'glUniform4f':gl_u4f,'glUniformMatrix4fv':gl_um4,'eglGetCurrentContext':lambda:p.x(0,0xabc)})
import zlib
p.callbacks['crc32']=lambda:p.x(0,zlib.crc32(bytes(p.u.mem_read(p.x(1),p.x(2))),p.x(0)) & 0xffffffff)
def _cstring(ptr,limit=4096): return bytes(p.u.mem_read(ptr,limit)).split(b'\0',1)[0]
def _strstr():
 hay,needle=p.x(0),p.x(1);h=_cstring(hay);n=_cstring(needle);i=h.find(n);p.x(0,hay+i if i>=0 else 0)
p.callbacks['strstr']=_strstr
def _strcmp():
 a,b=_cstring(p.x(0)),_cstring(p.x(1));p.x(0,(a>b)-(a<b))
p.callbacks['strcmp']=_strcmp
def _memchr():
 ptr,ch,n=p.x(0),p.x(1)&255,p.x(2);raw=bytes(p.u.mem_read(ptr,n));i=raw.find(bytes([ch]));p.x(0,ptr+i if i>=0 else 0)
p.callbacks['memchr']=_memchr
def _strncpy():
 dst,src,n=p.x(0),p.x(1),p.x(2)
 if n>65536:raise RuntimeError('strncpy size bound')
 raw=bytes(p.u.mem_read(src,n));z=raw.find(b'\0');raw=raw[:z]+bytes(n-z) if z>=0 else raw;p.u.mem_write(dst,raw);p.x(0,dst)
p.callbacks['strncpy']=_strncpy
def _posix_memalign():
 output,alignment,size=p.x(0),p.x(1),p.x(2)
 if not output or alignment<8 or alignment&(alignment-1) or alignment%8 or size>0x1000000:
  p.x(0,22);return
 raw=p.alloc(max(size,1)+alignment-1);pointer=(raw+alignment-1)&-alignment
 p.put(output,'Q',pointer);p.x(0,0)
p.callbacks['posix_memalign']=_posix_memalign
p.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)']=cap_blend
p.callbacks['SPen::RendererFactory::CreateDepthStencilState(SPen::DepthStencilStateDescriptor const&)']=depth_create
# Let the pinned RendererFactory wrapper reach RendererFactoryImpl and the real
# GraphicsObject::CreateBuffer path during selected RT::Init.
p.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)',None)
factory_trace=[];factory_addrs={base+0x459bc:'RendererFactory::CreateGraphicsObject',base+0x5ad14:'RendererFactoryImpl::GreateGraphicsObject',base+0x52f4c:'GraphicsObject::GraphicsObject',base+0x52fc0:'GraphicsObject::GraphicsObject()'}
def trace_factory(u,a,z,d):
 if a in factory_addrs:factory_trace.append({'symbol':factory_addrs[a],'pc':hex(a),'x0x5':[hex(p.x(i)) for i in range(6)]})
p.u.hook_add(4,trace_factory)
# Preserve actual same RT; capture geometry vector's native object from latest AddPoint callback.
ev=next(x for x in reversed(m.inkpen_buffer_events) if x['phase']=='saved_redraw')
vecobj=int(ev['vector_object'],16); print('CAP_VEC_DRAWING',hex(vecobj),p.u.mem_read(vecobj,24).hex(),len(bytes.fromhex(ev['after_bytes'])),hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest())
# Initialize the selected RT native object. Do not allocate another RT or replace its data.
p.put(rt+200,'Q',0);p.put(rt+216,'Q',0);p.put(rt+208,'Q',0)
# Construct ShaderManager singleton in the pinned Renderer image (its global ctor is normally run by Android loader).
shader_mgr=p.alloc(128);p.call('SPen::ShaderManager::ShaderManager()',[shader_mgr]);p.put(base+0x6d480,'Q',shader_mgr)
# Run actual shader constructors from the pinned InkPen and Renderer DSOs.
try:p.call('SPen::InkPenStrokeDrawableRTV1::Init()',x=(rt,),limit=5_000_000)
except Exception as e:print('RT_INIT_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'syms',[k for k in p.syms if 'RendererFactory' in k][:20]);raise
print('RT_INIT_OK',state['descriptor'].hex() if state['descriptor'] else None,'fields',[(o,p.u.mem_read(rt+o,8).hex()) for o in (80,88,120,200,208,216,224,232,240,248,256)],'shader_obj_bytes',[(o,p.u.mem_read(p.get(rt+o,'Q')[0],40).hex()) for o in (248,256)],'factorygraphics',state.get('graphics'))
# Bind the actual Renderer BlendStateObject implementation to each descriptor emitted by InkPen RT::Init.
for off,obj,desc in zip((224,232,240),state['blend_objects'],state['descriptors']):
 dp=p.alloc(40);p.u.mem_write(dp,desc);p.call('SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)',x=(obj,dp));vtable=base+0x6a078;p.put(obj,'Q',vtable);p.put(vtable+24,'Q',p.syms['SPen::GLES::BlendStateObject::Activate()']);p.put(rt+off,'Q',obj)
print('BLEND_DESCRIPTORS',[(off,d.hex()) for off,d in zip((224,232,240),state['descriptors'])])
# The caller's native queue has already installed the saved AddPoint vector in RT+88.
if p.get(rt+88,'Q')[0]!=vecobj or p.get(rt+96,'Q')[0]!=0: raise RuntimeError('native queue did not leave saved vector in RT+88 with RT+96 empty')
# Explicit Mesa targets: native shader programs, native blend activation and native Draw use this host EGL/GLES context.
W,H=128,64
textures=(ip.C.c_uint*2)();fbos=(ip.C.c_uint*2)();mesa.f['glGenTextures'](2,textures);mesa.f['glGenFramebuffers'](2,fbos)
# R8 mask plus RGBA8 output target.
for i,(internal,fmt) in enumerate(((0x8229,0x1903),(0x8058,0x1908))):
 mesa.f['glBindTexture'](0x0DE1,textures[i]);mesa.f['glTexParameteri'](0x0DE1,0x2801,0x2600);mesa.f['glTexParameteri'](0x0DE1,0x2800,0x2600);mesa.f['glTexParameteri'](0x0DE1,0x2802,0x812F);mesa.f['glTexParameteri'](0x0DE1,0x2803,0x812F);mesa.f['glTexImage2D'](0x0DE1,0,internal,W,H,0,fmt,0x1401,None);mesa.f['glBindFramebuffer'](0x8D40,fbos[i]);mesa.f['glFramebufferTexture2D'](0x8D40,0x8CE0,0x0DE1,textures[i],0);assert mesa.f['glCheckFramebufferStatus'](0x8D40)==0x8CD5
mesa.f['glViewport'](0,0,W,H);mesa.f['glBindFramebuffer'](0x8D40,fbos[0]);mesa.f['glClearColor'](0,0,0,0);mesa.f['glClear'](0x4000);mesa.f['glBindFramebuffer'](0x8D40,fbos[1]);mesa.f['glClear'](0x4000)
# Patch host CanvasProbe vtable boundaries to bind the mask/output FBOs while native RT::Draw owns sequencing.
render_ctx=p.alloc(16);render_vt=p.alloc(256);p.put(render_ctx,'Q',render_vt)
target_obj=p.alloc(16);target_vt=p.alloc(256);p.put(target_obj,'Q',target_vt)
sub=p.alloc(512);subvt=p.alloc(256);p.put(sub,'Q',subvt)
def newfake(name,fn):
 a=p.alloc(16);p.u.mem_write(a,bytes.fromhex('1f2003d5c0035fd6'));state['calls'].append({'name':name,'fn':fn,'address':a});return a
# PenCanvas context path binds the mask FBO; generic methods return the facade objects expected by InkPen RT::Draw.
pen_vt=p.get(p.pen_canvas,'Q')[0];p.put(pen_vt+192,'Q',newfake('canvas.get_renderer',lambda:p.x(0,render_ctx)))
for off,label in ((16,'canvas.clear'),(32,'canvas.activate'),(48,'canvas.prepare'),(56,'canvas.deactivate')):p.put(pen_vt+off,'Q',newfake(label,lambda:p.x(0,0)))
def canvas_target(w=0,h=0):mesa.f['glBindFramebuffer'](0x8D40,fbos[0]);mesa.f['glViewport'](0,0,W,H);p.x(0,0)
p.put(pen_vt+64,'Q',newfake('canvas.set_size',canvas_target))
p.put(render_vt+168,'Q',newfake('renderer.get_target',lambda:p.x(0,target_obj)))
def target_set():
 mesa.f['glBindFramebuffer'](0x8D40,fbos[0]);mesa.f['glViewport'](0,0,W,H);p.x(0,sub)
p.put(target_vt+64,'Q',newfake('target.set_texture',target_set))
p.put(target_vt+40,'Q',newfake('target.activate_mask',lambda:(mesa.f['glBindFramebuffer'](0x8D40,fbos[0]),mesa.f['glViewport'](0,0,W,H))))
# Saved target subbitmap texture is the RGBA output; activating it selects the external FBO.
def sub_get_tex():p.x(0,int(textures[1]))
def sub_activate():mesa.f['glBindFramebuffer'](0x8D40,fbos[1]);mesa.f['glViewport'](0,0,W,H);p.x(0,0)
p.put(subvt+16,'Q',newfake('subbitmap.method16',lambda:p.x(0,0)))
p.put(subvt+48,'Q',newfake('subbitmap.method48',lambda:p.x(0,0)))
p.put(subvt+72,'Q',newfake('subbitmap.get_texture',sub_get_tex));p.put(subvt+40,'Q',newfake('subbitmap.activate_output',sub_activate))
# Use the native graphics bitmap/canvas path for the RT mask canvas and external RGBA target.
# Bridge only GL object creation/storage and queue/resource host boundaries into this Mesa context.
def gl_gen_textures():
 n,out=p.x(0),p.x(1);ids=(ip.C.c_uint*n)();mesa.f['glGenTextures'](n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)]
def gl_gen_fbos():
 n,out=p.x(0),p.x(1);ids=(ip.C.c_uint*n)();ip.cfunc(mesa.gl,'glGenFramebuffers',None,[ip.C.c_int,ip.C.POINTER(ip.C.c_uint)])(n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)];state['gl_target_events'].append(['glGenFramebuffers',n,list(ids)])
def gl_teximage():
 vals=[p.x(i) for i in range(8)];state['gl_target_events'].append(['glTexImage2D',*vals[:7]]);ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0];raw=None
 if ptr:
  w,h=vals[3],vals[4]; channels={0x1903:1,0x1908:4}.get(vals[6],4);raw=bytes(p.u.mem_read(ptr,w*h*channels))
 buf=ip.C.create_string_buffer(raw) if raw else None;mesa.f['glTexImage2D'](*vals,ip.C.cast(buf,ip.C.c_void_p) if buf else None)
def gl_gen_buffers():
 n,out=p.x(0),p.x(1);ids=(ip.C.c_uint*n)();mesa.f['glGenBuffers'](n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)]
def gl_gen_vaos():
 n,out=p.x(0),p.x(1);gen=ip.cfunc(mesa.gl,'glGenVertexArrays',None,[ip.C.c_int,ip.C.POINTER(ip.C.c_uint)]);ids=(ip.C.c_uint*n)();gen(n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)]
def gl_buffer_data():
 target,size,ptr,usage=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size)) if ptr and size<1<<20 else None;buf=ip.C.create_string_buffer(raw) if raw is not None else None;mesa.f['glBufferData'](target,size,ip.C.cast(buf,ip.C.c_void_p) if buf else None,usage)
def gl_buffer_sub():
 target,offset,size,ptr=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size));bound=ip.C.c_int();getin(0x8894,ip.C.byref(bound));state.setdefault('mesa_buffer_subdata',[]).append({'target':target,'buffer':bound.value,'offset':offset,'size':size,'sha256':hashlib.sha256(raw).hexdigest()});buf=ip.C.create_string_buffer(raw);fn=ip.cfunc(mesa.gl,'glBufferSubData',None,[ip.C.c_uint,ip.C.c_ssize_t,ip.C.c_ssize_t,ip.C.c_void_p]);fn(target,offset,size,buf)
def gl_tex_param():mesa.f['glTexParameteri'](p.x(0),p.x(1),p.x(2))
def gl_bind_tex():mesa.f['glBindTexture'](p.x(0),p.x(1))
def gl_bind_fbo():state['gl_target_events'].append(['glBindFramebuffer',p.x(0),p.x(1)]);ip.cfunc(mesa.gl,'glBindFramebuffer',None,[ip.C.c_uint,ip.C.c_uint])(p.x(0),p.x(1))
def gl_fbo_tex():state['gl_target_events'].append(['glFramebufferTexture2D',*[p.x(i) for i in range(5)]]);ip.cfunc(mesa.gl,'glFramebufferTexture2D',None,[ip.C.c_uint,ip.C.c_uint,ip.C.c_uint,ip.C.c_uint,ip.C.c_int])(*[p.x(i) for i in range(5)])
def gl_viewport_cb():state['gl_target_events'].append(['glViewport',*[p.x(i) for i in range(4)]]);mesa.f['glViewport'](*[p.x(i) for i in range(4)])
def gl_tex_sub():
 vals=[p.x(i) for i in range(8)];ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0];raw=bytes(p.u.mem_read(ptr,vals[4]*vals[5]*4)) if ptr else None;buf=ip.C.create_string_buffer(raw) if raw else None;fn=ip.cfunc(mesa.gl,'glTexSubImage2D',None,[ip.C.c_uint,ip.C.c_int,ip.C.c_int,ip.C.c_int,ip.C.c_int,ip.C.c_int,ip.C.c_uint,ip.C.c_uint,ip.C.c_void_p]);fn(*vals,buf)
def gl_pixel_store():ip.cfunc(mesa.gl,'glPixelStorei',None,[ip.C.c_uint,ip.C.c_int])(p.x(0),p.x(1))
def gl_gen_samplers():
 n,out=p.x(0),p.x(1);ids=(ip.C.c_uint*n)();ip.cfunc(mesa.gl,'glGenSamplers',None,[ip.C.c_int,ip.C.POINTER(ip.C.c_uint)])(n,ids);[p.put(out+4*i,'I',ids[i]) for i in range(n)]
def gl_bind_sampler():ip.cfunc(mesa.gl,'glBindSampler',None,[ip.C.c_uint,ip.C.c_uint])(p.x(0),p.x(1))
def gl_sampler_parameter():ip.cfunc(mesa.gl,'glSamplerParameteri',None,[ip.C.c_uint,ip.C.c_uint,ip.C.c_int])(p.x(0),p.x(1),p.x(2))
p.callbacks.update({'glGenTextures':gl_gen_textures,'glGenFramebuffers':gl_gen_fbos,'glGenBuffers':gl_gen_buffers,'glGenVertexArrays':gl_gen_vaos,'glGenSamplers':gl_gen_samplers,'glBindSampler':gl_bind_sampler,'glSamplerParameteri':gl_sampler_parameter,'glBindTexture':gl_bind_tex,'glTexParameteri':gl_tex_param,'glTexImage2D':gl_teximage,'glBindFramebuffer':gl_bind_fbo,'glFramebufferTexture2D':gl_fbo_tex,'glViewport':gl_viewport_cb,'glBufferData':gl_buffer_data,'glBufferSubData':gl_buffer_sub,'glPixelStorei':gl_pixel_store,'glTexSubImage2D':gl_tex_sub,'glGetError':lambda:p.x(0,mesa.f['glGetError']())})
p.callbacks['glCheckFramebufferStatus']=lambda:p.x(0,mesa.f['glCheckFramebufferStatus'](p.x(0)))
def gl_get_boolean():
 pname,out=p.x(0),p.x(1);n=4 if pname in (0x0C23,0x0C22) else 1;buf=(ip.C.c_ubyte*n)();ip.cfunc(mesa.gl,'glGetBooleanv',None,[ip.C.c_uint,ip.C.POINTER(ip.C.c_ubyte)])(pname,buf);p.u.mem_write(out,bytes(buf))
p.callbacks['glGetBooleanv']=gl_get_boolean
def gl_get_string():
 enum=p.x(0);ptr=ip.cfunc(mesa.gl,'glGetString',ip.C.c_void_p,[ip.C.c_uint])(enum);raw=ip.C.string_at(ptr) if ptr else b'';out=p.alloc(len(raw)+1);p.u.mem_write(out,raw+b'\0');p.x(0,out)
p.callbacks['glGetString']=gl_get_string
def gl_uniformfv(n):
 loc,count,ptr=[p.x(i) for i in range(3)];vals=p.get(ptr,'f'*(n*count));fn=ip.cfunc(mesa.gl,f'glUniform{n}fv',None,[ip.C.c_int,ip.C.c_int,ip.C.POINTER(ip.C.c_float)]);fn(loc,count,(ip.C.c_float*len(vals))(*vals));gltrace.append([f'glUniform{n}fv',loc,count,[v.hex() for v in vals]])
for _n in (2,3,4):p.callbacks[f'glUniform{_n}fv']=lambda n=_n:gl_uniformfv(n)
native_syncs={}
def _sync_proc(name,ret,args):
 address=mesa.egl.eglGetProcAddress(name.encode())
 if not address: raise RuntimeError('Mesa lacks '+name)
 return ip.C.CFUNCTYPE(ret,*args)(address)
def gl_fence():
 token=p.alloc(8);host=_sync_proc('glFenceSync',ip.C.c_void_p,[ip.C.c_uint,ip.C.c_uint])(p.x(0),p.x(1))
 if not host:raise RuntimeError('glFenceSync failed')
 native_syncs[token]=host;p.x(0,token)
def gl_wait():p.x(0,_sync_proc('glClientWaitSync',ip.C.c_uint,[ip.C.c_void_p,ip.C.c_uint,ip.C.c_uint64])(native_syncs[p.x(0)],p.x(1),p.x(2)))
def gl_delete_sync():_sync_proc('glDeleteSync',None,[ip.C.c_void_p])(native_syncs.pop(p.x(0)));p.x(0,0)
def gl_wait_sync():
 token,flags,timeout=p.x(0),p.x(1),p.x(2);_sync_proc('glWaitSync',None,[ip.C.c_void_p,ip.C.c_uint,ip.C.c_uint64])(native_syncs[token],flags,timeout);p.x(0,0)
p.callbacks.update({'glFenceSync':gl_fence,'glClientWaitSync':gl_wait,'glWaitSync':gl_wait_sync,'glDeleteSync':gl_delete_sync,'glFlush':lambda:p.x(0,ip.cfunc(mesa.gl,'glFlush',None,[])()),'glFinish':lambda:p.x(0,ip.cfunc(mesa.gl,'glFinish',None,[])())})
for _name,_n in [('glDisable',1),('glEnable',1),('glActiveTexture',1),('glBlendEquationSeparate',2),('glBlendFuncSeparate',4),('glColorMask',4)]:
 _fn=ip.cfunc(mesa.gl,_name,None,[ip.C.c_uint]*_n)
 def _cb(fn,n):
  def cb():fn(*[p.x(i) for i in range(n)])
  return cb
 p.callbacks[_name]=_cb(_fn,_n)
p.callbacks['glClearColor']=lambda:mesa.f['glClearColor'](*[p.f(i) for i in range(4)])
p.callbacks['glClear']=lambda:mesa.f['glClear'](p.x(0))
def gl_scissor():
 args=[p.x(i) for i in range(4)];state['gl_target_events'].append(['glScissor',*args]);ip.cfunc(mesa.gl,'glScissor',None,[ip.C.c_int]*4)(*args)
p.callbacks['glScissor']=gl_scissor
for _name,_n in [('glBindBuffer',2),('glEnableVertexAttribArray',1),('glDisableVertexAttribArray',1),('glVertexAttribDivisor',2),('glBindVertexArray',1)]:
 _fn=ip.cfunc(mesa.gl,_name,None,[ip.C.c_uint]*_n)
 def _glcb(fn,n,name):
  def cb():
   vals=[p.x(i) for i in range(n)];state.setdefault('gl_array_events',[]).append([name,*vals]);fn(*vals)
  return cb
 p.callbacks[_name]=_glcb(_fn,_n,_name)
def gl_buffer_data_native():
 target,size,ptr,usage=[p.x(i) for i in range(4)];raw=bytes(p.u.mem_read(ptr,size)) if ptr and size else b'';buf=ip.C.create_string_buffer(raw) if raw else None;bound=ip.C.c_int();getin(0x8894,ip.C.byref(bound));state.setdefault('mesa_buffer_data',[]).append({'target':target,'buffer':bound.value,'size':size,'sha256':hashlib.sha256(raw).hexdigest()})
 ip.cfunc(mesa.gl,'glBufferData',None,[ip.C.c_uint,ip.C.c_ssize_t,ip.C.c_void_p,ip.C.c_uint])(target,size,buf,usage)
p.callbacks['glBufferData']=gl_buffer_data_native
def gl_vertex_attrib_pointer():
 index,size,typ,norm,stride,ptr=[p.x(i) for i in range(6)];fn=ip.cfunc(mesa.gl,'glVertexAttribPointer',None,[ip.C.c_uint,ip.C.c_int,ip.C.c_uint,ip.C.c_ubyte,ip.C.c_int,ip.C.c_void_p]);fn(index,size,typ,norm,stride,ip.C.c_void_p(ptr))
p.callbacks['glVertexAttribPointer']=gl_vertex_attrib_pointer
def gl_draw_instanced():
 args=[p.x(i) for i in range(4)];bound=ip.C.c_int();vp=(ip.C.c_int*4)();prog=ip.C.c_int();vao=ip.C.c_int();getin(0x8CA6,ip.C.byref(bound));getin(0x0BA2,vp);getin(0x8B8D,ip.C.byref(prog));getin(0x85B5,ip.C.byref(vao));attrs=[]
 for index in (0,1):
  enabled=ip.C.c_int();buf=ip.C.c_int();attrib=ip.cfunc(mesa.gl,'glGetVertexAttribiv',None,[ip.C.c_uint,ip.C.c_uint,ip.C.POINTER(ip.C.c_int)]);attrib(index,0x8622,ip.C.byref(enabled));attrib(index,0x889F,ip.C.byref(buf));size=ip.C.c_int();stride=ip.C.c_int();atype=ip.C.c_int();divisor=ip.C.c_int();attrib(index,0x8623,ip.C.byref(size));attrib(index,0x8624,ip.C.byref(stride));attrib(index,0x8625,ip.C.byref(atype));attrib(index,0x88FE,ip.C.byref(divisor));attrs.append([enabled.value,buf.value,size.value,stride.value,atype.value,divisor.value])
 sc=(ip.C.c_int*4)();getin(0x0C10,sc);isen=ip.cfunc(mesa.gl,'glIsEnabled',ip.C.c_ubyte,[ip.C.c_uint]);sc_on=isen(0x0C11);blend_on=isen(0x0BE2);state['draws'].append({'kind':'instanced','args':args,'framebuffer':bound.value,'viewport':list(vp),'scissor':list(sc),'scissor_enabled':int(sc_on),'blend_enabled':int(blend_on),'program':prog.value,'vao':vao.value,'attrs':attrs});ip.cfunc(mesa.gl,'glDrawArraysInstanced',None,[ip.C.c_uint,ip.C.c_int,ip.C.c_int,ip.C.c_int])(*args);state['draws'][-1]['gl_error_after']=mesa.f['glGetError']()
def gl_draw_arrays():
 args=[p.x(i) for i in range(3)];bound=ip.C.c_int();vp=(ip.C.c_int*4)();prog=ip.C.c_int();vao=ip.C.c_int();getin(0x8CA6,ip.C.byref(bound));getin(0x0BA2,vp);getin(0x8B8D,ip.C.byref(prog));getin(0x85B5,ip.C.byref(vao));blend_on=ip.cfunc(mesa.gl,'glIsEnabled',ip.C.c_ubyte,[ip.C.c_uint])(0x0BE2);state['draws'].append({'kind':'arrays','args':args,'framebuffer':bound.value,'viewport':list(vp),'blend_enabled':int(blend_on),'program':prog.value,'vao':vao.value});ip.cfunc(mesa.gl,'glDrawArrays',None,[ip.C.c_uint,ip.C.c_int,ip.C.c_int])(*args);state['draws'][-1]['gl_error_after']=mesa.f['glGetError']()
p.callbacks['glDrawArraysInstanced']=gl_draw_instanced;p.callbacks['glDrawArrays']=gl_draw_arrays
def gl_delete_buffers():
 n,ptr=p.x(0),p.x(1);ids=(ip.C.c_uint*n)(*[p.get(ptr+4*i,'I')[0] for i in range(n)]);ip.cfunc(mesa.gl,'glDeleteBuffers',None,[ip.C.c_int,ip.C.POINTER(ip.C.c_uint)])(n,ids)
p.callbacks['glDeleteBuffers']=gl_delete_buffers
def gl_invalidate():
 target,n,ptr=p.x(0),p.x(1),p.x(2);vals=[p.get(ptr+i*4,'I')[0] for i in range(n)];fn=ip.cfunc(mesa.gl,'glInvalidateFramebuffer',None,[ip.C.c_uint,ip.C.c_int,ip.C.POINTER(ip.C.c_uint)]);arr=(ip.C.c_uint*n)(*vals);fn(target,n,arr)
p.callbacks['glInvalidateFramebuffer']=gl_invalidate
# GraphicsObjects were constructed directly by the pinned Renderer factory during Init.
native_graphics=[p.get(rt+208,'Q')[0],p.get(rt+216,'Q')[0]]
print('RT_INIT_NATIVE_GRAPHICS',list(map(hex,native_graphics)),'early_gl_buffers',state.get('early_gl_buffer_data',[]))
rt_slots_before_update={str(o):hex(p.get(rt+o,'Q')[0]) for o in (80,88,96)}
vector_begin,vector_end,vector_capacity=struct.unpack('<3Q',p.u.mem_read(vecobj,24))
if not vector_begin<=vector_end<=vector_capacity or vector_end-vector_begin!=312: raise RuntimeError('invalid saved vector at native Update entry')
vector_at_update=bytes(p.u.mem_read(vector_begin,vector_end-vector_begin))
if vector_at_update!=bytes.fromhex(ev['after_bytes']): raise RuntimeError('saved vector changed before native Update')
vector_hash_at_update=hashlib.sha256(vector_at_update).hexdigest()
try:p.call('SPen::InkPenStrokeDrawableRTV1::Update(bool)',x=(rt,0),limit=5_000_000)
except Exception as e:print('RT_UPDATE_NATIVE_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)));raise
print('RT_UPDATE_NATIVE_GRAPHICS',state.get('mesa_buffer_data',[]),'fakes',[(r['name'],hex(r['address'])) for r in state['calls']],'fields',[(o,p.u.mem_read(rt+o,8).hex()) for o in (72,80,88,96,200,208,216)])
rt_slots_after_update={str(o):hex(p.get(rt+o,'Q')[0]) for o in (80,88,96)}
# Native queue is recovered through the selected RT's own PenGLDataManager.
manager=p.get(rt+136,'Q')[0];queue=m.queue; p.call('SPen::PenGLDataManager::SetMsgQueue(SPen::IGLMsgQueue*)',[manager,queue]);p.call('SPen::PenGLDataManager::GetMsgQueue()',[manager]);assert p.x(0)==queue
# Renderer reads its process display-limit pointer pair during texture creation.
# First preserve the actual zero-limit failure control, then initialize the same
# process-global pointer from this bounded Mesa context and retry.
max_tex=ip.C.c_int();getin(0x0D33,ip.C.byref(max_tex));limits=p.alloc(8);p.put(base+0x6d538,'Q',limits);p.put(limits,'ii',0,0);limit_throw_events=[]
def trace_cpp(u,a,z,d):
 for key in ('__cxa_allocate_exception','__cxa_throw','__cxa_begin_catch','__cxa_end_catch','std::terminate()','std::get_unexpected()'):
  if a==p.syms.get(key):
   event={'symbol':key,'pc':hex(a),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'x0x3':[hex(p.x(i)) for i in range(4)]};limit_throw_events.append(event);print('CPP_EXCEPTION_TRACE',key,'pc',hex(a),'lr',hex(u.reg_read(R.UC_ARM64_REG_LR)),'x0x3',[hex(p.x(i)) for i in range(4)])
p.u.hook_add(4,trace_cpp)
pen_factory='SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)';p.callbacks.pop(pen_factory,None)
zero_limit_error=None
try:p.call('SPen::InkPenStrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',x=(rt,W,H,queue),limit=5_000_000)
except Exception as e:zero_limit_error=repr(e)
zero_limit_control={'mesa_gl_max_texture_size':max_tex.value,'global_pointer_va':hex(base+0x6d538),'configured_pair':list(p.get(limits,'ii')),'return_or_error':zero_limit_error,'throw_events':list(limit_throw_events),'partial_pen_canvas':hex(p.get(rt+200,'Q')[0])}
p.put(rt+200,'Q',0)
p.put(limits,'ii',max_tex.value,max_tex.value)
limit_setter={'global_pointer_va':hex(base+0x6d538),'pointer':hex(limits),'values':list(p.get(limits,'ii'))}
p.call('SPen::InkPenStrokeDrawableRTV1::CreatePenCanvas(int, int, SPen::IGLMsgQueue*)',x=(rt,W,H,queue),limit=5_000_000)
actual_pen_canvas=p.get(rt+200,'Q')[0]
# Native RGBA bitmap/canvas produces the exact real ISPSubBitmap class expected by InkPenStrokeDrawableRTV1::Draw.
fmt=p.alloc(8);p.u.mem_write(fmt,b'RGBA\0');initial=p.alloc(W*H*4);p.u.mem_write(initial,bytes(W*H*4))
def call_stack_args(name,regs,stack):
 for i,val in enumerate(regs):p.x(i,val)
 sp=0x200f0000;p.u.reg_write(R.UC_ARM64_REG_SP,sp);p.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
 for i,val in enumerate(stack):p.put(sp+8*i,'Q',val)
 p.u.emu_start(p.syms[name],0x200f1000,count=5_000_000)
 if p.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:raise RuntimeError('native factory call did not return '+name)
bitmap_factory='SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
p.callbacks.pop(bitmap_factory,None);call_stack_args(bitmap_factory,[queue,W,H,fmt,initial,1,W*4,1],[6,2]);rgba_bitmap=p.x(0)
print('RGBA_BITMAP_FACTORY',hex(rgba_bitmap),'queue',hex(queue),'GLERR',mesa.f['glGetError']())
if not rgba_bitmap: raise RuntimeError('native RGBA CreateBitmap returned null')
rgba_canvas=p.alloc(512);p.call('SPen::SPCanvasImpl::SPCanvasImpl(SPen::IGLMsgQueue*, SPen::ISPBitmap*, bool)',x=(rgba_canvas,queue,rgba_bitmap,1),limit=5_000_000);p.call('SPen::SPCanvasImpl::GetBitmap()',[rgba_canvas]);canvas_bitmap=p.x(0);p.call('SPen::SPTextureBitmap::GetSubBitmap(int)',[canvas_bitmap,0]);native_sub=p.x(0)
# Install actual native pen canvas and actual external SubBitmap; no synthetic class stand-in reaches Draw.
p.put(rt+200,'Q',actual_pen_canvas);sub=native_sub
# Keep the actual RT-owned native pen canvas for mask rasterization.
mat=p.alloc(64);projection=(1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1);p.put(mat,'16f',*projection)
# Forward native blend-state calls into the same Mesa context and retain every argument.
blend_gl=[]
mesa.f['glColorMask']=ip.cfunc(mesa.gl,'glColorMask',None,[ip.C.c_ubyte,ip.C.c_ubyte,ip.C.c_ubyte,ip.C.c_ubyte])
for name,n,fn in [('glBlendEquationSeparate',2,mesa.f['glBlendEquationSeparate']),('glBlendFuncSeparate',4,mesa.f['glBlendFuncSeparate']),('glEnable',1,mesa.f['glEnable']),('glDisable',1,mesa.f['glDisable']),('glColorMask',4,mesa.f['glColorMask'])]:
 def make(name,n,fn):
  def cb():
   vals=[p.x(i) for i in range(n)];blend_gl.append([name,*vals]);fn(*vals)
  return cb
 p.callbacks[name]=make(name,n,fn)
# Native shader methods issue GLES uniforms; graphics object facades dispatch the exact native geometry buffers/draw counts.
render_obj=p.get(rt+216,'Q')[0];render_vt=p.get(render_obj,'Q')[0];print('RT_RENDER_OBJECT',hex(render_obj),hex(render_vt),bytes(p.u.mem_read(render_vt,64)).hex())
try:p.call('SPen::InkPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',x=(rt,sub,mat),limit=8_000_000)
except Exception as e:print('RT_DRAW_FAIL',repr(e),'pc',hex(p.u.reg_read(R.UC_ARM64_REG_PC)),'plt',p.plt.get(p.u.reg_read(R.UC_ARM64_REG_PC)),'x0x8',[hex(p.x(i)) for i in range(9)]);raise
ip.cfunc(mesa.gl,'glFinish',None,[])()
get_integer=ip.cfunc(mesa.gl,'glGetIntegerv',None,[ip.C.c_uint,ip.C.POINTER(ip.C.c_int)])
bind_fbo=ip.cfunc(mesa.gl,'glBindFramebuffer',None,[ip.C.c_uint,ip.C.c_uint])
def query_integer(name):
 value=ip.C.c_int();get_integer(name,ip.C.byref(value));return value.value
def read_rgba(framebuffer):
 bind_fbo(0x8D40,framebuffer)
 framebuffer_status=mesa.f['glCheckFramebufferStatus'](0x8D40)
 bound=query_integer(0x8CAA) # GL_READ_FRAMEBUFFER_BINDING
 read_format=query_integer(0x8B9B) # GL_IMPLEMENTATION_COLOR_READ_FORMAT
 read_type=query_integer(0x8B9A) # GL_IMPLEMENTATION_COLOR_READ_TYPE
 viewport=(ip.C.c_int*4)();get_integer(0x0BA2,viewport)
 before=mesa.f['glGetError']()
 pixels=ip.C.create_string_buffer(W*H*4)
 mesa.f['glReadPixels'](0,0,W,H,0x1908,0x1401,pixels)
 error=mesa.f['glGetError']()
 return pixels.raw,{'requested_framebuffer':framebuffer,'bound_read_framebuffer':bound,'framebuffer_status':framebuffer_status,'dimensions':[W,H],'viewport':list(viewport),'read_format':read_format,'read_type':read_type,'requested_format':0x1908,'requested_type':0x1401,'preexisting_gl_error':before,'gl_error_after_rgba_read':error}
read_fbo=state['draws'][-1]['framebuffer'] if state['draws'] else 0
image,fbo4_read=read_rgba(read_fbo)
fbo3rgba_raw,fbo3_read=read_rgba(3)
if len(image)!=W*H*4 or len(fbo3rgba_raw)!=W*H*4:raise RuntimeError('RGBA readback byte count mismatch')
if fbo3_read['framebuffer_status']!=0x8CD5 or fbo4_read['framebuffer_status']!=0x8CD5:raise RuntimeError('draw framebuffer incomplete')
if fbo4_read['bound_read_framebuffer']!=read_fbo or fbo3_read['bound_read_framebuffer']!=3:raise RuntimeError('RGBA readback framebuffer binding mismatch')
if any(read['dimensions']!=[W,H] or read['viewport']!=[0,0,W,H] for read in (fbo4_read,fbo3_read)):raise RuntimeError('RGBA readback dimensions or viewport mismatch')
if any(read['preexisting_gl_error'] or read['gl_error_after_rgba_read'] for read in (fbo4_read,fbo3_read)):raise RuntimeError('RGBA readback GL error')
if rt!=m.selected_rt:raise RuntimeError('selected InkPen RT changed during joined paint capture')
init_uploads=state.get('early_gl_buffer_data',[]);update_uploads=state.get('mesa_buffer_data',[])
if not any(row['size']==64 and row['sha256']=='a39b6ddc1dbf16f1c1efb544817dc4ca4de6b575365af723878516f6afb558e4' for row in init_uploads):raise RuntimeError('native RT::Init did not upload the exact static quad')
if not any(row['size']==48 and row['sha256']=='6b0b87ae7ed17d448d3c052f787a549360b705521bd41576f4943dd63b784c75' for row in init_uploads) or not any(row['size']==48 and row['sha256']=='121ec3fb8daf520affed22f475ac2e90931e48b4d49387fb658f51ffa50ced31' for row in update_uploads):raise RuntimeError('native Init/Update composite buffers are missing')
if not any(row['size']==312 and row['sha256']==hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest() for row in update_uploads):raise RuntimeError('native Update did not upload the complete saved AddPoint stream')
if not p.native_canvas_queue_events or any(row['queue']!=hex(queue) or row['post']!='0x20001000' for row in p.native_canvas_queue_events):raise RuntimeError('Canvas::GetDataQueue did not return the installed PenGLDataManager queue')
if not any(row['vtable']=='0x10774f8' and row['phase']=='draw_object_stroke' and 'SendDataToGPU' in (row['member_symbol'] or '') and row['slots_after'].get('88')==row['vector_object'] for row in m.queue_task_events) or not any(row['vtable']=='0x10774f8' and row['phase']=='saved_redraw' and 'SendDataToGPU' in (row['member_symbol'] or '') and row['slots_after'].get('88')==row['vector_object'] for row in m.queue_task_events):raise RuntimeError('native SendDataToGPU worker entry/return did not install the callback vector in RT+88')
if len(p.deferred_native_tasks)!=4 or any(p.get(rt+o,'Q')[0] for o in (96,)):raise RuntimeError('native vector cleanup deferrals or RT+96 state differ from the observed lifecycle')
if zero_limit_control['configured_pair']!=[0,0] or zero_limit_control['partial_pen_canvas']!='0x0' or not any(e['symbol']=='__cxa_throw' and e['lr']=='0x905c748' for e in zero_limit_control['throw_events']):raise RuntimeError('zero-limit failure control did not reach the captured native throw path')
if limit_setter['values']!=[max_tex.value,max_tex.value] or max_tex.value<=0 or not p.get(rt+200,'Q')[0]:raise RuntimeError('Mesa capability initialization did not permit native PenCanvas creation')
if len(state['draws'])!=2 or [row['framebuffer'] for row in state['draws']]!=[3,4] or any(row['gl_error_after'] for row in state['draws']) or any(row['blend_enabled']!=1 for row in state['draws']):raise RuntimeError('native Draw did not produce the two clean blended FBO passes')
if sum(1 for i in range(0,len(image),4) if image[i:i+4]!=b'\0\0\0\0')!=171 or sum(1 for i in range(3,len(image),4) if image[i])!=171:raise RuntimeError('native RGBA output readback did not retain its 171-pixel coverage witness')
if sum(1 for i in range(0,len(fbo3rgba_raw),4) if fbo3rgba_raw[i:i+4]!=b'\0\0\0\0')!=171:raise RuntimeError('native RGBA mask readback did not retain its 171-pixel coverage witness')
capture_dir=ROOT/'tmp/vector-parity-plan/inkpen-drawing-paint';capture_dir.mkdir(parents=True,exist_ok=True);(capture_dir/'inkpen-v4-fbo4.rgba').write_bytes(image);(capture_dir/'inkpen-v4-fbo3.rgba').write_bytes(fbo3rgba_raw)
source_capture=getattr(m,'result',None)
if not isinstance(source_capture,dict):raise RuntimeError('caller runner did not expose its actual native source capture')
result={'schema':'inkpen-joined-paint-native-v1','case_id':'inkpen_v4_black_1920x1080_to_128x64','source_caller':{k:source_capture[k] for k in ('case_id','profile','input','libraries','native_model_getters','route','screen_resolution','runtime_event_order','streams','native_calls')},'renderer_library_sha256':hashlib.sha256(b).hexdigest(),'factory_path':{'trace':factory_trace,'same_selected_rt':rt==m.selected_rt,'renderer_init_buffer_data':state.get('early_gl_buffer_data',[]),'native_update_buffer_data':state.get('mesa_buffer_data',[]),'updates_by_subdata':state.get('mesa_buffer_subdata',[]),'host_static_quad_upload':None,'native_vector_handoff':{'operation':'native worker calls PenDrawableRT::SendDataToGPU on the selected RT, setting RT+88; native Update(false) consumes the saved stream','vector_object':hex(vecobj),'stream_sha256':hashlib.sha256(bytes.fromhex(ev['after_bytes'])).hexdigest(),'rt_slots_before_update':rt_slots_before_update,'rt_slots_after_update':rt_slots_after_update,'vector_sha256_at_update':vector_hash_at_update,'canvas_get_data_queue_returns':p.native_canvas_queue_events,'executed_queue_tasks':m.queue_task_events,'deferred_vector_lifetimes':p.deferred_native_tasks},'managed_queue':{'manager':hex(manager),'queue':hex(queue),'native_SetMsgQueue_GetMsgQueue_verified':True,'canvas_get_data_queue_is_same_queue':True,'native_SetBuffer_worker_task_observed':False,'native_SendDataToGPU_task_observed':True}},'renderer_limit_control':{'zero_limit':zero_limit_control,'successful_mesa_limit_setter':limit_setter},'target':{'host':'Mesa EGL/GLES host context','viewport':[0,0,W,H],'format':'RGBA8','created_through_native_CreatePenCanvas_and_CreateBitmap':True,'fbo_ids':[row['framebuffer'] for row in state['draws']],'texture_and_framebuffer_events':state['gl_target_events'],'fbo3_status':fbo3_read['framebuffer_status'],'fbo4_status':fbo4_read['framebuffer_status'],'draws':state['draws'],'actual_blend_state_calls':blend_gl,'draw_gl_errors':[row['gl_error_after'] for row in state['draws']],'readback':{'fbo4_read':fbo4_read,'fbo3_read':fbo3_read,'fbo4_rgba8_sha256':hashlib.sha256(image).hexdigest(),'fbo4_channel_nonzero':[sum(1 for i in range(ch,len(image),4) if image[i]) for ch in range(4)],'fbo4_channel_max':[max(image[ch::4]) for ch in range(4)],'fbo4_nonzero_pixels':sum(1 for i in range(0,len(image),4) if image[i:i+4]!=b'\0\0\0\0'),'fbo3_rgba8_sha256':hashlib.sha256(fbo3rgba_raw).hexdigest(),'fbo3_channel_nonzero':[sum(1 for i in range(ch,len(fbo3rgba_raw),4) if fbo3rgba_raw[i]) for ch in range(4)],'fbo3_channel_max':[max(fbo3rgba_raw[ch::4]) for ch in range(4)],'fbo4_rgba8_zlib_base64':base64.b64encode(zlib.compress(image,9)).decode(),'fbo3_rgba8_zlib_base64':base64.b64encode(zlib.compress(fbo3rgba_raw,9)).decode()},'native_shader_uniform_trace':gltrace},'helper_hashes':{'runner':hashlib.sha256(Path(__file__).resolve().read_bytes()).hexdigest(),'saved_caller':hashlib.sha256((ROOT/'conformance/inkpen_saved_caller_native.py').read_bytes()).hexdigest(),'paint_helper':hashlib.sha256(Path(ip.__file__).resolve().read_bytes()).hexdigest(),'caller_helpers':source_capture['helper_hashes']},'host_boundaries':['ObjectStroke/Model source is synthetic but the selected Drawing, InkPen native getters, MotionEvent calls, selected V4 RedrawPen and RT AddPoint stream are executed by the pinned caller oracle.','The saved-replay AddPoint stream is handed through the native caller queue: its worker invokes PenDrawableRT::SendDataToGPU on the same selected RT, populating RT+88; native Update(false) uploads it. RT+96 is not host-written.','The native PenGLDataManager and Canvas::GetDataQueue resolve to the caller queue. Native SendDataToGPU worker tasks execute; vector/deallocation cleanup tasks remain deferred and are not executed in this capture.','Native Renderer factory, GraphicsObject buffer creation/upload/draw, InkPen RT Init/Draw, native PenCanvas/Bitmap creation and Mesa GLES calls execute; EGL creation/GL dispatch, queue/context setup, process max-texture-limit global and pixel readback are host-supplied.','The zero-limit control reaches native exception allocation/throw with recorded origin and input pair, then enters unsupported std::get_unexpected/Unicorn fault; native exception unwinding is not claimed complete. The same request succeeds after setting Mesa-reported GL_MAX_TEXTURE_SIZE=16384 in the renderer limit pair.','Target dimensions 128x64 are a synthetic Mesa output surface; caller screen profile is explicit synthetic 1920x1080. No Samsung device/export lifecycle or PDF parity is claimed.']}
result['factory_path']['host_renderer_initialization']={
 'shader_manager_native_constructor':'SPen::ShaderManager::ShaderManager()',
 'shader_manager_host_installed_global_pointer':hex(base+0x6d480),
 'shader_manager_object':hex(shader_mgr),
 'blend_native_constructor':'SPen::GLES::BlendStateObject::BlendStateObject(SPen::BlendStateDescriptor const&)',
 'blend_host_installed_vtable':hex(base+0x6a078),
 'blend_host_installed_activate_slot':hex(base+0x6a078+24),
 'blend_activate_native_target':hex(p.syms['SPen::GLES::BlendStateObject::Activate()']),
 'depth_stencil_factory':'synthetic object with no-op Activate callback',
}
result['host_boundaries'].append('Host setup constructs the native ShaderManager and installs its singleton global, constructs native BlendStateObjects and fixes their vtable/Activate slot, and supplies a synthetic no-op depth-stencil state. Complete Android loader and renderer initialization are not executed.')
fixture_path=ROOT/'conformance/inkpen-joined-paint-native.json'
if CLI_ARGS.write_fixture:
 fixture_path.write_text(json.dumps({'schema':'inkpen-joined-paint-fixture-v1','cases':{result['case_id']:result}},indent=2,sort_keys=True)+'\n');print('wrote',fixture_path.relative_to(ROOT))
elif CLI_ARGS.check_fixture:
 if not fixture_path.exists():raise SystemExit('InkPen joined paint fixture is missing')
 fixture=json.loads(fixture_path.read_text())
 if fixture.get('schema')!='inkpen-joined-paint-fixture-v1' or fixture.get('cases',{}).get(result['case_id'])!=result:raise SystemExit('native InkPen joined paint fixture mismatch')
 print('verified',fixture_path.relative_to(ROOT),result['case_id'])
else:print(json.dumps(result,indent=2,sort_keys=True))
