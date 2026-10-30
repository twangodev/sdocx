#!/usr/bin/env python3
"""Exercise DrawingData construction through the existing pinned Unicorn adapter."""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization; capture requires assertions')
if sys.argv[1:] not in ([], ['--write-receipt']):
    raise SystemExit('Use no arguments to verify or --write-receipt to update')

root = Path(__file__).resolve().parents[2]
queue_probe = root / 'tools/pencil2-page-v219/queue-probe.py'
assert hashlib.sha256(queue_probe.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
source = queue_probe.read_text()
anchor = ' events=[]; matrices=[]'
assert source.count(anchor) == 1
insert = '''
 # The bitmap-reference callback is an explicit adapter intervention; no bitmap
 # ownership or application graphics behavior is claimed.
 refs=[]
 def ref_bitmap():
  refs.append(p.x(0)); p.x(0,0)
 p.callbacks['SPen::SPGraphicsFactory::RefBitmap(SPen::ISPBitmap*)']=ref_bitmap
 optrect=p.alloc(32)
 name='SPen::ObjectAsyncDrawing::DrawingData::DrawingData(SPen::ISPBitmap*, SPen::ObjectList const*, std::__ndk1::optional<SPen::Rect>, std::__ndk1::optional<SPen::Matrix3<float> >, bool, SPen::ISPBitmap*, SPen::ISPBitmap*, SPen::ISPBitmap*)'
 observations=[]
 for present, bitmap in ((0,0),(1,0),(0,p.bitmap),(1,p.bitmap)):
  optmatrix=p.alloc(48);p.u.mem_write(optmatrix,bytes(p.u.mem_read(matrix,36)));p.put(optmatrix+36,'B',present)
  data=p.alloc(160); before=len(refs)
  p.call(name,[data,bitmap,ol,optrect,optmatrix,0,0,0,0],limit=5000000)
  stored=bytes(p.u.mem_read(data+68,36)); flag=p.get(data+104,'B')[0]
  assert flag==present and (not present or stored==bytes(p.u.mem_read(matrix,36)))
  observed=refs[before:];assert observed==[bitmap,0,0,0], observed
  observations.append({'present':present,'bitmap_nonnull':bool(bitmap),'stored_hex':stored.hex(),'flag':flag,'reference_calls':['nonnull' if x else 'null' for x in observed]})
 print('ASYNC_CONSTRUCTOR',json.dumps(observations),_always=True)
'''
source = source.replace(anchor, insert + anchor)
with tempfile.NamedTemporaryFile(mode='w', suffix='.py', prefix='.async-probe-', dir=queue_probe.parent) as f:
    f.write(source)
    f.flush()
    result = subprocess.run([sys.executable, f.name], cwd=root, text=True, capture_output=True)
if result.returncode:
    sys.stderr.write(result.stdout + result.stderr)
    raise SystemExit(result.returncode)
lines = [line for line in result.stdout.splitlines() if line.startswith('ASYNC_CONSTRUCTOR ')]
assert len(lines) == 1
cases = json.loads(lines[0].split(' ', 1)[1])
receipt = {
    'scope': 'Native ARM64 DrawingData constructor in Unicorn, synthetic ObjectList and optional Matrix3, with host RefBitmap callback intervention. No async scheduling, ownership, page input, Matrix4 or paint evidence.',
    'script': 'tools/pencil2-async-route/constructor-probe.py',
    'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'queue_probe_sha256': hashlib.sha256(queue_probe.read_bytes()).hexdigest(),
    'drawing_dso_sha256': hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so').read_bytes()).hexdigest(),
    'matrix_f32': [1, 0, 0, 0, -1, 0, 6, 183, 1],
    'cases': cases,
    'provenance': 'Existing pinned adapter and dependencies described in tools/pencil2-page-v219/README.md; callback replaces SPGraphicsFactory::RefBitmap for all four calls. Stored absent payload is allocator state and has no asserted meaning.',
}
receipt_path = Path(__file__).with_name('constructor-receipt.json')
if sys.argv[1:] == ['--write-receipt']:
    receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
else:
    assert not sys.argv[1:], 'Use no arguments to verify or --write-receipt to update'
    assert json.loads(receipt_path.read_text()) == receipt, 'Constructor receipt differs from capture'
print('Verified constructor cases and receipt.' if not sys.argv[1:] else 'Wrote constructor receipt.')
