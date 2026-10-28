#!/usr/bin/env python3
import hashlib,json,os,subprocess,sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
capture=root/'tools/montblanc/extended-streams/capture.py'
trace=root/'tools/montblanc/extended-streams/end-alpha-trace.py'
cases={}
for name,points,pressure,size in [
 ('threshold',[[0,0],[10,0],[25,0],[30,0]],[.3,.6,1,.7],2),
 ('bent',[[0,0],[10,0],[20,20],[30,0]],[.3,.6,1,.7],2),
 ('low',[[0,0],[10,0],[20,20],[30,0]],[.3,1,1,0],2),
 ('high',[[0,0],[10,0],[20,20],[30,0]],[.3,0,1,1],2),
 ('reverse',[[30,10],[20,10],[-5,15],[-12,4]],[.8,.2,.4,.9],1),
 ('diagonal',[[1,3],[10,10],[40,-10],[50,20]],[.1,.7,.2,.5],1.5),
 ('far',[[0,0],[20,0],[100,30],[120,10]],[.1,.4,.8,.3],2),
]:
 case={'points':points,'pressure':pressure,'pen_size':size}
 env=os.environ|{'PYTHONPATH':str(root/'scratch/apk-analysis-runtime/python'),'MB_CASE':json.dumps(case)}
 results=[json.loads(subprocess.run(['python3',str(script)],cwd=root,env=env,text=True,capture_output=True,check=True).stdout.splitlines()[-1]) for script in [capture,trace]]
 row=next(r for r in results[1] if r['rva']=='0x1dc68')
 cases[name]={'input':case,'stream':results[0]['stream'],'prior':int(row['x_registers']['x8'],16),'target':int(row['x_registers']['x9'],16)}
receipt={'schema':'montblanc-four-long-native-v1','boundary':'Synthetic native ARM64 Unicorn host adapters; no device, saved-note or paint parity.','binary_sha256':hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest(),'script_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [capture,trace]},'cases':cases}
path=root/'conformance/montblanc-four-long-native.json'
if '--write' in sys.argv:path.write_text(json.dumps(receipt,indent=2)+'\n')
else:assert receipt==json.loads(path.read_text());print('verified native four-point long streams')
