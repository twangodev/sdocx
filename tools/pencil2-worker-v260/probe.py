"""Measure a manual RGBA8 copy of the retained native R8 capture."""
import ast
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
base = root / 'tools/pencil2-worker-v255/probe.py'
source = base.read_text()
assert hashlib.sha256(base.read_bytes()).hexdigest() == '7da7436d8c06f3ec58fe863a76998d140122deb4a970de699fa8b2e05dfcc00b'
needle = " if label=='after': __import__('pathlib').Path(__import__('os').environ['PENCIL_CAPTURE_TEMP']).write_bytes(__import__('zlib').compress(raw,9))"
extra = " if label=='after':\n  gl=p.mesa.gl\n  fn=lambda n,ret,args: ip.cfunc(gl,n,ret,args)\n  U=C.c_uint; I=C.c_int; P=C.POINTER(U)\n  tex=U(); fbo=U()\n  fn('glGenTextures',None,[I,P])(1,C.byref(tex))\n  fn('glBindTexture',None,[U,U])(0x0de1,tex.value)\n  fn('glTexImage2D',None,[U,I,I,I,I,I,U,U,C.c_void_p])(0x0de1,0,0x8058,192,192,0,0x1908,0x1401,None)\n  fn('glCopyTexSubImage2D',None,[U,I,I,I,I,I,I,I])(0x0de1,0,0,0,0,0,192,192)\n  copy_error=fn('glGetError',U,[])()\n  fn('glGenFramebuffers',None,[I,P])(1,C.byref(fbo))\n  fn('glBindFramebuffer',None,[U,U])(0x8d40,fbo.value)\n  fn('glFramebufferTexture2D',None,[U,U,U,U,I])(0x8d40,0x8ce0,0x0de1,tex.value,0)\n  status=fn('glCheckFramebufferStatus',U,[U])(0x8d40)\n  copied=(C.c_ubyte*(192*192*4))()\n  fn('glReadPixels',None,[I,I,I,I,U,U,C.c_void_p])(0,0,192,192,0x1908,0x1401,copied)\n  copied=bytes(copied)\n  err=fn('glGetError',U,[])()\n  __import__('pathlib').Path(__import__('os').environ['PENCIL_V260_RESULT']).write_text(repr((copy_error,status,err,[(i,raw[i:i+4].hex(),copied[i:i+4].hex()) for i in range(0,len(raw),4) if raw[i:i+4] != copied[i:i+4]][:10],__import__('hashlib').sha256(copied).hexdigest(),sum(raw[i:i+4]!=copied[i:i+4] for i in range(0,len(raw),4)))))\n  fn('glBindFramebuffer',None,[U,U])(0x8d40,2)\n\n"
assert source.count(needle) == 1
source = source.replace(needle, needle + '\n' + extra)
with tempfile.TemporaryDirectory(prefix='pencil260-') as tmp:
    result = Path(tmp) / 'result'
    env = dict(os.environ, PENCIL_V260_RESULT=str(result))
    env.pop('PENCIL_SKIP_DRAW', None)
    with tempfile.NamedTemporaryFile('w', suffix='.py', dir=Path(__file__).parent) as script:
        script.write(source)
        script.flush()
        subprocess.run([sys.executable, script.name], cwd=root, env=env, check=True)
    copy_error, status, read_error, examples, digest, different = ast.literal_eval(result.read_text())
    assert (copy_error, status, read_error, examples, digest, different) == (0, 0x8cd5, 0, [], 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051', 0)
    print('Verified full R8 framebuffer copy to RGBA8: all RGBA bytes match.')
