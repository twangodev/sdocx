import base64,gzip,hashlib,json,subprocess,zlib,tempfile,os
from pathlib import Path
from PIL import Image
import pymupdf
root=Path(__file__).resolve().parents[3]; tmp=tempfile.TemporaryDirectory(prefix='montblanc-public-optin-'); out=Path(tmp.name); env=os.environ | {'CARGO_TARGET_DIR':'/tmp/sdocx-agent-target','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'}; subprocess.run(['cargo','run','--offline','--quiet','--manifest-path',str(root/'tools/montblanc/public-optin/Cargo.toml'),'--',str(out)],cwd=root,env=env,check=True)
fixture=root/'crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx'
assert hashlib.sha256(fixture.read_bytes()).hexdigest() == 'ec4476cba55a9e92e4d5c54eb5789e0842a223e120825009fffd004a952d7de5'
def metrics(n,a):
 ns=[any(abs(n[i+j]-252)>2 for j in range(3)) for i in range(0,len(n),3)]; aa=[any(abs(a[i+j]-252)>2 for j in range(3)) for i in range(0,len(a),3)]
 return [sum(ns),sum(aa),sum(x and y for x,y in zip(ns,aa)),sum(x or y for x,y in zip(ns,aa)),sum(abs(n[i*3+j]-a[i*3+j]) for i,(x,y) in enumerate(zip(ns,aa)) if x or y for j in range(3))]
for name in ['baseline','pressure','size','far_endpoint','four','five']:
 p=root/(f'tools/montblanc/extended-paint/{name}.json.gz' if name in ('four','five') else f'tools/montblanc/captures/{name}.json.gz'); capture=gzip.decompress(p.read_bytes()); receipt=json.loads((root/('conformance/montblanc-extended-paint-native.json' if name in ('four','five') else 'conformance/montblanc-public-fallback-host-comparison.json')).read_text()); assert hashlib.sha256(capture).hexdigest()==receipt['cases'][name]['capture_sha256']; data=json.loads(capture); raw=zlib.decompress(base64.b64decode(data['post_readback']['rgba8_zlib_base64'])); assert hashlib.sha256(raw).hexdigest()==data['post_readback']['sha256']; img=Image.frombytes('RGBA',(256,256),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM); b=img.tobytes(); native=bytes(min(255,b[i+j]+round(252*(255-b[i+3])/255)) for i in range(0,len(b),4) for j in range(3)); print(name,end=' ')
 actual=[]
 for variant in ['candidate','fallback']:
  subprocess.run(['resvg',str(out/f'{name}-{variant}.svg'),str(out/'page.png')],check=True); image=Image.open(out/'page.png'); assert image.size == (256,256); svg=image.convert('RGB').tobytes()
  with pymupdf.open(out/f'{name}-{variant}.pdf') as d:
   assert len(d)==1; pix=d[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=False); assert (pix.width,pix.height)==(256,256); pdf=pix.samples
  actual.extend([metrics(native,svg),metrics(native,pdf)]); print(variant,actual[-2],actual[-1],end='; ')
 print()
 assert actual == json.loads((root/'conformance/montblanc-public-optin-host-comparison.json').read_text())['cases'][name]
