import hashlib,json,sys
from pathlib import Path
src=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py').resolve()
s=src.read_text(); old='vals=self.get(ptr,"f"*(n*count)); arr=(C.c_float*len(vals))(*vals)'; new='vals=self.get(ptr,"f"*(n*count))\n            if n == 2 and loc == self.uniform_locations.get("grainOffset"):\n                assert list(vals) == [0.0, 0.0]\n                vals = [float(sys.argv[1]), 0.0]\n            arr=(C.c_float*len(vals))(*vals)'
assert s.count(old)==1;s=s.replace(old,new)
ns={'__name__':'grain_capture','__file__':str(src)};exec(compile(s,str(src),'exec'),ns)
r=ns['capture'](Path('/tmp')); a=bytes.fromhex(json.loads(Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text())['mesa']['readback_rgba_hex']); b=bytes.fromhex(r['mesa']['readback_rgba_hex'])
rows=[[i//4%192,i//4//192,a[i],b[i]] for i in range(3,len(a),4) if a[i]!=b[i]]
print(json.dumps({'baseline_sha256':hashlib.sha256(a).hexdigest(),'offset_sha256':hashlib.sha256(b).hexdigest(),'changed_alpha_pixels':len(rows),'absolute_difference':sum(abs(x[2]-x[3]) for x in rows),'max_difference':max((abs(x[2]-x[3]) for x in rows),default=0),'rows':rows,'uniforms':[e for e in r['mesa']['draw_gl_events'] if e.get('name')=='glUniform2fv']}))
