import os,json,subprocess,concurrent.futures,struct,hashlib,sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
inputs={}
for d in [9.49,9.5,9.51,34.99,35.0,35.01]: inputs['distance_'+str(d)]={'points':[[0,0],[10,0],[20,2],[20+d,2],[80,2]],'pressure':[.3,.6,1,.7,.5],'pen_size':2}
for p in [.46,.48,.5]: inputs['pressure_1_'+str(p)]={'points':[[0,0],[10,0],[20,2],[30,0],[40,1]],'pressure':[.3,p,1,.7,.5],'pen_size':2}
for p in [.625,.65]: inputs['pressure_1_down_'+str(p)]={'points':[[0,0],[10,0],[20,2],[30,0],[40,1]],'pressure':[.3,p,1,.05,.5],'pen_size':2}
def run(item):
 name,case=item;env={**os.environ,'MB_CASE':json.dumps(case),'PYTHONPATH':str(root/'scratch/apk-analysis-runtime/python')};p=subprocess.run(['python3',str(root/'tools/montblanc/extended-streams/alpha-state-evidence.py')],env=env,text=True,capture_output=True,check=True);result=json.loads(p.stdout.splitlines()[-1]);rows=result['alpha_rows'];a=next(r for r in rows if r['rva']=='0x1ebc4');b=next(r for r in rows if r['rva']=='0x1ec10');return name,{'input':case,'candidate_f32_bits':a['s_registers']['s1_f32_bits'],'factor_f32_bits':next(r for r in rows if r['rva']=='0x1eb78')['s_registers']['s13_f32_bits'],'target':int(b['x_registers']['x9'],16),'prior_f32_bits':b['s_registers']['s10_f32_bits']}
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool: results=dict(pool.map(run,inputs.items()))
receipt=json.loads((root/'conformance/montblanc-alpha-thresholds.json').read_text())
assert receipt['threshold_script_sha256']==hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
assert receipt['capture_script_sha256']==hashlib.sha256((root/'tools/montblanc/extended-streams/alpha-state-evidence.py').read_bytes()).hexdigest()
assert receipt['binary_sha256']==hashlib.sha256((root/'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest()
if results != receipt['cases']: raise SystemExit('native alpha threshold mismatch')
print(f'verified {len(results)} native alpha threshold cases')
