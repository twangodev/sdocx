import sys
import json, os, subprocess, hashlib, base64, zlib
from pathlib import Path

if len(sys.argv) != 2:
    raise SystemExit("usage: python3 " + sys.argv[0] + " NEW_OUTPUT.json")
output = Path(sys.argv[1])
if output.exists():
    raise FileExistsError(output)
root=Path(__file__).resolve().parents[2]
cases=[]
for width, tool, curve in [(0,1,1),(.5,1,1),(3,1,1),(6,1,1),(12,1,1),(30,1,1),(6,0,0),(6,2,0),(6,3,1),(6,4,1)]:
 env=os.environ.copy(); env.update(PYTHONPATH='scratch/apk-analysis-runtime/python:scratch/straightinkpen2-alias',FIXED_WIDTH=str(width),TOOL=str(tool),CURVE=str(curve))
 s=subprocess.check_output(['python3','tools/inkpen-v1/fixed_width_capture.py','--print-capture'],cwd=root,env=env,text=True)
 x=json.loads(s[s.index('\n{')+1:]); c=x['source_caller']; read=x['target']['readback']; rgba=zlib.decompress(base64.b64decode(read['fbo4_rgba8_zlib_base64'])); assert hashlib.sha256(rgba).hexdigest()==read['fbo4_rgba8_sha256']
 cases.append({'width':width,'tool':tool,'curve':bool(curve),'input':c['input'],'getters':c['native_calls']['ObjectStroke_getter_snapshots'],'route':c['route'],'streams':c['streams'],'rgba_sha256':hashlib.sha256(rgba).hexdigest(),'rgba_zlib_base64':read['fbo4_rgba8_zlib_base64']})
 print(width,tool,curve,c['streams']['draw_object_stroke']['sha256'],flush=True)
with output.open('x') as f: f.write(json.dumps({'schema':'sdocx-inkpen-v1-fixed-width-native-v1','scope':'Synthetic ObjectStroke native Drawing and host Mesa; not device or saved-note parity','cases':cases, 'libraries':x['source_caller']['libraries'], 'renderer_library_sha256':x['renderer_library_sha256'], 'helper_hashes':x['helper_hashes']},indent=2)+'\n')
