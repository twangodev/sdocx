#!/usr/bin/env python3
"""Trace repeated synthetic list entries under two caller-supplied matrices."""
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
base = source.read_text()
old = "p.call('SPen::ObjectList::GetCount() const',[ol]);"
assert base.count(old) == 1
base = base.replace(old, "obj2=p.alloc(0x100);p.call('SPen::ObjectStroke::ObjectStroke()',[obj2]);impl2=p.alloc(0x400);p.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)',[impl2,obj2]);p.put(obj2+32,'Q',impl2);p.call('SPen::ObjectBase::Construct(SPen::Object::Type, bool)',[obj2,1,0]);p.call('SPen::ObjectStroke::Copy(SPen::ObjectBase const*)',[obj2,obj]);p.call('SPen::ObjectBase::SetVisibility(bool)',[obj2,1]);p.f(0,1.0);p.call('SPen::ObjectBase::SetAlpha(float)',[obj2]);p.call('SPen::ObjectList::Add(SPen::ObjectBase const*)',[ol,obj2]);print('OBJECT_IDENTITIES',json.dumps({'objects':[hex(obj),hex(obj2)],'impls':[hex(p.get(obj+32,'Q')[0]),hex(p.get(obj2+32,'Q')[0])]}),_always=True);" + old)
base = base.replace('assert [e[0] for e in events]==names, events', 'assert [e[0] for e in events]==names*2, events')
base = base.replace("'LIST_COUNT 1'", "'LIST_COUNT 2'").replace("'\\\"addpoint_calls\\\": 84'", "'\\\"addpoint_calls\\\": 168'")
# The literal in the source has no backslashes.
base = base.replace("'\"addpoint_calls\": 84'", "'\"addpoint_calls\": 168'")
cases = [[1, 0, 0, 0, -1, 0, 6, 183, 1], [1.5, 0.25, 0, -0.5, -2, 0, 13, 117, 1]]
results = []
for matrix in cases:
    code = base.replace('1,0,0,0,-1,0,6,183,1', ','.join(map(str, matrix)))
    with tempfile.NamedTemporaryFile(mode='w', suffix='.py', dir=source.parent) as tmp:
        tmp.write(code)
        tmp.flush()
        run = subprocess.run([sys.executable, tmp.name], cwd=root, text=True, capture_output=True)
        if run.returncode:
            raise RuntimeError(run.stdout + run.stderr)
    def field(prefix):
        rows = [line[len(prefix):] for line in run.stdout.splitlines() if line.startswith(prefix)]
        assert len(rows) == 1
        return json.loads(rows[0])
    events = field('LIST_EVENTS ')
    observations = field('LIST_OBSERVATIONS ')
    assert observations['observed_matrices'] == [matrix, matrix]
    assert observations['addpoint_calls'] == 168
    identities = field('OBJECT_IDENTITIES ')
    assert len(set(identities['objects'])) == 2
    assert len(set(identities['impls'])) == 2 and '0x0' not in identities['impls']
    for index, obj in enumerate(identities['objects']):
        offset = index * 4
        assert events[offset + 1][4] == events[offset + 2][4] == events[offset + 3][2] == obj
    results.append({'matrix': matrix, 'observations': observations,
                    'event_names': [row[0] for row in events],
                    'identities': identities, 'dispatch_events': events})
receipt = {'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
           'queue_probe_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
           'scope': 'Two distinct synthetic ObjectStroke instances with native-copied content; synchronous queue and three context-query interventions. Caller-supplied matrices and host adapters; no GPU draw, saved page or device evidence.',
           'cases': results}
(root / 'tools/pencil2-page-v219/distinct-matrix-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt, indent=2))
