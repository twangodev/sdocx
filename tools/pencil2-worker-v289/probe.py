import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
renderer = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so'
assert hashlib.sha256(renderer.read_bytes()).hexdigest() == 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
assert hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenBase.so').read_bytes()).hexdigest() == 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'
assert hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so').read_bytes()).hexdigest() == '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd'
parent = root / 'tools/pencil2-worker-v254/probe.py'
assert hashlib.sha256(parent.read_bytes()).hexdigest() == '1a32b28eb890a4c77724f481ea42e5ab01ec415cc7d5bbc56860d2ed295547e3'
fixture = zlib.decompress((root / 'tools/pencil2-worker-v256/rgba8-frame.zlib').read_bytes())
assert len(fixture) == 192*192*4 and hashlib.sha256(fixture).hexdigest() == 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'
assert parent.read_text().count("root = pathlib.Path(__file__).resolve().parents[2]") == 1
s = parent.read_text().replace("root = pathlib.Path(__file__).resolve().parents[2]", f"root = pathlib.Path({str(root)!r})")
patch='''    start=code.index(' import os,re,subprocess\\n from pathlib import Path\\n renderer=')
    end=code.index(" vertex_name='SPen::VertexDescriptor::addAttribute",start)
    early=code[start:end]
    code=code[:start]+code[end:]
    start2=code.index(' def fence_sync():')
    end2=code.index(' target_gl_start=len(gl_simple)',start2)
    early+=code[start2:end2]
    code=code[:start2]+code[end2:]
    marker=' events=[]; matrices=[]'
    assert code.count(marker)==1
    setup=""" key='factory'; fmt=p.alloc(32);p.u.mem_write(fmt,bytes([82,71,66,65,56,56,56,56,0]))
 p.callbacks.pop('SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)',None)
 p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None)
 p.put(0x107a758,'Q',0x40c04ac);p.put(0x107a760,'Q',0x40c1454);p.put(0x107a770,'Q',0x40c1398)
 p.put(0x200ef000,'Q',2)
 manager=p.alloc(256);p.call('SPen::PenGLDataManager::PenGLDataManager()',[manager])
 name=p.alloc(32);p.call('SPen::String::String()',[name]);p.call('SPen::String::Construct()',[name]);p.call('SPen::String::Set(char const*)',[name,fmt])
 p.syms[key]=0x1049260;p.call(key,[manager,p.handoff_queue,name,192,192,0,0,6,2],limit=8000000);cv=p.x(0)
 p.call('SPen::SPPenCanvas::GetBitmap()',[cv]);manager_bitmap=p.x(0);p.syms[key]=0x4090a2c;p.call(key,[manager_bitmap,0]);manager_sub=p.x(0);assert manager_sub
 created=[row for row in gl_simple if row[0] in ('glBindFramebuffer','glFramebufferTexture2D')]
 assert len(created)>=2 and created[-2][0]=='glBindFramebuffer' and created[-1][0]=='glFramebufferTexture2D',created
 manager_fb=created[-2][2];manager_tex=created[-1][4];assert manager_fb and manager_tex

 p.callbacks['SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)']=lambda:p.x(0,cv)
 init_events=[]
 def observe_init(u,a,z,d):
  if a==p.syms['SPen::Pencil2StrokeDrawableRTV4::Init()']: init_events.append([hex(p.x(0)),hex(u.reg_read(R.UC_ARM64_REG_LR))])
 p.u.hook_add(UC_HOOK_CODE,observe_init)
"""
    code=code.replace(marker,early+setup+marker)
    assert code.count("  p.syms['render_message_run']=0x104676c")==1
    code=code.replace("  p.syms['render_message_run']=0x104676c", "  p.syms['render_message_run']=p.get(p.get(task,'Q')[0]+16,'Q')[0]; assert p.syms['render_message_run']==0x104676c")
    init_call="  p.call('SPen::Pencil2StrokeDrawableRTV4::Init()',[selected_rt])"
    assert code.count(init_call)==1
    code=code.replace(init_call, "  pass")
    assert init_call not in code
    ins="""  def snap():
   get=ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)]); query=ip.cfunc(p.mesa.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])
   f=C.c_int();typ=C.c_int();tex=C.c_int();get(0x8ca6,C.byref(f));query(0x8d40,0x8ce0,0x8cd0,C.byref(typ));query(0x8d40,0x8ce0,0x8cd1,C.byref(tex))
   return [f.value,typ.value,tex.value]
  ip.cfunc(p.mesa.gl,'glBindFramebuffer',None,[C.c_uint,C.c_uint])(0x8d40,manager_fb)
  before_fb=snap();assert before_fb==[manager_fb,5890,manager_tex],before_fb
  if __import__('os').environ.get('PENCIL_SKIP_DRAW'):
   def skip(u,a,z,d):
    if a==p.syms['SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']: u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
   p.u.hook_add(UC_HOOK_CODE,skip)
"""
    queue_patch="""  queue_owner=p.alloc(2048); queue_head=queue_owner+32
  def cas_hook(u,a,z,d):
   if a==0x2099350:
    expected,new,dst=p.x(0),p.x(1),p.x(2); old=p.get(dst,'Q')[0]
    if old==expected: p.put(dst,'Q',new)
    p.x(0,old); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  p.u.hook_add(UC_HOOK_CODE,cas_hook)
  p.syms['agll_ctor']=0x904f4f0
  p.call('agll_ctor',[queue_owner,0,0],limit=8000000)
  init_task=p.get(queue_head,'Q')[0]-8
  assert init_task and p.get(queue_head+8,'Q')[0]==queue_head
  assert p.get(queue_owner+56,'QQQ')==(0,queue_owner+56,queue_owner+56)
  init_run=p.get(p.get(init_task,'Q')[0]+16,'Q')[0];init_slot8=p.get(p.get(init_task,'Q')[0]+8,'Q')[0]
  init_slot8=p.get(p.get(init_task,'Q')[0]+8,'Q')[0]
  def skip_init(u,a,z,d):
   if a==init_run: u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  p.u.hook_add(UC_HOOK_CODE,skip_init)
  native_events=[]
  slot8=p.get(p.get(task,'Q')[0]+8,'Q')[0]
  assert len({init_run,init_slot8,slot8,0x104676c,0x9063fc0,0x9063fe0,0x9063fb0,0x9064250})==8
  assert init_run not in (p.syms['SPen::Pencil2StrokeDrawableRTV4::Init()'],p.syms['SPen::Pencil2StrokeDrawableRTV4::Update(bool)'],p.syms['SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'])
  def native_queue_hook(u,a,z,d):
   if a in (0x9063fc0,0x9063fe0,0x9063fb0): u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
   elif a==0x9064250:
    dst,value=p.x(0),p.x(1); old=p.get(dst,'Q')[0];p.put(dst,'Q',value);p.x(0,old);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
   elif a==slot8: native_events.append(['slot8',hex(p.x(0))])
   elif a==init_slot8: native_events.append(['init_slot8',hex(p.x(0))])
  p.u.hook_add(UC_HOOK_CODE,native_queue_hook)
  p.syms['native_enqueue']=0x904fbf0;p.syms['native_drain']=0x904fac8
  p.call('native_drain',[queue_owner,queue_head],limit=8000000)
  assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)
  assert native_events==[['init_slot8',hex(init_task)]],native_events
  native_events.clear()
  p.call('native_enqueue',[queue_owner,task],limit=8000000);assert p.x(0)==1
  assert p.get(queue_head,'Q')[0]==native_submissions[0][1]+8
"""
    ins+=queue_patch
    call="   p.call(bytes([114,101,110,100,101,114,95,109,101,115,115,97,103,101,95,114,117,110]).decode(),[task],limit=8000000)"
    assert code.count(call)==1
    code=code.replace(call,"   p.call('native_drain',[queue_owner,queue_head],limit=8000000)\\n   assert native_events==[['slot8',hex(task)]],native_events\\n   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\\n   p.call('native_drain',[queue_owner,queue_head],limit=8000000)\\n   assert native_events==[['slot8',hex(task)]]")
    assert code.count('  prior=(C.c_ubyte')==1
    code=code.replace('  prior=(C.c_ubyte',ins+'  prior=(C.c_ubyte')
    assert code.count('result=repr((observed,hex(selected_rt),')==1
    code=code.replace('result=repr((observed,hex(selected_rt),',"result=repr((mutex_lifecycle,mutex_events,guard_events,async_identity,init_events,hex(task),hex(manager_sub),before_fb,snap(),[row for row in gl_simple if row[0] in ('glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D','glGenTextures')],observed,hex(selected_rt),")
    begin=code.index('  key=bytes([102,97,99,116,111,114,121]).decode()')
    stop=code.index('  observed=[]',begin)
    code=code[:begin]+"  assert original_canvas==cv, (hex(original_canvas),hex(cv))"+chr(10)+code[stop:]
    assert code.count(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices')==1
    code=code.replace(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices', ' assert matrices==[[1,0,0,0,-1,0,6,183,1]], matrices')
'''
early_queue = """ def strtof_adapter():
  addr,endptr=p.x(0),p.x(1); raw=bytearray()
  for i in range(4096):
   byte=bytes(p.u.mem_read(addr+i,1))
   if byte==bytes(1):break
   raw.extend(byte)
  else:raise RuntimeError('unterminated strtof input')
  buf=C.create_string_buffer(bytes(raw)); end=C.c_void_p(); libc=C.CDLL(None)
  libc.strtof.argtypes=[C.c_void_p,C.POINTER(C.c_void_p)];libc.strtof.restype=C.c_float
  value=libc.strtof(C.addressof(buf),C.byref(end)); consumed=end.value-C.addressof(buf)
  assert 0<=consumed<=len(raw)
  if endptr:p.put(endptr,'Q',addr+consumed)
  p.f(0,value)
 p.callbacks['strtof']=strtof_adapter
 def posix_memalign_adapter():
  out,alignment,size=p.x(0),p.x(1),p.x(2)
  assert alignment>=8 and alignment&(alignment-1)==0 and alignment%8==0 and size<=0x1000000
  ptr=(p.alloc(size+alignment-1)+alignment-1)&-alignment;p.put(out,'Q',ptr);p.x(0,0)
 p.callbacks['posix_memalign']=posix_memalign_adapter
 original_get_integer=p.callbacks['glGetIntegerv']; viewport=[]
 def get_integer():
  if p.x(0)==0xd3a:
   values=(C.c_int*2)();ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0xd3a,values)
   viewport.append(list(values));p.put(p.x(1),'ii',*values)
  else:original_get_integer()
 p.callbacks['glGetIntegerv']=get_integer
 queue_owner=p.alloc(2048); queue_head=queue_owner+32; native_events=[]; native_submissions=[]; init_slot8=0
 sync_calls=[]
 mutex_events=[]; guard_events=[]; async_identity=[]; mutex_objects={}; mutex_lifecycle=[]; attrs={}
 def mutex_attr_init():
  addr=p.x(0); assert addr and addr not in attrs; attrs[addr]=0; mutex_lifecycle.append(('attr_init',hex(addr))); p.x(0,0)
 def mutex_attr_settype():
  addr=p.x(0); assert addr in attrs; attrs[addr]=p.x(1); mutex_lifecycle.append(('attr_settype',hex(addr),p.x(1))); p.x(0,0)
 def mutex_attr_destroy():
  addr=p.x(0); assert addr in attrs; del attrs[addr]; mutex_lifecycle.append(('attr_destroy',hex(addr))); p.x(0,0)
 def mutex_init():
  addr,attr=p.x(0),p.x(1); assert addr and addr not in mutex_objects; assert not attr or attr in attrs; mutex_objects[addr]=(__import__('threading').RLock() if attrs.get(attr)==1 else __import__('threading').Lock()); mutex_lifecycle.append(('init',hex(addr),hex(attr),attrs.get(attr))); print('MUTEX_INIT',mutex_lifecycle[-1],flush=True); p.x(0,0)
 p.callbacks['pthread_mutexattr_init']=mutex_attr_init
 p.callbacks['pthread_mutexattr_settype']=mutex_attr_settype
 p.callbacks['pthread_mutexattr_destroy']=mutex_attr_destroy
 p.callbacks['pthread_mutex_init']=mutex_init
 def mutex_call(op):
  addr=p.x(0); assert addr
  if op=='lock': mutex_objects.setdefault(addr,__import__('threading').RLock()).acquire()
  else: assert addr in mutex_objects; mutex_objects[addr].release()
  mutex_events.append((op,hex(addr))); p.x(0,0)
 p.callbacks['pthread_mutex_lock']=lambda:mutex_call('lock')
 p.callbacks['pthread_mutex_unlock']=lambda:mutex_call('unlock')
 def early_queue_hook(u,a,z,d):
  if a in (0x2099c70,0x2099cc8):
   guard_ptr=p.x(1) if a==0x2099c70 else p.get(p.x(0),'Q')[0]
   if u.reg_read(R.UC_ARM64_REG_LR) in (0x30799c0,0x3079d68,0x307b76c,0x307b82c,0x307b848,0x307b88c): guard_events.append((hex(a),hex(p.x(0)),hex(p.x(1)),hex(u.reg_read(R.UC_ARM64_REG_LR)),hex(p.get(p.x(0),'Q')[0]),hex(p.get(guard_ptr,'Q')[0])))
  if a in (0x9063fc0,0x9063fe0,0x9063fb0):sync_calls.append(hex(a))
  if a==0x2099350:
   expected,new,dst=p.x(0),p.x(1),p.x(2);old=p.get(dst,'Q')[0]
   if old==expected:p.put(dst,'Q',new)
   p.x(0,old);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==0x9064250:
   dst,value=p.x(0),p.x(1);old=p.get(dst,'Q')[0];p.put(dst,'Q',value);p.x(0,old);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==0x904fbf0:native_submissions.append((p.x(0),p.x(1),p.get(p.x(1),'Q')[0]))
  elif a in (0x904faf0,0x904fb00,0x904fb10):native_events.append((hex(a),u.reg_read(R.UC_ARM64_REG_X21)))
  elif a==init_slot8:native_events.append(('init_slot8',p.x(0)))
 p.u.hook_add(UC_HOOK_CODE,early_queue_hook)
 p.syms['agll_ctor']=0x904f4f0;p.syms['native_drain']=0x904fac8
 p.call('agll_ctor',[queue_owner,0,0],limit=8000000)
 init_task=p.get(queue_head,'Q')[0]-8;init_run=p.get(p.get(init_task,'Q')[0]+16,'Q')[0];init_slot8=p.get(p.get(init_task,'Q')[0]+8,'Q')[0]
 p.call('native_drain',[queue_owner,queue_head],limit=8000000)
 assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)
 assert native_events==[(phase,init_task) for phase in ('0x904faf0','0x904fb00','init_slot8','0x904fb10')],native_events
 assert len(viewport)==1 and all(x>=192 for x in viewport[0]),viewport
 print('CONSTRUCTOR_RUN',hex(init_run),'VIEWPORT',viewport,flush=True)
 native_submissions.clear();native_events.clear();p.handoff_queue=queue_owner+8
"""
patch=patch.replace(" key='factory'; fmt=p.alloc(32)", early_queue+" key='factory'; fmt=p.alloc(32)")
a=patch.index('    queue_patch="""');b=patch.index('    ins+=queue_patch',a)
patch=patch[:a]+"    ins+=\"  assert [v for o,t,v in native_submissions]==[0x1077ba8,0x10774f8,0x10774c8,0x1077528,0x30c3070] and all(o==queue_owner for o,t,v in native_submissions) and task==native_submissions[2][1], native_submissions\\n  assert p.get(queue_head,'Q')[0]==native_submissions[0][1]+8\\n\"\n"+patch[b+len('    ins+=queue_patch\n'):]
patch=patch.replace("  rows=[row for row in queue_events if row.get('event')=='deferred' and row.get('vtable')=='0x10774c8']", "  rows=[{'task':hex(t)} for owner,t,vt in native_submissions if vt==0x10774c8]")
patch=patch.replace("   assert native_events==[['slot8',hex(task)]],native_events\\n", "")
patch=patch.replace("   assert native_events==[['slot8',hex(task)]]", "   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\\n   assert native_events==[(phase,t) for owner,t,vt in native_submissions for phase in ('0x904faf0','0x904fb00','0x904fb10')], (native_submissions,native_events)")
patch += "    code=code.replace(\"   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\", \"   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\\n   assert native_events==[(phase,t) for owner,t,vt in native_submissions for phase in ('0x904faf0','0x904fb00','0x904fb10')], (native_submissions,native_events)\\n   assert {'0x9063fc0','0x9063fe0'} <= set(sync_calls), sync_calls\\n   print('NATIVE_SYNC_CALLS', sync_calls, flush=True)\\n   print('HOST_MUTEX_EVENTS', mutex_events, flush=True)\\n   assert mutex_events, mutex_events\\n   print('NATIVE_SUBMISSIONS', [(hex(o),hex(t),hex(v)) for o,t,v in native_submissions], flush=True)\",1)\n"
patch += "    code=code.replace(\"  rows=[row for row in queue_events if row.get('event')=='deferred' and row.get('vtable')=='0x10774c8']\", \"  rows=[{'task':hex(t)} for owner,t,vt in native_submissions if vt==0x10774c8]\")\n"
patch += "    assert code.count(\"  p.callbacks['SPen::AutoCriticalSection::~AutoCriticalSection()']=lambda:p.x(0,0)\")==1\n"
patch += "    code=code.replace(\"  p.callbacks['SPen::AutoCriticalSection::~AutoCriticalSection()']=lambda:p.x(0,0)\", \"  p.callbacks.pop('SPen::AutoCriticalSection::~AutoCriticalSection()',None)\")\n"
patch += "    assert code.count(\"  p.callbacks['SPen::AutoCriticalSection::AutoCriticalSection(SPen::CriticalSection*, char const*, int)']=lambda:p.x(0,0)\")==1\n"
patch += "    code=code.replace(\"  p.callbacks['SPen::AutoCriticalSection::AutoCriticalSection(SPen::CriticalSection*, char const*, int)']=lambda:p.x(0,0)\", \"  p.callbacks.pop('SPen::AutoCriticalSection::AutoCriticalSection(SPen::CriticalSection*, char const*, int)',None)\")\n"
patch += "    assert code.count(\"  node=p.get(async_obj+40,'Q')[0]; assert node!=async_obj+32\")==1\n"
patch += "    code=code.replace(\"  node=p.get(async_obj+40,'Q')[0]; assert node!=async_obj+32\", \"  node=p.get(async_obj+40,'Q')[0]; assert node!=async_obj+32; async_identity.extend([hex(async_obj),hex(async_obj+32),hex(node)])\")\n"
patch += "    assert code.count(\"  p.call(sy,[async_obj,1,1,vector],limit=50000000)\")==1\n"
patch += "    code=code.replace(\"  p.call(sy,[async_obj,1,1,vector],limit=50000000)\", \"  p.syms['critical_ctor']=0x2099b38; p.call('critical_ctor',[async_obj+16,0],limit=8000000); wrapper=p.alloc(256); p.syms['wrapper_ctor']=0x307973c; p.call('wrapper_ctor',[wrapper,0],limit=8000000); p.put(wrapper+120,'B',1); p.put(wrapper+64,'QQ',fake,p.drawing); sections=[p.get(wrapper+32,'Q')[0],p.get(wrapper+40,'Q')[0],async_obj+16]; mutexes=[p.get(section,'Q')[0] for section in sections]; assert all(sections) and len(set(sections))==3 and all(mutexes) and len(set(mutexes))==3; assert all(m in mutex_objects for m in mutexes) and not attrs, (mutexes,mutex_lifecycle,attrs); assert [next(e[3] for e in mutex_lifecycle if e[0]=='init' and e[1]==hex(m)) for m in mutexes]==[1,None,None], mutex_lifecycle; async_identity.extend([hex(v) for v in sections+mutexes]); p.call(sy,[wrapper,1,1,vector],limit=50000000)\")\n"
needle="native=root/'tools/pencil2-page-v219/native'"
# add code transformation into generated builder immediately before native execution
insert="assert p.count('    return code\\n')==1\np=p.replace('    return code\\n', "+repr(patch+"    return code\n")+")\n"
needle="with tempfile.TemporaryDirectory(prefix='pencil242-') as output:"
assert s.count(needle)==1
s=s.replace(needle,"assert source.count(\"native=root/'tools/pencil2-page-v219/native'\")>=1\nsource = source.replace("+repr("native=root/'tools/pencil2-page-v219/native'")+", "+repr(insert+"native=root/'tools/pencil2-page-v219/native'")+")\n"+needle)
assert s.count("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)")==1
s = s.replace("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)", "        assert run.returncode == 7 and message_file.exists(), (run.stdout,run.stderr)\n        message=ast.literal_eval(message_file.read_text()); lifecycle,locks,guards,identity,inits,task,sub,before,after,calls,events,selected,error,nonzero,changed,digest=message\n        print('MUTEX_LIFECYCLE', lifecycle, flush=True)\n        g=guards\n        assert len(identity)==9, identity\n        section1,section2,worker,mutex1,mutex2,worker_mutex,async_obj,head,node=identity\n        for i, event in enumerate(lifecycle):\n            if event[0]=='init' and event[2]!='0x0':\n                attr=event[2]\n                assert i>=2 and lifecycle[i-2:i+2]==[('attr_init',attr),('attr_settype',attr,1),event,('attr_destroy',attr)], lifecycle\n        assert int(worker,16)==int(async_obj,16)+16 and int(head,16)==int(async_obj,16)+32 and node!=head, identity\n        assert [(x[0],x[3]) for x in g]==[('0x2099c70','0x30799c0'),('0x2099cc8','0x3079d68'),('0x2099c70','0x307b76c'),('0x2099cc8','0x307b82c'),('0x2099c70','0x307b848'),('0x2099cc8','0x307b88c')], g\n        assert g[0][2]==g[1][4]==section1 and g[0][5]==g[1][5]==mutex1, (g,identity)\n        assert g[2][2]==g[3][4]==g[4][2]==g[5][4]==worker and all(g[i][5]==worker_mutex for i in (2,3,4,5)), (g,identity)\n        assert len(locks)==36 and all(addr!='0x0' for op,addr in locks), locks\n        stack=[]\n        for op,addr in locks:\n            if op=='lock': stack.append(addr)\n            else: assert stack and stack.pop()==addr, locks\n        assert not stack and g[0][2]==g[1][4] and g[0][5]==g[1][5]==locks[6][1]==locks[7][1], (g,locks)\n        assert g[2][2]==g[3][4]==g[4][2]==g[5][4] and g[2][5]==g[3][5]==g[4][5]==g[5][5] and g[2][5]!='0x0', g\n        print('GUARD_TRACE',g,flush=True)\n        print('MUTEX_EVENTS',locks,flush=True)\n        assert inits == [[selected,'0x904fb00']], (inits,selected); skip=bool(os.environ.get('PENCIL_SKIP_DRAW'))\n        assert before == after and before[0] and before[1] == 5890 and before[2], message\n        assert calls[:4] == [['glGenTextures',before[2]],['glGenFramebuffers',before[0]],['glBindFramebuffer',36160,before[0],None],['glFramebufferTexture2D',36160,36064,3553,before[2],0,None]], calls\n        assert (calls[-1] == ['glBindFramebuffer',36160,before[0],None]) == (not skip), calls\n        assert [x[0] for x in events] == ['run','update','draw'] and events[0][1] == task and events[1][1] == events[2][1] == selected, message\n        assert events[2][2] == sub, message\n        assert events[2][3] == [1,0,0,0,0,-1,0,0,0,0,1,0,6,183,0,1], message\n        assert error == 0 and nonzero == changed == (0 if skip else 322), message\n        assert digest == ('8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a' if skip else 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'), message\n        print(run.stdout,flush=True)\n        print('Verified five producer submissions and native FIFO run and slot-8 returns, original manager canvas, attachment and '+('skip' if skip else 'draw')+' control.')\n        raise SystemExit(0)")
s=s.replace('source = source.replace(child_exit, " if run.returncode: raise SystemExit(run.returncode)")', 'source = source.replace(child_exit, " if run.returncode: print(run.stdout,run.stderr,flush=True); raise SystemExit(run.returncode)")')
s=s.replace("source = base.read_text()", "source = base.read_text()\nsource = source.replace(\" pixels=__import__('zlib').decompress(raw.read_bytes())\", \" assert raw.exists(), (run.returncode, run.stdout, run.stderr, (out/'result.json').read_text() if (out/'result.json').exists() else 'no result')\\n pixels=__import__('zlib').decompress(raw.read_bytes())\")")
with tempfile.TemporaryDirectory(prefix='pencil277-') as td:
    script=Path(td)/'probe.py'
    s=s.replace('(run.stdout,run.stderr)', '(run.returncode,run.stdout,run.stderr)')
    script.write_text(s)
    subprocess.run([sys.executable,str(script)],cwd=root,env=os.environ.copy(),check=True)
