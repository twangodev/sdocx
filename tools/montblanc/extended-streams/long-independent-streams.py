import os, sys, json, subprocess, hashlib
from pathlib import Path
root=Path(__file__).resolve().parents[3]
# The pinned harness verifies all DSO and helper files before executing native code.
expected_capture = 'b277e27bf15167db554f5ecd3f5ed4833642a4afb9b206fa30438486190277ad'
expected_harness = 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8'
expected_binaries = {'Drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd', 'Model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a', 'MontblancFountainPen': '4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350'}
reference = json.loads((root / 'conformance/montblanc-many-point-controls.json').read_text())
if reference['capture_sha256'] != expected_capture or reference['source_harness_sha256'] != expected_harness or reference['binary_sha256'] != expected_binaries:
    raise SystemExit('reference identity mismatch')
if hashlib.sha256((root / 'tools/montblanc/extended-streams/capture.py').read_bytes()).hexdigest() != expected_capture:
    raise SystemExit('capture script hash mismatch')
if hashlib.sha256((root / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py').read_bytes()).hexdigest() != expected_harness:
    raise SystemExit('native harness hash mismatch')
cases={
 'fourteen_turns': {'points': [[0,0],[7,3],[14,-2],[21,4],[28,-1],[35,3],[42,-4],[49,2],[56,-3],[63,4],[70,-2],[77,3],[84,-4],[91,2]], 'pressure': [0.1,0.8,0.3,0.9,0.2,0.7,0.4,1,0,0.6,0.9,0.2,0.7,0.4], 'pen_size': 2},
 'ten_turns': {'points': [[0,0],[7,3],[14,-2],[21,4],[28,-1],[35,3],[42,-4],[49,2],[56,-3],[63,4]], 'pressure': [0.1,0.8,0.3,0.9,0.2,0.7,0.4,1,0,0.6], 'pen_size': 2},
 'twelve_skip': {'points': [[0,0],[8,0],[9,1],[16,4],[23,-1],[24,0],[30,5],[37,0],[44,6],[45,7],[51,1],[58,4]], 'pressure': [0.4,0.7,0.1,0.9,0.3,0.8,0.2,0.6,1,0,0.5,0.4], 'pen_size': 0.5},
 'eleven_reverse': {'points': [[0,0],[8,0],[16,3],[11,10],[4,13],[-3,8],[-8,1],[-3,-6],[4,-9],[11,-5],[15,2]], 'pressure': [1,0,0.8,0.2,0.6,0.4,0.9,0.1,0.7,0.3,0.5], 'pen_size': 1.5},
}
result={}
for name, case in cases.items():
 env={**os.environ,'PYTHONPATH':str(root/'scratch/apk-analysis-runtime/python'),'MB_CASE':json.dumps(case)}
 p=subprocess.run([str(root/'.venv/bin/python'),str(root/'tools/montblanc/extended-streams/capture.py')],cwd=root,env=env,check=True,capture_output=True,text=True)
 native=json.loads(p.stdout.splitlines()[-1]); native['stream_sha256']=hashlib.sha256(json.dumps(native['stream'],separators=(',',':')).encode()).hexdigest(); result[name]={'input':case,**native}
 print(name, native['count'],flush=True)
identity=json.loads((root/'conformance/montblanc-many-point-controls.json').read_text())
out={'schema':'montblanc-long-independent-streams-v1','boundary':identity['boundary'],'source_harness_sha256':identity['source_harness_sha256'],'binary_sha256':identity['binary_sha256'],'capture_sha256':hashlib.sha256((root/'tools/montblanc/extended-streams/capture.py').read_bytes()).hexdigest(),'cases':result}
path = root/'conformance/montblanc-long-independent-streams.json'
if sys.argv[1:] == ['--write']:
 path.write_text(json.dumps(out,indent=2)+'\n')
elif not sys.argv[1:]:
 if out != json.loads(path.read_text()): raise SystemExit('native independent streams mismatch')
 print('verified independent streams')
else: raise SystemExit('usage: many-point-independent-streams.py [--write]')
