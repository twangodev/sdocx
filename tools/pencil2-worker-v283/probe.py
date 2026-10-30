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
    code=code.replace('result=repr((observed,hex(selected_rt),',"result=repr((init_events,hex(task),hex(manager_sub),before_fb,snap(),[row for row in gl_simple if row[0] in ('glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D','glGenTextures')],observed,hex(selected_rt),")
    begin=code.index('  key=bytes([102,97,99,116,111,114,121]).decode()')
    stop=code.index('  observed=[]',begin)
    code=code[:begin]+"  assert original_canvas==cv, (hex(original_canvas),hex(cv))"+chr(10)+code[stop:]
    assert code.count(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices')==1
    code=code.replace(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices', ' assert matrices==[[1,0,0,0,-1,0,6,183,1]], matrices')
'''
early_queue = """ queue_owner=p.alloc(2048); queue_head=queue_owner+32; native_events=[]; native_submissions=[]; init_slot8=0
 def early_queue_hook(u,a,z,d):
  if a==0x2099350:
   expected,new,dst=p.x(0),p.x(1),p.x(2);old=p.get(dst,'Q')[0]
   if old==expected:p.put(dst,'Q',new)
   p.x(0,old);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a in (0x9063fc0,0x9063fe0,0x9063fb0):u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==0x9064250:
   dst,value=p.x(0),p.x(1);old=p.get(dst,'Q')[0];p.put(dst,'Q',value);p.x(0,old);u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
  elif a==0x904fbf0:native_submissions.append((p.x(0),p.x(1),p.get(p.x(1),'Q')[0]))
  elif a in (0x904faf0,0x904fb00,0x904fb10):native_events.append((hex(a),u.reg_read(R.UC_ARM64_REG_X21)))
  elif a==init_slot8:native_events.append(('init_slot8',p.x(0)))
 p.u.hook_add(UC_HOOK_CODE,early_queue_hook)
 p.syms['agll_ctor']=0x904f4f0;p.syms['native_drain']=0x904fac8
 p.call('agll_ctor',[queue_owner,0,0],limit=8000000)
 init_task=p.get(queue_head,'Q')[0]-8;init_run=p.get(p.get(init_task,'Q')[0]+16,'Q')[0];init_slot8=p.get(p.get(init_task,'Q')[0]+8,'Q')[0]
 def skip_init(u,a,z,d):
  if a==init_run:u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))
 p.u.hook_add(UC_HOOK_CODE,skip_init)
 p.call('native_drain',[queue_owner,queue_head],limit=8000000)
 assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)
 assert native_events==[(phase,init_task) for phase in ('0x904faf0','0x904fb00','init_slot8','0x904fb10')],native_events
 native_submissions.clear();native_events.clear();p.handoff_queue=queue_owner+8
"""
patch=patch.replace(" key='factory'; fmt=p.alloc(32)", early_queue+" key='factory'; fmt=p.alloc(32)")
a=patch.index('    queue_patch="""');b=patch.index('    ins+=queue_patch',a)
patch=patch[:a]+"    ins+=\"  assert [v for o,t,v in native_submissions]==[0x1077ba8,0x10774f8,0x10774c8,0x1077528,0x30c3070] and all(o==queue_owner for o,t,v in native_submissions) and task==native_submissions[2][1], native_submissions\\n  assert p.get(queue_head,'Q')[0]==native_submissions[0][1]+8\\n\"\n"+patch[b+len('    ins+=queue_patch\n'):]
patch=patch.replace("  rows=[row for row in queue_events if row.get('event')=='deferred' and row.get('vtable')=='0x10774c8']", "  rows=[{'task':hex(t)} for owner,t,vt in native_submissions if vt==0x10774c8]")
patch=patch.replace("   assert native_events==[['slot8',hex(task)]],native_events\\n", "")
patch=patch.replace("   assert native_events==[['slot8',hex(task)]]", "   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\\n   assert native_events==[(phase,t) for owner,t,vt in native_submissions for phase in ('0x904faf0','0x904fb00','0x904fb10')], (native_submissions,native_events)")
patch += "    code=code.replace(\"   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\", \"   assert p.get(queue_head,'QQQ')==(0,queue_head,queue_head)\\n   assert native_events==[(phase,t) for owner,t,vt in native_submissions for phase in ('0x904faf0','0x904fb00','0x904fb10')], (native_submissions,native_events)\\n   print('NATIVE_SUBMISSIONS', [(hex(o),hex(t),hex(v)) for o,t,v in native_submissions], flush=True)\",1)\n"
patch += "    code=code.replace(\"  rows=[row for row in queue_events if row.get('event')=='deferred' and row.get('vtable')=='0x10774c8']\", \"  rows=[{'task':hex(t)} for owner,t,vt in native_submissions if vt==0x10774c8]\")\n"
needle="native=root/'tools/pencil2-page-v219/native'"
# add code transformation into generated builder immediately before native execution
insert="assert p.count('    return code\\n')==1\np=p.replace('    return code\\n', "+repr(patch+"    return code\n")+")\n"
needle="with tempfile.TemporaryDirectory(prefix='pencil242-') as output:"
assert s.count(needle)==1
s=s.replace(needle,"assert source.count(\"native=root/'tools/pencil2-page-v219/native'\")>=1\nsource = source.replace("+repr("native=root/'tools/pencil2-page-v219/native'")+", "+repr(insert+"native=root/'tools/pencil2-page-v219/native'")+")\n"+needle)
assert s.count("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)")==1
s = s.replace("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)", "        assert run.returncode == 7 and message_file.exists(), (run.stdout,run.stderr)\n        message=ast.literal_eval(message_file.read_text()); inits,task,sub,before,after,calls,events,selected,error,nonzero,changed,digest=message\n        assert inits == [[selected,'0x904fb00']], (inits,selected); skip=bool(os.environ.get('PENCIL_SKIP_DRAW'))\n        assert before == after and before[0] and before[1] == 5890 and before[2], message\n        assert calls[:4] == [['glGenTextures',before[2]],['glGenFramebuffers',before[0]],['glBindFramebuffer',36160,before[0],None],['glFramebufferTexture2D',36160,36064,3553,before[2],0,None]], calls\n        assert (calls[-1] == ['glBindFramebuffer',36160,before[0],None]) == (not skip), calls\n        assert [x[0] for x in events] == ['run','update','draw'] and events[0][1] == task and events[1][1] == events[2][1] == selected, message\n        assert events[2][2] == sub, message\n        assert events[2][3] == [1,0,0,0,0,-1,0,0,0,0,1,0,6,183,0,1], message\n        assert error == 0 and nonzero == changed == (0 if skip else 322), message\n        assert digest == ('8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a' if skip else 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'), message\n        print('Verified five producer submissions and native FIFO run and slot-8 returns, original manager canvas, attachment and '+('skip' if skip else 'draw')+' control.')\n        raise SystemExit(0)")
s=s.replace('source = source.replace(child_exit, " if run.returncode: raise SystemExit(run.returncode)")', 'source = source.replace(child_exit, " if run.returncode: print(run.stdout,run.stderr,flush=True); raise SystemExit(run.returncode)")')
s=s.replace("source = base.read_text()", "source = base.read_text()\nsource = source.replace(\" pixels=__import__('zlib').decompress(raw.read_bytes())\", \" assert raw.exists(), (run.returncode, run.stdout, run.stderr, (out/'result.json').read_text() if (out/'result.json').exists() else 'no result')\\n pixels=__import__('zlib').decompress(raw.read_bytes())\")")
with tempfile.TemporaryDirectory(prefix='pencil277-') as td:
    script=Path(td)/'probe.py'
    s=s.replace('(run.stdout,run.stderr)', '(run.returncode,run.stdout,run.stderr)')
    script.write_text(s)
    subprocess.run([sys.executable,str(script)],cwd=root,env=os.environ.copy(),check=True)
