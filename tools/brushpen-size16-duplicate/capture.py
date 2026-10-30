from pathlib import Path
import argparse, base64, contextlib, io, json, hashlib, os, tempfile
parser=argparse.ArgumentParser()
parser.add_argument("--setting", choices=("10", "11"), default="10")
args=parser.parse_args()
root=Path(os.environ.get("BRUSHPEN_CAPTURE_ROOT", Path.cwd()))
receipt=json.loads((root/'tools/brushpen-size16-duplicate/receipt.json').read_text())
for part in ('frame', 'payload', 'table'):
    data=(root/f'conformance/brushpen-serialized/v10-{part}.bin').read_bytes()
    assert hashlib.sha256(data).hexdigest()==receipt[f'source_{part}_sha256']
PINS = {'tools/brushpen-transform/second_mapdraw.py': '9e9632df53c725b6018ba6892edc3605649762f7eba6e7b7098f25af6a8d185f', 'tools/brushpen-transform/long_probe.py': '77e48b4625361cd14eb5aa4637e31af5e7e3da52794547e93681f5b0e6bbeb40', 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_queue_native.py': '8d9672741385b77c02445e039e5982e0deb744a01ae51c77b1dc2c554baf5e26', 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_paint_positive_native.py': '3223d0d0d85a2d228c6e348253c6c90c7bcc1944304a8de15b0a760bf0234242', 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_saved_native.py': 'af92a74d666f365ffe92fb50b0ab09f77a233bf9ac10e884e05d2a632dfaa9cd', 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_texture_native.py': 'ba6a8280580f1c737a90849389b5772fc1dd1d82e4837f27fae68f27218ac267', 'tmp/conformance-cleanup/native-capture-a241793/conformance/fountain_native.py': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'}
for name, expected in PINS.items():
    path = root / name
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'Pinned helper changed: {path}')
for name, expected in json.loads((root/'conformance/brushpen-serialized/receipt.json').read_text())['native_dso_sha256'].items():
    if hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a'/name).read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'Pinned native DSO changed: {name}')
os.environ['BRUSHPEN_RUN_CREATE_CANVAS_TASK']='1'
os.environ['BRUSHPEN_NATIVE_PEN_DATA_MANAGER']='1'
runner=root/'tools/brushpen-transform/second_mapdraw.py'
s=runner.read_text()
start=s.index(' obj,impl=self.alloc(0x100),self.alloc(0x400)',s.index('def drawing_source'))
end=s.index(" pen=self.paint_state['pen'];",start)
replacement=f''' table=(ROOT/'conformance/brushpen-serialized/v10-table.bin').read_bytes()
 if {args.setting!r} == '11':
  assert table.count('10;'.encode('utf-16le')) == 1
  table=table.replace('10;'.encode('utf-16le'), '11;'.encode('utf-16le'), 1)
  assert __import__('hashlib').sha256(table).hexdigest() == 'fc83e379d20450fb10d9ffe5e45bbd55c0a98cdd301b6fff21348bef8ae29cbc'
 frame=(ROOT/'conformance/brushpen-serialized/v10-frame.bin').read_bytes()
 sm=self.alloc(8); self.u.mem_write(sm,bytes(8)); self.call('SPen::StringIDManager::StringIDManager()',[sm]); self.call('SPen::StringIDManager::Construct()',[sm])
 src=self.alloc(len(table)); self.u.mem_write(src,table); self.call('SPen::StringIDManager::ApplyBinary(unsigned char*, int)',[sm,src,len(table)])
 assert self.x(0)==1
 ctx=self.alloc(0x800); self.u.mem_write(ctx,bytes(0x800)); self.call('SPen::ModelContext::ModelContext()',[ctx]); self.call('SPen::ModelContext::SetStringIDManager(SPen::StringIDManager*)',[ctx,sm])
 for name in ('uuid_create','uuid_make','uuid_export','uuid_destroy'): self.callbacks[name]=lambda:self.x(0,0)
 obj=self.alloc(0x100); self.call('SPen::ObjectStroke::ObjectStroke()',[obj]); self.call('SPen::ObjectStroke::Construct()',[obj]); impl=self.get(obj+32,'Q')[0]
 f=self.alloc(len(frame)); self.u.mem_write(f,frame); h=self.alloc(0x100); self.u.mem_write(h,bytes(0x100)); self.call('SPen::ObjectStrokeBinaryHandler::ObjectStrokeBinaryHandler(SPen::ObjectStrokeImpl*)',[h,impl]); self.f(0,1.0); self.call('SPen::ObjectStrokeBinaryHandler::NewApplyBinary(unsigned char*, int, unsigned int, float)',[h,f,len(frame),0]); assert self.x(0)&0xffffffff==len(frame)
 self.call('SPen::ObjectStroke::OnAttach(SPen::ModelContext*, int)',[obj,ctx,0])
 self.f(0,16.0); self.call('SPen::ObjectStroke::SetPenSize(float)',[obj])
 self.call('SPen::ObjectStroke::GetPoint() const',[obj]); point_ptr=self.x(0)
 self.u.mem_write(point_ptr+16,bytes(self.u.mem_read(point_ptr,8)))
 for key in list(self.callbacks):
  if key.startswith('SPen::MotionEvent::'): self.callbacks.pop(key)
 for getter in ('GetPointCount','GetPoint','GetPressure','GetTimeStamp','GetTilt','GetOrientation','GetPenSize','GetToolType','GetStrokeType'): self.callbacks.pop('SPen::ObjectStroke::'+getter+'() const',None)
'''
s=s[:start]+replacement+s[end:]
s=s.replace("task2_status=[row for row in framebuffer_status_calls", "print('SERIALIZED_RESULT_KEYS',json.dumps({k:v for k,v in result.items() if k in ('native_pattern_material','buffer_handoff','provenance','native_callback','host_mesa_paint')},sort_keys=True))\n task2_status=[row for row in framebuffer_status_calls")
s=s.replace('len(raw) != 221 * 44','len(raw) == 0')
s=s.replace("old=\"\"\"    if callback_rt", "source=source.replace(\"    if len(self.end_pen_records) != 1:\\n        raise RuntimeError(f'expected one completed native endPen, got {len(self.end_pen_records)}')\\n\", '')\nsource=source.replace(\"self.end_pen_records[0]['emitted_addpoint_count']\", \"None\")\nold=\"\"\"    if callback_rt")
a=s.index('start_stream=source.index('); b=s.index('\n# Keep HandoffProbe',a)
s=s[:a]+'''start_stream=source.index("    saved = json.loads(GEOMETRY.read_text())")
end_stream=source.index("    task2 = next(row", start_stream)
source=source[:start_stream]+"    import zlib\\n    expected_case = {'synthetic_input': 'serialized GLV10 fixture'}\\n    expected_bytes = zlib.decompress((ROOT / 'conformance/brushpen-serialized/v10-native-stream.zlib').read_bytes())\\n    open('/tmp/brush-variant-stream.bin','wb').write(raw)\\n\\n"+source[end_stream:]
'''+s[b:]
with tempfile.TemporaryDirectory(prefix='brushpen-serialized-paint-') as temp:
    for name in ('brushpen-rebound-source.py','brushpen-source-dump.py','brushpen-source-after-injection.py'):
        s=s.replace('/tmp/'+name, str(Path(temp)/name))
    log=io.StringIO()
    with contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
        exec(compile(s,str(runner),'exec'),{'__name__':'__main__','__file__':str(runner),'setting':args.setting})
    lines=log.getvalue().splitlines()
    def record(prefix):
        return json.loads(next(line[len(prefix)+1:] for line in lines if line.startswith(prefix+' ')))
    task2=record('TASK2_STATE')
    target=task2['native_canvas_rgba_target']
    rgba=base64.b64decode(target['after_task2_base64'])
    Path('/tmp/brush-variant-frame.bin').write_bytes(rgba)
    print('frame', len(rgba), hashlib.sha256(rgba).hexdigest())
