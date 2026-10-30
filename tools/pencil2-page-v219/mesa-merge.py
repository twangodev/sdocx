from pathlib import Path
import ast, hashlib, json, os, subprocess, tempfile
import sys
if sys.flags.optimize:
 raise SystemExit('Run without Python optimization; capture requires assertions')
root=Path.cwd()
pins={'tools/pencil2-page-v219/queue-probe.py': '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c', 'tools/pencil2-public-replay/drawing_native_target_translated_probe.py': '675279014dfeb4778d8118a02d8d9d77779f4cec1c6f18444a5f7b28751e6861', 'tools/pencil2-page-v219/native/tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/inkpen_paint_native.py': 'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae', 'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'}
for name, digest in pins.items():
 if hashlib.sha256((root/name).read_bytes()).hexdigest()!=digest: raise SystemExit(f"pin mismatch: {name}")
q=(root/'tools/pencil2-page-v219/queue-probe.py').read_text(); mod=ast.parse(q); ins=next(ast.literal_eval(n.value) for n in mod.body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='insert' for t in n.targets)); ins=ins.replace(' raise SystemExit(0)\n','').replace('_always=True','_always=False')
p=(root/'tools/pencil2-public-replay/drawing_native_target_translated_probe.py').read_text()
p=p.replace('/home/jding/RustroverProjects/sdocx/tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance', str(root/'tools/pencil2-page-v219/native/tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance'))
p=p.replace('root = Path(__file__).resolve().parents[2]',f'root = Path({str(root)!r})',1)
p=p.replace("source = root / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py'", "source = root / 'tools/pencil2-page-v219/native/tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py'")
p=p.replace('spec.loader.exec_module(m)\n','spec.loader.exec_module(m)\nm.check_pins()\n')
p=p.replace("if a==oc.STUB_CANVAS_DATA and u.reg_read", "if a==oc.STUB_GET_CONTEXT and u.reg_read(R.UC_ARM64_REG_LR) in (0x307f120,0x307f154,0x307f658):\n  self.x(0,self.handoff_queue); u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))\n elif a==oc.STUB_CANVAS_DATA and u.reg_read")
anchor=" try: p.call('SPen::ObjectDrawing::drawObjectStroke(SPen::ISPCanvas&, SPen::ISPPenCanvas&, SPen::ObjectStroke const*, bool)',[p.drawing,p.draw_canvas,p.pen_canvas,obj,1],limit=5000000)"
replacement=ins+" try: p.x(0,1)"
assert p.count('    return code\n')==1
p=p.replace('    return code\n',f"    anchor = {anchor!r}\n    assert code.count(anchor) == 1\n    code = code.replace(anchor, {replacement!r})\n    return code\n")
p=p.replace("Path('/home/jding/RustroverProjects/sdocx/tools/pencil2-public-replay/drawing-native-target-translated.rgba.zlib')", "Path('/tmp/pencil219-merge.rgba.zlib')")
native=root/'tools/pencil2-page-v219/native'
for ancestor in (native, *native.parents):
 if ancestor.is_symlink(): raise RuntimeError(f'symlink in temporary directory ancestry: {ancestor}')
with tempfile.TemporaryDirectory(prefix='pencil219-merge-', dir=native) as td:
 out=Path(td); script=out/'probe.py'; raw=out/'frame.zlib'
 p=p.replace("m.OUTPUTS = Path(tempfile.mkdtemp(prefix='pencil2-queue-intervention-', dir=root / 'tmp'))", f"m.OUTPUTS = Path({str(out)!r})")
 p=p.replace("Path('/tmp/pencil219-merge.rgba.zlib')", f"Path({str(raw)!r})")
 script.write_text(p)
 env=os.environ.copy()
 env['PYTHONPATH']=str(native/'scratch/apk-analysis-runtime/python')
 env['PATH']=str(native/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin')+':/usr/bin:/bin'
 env['LD_LIBRARY_PATH']=str(native/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
 run=subprocess.run(['/usr/bin/python3',str(script)],cwd=root,env=env,text=True,capture_output=True)
 if run.returncode: raise RuntimeError(run.stdout[-4000:]+run.stderr[-4000:])
 written=json.loads(run.stdout.strip().splitlines()[-1])['written']; capture=out/'handoff/capture.json'
 if written != str(capture.relative_to(native)) or capture.is_symlink() or capture.parent.is_symlink(): raise RuntimeError('unexpected capture path')
 if not capture.is_file(): raise RuntimeError('missing capture')
 if True:
  d=json.loads(capture.read_text()); t=d['native_target_probe']; pixels=__import__('zlib').decompress(raw.read_bytes())
  assert t['draw']=='returned' and t['gl_error']==0 and t['nonzero_bytes']>0
  assert hashlib.sha256(pixels).hexdigest()==t['readback_sha256'] and t['selected_rt']==d['route']['selected_rt']
  receipt={'probe_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'runtime_limitations':'Host EGL, GLES, Mesa drivers and transitive shared libraries are unpinned; the result describes this host runtime only.', 'scope':'Synthetic one-object list dispatch followed by explicit saved redraw replay and explicit RTV4 draw to host Mesa. No automatic list paint, real page matrix, saved-note or device claim.', 'list':'Native list dispatch and both supplied Matrix3 arguments asserted; 84 AddPoint calls', 'selected_rt':t['selected_rt'], 'rt_identity':'same selected RTV4 address in route and explicit draw', 'matrix3':[1,0,0,0,-1,0,6,183,1], 'explicit_draw_matrix4':t['matrix'], 'readback_sha256':t['readback_sha256'], 'nonzero_bytes':t['nonzero_bytes'], 'prior_sha256':t['prior_sha256'], 'gl_error':t['gl_error'], 'row_origin':t['row_origin'], 'input_sha256':{str(x.relative_to(root)):hashlib.sha256(x.read_bytes()).hexdigest() for x in [root/'tools/pencil2-page-v219/queue-probe.py',root/'tools/pencil2-public-replay/drawing_native_target_translated_probe.py',root/'tools/pencil2-page-v219/native/tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/inkpen_paint_native.py',root/'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so']}}
  (root/'tools/pencil2-page-v219/mesa-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n'); print(json.dumps(receipt,indent=2))

