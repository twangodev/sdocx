"""Repeat the bounded host-Mesa tolerance controls and verify retained hashes."""
import json, os, subprocess, sys, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[2]; receipt=json.loads((root/'tools/inkpen2-v1-model-tolerance/paint-receipt.json').read_text())
sources=json.loads((root/'crates/sdocx/tests/assets/fountain-v14/scene-sources.json').read_text())['strokes']
def note(index, dx, dy):
 s=next(x['stroke'] for x in sources if x['index']==index);n=len(s['points'])
 return {'points':[[p['x']+dx,p['y']+dy] for p in s['points']],'size':9,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':s['pressures'],'timestamps':s['timestamps'],'tilt':s['tilts'][:n],'orientation':s['orientations'][:n]}
turn={'points':[[16,16],[24,16],[24,24],[16,24]],'size':4,'curve_enabled':True,'tool_type':3,'stroke_type':0,'pressure':[.2,.4,.6,.8],'timestamps':[0,10,20,30],'tilt':[0]*4,'orientation':[0]*4}
env=os.environ.copy();env['PYTHONPATH']=os.pathsep.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
with tempfile.TemporaryDirectory() as d:
 for name,case,profile,scale,values in [('note29',note(29,-96,-268),(1920,1080),1,[None,.5]),('scene7',note(7,-148.24111938476562,-116.8331298828125),(1920,1080),1,[0,.5,100]),('tool3',turn,(1440,2560),2,[0,.5,100])]:
  for t in values:
   case['tolerance']=t; inp=Path(d)/'input.json';inp.write_text(json.dumps(case));key=('paint' if t is None else 'paint-0.5') if name=='note29' else f'paint-{name}-{t}'
   cmd=[sys.executable,str(root/'tools/inkpen2-v1-model-tolerance/paint_caller.py'),'--print-capture','--width',str(profile[0]),'--height',str(profile[1]),'--matrix-scale',str(float(scale)),'--matrix-translate-x','0','--matrix-translate-y','0','--output-width',str(64*scale),'--output-height',str(64*scale),'--input-json',str(inp)]
   x=json.loads(subprocess.check_output(cmd,cwd=root,env=env)); assert all(x[k]==v for k,v in receipt[key].items()),key
   print(key,'matched',flush=True)
