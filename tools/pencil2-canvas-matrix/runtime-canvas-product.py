#!/usr/bin/env python3
"""Explicitly connect the list's observed Matrix3 to the native canvas getter."""
from pathlib import Path
import ast, hashlib, json, subprocess, sys, tempfile

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
src = root / 'tools/pencil2-page-v219/queue-probe.py'
assert hashlib.sha256(src.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
code = src.read_text()
old = " raise SystemExit(0)\n'''"
new = """
 # Explicit host handoff: fabricate the canvas and bitmap wiring, and adapt
 # GetModelMatrix to return the matrix observed at the native list call.
 canvas=p.alloc(64); impl=p.alloc(32); bitmap=p.alloc(256)
 cvt=p.alloc(256); bvt=p.alloc(256); secondary=p.alloc(32); out=p.alloc(64)
 p.put(canvas,'Q',cvt); p.put(canvas+32,'Q',impl); p.put(impl+8,'Q',bitmap)
 p.put(bitmap,'Q',bvt); p.put(bitmap+176,'Q',secondary)
 p.put(bitmap+32,'4i',3,5,195,197); p.put(bitmap+204,'B',0)
 p.put(bvt+152,'Q',p.syms['SPen::SPScreenBitmap::GetBitmapWritable()'])
 p.put(secondary+16,'Q',p.syms['non-virtual thunk to SPen::SPScreenBitmap::GetProjectionMatrix()'])
 stub=0x200f2600; p.u.mem_write(stub,bytes.fromhex('c0035fd6')); p.put(cvt+216,'Q',stub)
 current=[None]; calls=[]
 def model_hook(u,a,z,d):
  if a==stub:
   assert p.x(0)==canvas
   u.mem_write(u.reg_read(R.UC_ARM64_REG_X8),struct.pack('<9f',*current[0]))
   calls.append(list(current[0]))
 p.u.hook_add(UC_HOOK_CODE,model_hook)
 cases=[matrices[0], [1.25,0.5,0, -0.25,2,0, 7,-3,1]]
 for model in cases:
  current[0]=model
  p.call('SPen::SPPenCanvas::GetCanvasMatrix()',[canvas],x8=out)
  print('CANVAS_PRODUCT',json.dumps({'model':model,'product':list(p.get(out,'16f'))}),_always=True)
 assert calls==cases
 raise SystemExit(0)
'''"""
assert code.count(old) == 1
code = code.replace(old, new)
with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=src.parent) as f:
    f.write(code); f.flush()
    run = subprocess.run([sys.executable, f.name], cwd=root, text=True, capture_output=True)
assert run.returncode == 0, (run.stdout, run.stderr)
rows = [json.loads(line.removeprefix('CANVAS_PRODUCT ')) for line in run.stdout.splitlines() if line.startswith('CANVAS_PRODUCT ')]
assert len(rows) == 2
for row in rows:
    m = row['model']; got = row['product']; scale = 2 / 192
    expected = [scale*m[0], -scale*m[1], 0, m[2], scale*m[3], -scale*m[4], 0, m[5], 0, 0, -3.0518509447574615e-5, 0, scale*m[6]-m[8], -scale*m[7]+m[8], 0, m[8]]
    assert all(abs(a-b) < 1e-6 for a,b in zip(got,expected)), (got,expected)
receipt = {'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'queue_probe_sha256': hashlib.sha256(src.read_bytes()).hexdigest(), 'scope': 'Explicit host handoff after synthetic Pencil2 list dispatch. Fabricated canvas and screen bitmap with native writable accessor, projection thunk, projection getter, canvas getter and product helper; host supplies model through virtual adapter. Not automatic page execution, renderer handoff or device evidence.', 'rectangle': [3,5,195,197], 'orientation': 0, 'list_addpoint_calls': 84, 'cases': rows}
(root/'tools/pencil2-canvas-matrix/runtime-canvas-product-receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
print(json.dumps(receipt, indent=2))
