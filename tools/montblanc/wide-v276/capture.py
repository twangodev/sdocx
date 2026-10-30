import argparse, json, os, subprocess, sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
parser=argparse.ArgumentParser()
parser.add_argument('--output-dir', type=Path, required=True, help='new directory for capture and input')
out=parser.parse_args().output_dir.resolve()
out.mkdir(parents=True, exist_ok=False)
case={'points':[[-45,-15],[-38,-23],[-30,-27],[-22,-25],[-15,-18],[-10,-8],[-8,2],[-11,12],[-18,20],[-27,24],[-36,22],[-43,15]],'pressure':[0.8,0.65,0.4,0.25,0.3,0.55,0.85,0.95,0.7,0.45,0.3,0.6],'pen_size':2.4}
env=os.environ|{'SDOCX_CAPTURE_ROOT':str(root),'PYTHONPATH':str(root/'scratch/apk-analysis-runtime/python')+os.pathsep+str(root/'tools/montblanc/extended-paint/conformance'),'MB_CASE':json.dumps(case)}
p=subprocess.run([sys.executable,str(root/'tools/montblanc/narrow-probe/capture.py'),'--case','baseline','--output',str(out/'capture-1.json')],cwd=root,env=env,text=True,capture_output=True,check=True)
stream=json.loads(next(x.partition('=')[2] for x in p.stderr.splitlines() if x.startswith('CAPTURE_STREAM=')))
(out/'native.json').write_text(json.dumps({'input':case,'stream':stream})+'\n')
print('stream',len(stream))
