"""Combine the retained worker iteration and explicit Mesa renderer handoff."""
import ast
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
if sys.argv[1:] not in ([], ['--write-receipt']):
    raise SystemExit('Use no arguments to verify or --write-receipt to update')

root = pathlib.Path(__file__).resolve().parents[2]
worker = root / 'tools/pencil2-worker-v241/probe.py'
draw = root / 'tools/pencil2-async-route/draw-attempt.py'
base = root / 'tools/pencil2-page-v219/no-replay.py'
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
pins = {
    worker: 'eea7a5e0c43b2e0e72c5c01679112dee5f6608e95bf9aba2ece8b1d628862222',
    draw: '17f74e27d9dc0e0dc4173f08901444bd0627eae7a1b6e023232ff78cb9f72982',
    base: '3533e38ae7cf051d28dd07666b1b2c4319c35a24eb25e74cd909ac2f1af7c07d',
}
for path, digest in pins.items():
    assert sha(path) == digest, f'pin mismatch: {path}'
tree = ast.parse(worker.read_text())
new = next(ast.literal_eval(n.value) for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'new' for t in n.targets))
draw_text = draw.read_text()
insert = draw_text.split("insert = '''", 1)[1].split("'''\nattach =", 1)[0]
attach = draw_text.split("attach = '''", 1)[1].split("'''\nfor attached", 1)[0]
insert = insert.replace('ATTACH_CONTEXT', attach).replace('_always=True', '_always=False')
insert = insert.replace(' refs=[]', ' clone_points=[]; refs=[]')
insert = insert.replace("if a==0x3081a68: print('POINTCOUNT',p.x(0),_always=False)", "if a==0x3081a68: clone_points.append(p.x(0)); assert p.x(0)==4")
new = new.replace('_always=True', '_always=False')
new = new.replace("  print('DRAIN_RETURN',_always=False)", "  assert clone_points == [4], clone_points\n  assert worker == [0x307b81c,0x307b830,0x307b848,0x307b868,0x307b880,0x307b88c], worker\n  assert p.get(head,'QQ') == (head,head)\n  print('DRAIN_RETURN',_always=False)")
source = base.read_text()
anchor = "failure=\" except Exception as e:"
assert source.count(anchor) == 1
setup = '''
old_call = "p.call('SPen::ObjectDrawing::DrawObjectList(SPen::ISPBitmap*, SPen::ObjectList const*, SPen::Rect const*, SPen::Matrix3<float> const*, bool, std::__ndk1::function<bool (int, int)>*)',[p.drawing,p.bitmap,ol,0,matrix,0,0],limit=50000000)"
assert ins.count(old_call) == 1
ins = ins.replace(' events=[]; matrices=[]', INSERT + ' events=[]; matrices=[]')
ins = ins.replace(old_call, NEW)
ins = ins.replace("  assert p.x(0)==1, f'native list returned {p.x(0)}'", "  assert p.get(head+16,'Q')[0] == 0")
ins = ins.replace(' assert [e[0] for e in events]==names, events', ' assert [e[0] for e in events] == [names[0]] + names, events')
'''.replace('INSERT', repr(insert)).replace('NEW', repr(new))
source = source.replace(anchor, setup + anchor)
source = source.replace("  assert p.x(0)==1, f'native list returned {p.x(0)}'", "  assert p.get(head+16,'Q')[0] == 0")
source = source.replace("native=root/'tools/pencil2-page-v219/native'", "assert p.count(\" init_diagnostic={'before200'\") == 1\np = p.replace(\" init_diagnostic={'before200'\", \" assert len(pencil_buffer_events)==84 and {row['rt'] for row in pencil_buffer_events} == {hex(selected_rt)}\\n init_diagnostic={'before200'\")\nnative=root/'tools/pencil2-page-v219/native'")

old_matrix = "  matrix=p.alloc(64);p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,32,32,0,1)"
new_matrix = """  canvas=p.alloc(64); impl=p.alloc(32); screen=p.alloc(256)
  cvt=p.alloc(256); bvt=p.alloc(256); secondary=p.alloc(32); matrix=p.alloc(64)
  p.put(canvas,'Q',cvt); p.put(canvas+32,'Q',impl); p.put(impl+8,'Q',screen)
  p.put(screen,'Q',bvt); p.put(screen+176,'Q',secondary)
  p.put(screen+32,'4i',3,5,195,197); p.put(screen+204,'B',0)
  p.put(bvt+152,'Q',p.syms['SPen::SPScreenBitmap::GetBitmapWritable()'])
  p.put(secondary+16,'Q',p.syms['non-virtual thunk to SPen::SPScreenBitmap::GetProjectionMatrix()'])
  stub=0x200f2600; p.u.mem_write(stub,bytes.fromhex('c0035fd6')); p.put(cvt+216,'Q',stub)
  def model_hook(u,a,z,d):
   if a==stub:
    assert p.x(0)==canvas
    u.mem_write(u.reg_read(R.UC_ARM64_REG_X8),struct.pack('<9f',*matrices[0]))
  p.u.hook_add(UC_HOOK_CODE,model_hook)
  p.call('SPen::SPPenCanvas::GetCanvasMatrix()',[canvas],x8=matrix)
  native_matrix=list(p.get(matrix,'16f'))
  if __import__('os').environ.get('PENCIL_RAW') == '1': p.put(matrix,'16f',1,0,0,0,0,-1,0,0,0,0,1,0,6,183,0,1)
  supplied_matrix=list(p.get(matrix,'16f'))
"""
source = source.replace("native=root/'tools/pencil2-page-v219/native'", "old_matrix = " + repr(old_matrix) + "\nnew_matrix = " + repr(new_matrix) + "\nassert p.count(old_matrix)==1\np=p.replace(old_matrix,new_matrix)\np=p.replace(\"'matrix':[1,0,0,0,0,1,0,0,0,0,1,0,32,32,0,1]\", \"'matrix':supplied_matrix\")\np=p.replace(\"target['nonzero_bytes']==317\", \"target['nonzero_bytes']==0\")\nnative=root/'tools/pencil2-page-v219/native'")
source = source.replace("target['nonzero_bytes']==317", "target['nonzero_bytes']>=0")
source = source.replace("'gl_error':target['gl_error']", "'gl_error':target['gl_error'], 'uniforms':[v for v in target['gl_calls'] if v[0]=='glUniformMatrix4fv']")
receipt_path = root / ('tools/pencil2-worker-v244/raw-receipt.json' if os.environ.get('PENCIL_RAW') == '1' else 'tools/pencil2-worker-v244/product-receipt.json')
with tempfile.TemporaryDirectory(prefix='pencil242-') as output:
    captured = pathlib.Path(output) / 'receipt.json'
    source = source.replace("(root/'tools/pencil2-page-v219/no-replay-receipt.json').write_text", f"Path({str(captured)!r}).write_text")
    with tempfile.NamedTemporaryFile('w', suffix='.py', dir=root / 'tools/pencil2-worker-v243') as tmp:
        tmp.write(source)
        tmp.flush()
        run = subprocess.run([sys.executable, tmp.name], cwd=root, text=True, capture_output=True)
        if run.returncode:
            print(run.stdout)
            print(run.stderr, file=sys.stderr)
            raise SystemExit(run.returncode)
    receipt = json.loads(captured.read_text())
receipt['script_sha256'] = sha(pathlib.Path(__file__))
receipt['parent_sha256'] = {str(p.relative_to(root)): sha(p) for p in (worker, draw, base)}
receipt['scope'] = ('Synthetic native EventLoop consumes one transplanted queue request, then an explicit host Mesa bridge initializes, updates and draws the selected renderer. The native canvas getter computes Matrix4 from a host-adapted observed Matrix3 and fabricated screen bitmap before the explicit draw; this does not establish automatic paint, normal scheduling, saved-page or device behavior.')
raw = os.environ.get('PENCIL_RAW') == '1'
expected = ('a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e', 322) if raw else ('8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a', 0)
assert (receipt['readback_sha256'], receipt['nonzero_bytes']) == expected
raw_input = [1,0,0,0,0,-1,0,0,0,0,1,0,6,183,0,1]
product_input = [0.010416666977107525,0,0,0,0,0.010416666977107525,0,0,0,0,-3.0518509447574615e-05,0,-0.9375,-0.90625,0,1]
raw_uniform = [0.010416666977107525,0,0,0,0,-0.010416666977107525,0,0,0,0,-3.0518509447574615e-05,0,-0.9375,0.90625,0,1]
product_uniform = [0.0001085069525288418,0,0,0,0,0.0001085069525288418,0,0,0,0,9.313794180343393e-10,0,-1.009765625,-1.0094400644302368,0,1]
assert receipt['explicit_draw_matrix4'] == (raw_input if raw else product_input)
assert receipt['uniforms'] == [['glUniformMatrix4fv',0,1,0,raw_uniform if raw else product_uniform]]
assert receipt['prior_sha256'] == '8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a'
assert receipt['matrix3'] == [1,0,0,0,-1,0,6,183,1] and receipt['update_count'] == 84 and receipt['gl_error'] == 0
print('CONTROL', 'raw_model' if raw else 'canvas_product', receipt['readback_sha256'], receipt['nonzero_bytes'], receipt['uniforms'])
receipt['worker'] = 'Submission, queue entry sequence, worker cleanup and empty sentinel asserted by the worker adapter; bitmap, context, locks and destructor remain synthetic.'
if sys.argv[1:] == ['--write-receipt']:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert not sys.argv[1:] and json.loads(receipt_path.read_text()) == receipt
print('Verified synthetic worker consumption followed by explicit host renderer Init, Update and Draw.')
