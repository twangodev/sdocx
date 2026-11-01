"""Verify the 0.5-tolerance native Drawing and saved-redraw streams."""
import json, os, subprocess, sys, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[2]
sources=json.loads((root/'crates/sdocx/tests/assets/fountain-v14/scene-sources.json').read_text())['strokes'];s=next(x['stroke'] for x in sources if x['index']==7);n=len(s['points'])
case={'points':[[p['x']-148.24111938476562,p['y']-116.8331298828125] for p in s['points']],'size':9,'curve_enabled':True,'tool_type':2,'stroke_type':0,'pressure':s['pressures'],'timestamps':s['timestamps'],'tilt':s['tilts'][:n],'orientation':s['orientations'][:n],'tolerance':0.5}
env=os.environ.copy();env['PYTHONPATH']=os.pathsep.join(str(root/p) for p in ('scratch/straightinkpen2-alias','scratch/apk-analysis-runtime/python','scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
with tempfile.TemporaryDirectory() as d:
 inp=Path(d)/'input.json';inp.write_text(json.dumps(case));raw=subprocess.check_output([sys.executable,str(root/'tools/inkpen2-v1-model-tolerance/caller.py'),'--print-capture','--pen-name','InkPen2','--advanced-setting','null','--input-json',str(inp)],cwd=root,env=env)
 x=json.loads(raw);expected=json.loads((root/'tools/inkpen2-v1-model-tolerance/scene7-redraw.json').read_text())
 for phase in expected:
  actual=[e for e in x['callback_buffer_events'] if e['phase']==phase][-1]['after_bytes'];assert actual==expected[phase],phase
print('scene 7 Drawing and saved redraw matched')
