import json,hashlib
from pathlib import Path
src=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py').resolve()
code=src.read_text()
old='struct.pack("<16f",1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1)'
assert code.count(old)==1
code=code.replace(old,'struct.pack("<16f",0,1,0,0,-1,0,0,0,0,0,1,0,192,0,0,1)')
ns={'__name__':'rotate_capture','__file__':str(src)}
exec(compile(code,str(src),'exec'),ns)
r=ns['capture'](Path('/tmp'))
a=bytes.fromhex(json.loads(Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text())['mesa']['readback_rgba_hex'])
b=bytes.fromhex(r['mesa']['readback_rgba_hex'])
rows=[]
for y in range(192):
 for x in range(192):
  av=a[(y*192+x)*4+3];bv=b[(x*192+191-y)*4+3]
  if av or bv: rows.append((x,y,av,bv))
diff=[v for v in rows if v[2]!=v[3]]
print(json.dumps({'matrix':[0,1,0,0,-1,0,0,0,0,0,1,0,192,0,0,1], 'baseline_hash':hashlib.sha256(a).hexdigest(),'rotated_hash':hashlib.sha256(b).hexdigest(),'union':len(rows),'mismatches':len(diff),'absolute_sum':sum(abs(v[2]-v[3]) for v in rows),'max_difference':max((abs(v[2]-v[3]) for v in rows),default=0),'examples':diff[:10], 'viewport_events':[e for e in r['mesa']['draw_gl_events'] if e['name']=='glViewport'], 'uniform_events':[e for e in r['mesa']['draw_gl_events'] if 'Uniform' in e['name']]},indent=2))
