#!/usr/bin/env python3
import argparse, base64, gzip, hashlib, json, os, subprocess, sys, tarfile, tempfile, zlib
from pathlib import Path
import pymupdf
from PIL import Image
import PIL

package=Path(__file__).resolve().parent
conformance=package.parents[2]/'conformance'
parser=argparse.ArgumentParser()
parser.add_argument('--repo',type=Path,required=True)
parser.add_argument('--capture',action='store_true',help='also rerun native capture; requires scratch native inputs and adapters')
args=parser.parse_args(); repo=args.repo.resolve()
manifest=json.loads((package/'manifest.json').read_text())
sha=lambda data:hashlib.sha256(data).hexdigest()
for name,digest in manifest['artifacts_sha256'].items(): assert sha(((conformance if name.endswith('.rgba') or name in ('montblanc-2x-comparison.json','montblanc-extended-2x-provenance.json') else package)/('montblanc-extended-2x-comparison.json' if name=='montblanc-2x-comparison.json' else name)).read_bytes())==digest,name
assert subprocess.check_output(['resvg','--version'],text=True).strip()==manifest['versions']['resvg']
assert pymupdf.VersionBind==manifest['versions']['pymupdf'] and PIL.__version__==manifest['versions']['pillow']
with tempfile.TemporaryDirectory(prefix='montblanc-2x-verify-') as directory:
 root=Path(directory); archive=root/'source.tar'
 subprocess.run(['git','archive','--format=tar','--output',str(archive),manifest['revision']],cwd=repo,check=True)
 with tarfile.open(archive) as source: source.extractall(root,filter='data')
 archive.unlink()
 for name,digest in manifest['source_sha256'].items(): assert sha((root/name).read_bytes())==digest,name
 patch=root/'gate.patch';patch.write_bytes(gzip.decompress((package/'gate.patch.gz').read_bytes()))
 subprocess.run(['git','apply','--check',str(patch)],cwd=root,check=True)
 subprocess.run(['git','apply',str(patch)],cwd=root,check=True)
 for name,digest in manifest['patched_sha256'].items(): assert sha((root/name).read_bytes())==digest,name
 sys.path.insert(0,str(root/'tools/montblanc'))
 from public_fallback_compare import composite_native_frame, metrics
 env=os.environ | {'CARGO_TARGET_DIR':'/tmp/sdocx-agent-target','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'}
 out=root/'output';out.mkdir()
 subprocess.run(['cargo','run','--locked','--offline','--quiet','--manifest-path',str(root/'tools/montblanc/public-optin-2x/Cargo.toml'),'--',str(out)],cwd=root,env=env,check=True)
 receipt=json.loads((conformance/'montblanc-extended-2x-comparison.json').read_text())
 provenance=json.loads((conformance/'montblanc-extended-2x-provenance.json').read_text())['cases']
 if args.capture:
  (root/'scratch').symlink_to(repo/'scratch',target_is_directory=True)
  env['PYTHONPATH']=str(root/'scratch/apk-analysis-runtime/python')+os.pathsep+str(root/'tools/montblanc/extended-paint/conformance')
  env['SDOCX_CAPTURE_ROOT']=str(root)
 cases={'four':{'points':[[1,3],[10,10],[20,2],[30,0]],'pressure':[0.3,0.6,1,0.7],'pen_size':2},'five':{'points':[[0,0],[5,8],[11,-1],[15,6],[22,-3]],'pressure':[0.3,0.6,1,0.7,0.5],'pen_size':2}}
 for name,info in receipt.items():
  assert cases[name]==provenance[name]['input'] and info['frame_sha256']==provenance[name]['frame_sha256']
  fixture=(root/provenance[name]['stream_fixture_path']).read_bytes();assert sha(fixture)==provenance[name]['stream_fixture_sha256']
  data=json.loads(fixture);data=data if name=='four' else data['cases']['bent'];stream=data['stream']['stream'] if name=='four' else data['result']['stream'];assert sha(json.dumps(stream,separators=(',',':')).encode())==provenance[name]['stream_json_sha256']
  crop=(conformance/f'montblanc-{name}-2x.rgba').read_bytes(); assert sha(crop)==info['crop_sha256']
  l,t,r,b=info['bounds']; frame=Image.new('RGBA',(256,256));frame.paste(Image.frombytes('RGBA',(r-l,b-t),crop),(l,t)); assert sha(frame.tobytes())==info['frame_sha256']
  if args.capture:
   capture=root/f'{name}.json';env['MB_CASE']=json.dumps(cases[name],separators=(',',':'))
   subprocess.run([sys.executable,str(package/'montblanc-2x-capture.py'),'--case','baseline','--output',str(capture)],cwd=root,env=env,check=True)
   captured=json.loads(capture.read_text());raw=zlib.decompress(base64.b64decode(captured['post_readback']['rgba8_zlib_base64']))
   assert captured['matrix_f32']==provenance[name]['matrix']
   assert captured['input_pins']==provenance[name]['input_pins']
   assert captured['host_gl_sha256']==provenance[name]['host_gl_sha256']
   assert [x['sha256'] for x in captured['native_add_vectors']]==provenance[name]['vector_sha256']
   assert sha(raw)==captured['post_readback']['sha256']==info['frame_sha256']
  reference=composite_native_frame(frame.tobytes(),(256,256)).tobytes()
  for mode,expected in info['modes'].items():
   png=out/'render.png';subprocess.run(['resvg','--width','256',str(out/f'{name}-{mode}.svg'),str(png)],check=True)
   with Image.open(png) as image: svg=metrics(reference,image.convert('RGB').tobytes())
   with pymupdf.open(out/f'{name}-{mode}.pdf') as doc:
    pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(8/3,8/3),alpha=False);assert (pix.width,pix.height)==(256,256);pdf=metrics(reference,pix.samples)
   assert {'svg':svg,'pdf':pdf}==expected,(name,mode)
 print('Verified archived patched source, public output, native crops'+(' and recaptures' if args.capture else ''))
