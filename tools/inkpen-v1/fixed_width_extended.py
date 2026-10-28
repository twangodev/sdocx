import sys
import json, os, subprocess, hashlib, base64,zlib
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit("usage: python3 " + sys.argv[0] + " NEW_OUTPUT.json")
output = Path(sys.argv[1])
if output.exists():
    raise FileExistsError(output)
root=Path(__file__).resolve().parents[2]; cases=[]
for width,tool,curve,scale,shape in [(50,1,1,1,'straight'),(100,1,1,1,'straight'),(200,1,1,1,'straight'),(6,0,1,1,'straight'),(6,1,0,1,'straight'),(6,2,1,1,'straight'),(6,3,0,1,'straight'),(6,4,0,1,'straight'),(6,1,1,2,'bent'),(6,1,1,3,'phase'),(6,1,1,4,'straight')]:
 env=os.environ.copy();env.update(PYTHONPATH='scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias', FIXED_WIDTH=str(width), TOOL=str(tool),CURVE=str(curve),INKPEN_CAPTURE_SCALE=str(scale))
 if shape=='bent':env['INKPEN_CAPTURE_BENT']='1'
 if shape=='phase':env['INKPEN_CAPTURE_PHASE']='1'
 s=subprocess.check_output(['python3','tools/inkpen-v1/fixed_width_capture.py','--print-capture'],cwd=root,env=env,text=True);x=json.loads(s[s.index('\n{')+1:]);c=x['source_caller'];r=x['target']['readback'];rgba=zlib.decompress(base64.b64decode(r['fbo4_rgba8_zlib_base64']));assert hashlib.sha256(rgba).hexdigest()==r['fbo4_rgba8_sha256']
 cases.append({'width':width,'tool':tool,'curve':bool(curve),'scale':scale,'shape':shape,'input':c['input'],'getters':c['native_calls']['ObjectStroke_getter_snapshots'],'route':c['route'],'streams':c['streams'],'rgba_sha256':hashlib.sha256(rgba).hexdigest(),'rgba_zlib_base64':r['fbo4_rgba8_zlib_base64']});print(width,tool,curve,scale,shape,flush=True)
with output.open('x') as f: f.write(json.dumps({'schema':'sdocx-inkpen-v1-fixed-width-extended-native-v1','scope':'Synthetic ObjectStroke native Drawing and host Mesa; not device or saved-note parity','cases':cases,'libraries':c['libraries'],'renderer_library_sha256':x['renderer_library_sha256'],'helper_hashes':x['helper_hashes']},indent=2)+'\n')
