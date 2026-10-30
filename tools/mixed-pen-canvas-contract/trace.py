"""Trace the native Fountain pen canvas and bitmap contract in one saved draw."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'tools/fountain_v14/drawing_capture.py'
SOURCE_SHA = '969fef6e967a41c888aa1121783da525bda435082533733b8807c425eda5358a'
PINS = {'FountainPen': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec', 'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d', 'Graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621'}
for name, expected in PINS.items():
    path = ROOT / 'scratch/apk-analysis-native/arm64-v8a' / f'libSPen{name}.so'
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit(f'library changed: {name}')
if hashlib.sha256(SOURCE.read_bytes()).hexdigest() != SOURCE_SHA:
    raise SystemExit('Fountain capture source changed')
if '--child' not in sys.argv:
    env = dict(os.environ)
    env['PYTHONPATH'] = str(ROOT / 'scratch/apk-analysis-runtime/python')
    result = subprocess.run([sys.executable, __file__, '--child'], env=env, capture_output=True, text=True)
    if result.returncode:
        raise SystemExit(result.stderr + result.stdout)
    lines = [line for line in result.stdout.splitlines() if line.startswith('CONTRACT ')]
    if len(lines) != 1:
        raise SystemExit('expected one contract observation: ' + result.stdout)
    observed = json.loads(lines[0][9:])
    receipt = {'scope': 'First saved Fountain Drawing in the existing native Unicorn and host Mesa capture; host context, loader, allocator and synchronous queue adapters apply. Pinned tmp/ and scratch/ capture helpers, libraries and runtime are external local dependencies. No Pencil2, mixed-scene or device parity claim.', 'source_sha256': SOURCE_SHA, 'library_sha256': PINS, 'address_provenance': 'Graphics base 0x4000000: GetBitmap RVA 0xaef08, SPTextureBitmap::GetTextureBitmap RVA 0x92020; PenCommon base 0x1000000: indirect-call site RVA 0x467d8. Exported names via nm -D -C on pinned DSOs.', 'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'observation': observed}
    print(json.dumps(receipt, indent=2))
    raise SystemExit(0)

spec = importlib.util.spec_from_file_location('fountain_capture', SOURCE)
class TraceComplete(Exception):
    pass

module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
anchor = "    state={'clear_calls':0,'init_calls':0,'canvas_calls':0,'rt':0,'factory_return':None,'pen':0}"
instrument = '''    canvas_vt=p.get(pen_canvas,'Q')[0]
    getter=p.get(canvas_vt+192,'Q')[0]
    if getter!=0x40aef08:raise RuntimeError('unexpected native pen canvas getter')
    observed={'pen_canvas':hex(pen_canvas),'pen_canvas_vtable':hex(canvas_vt),'slot_192':hex(getter),'getter_returns':[],'branch':None,'target_entered':False,'branch_returned':False}
    pending=set()
    def trace_contract(uc,address,size,data):
        if address==getter:
            pending.add(uc.reg_read(R.UC_ARM64_REG_LR))
        elif address in pending:
            pending.remove(address)
            bitmap=p.x(0); vt=p.get(bitmap,'Q')[0]
            observed['getter_returns'].append({'caller':hex(address),'bitmap':hex(bitmap),'vtable':hex(vt),'slot_168':hex(p.get(vt+168,'Q')[0])})
        if address==0x10467d8:
            observed['branch']={'receiver':hex(p.x(0)),'target':hex(p.x(8))}
        elif address==0x4092020:
            observed['target_entered']=True
        elif address==0x10467dc:
            observed['branch_returned']=True
    p.u.hook_add(UC_HOOK_CODE,trace_contract)
'''
assert module.INJECT.count(anchor) == 1
module.INJECT = module.INJECT.replace(anchor, instrument + anchor)
anchor = "          print(name,'AA',RESULTS[name]['aa'],'redraw',RESULTS[name]['redraw'],RESULTS[name]['gl_draw_names'],flush=True)"
end = '''          if not any(row['caller']=='0x10467d0' and row['slot_168']=='0x4092020' and row['bitmap']==observed['branch']['receiver'] for row in observed['getter_returns']) or observed['branch']['target']!='0x4092020' or not observed['target_entered'] or not observed['branch_returned']:raise RuntimeError('native call contract not observed')
          if name!='drawing_center_top_1_ff000000_clear' or hashlib.sha256(rgba).hexdigest()!='aba612cffd994c04a11c065763b3c7799993d85a1151f375682b5d238c1b6dfe':raise RuntimeError('first drawing control differs')
          if len(observed['getter_returns']) != 4 or len({row['bitmap'] for row in observed['getter_returns']}) != 1:raise RuntimeError('unexpected getter counts or identity')
          observed['control']={'case':name,'rgba_sha256':hashlib.sha256(rgba).hexdigest(),'draw_count':len(draws)}
          print('CONTRACT '+json.dumps(observed),flush=True)
          raise __import__('builtins')._sdocx_trace_complete()
'''
assert module.INJECT.count(anchor) == 1
module.INJECT = module.INJECT.replace(anchor, end + anchor)
import builtins
builtins._sdocx_trace_complete = TraceComplete
try:
    module.main()
except TraceComplete:
    pass
else:
    raise SystemExit('capture completed without trace checkpoint')
