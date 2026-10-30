#!/usr/bin/env python3
import base64, hashlib, json, pathlib, subprocess, sys, tempfile, zlib
root = pathlib.Path(__file__).resolve().parents[2]
src = root / 'scratch/oilbrush3-public-drawing-v17'
original = src / 'oilbrush3_public_drawing_capture.py'
raw = original.read_bytes()
prior = json.loads((root / 'tools/oilbrush3-cadence/controls-receipt.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha(raw) == prior['original_runner_sha256']
manifest_raw = (src / 'preflight.json').read_bytes()
assert sha(manifest_raw) == prior['preflight_sha256']
old = '[[0.0,0.0],[10.0,0.0],[20.0,5.0],[30.0,5.0]]'
new = '[[600.0,420.0],[608.0,429.0],[619.0,423.0],[631.0,436.0]]'
assert raw.decode().count(old) == 1
with tempfile.TemporaryDirectory(dir='/tmp') as tmp:
 p = pathlib.Path(tmp); runner = p / original.name
 runner.write_text(raw.decode().replace(old,new))
 (p / 'oilbrush3_canvas_native.py').write_bytes((src / 'oilbrush3_canvas_native.py').read_bytes())
 manifest = json.loads(manifest_raw); manifest['runner_sha256'] = sha(runner.read_bytes())
 mp = p / 'preflight.json'; mp.write_text(json.dumps(manifest))
 result = subprocess.run([sys.executable,str(runner),'--preflight-manifest',str(mp),'--preflight-sha256',sha(mp.read_bytes()),'--compact','--failure-json',str(p/'failure.json')],cwd=root,capture_output=True,check=True)
 capture=json.loads(result.stdout)
 draws=[d for d in capture['mesa_draw_observations'] if d['after']['viewport']==[0,0,1920,1080]]
 pixels=draws[-1]['after']['pixels']; assert (pixels['status'],pixels['width'],pixels['height'],pixels['origin']) == ('ok',1920,1080,'GL lower-left')
 frame=zlib.decompress(base64.b64decode(pixels['payload_base64'])); assert len(frame)==1920*1080*4
 assert capture['native_execution_evidence']['queue_end_error_observation']['actual_mesa_error']==0
 out=root/'conformance/oilbrush3-fresh-trajectory'; out.mkdir(exist_ok=True)
 (out/'frame.rgba.zlib').write_bytes(zlib.compress(frame,9))
 receipt={'scope':'Synthetic cold native Drawing on host Mesa; GL lower-left RGBA8. No device or saved-note parity.', 'points':json.loads(new),'timestamps':[0,8,19,37], 'source_runner_sha256':sha(raw),'preflight_sha256':sha(manifest_raw),'derived_runner_sha256':sha(runner.read_bytes()),'native_hashes':capture['native_hashes'],'p4_draws':len(draws),'frame_sha256':sha(frame),'queue_end_gl_error':0}
 (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n'); print(json.dumps(receipt,indent=2))
