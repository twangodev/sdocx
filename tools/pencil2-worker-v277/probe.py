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
"""
    code=code.replace(marker,early+setup+marker)
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
    assert code.count('  prior=(C.c_ubyte')==1
    code=code.replace('  prior=(C.c_ubyte',ins+'  prior=(C.c_ubyte')
    assert code.count('result=repr((observed,hex(selected_rt),')==1
    code=code.replace('result=repr((observed,hex(selected_rt),',"result=repr((hex(manager_sub),before_fb,snap(),[row for row in gl_simple if row[0] in ('glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D','glGenTextures')],observed,hex(selected_rt),")
    begin=code.index('  key=bytes([102,97,99,116,111,114,121]).decode()')
    stop=code.index('  observed=[]',begin)
    code=code[:begin]+"  assert original_canvas==cv, (hex(original_canvas),hex(cv))"+chr(10)+code[stop:]
    assert code.count(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices')==1
    code=code.replace(' assert matrices==[[1,0,0,0,-1,0,6,183,1]]*2, matrices', ' assert matrices==[[1,0,0,0,-1,0,6,183,1]], matrices')
'''
needle="native=root/'tools/pencil2-page-v219/native'"
# add code transformation into generated builder immediately before native execution
insert="assert p.count('    return code\\n')==1\np=p.replace('    return code\\n', "+repr(patch+"    return code\n")+")\n"
needle="with tempfile.TemporaryDirectory(prefix='pencil242-') as output:"
assert s.count(needle)==1
s=s.replace(needle,"assert source.count(\"native=root/'tools/pencil2-page-v219/native'\")>=1\nsource = source.replace("+repr("native=root/'tools/pencil2-page-v219/native'")+", "+repr(insert+"native=root/'tools/pencil2-page-v219/native'")+")\n"+needle)
assert s.count("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)")==1
s = s.replace("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.stdout,run.stderr)", "        assert run.returncode == 7 and message_file.exists(), (run.stdout,run.stderr)\n        message=ast.literal_eval(message_file.read_text()); sub,before,after,calls,events,selected,error,nonzero,changed,digest=message; skip=bool(os.environ.get('PENCIL_SKIP_DRAW'))\n        assert before == after and before[0] and before[1] == 5890 and before[2], message\n        assert calls[:4] == [['glGenTextures',before[2]],['glGenFramebuffers',before[0]],['glBindFramebuffer',36160,before[0],None],['glFramebufferTexture2D',36160,36064,3553,before[2],0,None]], calls\n        assert (calls[-1] == ['glBindFramebuffer',36160,before[0],None]) == (not skip), calls\n        assert [x[0] for x in events] == ['run','update','draw'] and events[1][1] == events[2][1] == selected, message\n        assert events[2][2] == sub, message\n        assert events[2][3] == [1,0,0,0,0,-1,0,0,0,0,1,0,6,183,0,1], message\n        assert error == 0 and nonzero == changed == (0 if skip else 322), message\n        assert digest == ('8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a' if skip else 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'), message\n        print('Verified original manager canvas, attachment and '+('skip' if skip else 'draw')+' control.')\n        raise SystemExit(0)")
with tempfile.TemporaryDirectory(prefix='pencil277-') as td:
    script=Path(td)/'probe.py'
    script.write_text(s)
    subprocess.run([sys.executable,str(script)],cwd=root,env=os.environ.copy(),check=True)
