#!/usr/bin/env python3
"""Capture tool-3 Drawing/Mesa controls; output directory is the sole argument.

Requires the pinned local runner and dependencies listed in the receipt.
"""
import os,sys,subprocess,tempfile,pathlib,json,hashlib
root=pathlib.Path(__file__).resolve().parent.parent; out=pathlib.Path(sys.argv[1]).resolve(); out.mkdir(parents=True, exist_ok=True); runner=root/'tmp/vector-parity-plan/pen-family-audit/airbrush-v1-short-native/frame-capture.py'; source=runner.read_text(); assert hashlib.sha256(source.encode()).hexdigest()=='d38a83f3cf71b228f58654c08a135d3f91d3bda0349df1d6014926752dc3b73e'
env=os.environ.copy(); env['AB_N']='5'; env['PYTHONPATH']=':'.join(str(root/p) for p in ('scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin','tmp/conformance-cleanup/native-capture-a241793/conformance')); env['LD_LIBRARY_PATH']=str(root/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
cases=[('zigzag','[[_probe_start[0]+i*4.0, _probe_start[1]+(i%2)*7.0] for i in range(n)]','8.75','0x8044ee11',''),('reverse','[[_probe_start[0]-i*2.5, _probe_start[1]-i*1.25] for i in range(n)]','2.25','0xffdf2255',''),('literal2','[[_probe_start[0]+i*3.0, _probe_start[1]+i*i*.5] for i in range(n)]','3.144','0xff44aaee','2;')]
for name,points,size,color,setting in cases:
 s=source.replace("'points':[_probe_start]*n",f"'points':{points}").replace("'tool_type':2,'stroke_type':0","'tool_type':3,'stroke_type':0")
 with tempfile.NamedTemporaryFile(mode='w',suffix='.py',dir=runner.parent) as f:
  f.write(s); f.flush()
  with (out / f'{name}.json').open('wb') as o: subprocess.run([sys.executable,f.name,'--single-case','--print-capture','--advanced-mode','explicit-empty','--advanced-setting',setting,'--width','256','--height','256','--source-size',size,'--color-argb',color],cwd=root,env=env,stdout=o,check=True)
