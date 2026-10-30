#!/usr/bin/env python3
"""Trace synthetic queue adapter entry points during the native list call."""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization; capture requires assertions')
root = Path(__file__).resolve().parents[2]
source = root / 'tools/pencil2-page-v219/queue-probe.py'
assert hashlib.sha256(source.read_bytes()).hexdigest() == '1e9e1f7445548f7d23f3b194f0cf6502094519a4a0c05ef2a38950f66f47670c'
code = source.read_text()
old = ' events=[]; matrices=[]'
new = ' events=[]; matrices=[]; dispatch=[]'
assert code.count(old) == 1
code = code.replace(old, new)
old = '  if a==p.syms[oc.GRAPHICS_SET_MATRIX]:'
new = '''  if a==0x20001000:
   task=p.x(1); vt=struct.unpack('<Q',u.mem_read(task,8))[0]; method=struct.unpack('<Q',u.mem_read(vt+16,8))[0]; dispatch.append({'task':hex(task),'vtable':hex(vt),'run':hex(method),'member_pointer_words':[hex(x) for x in struct.unpack('<2Q',u.mem_read(task+24,16))]})
  if a==p.syms[oc.GRAPHICS_SET_MATRIX]:'''
assert code.count(old) == 1
code = code.replace(old, new)
old = " print('LIST_EVENTS',"
new = " assert [d['member_pointer_words'][0] for d in dispatch] == ['0x104a4a4','0x104a558','0x5098a88','0x104a4ac','0x1050fd0','0x28'], dispatch\n print('DISPATCH_EVENTS',json.dumps(dispatch),_always=True)\n print('LIST_EVENTS',"
assert code.count(old) == 1
code = code.replace(old, new)
with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=source.parent) as tmp:
    tmp.write(code)
    tmp.flush()
    run = subprocess.run([sys.executable, tmp.name], cwd=root, text=True, capture_output=True)
    if run.returncode: raise RuntimeError(run.stdout + run.stderr)
lines = run.stdout.splitlines()
def field(prefix):
    found = [line[len(prefix):] for line in lines if line.startswith(prefix)]
    assert len(found) == 1
    return json.loads(found[0])
symbols = [
    ('libSPenPenCommon.so', 0x1000000, 0x4a4a4, 'SPen::PenDrawableRT::SetRedrawState(bool)'),
    ('libSPenPenCommon.so', 0x1000000, 0x4a558, 'SPen::PenDrawableRT::SetPenData(float, int)'),
    ('libSPenPencil2.so', 0x5000000, 0x98a88, 'SPen::Pencil2StrokeDrawableRTV4::SetStartPoint(float, float)'),
    ('libSPenPenCommon.so', 0x1000000, 0x4a4ac, 'SPen::PenDrawableRT::SetRect(SPen::RectF)'),
    ('libSPenPenCommon.so', 0x1000000, 0x50fd0, 'SPen::PenStrokeDataManager::ReleaseStrokeData(int)'),
]
dispatch = field('DISPATCH_EVENTS ')
for row, (library, base, offset, signature) in zip(dispatch, symbols, strict=False):
    path = root / 'scratch/apk-analysis-native/arm64-v8a' / library
    output = subprocess.run(['nm', '-D', '-C', str(path)], text=True, capture_output=True, check=True).stdout
    assert any(line == f'{offset:016x} T {signature}' for line in output.splitlines()), signature
    assert int(row['member_pointer_words'][0], 16) == base + offset
    assert row['member_pointer_words'][1] == '0x0'
    row['resolution'] = {'library': library, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                         'load_base': hex(base), 'symbol_offset': hex(offset), 'signature': signature}
assert dispatch[5]['member_pointer_words'] == ['0x28', '0x1']
drawing = root / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so'
output = subprocess.run(['nm', '-D', '-C', str(drawing)], text=True, capture_output=True, check=True).stdout
signature = 'vtable for SPen::WorkerTask::Member0<SPen::IContextHolder, void (SPen::IContextHolder::*)()>'
assert any(line == f'{0xc3060:016x} V {signature}' for line in output.splitlines())
assert dispatch[5]['vtable'] == hex(0x3000000 + 0xc3060 + 16)
dispatch[5]['resolution'] = {
    'library': drawing.name, 'sha256': hashlib.sha256(drawing.read_bytes()).hexdigest(),
    'load_base': '0x3000000', 'vtable_symbol_offset': '0xc3060', 'signature': signature,
    'arm64_member_pointer': 'The low bit of the adjustment word marks a virtual member; adjustment is zero and the first word is byte offset 0x28 in the target vtable.',
    'synthetic_target': 'The queue probe installs stub 0x200f2500 at holder vtable offset 0x28; no real holder method identity is established.'
}
receipt = {
    'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'queue_probe_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'scope': 'Synthetic one-object DrawObjectList with three context-query interventions. Traces adapter queue task entry 0x20001000 only during this call; the adapter executes each task synchronously. Does not establish native queue scheduling, real-page lifecycle or matrix conversion.',
    'adapter_dispatch_entries': dispatch,
    'list_observations': field('LIST_OBSERVATIONS '),
}
(root / 'tools/pencil2-page-v219/dispatch-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
