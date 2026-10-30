"""Control the destination factory integer option in the v262 native copy probe."""
import ast
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')

root = Path(__file__).resolve().parents[2]
base = root / 'tools/pencil2-worker-v255/probe.py'
source = base.read_text()
assert hashlib.sha256(base.read_bytes()).hexdigest() == '7da7436d8c06f3ec58fe863a76998d140122deb4a970de699fa8b2e05dfcc00b'
needle = " if label=='after': __import__('pathlib').Path(__import__('os').environ['PENCIL_CAPTURE_TEMP']).write_bytes(__import__('zlib').compress(raw,9))"
extra = """ if label=='after':
  mode='no-draw' if __import__('os').environ.get('PENCIL_SKIP_DRAW') else 'paint'
  fixture=__import__('zlib').decompress(__import__('pathlib').Path(__import__('os').environ['PENCIL_V261_FIXTURE']).read_bytes())
  assert raw == fixture and [row[0] for row in observed] == ['run','update','draw']
  log=[]; saved_context=p.u.context_save()
  try:
   def layout_upload():
    args=[p.x(i) for i in range(8)]; ptr=p.get(p.u.reg_read(R.UC_ARM64_REG_SP),'Q')[0]
    log.append(('layout_upload',args,hex(ptr)))
    assert ptr==0
    ip.cfunc(p.mesa.gl,'glTexImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(*args,None)
    gl_simple.append(['glTexImage2D',*args,hex(ptr),None])
    log.append(('immediate_error',ip.cfunc(p.mesa.gl,'glGetError',C.c_uint,[])()))
   p.callbacks['glTexImage2D']=layout_upload
   p.syms['copy_factory']=0x40c04ac
   fmt2=p.alloc(32);p.u.mem_write(fmt2,bytes([82,71,66,65,56,56,56,56,0]))
   p.call('copy_factory',[p.handoff_queue,192,192,fmt2,0,0,1,0,0,0],limit=8000000)
   dst=p.x(0); log.append(('factory_return',hex(dst),ip.cfunc(p.mesa.gl,'glGetError',C.c_uint,[])())); raise RuntimeError('stop after allocation'); rect=p.alloc(16);p.put(rect,'4i',0,0,192,192);log.append(('objects',hex(obj),hex(dst),values,typ.value,name.value))
   def copy_gl():
    args=[p.x(i) for i in range(8)]; bound=C.c_int();ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])(0x8069,C.byref(bound));log.append(('glCopyTexSubImage2D',args,bound.value));ip.cfunc(p.mesa.gl,'glCopyTexSubImage2D',None,[C.c_uint]+[C.c_int]*7)(*args)
   p.callbacks['glCopyTexSubImage2D']=copy_gl
   p.syms['native_copy']=0x4090dfc
   def cp_hook(u,a,z,d):
    if a in (0x4090dfc,0x4090e88,0x4090e9c,0x4090ea8,0x4090ee8,0x4090f60,0x405f3c8,0x405f440):
     log.append((hex(a),[hex(p.x(i)) for i in range(7)], list(p.get(p.x(0)+40,'2I')) if a==0x4090ea8 else ([hex(p.x(23))] if a==0x4090e88 else [])))
   p.u.hook_add(UC_HOOK_CODE,cp_hook)
   p.call('native_copy',[dst,obj,0,0,rect],limit=8000000)
   log.append(('returned',hex(p.x(0))))
   gl=p.mesa.gl; fn=lambda n,ret,args: ip.cfunc(gl,n,ret,args); U=C.c_uint; I=C.c_int; P=C.POINTER(U)
   bound=I();fn('glGetIntegerv',None,[U,C.POINTER(I)])(0x8069,C.byref(bound)); fbo=U()
   fn('glGenFramebuffers',None,[I,P])(1,C.byref(fbo));fn('glBindFramebuffer',None,[U,U])(0x8d40,fbo.value);fn('glFramebufferTexture2D',None,[U,U,U,U,I])(0x8d40,0x8ce0,0x0de1,bound.value,0)
   status=fn('glCheckFramebufferStatus',U,[U])(0x8d40); buf=(C.c_ubyte*(192*192*4))();fn('glReadPixels',None,[I,I,I,I,U,U,C.c_void_p])(0,0,192,192,0x1908,0x1401,buf); copied=bytes(buf)
   log.append(('read',bound.value,status,fn('glGetError',U,[])(),__import__('hashlib').sha256(copied).hexdigest(),sum(a!=b for a,b in zip(raw,copied)),[r for r in gl_simple if r[0]=='glTexImage2D']))
   fn('glBindFramebuffer',None,[U,U])(0x8ca9,values[0]);fn('glBindFramebuffer',None,[U,U])(0x8ca8,values[1])
  except Exception as exc: log.append(('error',repr(exc),hex(p.u.reg_read(R.UC_ARM64_REG_PC))))
  p.u.context_restore(saved_context)
  __import__('pathlib').Path(__import__('os').environ['PENCIL_V261_RESULT']).write_text(repr(log))
  raise SystemExit(72)
"""
assert source.count("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.returncode,run.stdout,run.stderr)") == 1
assert extra.count("[p.handoff_queue,192,192,fmt2,0,0,1,0,0,0]") == 1
extra = extra.replace("   p.call('copy_factory'", "   p.put(0x200ef000,'2Q'," + os.environ.get("PENCIL_LAYOUT", "0") + "," + os.environ.get("PENCIL_TARGET", "0") + ")\n   p.call('copy_factory'")
source=source.replace("        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.returncode,run.stdout,run.stderr)", "        if run.returncode == 72: raise SystemExit(72)\n        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.returncode,run.stdout,run.stderr)")
assert source.count(needle) == 1
source = source.replace(needle, needle + '\n' + extra)
with tempfile.TemporaryDirectory(prefix='pencil260-') as tmp:
    result = Path(tmp) / 'result'
    assert source.count('/tmp/pencil255-error') == 1
    source = source.replace('/tmp/pencil255-error', str(Path(tmp) / 'error'))
    env = dict(os.environ, PENCIL_V261_RESULT=str(result), PENCIL_V261_FIXTURE=str(root / 'tools/pencil2-worker-v255' / ('no-draw.rgba.zlib' if os.environ.get('PENCIL_SKIP_DRAW') else 'paint.rgba.zlib')))
    with tempfile.NamedTemporaryFile('w', suffix='.py', dir=Path(__file__).parent) as script:
        script.write(source)
        script.flush()
        run = subprocess.run([sys.executable, script.name], cwd=root, env=env, capture_output=True, text=True)
    assert run.returncode == 72 and result.exists(), (run.returncode, run.stderr)
    rows = ast.literal_eval(result.read_text())
    memory = int(os.environ.get('PENCIL_LAYOUT', '0'))
    target = int(os.environ.get('PENCIL_TARGET', '0'))
    expected = {(0,0):((33321,6403,5121),0), (1,0):((33332,6403,5123),1282), (6,2):((32856,6408,5121),0)}
    assert (memory,target) in expected, 'Supported controls: 0/0, 1/0, 6/2'
    assert len(rows)==4 and rows[0][0]=='layout_upload' and rows[0][2]=='0x0', rows
    args=rows[0][1]
    assert args[:2]==[3553,0] and args[3:6]==[192,192,0]
    assert (args[2],args[6],args[7])==expected[memory,target][0], rows
    assert rows[1]==('immediate_error',expected[memory,target][1])
    assert rows[2][0]=='factory_return' and rows[2][1]!='0x0' and rows[2][2]==0
    assert rows[3][1]=="RuntimeError('stop after allocation')"
    print(f'memory={memory} target={target}: glTexImage2D {expected[memory,target][0]}; immediate GL error={rows[1][1]}; factory returned; deliberate stop')
