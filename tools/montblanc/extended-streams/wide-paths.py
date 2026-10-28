import concurrent.futures, hashlib, json, os, subprocess, sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
script=root/'tools/montblanc/extended-streams/capture.py'
trace=root/'tools/montblanc/extended-streams/trace.py'
cases={'bent-3':{'points':[[1.25,-2.5],[5.5,9.25],[12.75,-3.5]],'pressure':[0.8,0.2,0.4],'pen_size':3},'reverse-10':{'points':[[20.25,5.5],[9.75,-4.25],[-3.5,2.25]],'pressure':[0.1,0.9,0.7],'pen_size':10},'bent-40':{'points':[[-10.5,7.25],[2.25,18.5],[13.75,-6.5]],'pressure':[0.7,0.4,0.2],'pen_size':40}}
def run(item):
 name, case=item; env=os.environ.copy(); env['PYTHONPATH']=str(root/'scratch/apk-analysis-runtime/python'); env['MB_CASE']=json.dumps(case); p=subprocess.run([sys.executable,str(script)],cwd=root,env=env,text=True,capture_output=True,check=True); result=json.loads(p.stdout.splitlines()[-1]); p=subprocess.run([sys.executable,str(trace)],cwd=root,env=env,text=True,capture_output=True,check=True); events=json.loads(p.stdout.splitlines()[-1])['events']; repeat=next(e for e in events if 'getRepeatEff(' in e['symbol']); return name,{'input':case,'stream':result['stream'],'repeat_trace':repeat}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool: result=dict(pool.map(run,cases.items()))
out={'dso_sha256':hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest(),'capture_sha256':hashlib.sha256(script.read_bytes()).hexdigest(),'trace_sha256':hashlib.sha256(trace.read_bytes()).hexdigest(),'boundary':'Three-point native RTV1 calls under the project Unicorn harness; not painted output or device parity.','cases':result}
print(json.dumps(out,indent=2))
