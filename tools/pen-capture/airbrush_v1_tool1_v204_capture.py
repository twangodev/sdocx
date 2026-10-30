#!/usr/bin/env python3
"""Capture V1 tool 1 Drawing/Mesa controls; output directory is the sole argument.

Requires the runner and dependency manifest pinned by the earlier tool-1 receipt.
"""
import os,sys,subprocess,tempfile,pathlib,json,hashlib
if not __debug__:
    raise SystemExit('Python optimization disables validation assertions; run without -O.')

root=pathlib.Path(__file__).resolve().parents[2]; out=pathlib.Path(sys.argv[1]).resolve(); runner=root/'tmp/vector-parity-plan/pen-family-audit/airbrush-v1-short-native/frame-capture.py'; source=runner.read_text(); assert hashlib.sha256(source.encode()).hexdigest()=='d38a83f3cf71b228f58654c08a135d3f91d3bda0349df1d6014926752dc3b73e'
manifest_path=root/'tools/pen-capture/airbrush_v1_curved_prerequisites.json'
assert hashlib.sha256(manifest_path.read_bytes()).hexdigest() == '4fce2b56bbc1ed96c20bc80242a39be7cc789185ef5df93a156bd79d6e97e8f1'
manifest=json.loads(manifest_path.read_text())
for path, expected in manifest.items():
 if hashlib.sha256((root/path).read_bytes()).hexdigest()!=expected: raise RuntimeError(f'prerequisite hash mismatch: {path}')
out.mkdir(parents=False, exist_ok=False)
env=os.environ.copy(); env['AB_N']='5'; env['PYTHONPATH']=':'.join(str(root/p) for p in ('scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin','tmp/conformance-cleanup/native-capture-a241793/conformance')); env['LD_LIBRARY_PATH']=str(root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
cases=[('curve','[[_probe_start[0]+i*3.0, _probe_start[1]+i*i*.5] for i in range(n)]','3.144','0xff44aaee',''),('turn','[[_probe_start[0]+(i%3)*5.0, _probe_start[1]+i*3.0] for i in range(n)]','6.5','0x883377bb',''),('vertical','[[_probe_start[0], _probe_start[1]+i*4.0] for i in range(n)]','12.0','0xff112233','')]

for name,points,size,color,setting in cases:
 for tool in (1,):
  s=source.replace("'points':[_probe_start]*n",f"'points':{points}").replace("'tool_type':2,'stroke_type':0",f"'tool_type':{tool},'stroke_type':0")
  with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=runner.parent) as f:
   f.write(s); f.flush()
   with (out / f'{name}-{tool}.json').open('xb') as o: subprocess.run([sys.executable,f.name,'--single-case','--print-capture','--advanced-mode','unset','--width','256','--height','256','--source-size',size,'--color-argb',color],cwd=root,env=env,stdout=o,check=True)

receipt=json.loads((root/'tools/pen-capture/airbrush_v1_tool1_v204_receipt.json').read_text())
for name,case in receipt['cases'].items():
 raw=(out/f'{name}.json').read_bytes()
 if hashlib.sha256(raw).hexdigest()!=case['capture_sha256']:
  raise RuntimeError(f'capture hash mismatch: {name}')
print('All captured output hashes match the receipt.')
