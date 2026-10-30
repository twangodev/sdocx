import hashlib, json, pathlib, tempfile, subprocess, sys
if sys.flags.optimize:
 raise SystemExit("Run without Python optimization; capture requires assertions")
if sys.argv[1:] not in ([], ["--write-receipt"]):
 raise SystemExit("Use no arguments to verify or --write-receipt to update")
root=pathlib.Path(__file__).resolve().parents[2]; q=root/'tools/pencil2-page-v219/queue-probe.py'; s=q.read_text()
assert hashlib.sha256(q.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
a=' events=[]; matrices=[]'
i='''
 refs=[]
 p.callbacks['SPen::SPGraphicsFactory::RefBitmap(SPen::ISPBitmap*)']=lambda:(refs.append(p.x(0)),p.x(0,0))
 optrect=p.alloc(32);optmatrix=p.alloc(48);p.u.mem_write(optmatrix,bytes(p.u.mem_read(matrix,36)));p.put(optmatrix+36,'B',1)
 data=p.alloc(160)
 name='SPen::AsyncDrawingData::AsyncDrawingData(SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ObjectList const*, std::__ndk1::optional<SPen::Rect>, std::__ndk1::optional<SPen::Matrix3<float> >, bool)'
 try:
  p.call(name,[data,p.bitmap,0,0,0,ol,optrect,optmatrix,0],limit=50000000)
  clone=p.get(data+32,'Q')[0];p.call('SPen::ObjectList::GetCount() const',[clone]);count=p.x(0)
  assert refs==[p.bitmap,0,0,0]
  print('ASYNC_COPY',count,p.get(data+96,'B')[0],bytes(p.u.mem_read(data+60,36)).hex(),refs,_always=True)
 except Exception as e: print('ASYNC_ERROR',repr(e),hex(p.u.reg_read(R.UC_ARM64_REG_PC)),_always=True)
'''
assert s.count(a)==1;s=s.replace(a,i+a)
with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=q.parent) as f:
 f.write(s);f.flush();r=subprocess.run([sys.executable,f.name],cwd=root,text=True,capture_output=True)
assert r.returncode == 0, (r.stdout, r.stderr)
lines = [x for x in r.stdout.splitlines() if x.startswith('ASYNC_COPY ')]
assert len(lines) == 1 and lines[0].split()[1:4] == ['1', '1', '0000803f000000000000000000000000000080bf000000000000c040000037430000803f'], (lines, r.stdout, r.stderr)
assert 'ASYNC_ERROR' not in r.stdout
receipt = {'scope': 'Native AsyncDrawingData constructor and list count using synthetic input and a host bitmap-reference callback; no bitmap ownership, destructor, object independence, queued execution, or cloned-stroke paint established.', 'script': 'tools/pencil2-async-route/clone-probe.py', 'script_sha256': hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(), 'queue_probe_sha256': hashlib.sha256(q.read_bytes()).hexdigest(), 'drawing_dso_sha256': hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so').read_bytes()).hexdigest(), 'matrix_hex': lines[0].split()[3], 'matrix_present': 1, 'list_count': 1, 'reference_calls': ['nonnull', 'null', 'null', 'null'], 'reproduction': 'python3 tools/pencil2-async-route/clone-probe.py'}
path = pathlib.Path(__file__).with_name('clone-receipt.json')
if sys.argv[1:]:
 path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
 assert json.loads(path.read_text()) == receipt, 'Clone receipt differs from capture'
print('Wrote clone receipt.' if sys.argv[1:] else 'Verified clone receipt.')
