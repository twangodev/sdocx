"""Combine the retained worker iteration and explicit Mesa renderer handoff."""
import ast
import hashlib
import json
import os
import pathlib
import subprocess
import sys
import tempfile

os.environ.pop('PENCIL_RAW', None)

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
if sys.argv[1:]:
    raise SystemExit('Use no arguments')

root = pathlib.Path(__file__).resolve().parents[2]
worker = root / 'tools/pencil2-worker-v241/probe.py'
draw = root / 'tools/pencil2-async-route/draw-attempt.py'
base = root / 'tools/pencil2-page-v219/no-replay.py'
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
pins = {
    worker: 'eea7a5e0c43b2e0e72c5c01679112dee5f6608e95bf9aba2ece8b1d628862222',
    draw: '17f74e27d9dc0e0dc4173f08901444bd0627eae7a1b6e023232ff78cb9f72982',
    base: '3533e38ae7cf051d28dd07666b1b2c4319c35a24eb25e74cd909ac2f1af7c07d',
    root / 'tools/pencil2-page-v219/native/tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py': '996ef6db3a376619e9e5d1d05b7d5e1a940726f3feb2ef6895275a705576ee3e',
}
guest_libs = root / 'tools/pencil2-page-v219/native/scratch/apk-analysis-native/arm64-v8a'
pins.update({guest_libs / name: digest for name, digest in {
 'libSPenModel.so':'4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
 'libSPenPenCommon.so':'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'libSPenBase.so':'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'libSPenPencil2.so':'3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f',
 'libSPenFountainPen.so':'1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
 'libSPenDrawing.so':'788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'libSPenGraphics.so':'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libSPenComposer.so':'52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f',
 'libc++_shared.so':'4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}.items()})
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
child_exit = " if run.returncode: raise RuntimeError(run.stdout[-4000:]+run.stderr[-4000:])"
assert source.count(child_exit) == 1
source = source.replace(child_exit, " if run.returncode: raise SystemExit(run.returncode)")
assert source.count("p=(root/'tools/pencil2-public-replay/drawing_native_target_translated_probe.py').read_text()") == 1
source = source.replace("p=(root/'tools/pencil2-public-replay/drawing_native_target_translated_probe.py').read_text()", "p=(root/'tools/pencil2-public-replay/drawing_native_target_translated_probe.py').read_text()\nassert p.count('fmt!=0x1908 or kind!=0x1401') == 2\nassert p.count('width*height*4)); data=') == 2\np=p.replace('fmt!=0x1908 or kind!=0x1401', 'fmt not in (0x1908,0x1903) or (fmt==0x1903 and ptr) or kind!=0x1401',1).replace('width*height*4)); data=', 'width*height*(1 if fmt==0x1903 else 4))); data=',1)")
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

source = source.replace("native=root/'tools/pencil2-page-v219/native'", 'old_draw="  p.call(\'SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)\',[selected_rt,bitmap,matrix],limit=8000000)"\nnew_draw="  rows=[row for row in queue_events if row.get(\'event\')==\'deferred\' and row.get(\'vtable\')==\'0x10774c8\']\\n  assert len(rows)==1, rows\\n  task=int(rows[0][\'task\'],16); assert p.get(task+32,\'Q\')[0]==selected_rt\\n  original_canvas=p.get(task+40,\'Q\')[0]; target[\'original_canvas\']={\'ptr\':hex(original_canvas),\'bytes\':p.u.mem_read(original_canvas,64).hex(),\'vtable\':hex(p.get(original_canvas,\'Q\')[0]),\'is_pen_canvas\':original_canvas==p.pen_canvas,\'is_draw_canvas\':original_canvas==p.draw_canvas,\'vtable_slots\':{str(offset):hex(p.get(p.get(original_canvas,\'Q\')[0]+offset,\'Q\')[0]) for offset in (192,216,232,240)},\'message_matrix\':list(p.get(task+72,\'16f\'))}\\n  key=bytes([102,97,99,116,111,114,121]).decode()\\n  p.syms[key]=0x40c04ac\\n  fmt=p.alloc(32);p.u.mem_write(fmt,bytes([82,71,66,65,56,56,56,56,0]))\\n  p.put(0x200ef000,bytes([81,81]).decode(),0,0); upload_start=len(gl_simple)\\n  try:\\n   p.call(key,[p.handoff_queue,192,192,fmt,0,0,0,0,0,0],limit=8000000)\\n   obj=p.x(0); factory_uploads=[r for r in gl_simple[upload_start:] if r[0]==bytes([103,108,84,101,120,73,109,97,103,101,50,68]).decode()]; p.syms[key]=0x4090a74; p.call(key,[obj]); count=p.x(0); p.syms[key]=0x4090a2c; p.call(key,[obj,0]); sub=p.x(0); p.syms[key]=0x40c1454; p.call(key,[p.handoff_queue,obj,1]); cv=p.x(0); err=ip.cfunc(p.mesa.gl,bytes([103,108,71,101,116,69,114,114,111,114]).decode(),C.c_uint,[])(); result=repr((hex(obj),count,hex(sub),hex(cv),hex(err),factory_uploads,[r for r in gl_simple if r[0]==bytes([103,108,84,101,120,73,109,97,103,101,50,68]).decode()]))\\n  except Exception as exc:\\n   result=repr(exc)+hex(p.u.reg_read(R.UC_ARM64_REG_PC))+repr([hex(p.x(i)) for i in range(8)])\\n  open(bytes([47,116,109,112,47,112,101,110,99,105,108,50,45,102,97,99,116,111,114,121]).decode(),bytes([119]).decode()).write(result)\\n  p.put(task+40,bytes([81]).decode(),cv)\\n  observed=[]\\n  def observe_run(u,a,z,d):\\n   if a==0x104676c: observed.append([\'run\',hex(p.x(0))])\\n   if a==p.syms[\'SPen::Pencil2StrokeDrawableRTV4::Update(bool)\']: observed.append([\'update\',hex(p.x(0)),p.x(1)])\\n   if a==p.syms[\'SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)\']: observed.append([\'draw\',hex(p.x(0)),hex(p.x(1)),list(p.get(p.x(2),\'16f\'))])\\n  p.u.hook_add(UC_HOOK_CODE,observe_run)\\n  p.syms[\'render_message_run\']=0x104676c\\n  prior=(C.c_ubyte*(192*192*4))(); ip.cfunc(p.mesa.gl,bytes([103,108,82,101,97,100,80,105,120,101,108,115]).decode(),None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(0,0,192,192,0x1908,0x1401,prior); prior=bytes(prior)\\n  try:\\n   p.call(bytes([114,101,110,100,101,114,95,109,101,115,115,97,103,101,95,114,117,110]).decode(),[task],limit=8000000)\\n   buf=(C.c_ubyte*(192*192*4))(); ip.cfunc(p.mesa.gl,bytes([103,108,82,101,97,100,80,105,120,101,108,115]).decode(),None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(0,0,192,192,0x1908,0x1401,buf); raw=bytes(buf); result=repr((observed,hex(selected_rt),ip.cfunc(p.mesa.gl,bytes([103,108,71,101,116,69,114,114,111,114]).decode(),C.c_uint,[])(),sum(x!=0 for x in raw),sum(a!=b for a,b in zip(prior,raw)),__import__(bytes([104,97,115,104,108,105,98]).decode()).sha256(raw).hexdigest()))\\n  except Exception as exc:\\n   result=repr((repr(exc),hex(p.u.reg_read(R.UC_ARM64_REG_PC)),observed))\\n  open(bytes([47,116,109,112,47,112,101,110,99,105,108,50,45,109,101,115,115,97,103,101]).decode(),bytes([119]).decode()).write(result)\\n  raise SystemExit(7)\\n"\nassert p.count(old_draw)==1\np=p.replace(old_draw,new_draw)\np=p.replace("\'gl_error\':target[\'gl_error\']", "\'gl_error\':target[\'gl_error\'], \'observed_run_draw\':target.get(\'observed_run_draw\')")\n' + "native=root/'tools/pencil2-page-v219/native'")

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
source = source.replace("'gl_error':target['gl_error']", "'gl_error':target['gl_error'], 'observed_run_draw':target.get('observed_run_draw'), 'original_canvas':target.get('original_canvas'), 'uniforms':[v for v in target['gl_calls'] if v[0]=='glUniformMatrix4fv']")
source = source.replace("native=root/'tools/pencil2-page-v219/native'", "assert p.count(\"  p.call('SPen::Pencil2StrokeDrawableRTV4::Update(bool)',[selected_rt,0])\")==1\np=p.replace(\"  p.call('SPen::Pencil2StrokeDrawableRTV4::Update(bool)',[selected_rt,0])\", \"  pass\")\nnative=root/'tools/pencil2-page-v219/native'")
source = source.replace("assert init['outcome']=='returned' and update['outcome']=='returned' and update['after_count']==84", "assert init['outcome']=='returned'")
source = source.replace("assert target['selected_rt']==update['selected_rt'] and update['entry_matches_selected_rt']", "assert target['selected_rt']==update['selected_rt']")
instrument = r"""
def snapshot(label):
 get=ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])
 attachment=ip.cfunc(p.mesa.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])
 istex=ip.cfunc(p.mesa.gl,'glIsTexture',C.c_ubyte,[C.c_uint])
 values=[]
 for enum in (0x8ca6,0x8caa,0x8069):
  value=C.c_int(); get(enum,C.byref(value)); values.append(value.value)
 typ=C.c_int(); name=C.c_int()
 attachment(0x8d40,0x8ce0,0x8cd0,C.byref(typ)); attachment(0x8d40,0x8ce0,0x8cd1,C.byref(name))
 buf=(C.c_ubyte*(192*192*4))(); ip.cfunc(p.mesa.gl,'glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(0,0,192,192,0x1908,0x1401,buf)
 raw=bytes(buf)
 if label=='after': __import__('pathlib').Path(__import__('os').environ['PENCIL_CAPTURE_TEMP']).write_bytes(__import__('zlib').compress(raw,9))
 return [label,values,typ.value,name.value,int(istex(name.value)),sum(x!=0 for x in raw),__import__('hashlib').sha256(raw).hexdigest()]
"""
instrument = ''.join('  ' + line if line.strip() else line for line in instrument.splitlines(keepends=True))
patch = "new_draw = new_draw.replace(\"  prior=(C.c_ubyte\", " + repr(instrument + "  try: snapshots=[snapshot('before')]\n  except Exception as exc: open('/tmp/pencil255-error','w').write(repr(exc)); raise\n  prior=(C.c_ubyte") + ")\n"
patch += "new_draw = new_draw.replace(\"[task],limit=8000000)\\n\", \"[task],limit=8000000)\\n   snapshots.append(snapshot('after'))\\n\")\n"
patch += "new_draw = new_draw.replace(\"result=repr((observed,hex(selected_rt),\", \"result=repr((snapshots, [row for row in gl_simple if row[0] in ('glGenTextures','glBindTexture','glTexImage2D','glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D','glCheckFramebufferStatus')], observed,hex(selected_rt),\")\n"
patch += "new_draw = new_draw.replace(\"  p.u.hook_add(UC_HOOK_CODE,observe_run)\", " + repr("  p.u.hook_add(UC_HOOK_CODE,observe_run)\n  if __import__('os').environ.get('PENCIL_SKIP_DRAW'):\n   def skip_draw(u,a,z,d):\n    if a==p.syms['SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']: u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))\n   p.u.hook_add(UC_HOOK_CODE,skip_draw)\n") + ")\n"
source = source.replace("assert p.count(old_draw)==1", patch + "assert p.count(old_draw)==1")
with tempfile.TemporaryDirectory(prefix='pencil242-') as output:
    captured = pathlib.Path(output) / 'receipt.json'
    os.environ['PENCIL_CAPTURE_TEMP'] = str(pathlib.Path(output) / 'frame.zlib')
    factory_file=pathlib.Path(output)/'factory'; message_file=pathlib.Path(output)/'message'
    for old, new in (('/tmp/pencil2-factory',factory_file),('/tmp/pencil2-message',message_file)):
        source=source.replace(','.join(map(str,old.encode())), ','.join(map(str,str(new).encode())))
    source = source.replace("(root/'tools/pencil2-page-v219/no-replay-receipt.json').write_text", f"Path({str(captured)!r}).write_text")
    with tempfile.NamedTemporaryFile('w', suffix='.py', dir=root / 'tools/pencil2-worker-v243') as tmp:
        tmp.write(source)
        tmp.flush()
        run = subprocess.run([sys.executable, tmp.name], cwd=root, text=True, capture_output=True)
        assert run.returncode == 7 and factory_file.exists() and message_file.exists(), (run.returncode,run.stdout,run.stderr)
        factory=ast.literal_eval(factory_file.read_text()); message=ast.literal_eval(message_file.read_text())
        factory_file.unlink(); message_file.unlink()
        assert factory[0] != '0x0' and factory[1] == 1 and factory[2] != '0x0' and factory[3] != '0x0' and factory[4] == '0x0', factory
        assert len(message) == 8, message
        snapshots, calls, observed, selected, error, nonzero, changed, digest = message
        assert snapshots[0][1][:2] == snapshots[1][1][:2] == [2, 2]
        assert [row[2:5] for row in snapshots] == [[5890,4,1],[5890,4,1]]
        assert [row[0] for row in observed] == ['run', 'update', 'draw']
        assert error == 0
        assert snapshots[0][6] == '5574536817c27b5cbfa5536e0235b9a5dd0a3e2705ba0dde2a1c79a5d7e879ee'
        if os.environ.get('PENCIL_SKIP_DRAW'):
            assert changed == 0 and snapshots[1][6] == snapshots[0][6], message
        else:
            assert changed == 72 and digest == 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051', message
        import zlib
        mode = 'no_draw' if os.environ.get('PENCIL_SKIP_DRAW') else 'paint'
        fixture = root / 'tools/pencil2-worker-v255' / ('no-draw.rgba.zlib' if mode == 'no_draw' else 'paint.rgba.zlib')
        assert zlib.decompress(pathlib.Path(os.environ['PENCIL_CAPTURE_TEMP']).read_bytes()) == zlib.decompress(fixture.read_bytes())
        receipt = json.loads((root / 'tools/pencil2-worker-v255/receipt.json').read_text())
        assert receipt[mode] == {'snapshots': snapshots, 'changed_bytes': changed, 'digest': digest, 'skip_draw': mode == 'no_draw', 'calls':calls}
        paint = zlib.decompress((fixture.parent / 'paint.rgba.zlib').read_bytes())
        control = zlib.decompress((fixture.parent / 'no-draw.rgba.zlib').read_bytes())
        diffs = [(i // 4 % 192, i // 4 // 192, i % 4, a) for i, (a, b) in enumerate(zip(paint, control)) if a != b]
        assert len(paint) == len(control) == 192 * 192 * 4
        assert all(channel == 0 for _, _, channel, _ in diffs)
        assert receipt['comparison'] == {'changed_red_pixels':len(diffs), 'other_changed_channels':0, 'bounds_bottom_origin':[min(x for x,y,c,v in diffs),min(y for x,y,c,v in diffs),max(x for x,y,c,v in diffs),max(y for x,y,c,v in diffs)], 'red_range':[min(v for x,y,c,v in diffs),max(v for x,y,c,v in diffs)]}
        print(f'Verified {mode} frame, graphics states and receipt.')
