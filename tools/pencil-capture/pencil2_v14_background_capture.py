import hashlib,json
from pathlib import Path
src=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py').resolve()
s=src.read_text(); old='f["glClearColor"](0.0,0.0,0.0,0.0)'; new='f["glClearColor"](0.25,0.125,0.375,0.5)'
assert s.count(old)==1;s=s.replace(old,new)
ns={'__name__':'background_capture','__file__':str(src)};exec(compile(s,str(src),'exec'),ns)
r=ns['capture'](Path('/tmp')); a=bytes.fromhex(json.loads(Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text())['mesa']['readback_rgba_hex']); b=bytes.fromhex(r['mesa']['readback_rgba_hex']); pre=bytes.fromhex(r['mesa']['preclear']['bytes_rgba_hex']); rows=[]
for i in range(0,len(a),4):
 if b[i:i+4]!=pre[i:i+4]: rows.append([i//4%192,i//4//192,list(a[i:i+4]),list(b[i:i+4])])
print(json.dumps({'baseline_sha256':hashlib.sha256(a).hexdigest(),'background_sha256':hashlib.sha256(b).hexdigest(),'preclear':list(pre[:4]),'changed_pixels':len(rows),'rows':rows,'blend_calls':[x for x in r['mesa']['draw_gl_events'] if x.get('name') in ('glBlendFuncSeparate','glBlendEquationSeparate')]}))
