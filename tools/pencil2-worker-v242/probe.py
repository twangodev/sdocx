"""Combine the retained worker iteration and explicit Mesa renderer handoff."""
import ast
import hashlib
import json
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
receipt_path = root / 'tools/pencil2-worker-v242/receipt.json'
with tempfile.TemporaryDirectory(prefix='pencil242-') as output:
    captured = pathlib.Path(output) / 'receipt.json'
    source = source.replace("(root/'tools/pencil2-page-v219/no-replay-receipt.json').write_text", f"Path({str(captured)!r}).write_text")
    with tempfile.NamedTemporaryFile('w', suffix='.py', dir=root / 'tools/pencil2-worker-v242') as tmp:
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
receipt['scope'] = ('Synthetic native EventLoop consumes one transplanted queue request, then an explicit host Mesa bridge initializes, updates and draws the selected renderer. The Matrix4 is independently supplied; this does not establish automatic paint, matrix conversion, normal scheduling, saved-page or device behavior.')
receipt['worker'] = 'Submission, queue entry sequence, worker cleanup and empty sentinel asserted by the worker adapter; bitmap, context, locks and destructor remain synthetic.'
if sys.argv[1:]:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert json.loads(receipt_path.read_text()) == receipt, 'Receipt differs from observed run'
print('Verified synthetic worker consumption followed by explicit host renderer Init, Update and Draw.')
