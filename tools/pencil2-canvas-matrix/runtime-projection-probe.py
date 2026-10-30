from pathlib import Path
import hashlib, json, subprocess, tempfile, sys, struct
if sys.flags.optimize:
 raise SystemExit('Run without Python optimization; capture requires assertions')
root=Path('/home/jding/RustroverProjects/sdocx')
src=root/'tools/pencil2-page-v219/queue-probe.py'
assert hashlib.sha256(src.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
code=src.read_text()
old=" raise SystemExit(0)\n'''"
new="""\n name='SPen::SPScreenBitmap::GetProjectionMatrix()'
 print('PROJECTION_SYMBOLS',[n for n in p.syms if 'GetProjectionMatrix' in n],_always=True)
 assert name in p.syms
 bitmap=p.alloc(256); out=p.alloc(64)
 p.put(bitmap+32,'4i',3,5,195,197)
 for flip in (0,1):
  p.put(bitmap+204,'B',flip)
  p.call(name,[bitmap],x8=out)
  print('PROJECTION_RESULT',flip,list(p.get(out,'16f')),_always=True)
 raise SystemExit(0)
'''"""
assert code.count(old)==1
code=code.replace(old,new)
with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=src.parent) as f:
 f.write(code);f.flush();r=subprocess.run([sys.executable,f.name],cwd=root,text=True,capture_output=True)
 assert r.returncode == 0, (r.stdout, r.stderr)
 lines=[line for line in r.stdout.splitlines() if line.startswith('PROJECTION_RESULT ')]
 assert len(lines)==2
 import ast
 values=[ast.literal_eval(line.split(' ',2)[2]) for line in lines]
 expected=[[2/192,0,0,0,0,sign*2/192,0,0,0,0,struct.unpack('<f',bytes.fromhex('000100b8'))[0],0,-1,-sign,-0.0,1] for sign in (-1,1)]
 for got,want in zip(values,expected):
  assert struct.pack('<16f',*got)==struct.pack('<16f',*want)
 receipt={'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'queue_probe_sha256':hashlib.sha256(src.read_bytes()).hexdigest(),'scope':'Direct native ARM64 projection getter calls in the synthetic list runtime using fabricated bitmap fields; not an observed list-to-canvas handoff or real page.', 'rectangle':[3,5,195,197], 'orientations':[0,1], 'matrix4':values, 'list_return':1, 'list_addpoint_calls':84}
 (root/'tools/pencil2-canvas-matrix/runtime-projection-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt,indent=2))
