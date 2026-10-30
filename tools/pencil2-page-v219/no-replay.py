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
failure=" except Exception as e: print('LIST_FAILURE',repr(e),hex(p.u.reg_read(R.UC_ARM64_REG_PC)),[hex(p.x(i)) for i in range(8)],'x23',hex(p.u.reg_read(R.UC_ARM64_REG_X23)),_always=False)"
assert ins.count(failure)==1
ins=ins.replace(failure, " except Exception as e: raise RuntimeError('native list dispatch failed') from e")
needle="  print('LIST_RETURN',p.x(0),_always=False)"
assert ins.count(needle)==1
ins=ins.replace(needle, "  assert p.x(0)==1, f'native list returned {p.x(0)}'\n  assert len(pencil_buffer_events)==84, f'expected 84 AddPoint events, got {len(pencil_buffer_events)}'")
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
p=p.replace("    return code\n", '    start = code.index(\' import os,re,subprocess\\n from pathlib import Path\\n renderer=\')\n    end = code.index(" encoded=json.dumps(result,indent=2,sort_keys=True)+\'\\\\n\'", start)\n    block = code[start:end]\n    marker = " snapshot_object_stroke(\'before_saved_RedrawPen\',obj);"\n    assert code.count(marker)==1\n    code = code[:start] + code[end:]\n    block = \' result={}\\n\' + block + " Path(\'/tmp/pencil-no-replay-result.json\').write_text(json.dumps(result)); raise SystemExit(0)\\n"\n    code = code.replace(marker, block+marker)\n    return code\n')
p=p.replace("Path('/home/jding/RustroverProjects/sdocx/tools/pencil2-public-replay/drawing-native-target-translated.rgba.zlib')", "Path('/tmp/pencil219-merge.rgba.zlib')")
native=root/'tools/pencil2-page-v219/native'
for ancestor in (native, *native.parents):
 if ancestor.is_symlink(): raise RuntimeError(f'symlink in temporary directory ancestry: {ancestor}')
with tempfile.TemporaryDirectory(prefix='pencil219-merge-', dir=native) as td:
 out=Path(td); script=out/'probe.py'; raw=out/'frame.zlib'
 p=p.replace("m.OUTPUTS = Path(tempfile.mkdtemp(prefix='pencil2-queue-intervention-', dir=root / 'tmp'))", f"m.OUTPUTS = Path({str(out)!r})")
 p=p.replace("Path('/tmp/pencil219-merge.rgba.zlib')", f"Path({str(raw)!r})")
 p=p.replace('/tmp/pencil-no-replay-result.json', str(out/'result.json'))
 script.write_text(p)
 env=os.environ.copy()
 env['PYTHONPATH']=str(native/'scratch/apk-analysis-runtime/python')
 env['PATH']=str(native/'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin')+':/usr/bin:/bin'
 env['LD_LIBRARY_PATH']=str(native/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
 run=subprocess.run(['/usr/bin/python3',str(script)],cwd=root,env=env,text=True,capture_output=True)
 if run.returncode: raise RuntimeError(run.stdout[-4000:]+run.stderr[-4000:])
 result=json.loads((out/'result.json').read_text())
 target=result['native_target_probe']; update=result['update_after_handoff']; init=result['init_diagnostic']
 pixels=__import__('zlib').decompress(raw.read_bytes())
 assert init['outcome']=='returned' and update['outcome']=='returned' and update['after_count']==84
 assert target['draw']=='returned' and target['gl_error']==0 and target['nonzero_bytes']==317
 assert hashlib.sha256(pixels).hexdigest()==target['readback_sha256']
 assert target['selected_rt']==update['selected_rt'] and update['entry_matches_selected_rt']
 receipt={'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'scope':'Synthetic one-object native list dispatch, followed by explicit native RTV4 Init, Update and Draw before any explicit saved-redraw replay. Host bitmap, canvas, context, queue, renderer holder, allocator and loader adapters remain; this is not automatic page paint, saved-note or device evidence.', 'list':'Native ordered list dispatch, supplied Matrix3 and 84 AddPoint calls asserted by adapter', 'selected_rt':target['selected_rt'], 'update_count':update['after_count'], 'matrix3':[1,0,0,0,-1,0,6,183,1], 'explicit_draw_matrix4':target['matrix'], 'readback_sha256':target['readback_sha256'], 'nonzero_bytes':target['nonzero_bytes'], 'prior_sha256':target['prior_sha256'], 'gl_error':target['gl_error'], 'row_origin':target['row_origin'], 'runtime_limitations':'Host EGL, GLES, Mesa drivers and transitive shared libraries are unpinned.'}
 (root/'tools/pencil2-page-v219/no-replay-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt,indent=2))
