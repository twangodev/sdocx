#!/usr/bin/env python3
"""Trace native canvas conversion entry points during the synthetic list call."""
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
new = ''' events=[]; matrices=[]; lifecycle=[]
 lifecycle_names=['SPen::SPPenCanvas::GetCanvasMatrix()', 'SPen::SPPenCanvasRT::GetCanvasMatrix()']
 assert all(n in p.syms for n in lifecycle_names)
 print('LIFECYCLE_SYMBOLS',json.dumps({n:hex(p.syms[n]) for n in lifecycle_names}),_always=True)'''
assert code.count(old) == 1
code = code.replace(old, new)
old = '  if a==p.syms[oc.GRAPHICS_SET_MATRIX]:'
new = '''  if a in [p.syms[n] for n in lifecycle_names]: lifecycle.append(hex(a))
  if a==p.syms[oc.GRAPHICS_SET_MATRIX]:'''
assert code.count(old) == 1
code = code.replace(old, new)
old = " print('LIST_EVENTS',"
new = " assert lifecycle == [], lifecycle\n print('LIFECYCLE_EVENTS',json.dumps(lifecycle),_always=True)\n print('LIST_EVENTS',"
assert code.count(old) == 1
code = code.replace(old, new)
with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=source.parent) as tmp:
    tmp.write(code)
    tmp.flush()
    run = subprocess.run([sys.executable, tmp.name], cwd=root, text=True, capture_output=True, check=True)
lines = run.stdout.splitlines()
def field(prefix):
    found = [line[len(prefix):] for line in lines if line.startswith(prefix)]
    assert len(found) == 1
    return json.loads(found[0])
receipt = {
    'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'queue_probe_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
    'scope': 'Synthetic one-object DrawObjectList with host adapters and three context-query interventions. Entry-point trace is limited to the two Graphics GetCanvasMatrix methods during this synchronous call; it does not establish real-page scheduling or matrix conversion.',
    'symbols': field('LIFECYCLE_SYMBOLS '),
    'entries': field('LIFECYCLE_EVENTS '),
    'list_observations': field('LIST_OBSERVATIONS '),
}
(root / 'tools/pencil2-page-v219/canvas-entry-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
