import os,sys,json,base64,zlib,hashlib,math,subprocess,tempfile
import PIL
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
root=Path(__file__).resolve().parents[2]; os.chdir(root); env=dict(os.environ,CARGO_TARGET_DIR='/tmp/sdocx-agent-target',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
asset=root/'scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg'
assert hashlib.sha256(asset.read_bytes()).hexdigest()=='9f8b208a01db838235f795226acd6e68aa42e609b06c6e171a29447f8b68322b'
provenance=json.loads((root/'tools/tapepen-public-drawing/v247-provenance.json').read_text())
assert PIL.__version__ == provenance['versions']['pillow']
assert pymupdf.VersionBind == provenance['versions']['mupdf']
assert str(pypdfium2.PDFIUM_INFO) == provenance['versions']['pdfium']
assert subprocess.check_output(['resvg','--version'],text=True).strip() == provenance['versions']['resvg']
receipt=json.loads((root/'tools/tapepen-public-drawing/v249-size-receipt.json').read_text())
if len(sys.argv) not in (1,4): raise SystemExit('pass no arguments or native logs for sizes 2, 8, and 16')
for size, filename in zip((2,8,16),sys.argv[1:]):
 raw=Path(filename).read_bytes(); text=raw.decode(); j,_=json.JSONDecoder().raw_decode(text[text.index('\n{')+1:]); expected=receipt['cases'][str(size)]
 assert hashlib.sha256(raw).hexdigest()==expected['native_log_sha256']
 assert j['input']==expected['input'] and j['profile']==expected['profile'] and j['native_hashes']==expected['native_hashes']
 uniforms={}
 for d in j['mesa_draw_observations']:
  for e in d['uniform_events_since_prior_draw']:
   if e['call']=='glUniform2fv': uniforms[e['uniform_location_binding']['name']]=base64.b64decode(e['payload_base64'],validate=True).hex()
 assert uniforms==expected['uniform_f32_le_hex']
 mask_capture=j['mesa_draw_observations'][0]['after']['pixels']; mask_rgba=zlib.decompress(base64.b64decode(mask_capture['payload_base64'])); assert hashlib.sha256(mask_rgba).hexdigest()==mask_capture['sha256']; assert hashlib.sha256(mask_rgba[3::4]).hexdigest()==expected['native_mask_alpha_sha256']
 f=j['mesa_frame_observations'][-1]['rgba8_readback_bound']; assert hashlib.sha256(zlib.decompress(base64.b64decode(f['data_base64']))).hexdigest()==expected['native_sha256']
results={}
def sample(a,w,h,x,y,ch,repeat):
 ix,iy=math.floor(x),math.floor(y); fx,fy=x-ix,y-iy; out=[0.]*ch
 for dx,wx in ((0,1-fx),(1,fx)):
  for dy,wy in ((0,1-fy),(1,fy)):
   xx=(ix+dx)%w if repeat else min(max(ix+dx,0),w-1); yy=(iy+dy)%h if repeat else min(max(iy+dy,0),h-1)
   for c in range(ch): out[c]+=a[(yy*w+xx)*ch+c]*wx*wy/255
 return out
def metric(a,b):
 assert len(a)==len(b); d=[abs(x-y) for x,y in zip(a,b)]; return {'l1':sum(d),'max':max(d),'different_channels':sum(v!=0 for v in d)}
with tempfile.TemporaryDirectory() as d:
 tmp=Path(d)
 harness=tmp/'harness'; (harness/'src/bin').mkdir(parents=True)
 (harness/'src/bin/svg_pdf.rs').write_bytes((root/'tools/tapepen-public-drawing/v248-svg-pdf.rs').read_bytes())
 (harness/'src/bin/asset.rs').write_text('use std::{env,fs,io::{self,Write}}; fn main(){let a:Vec<_>=env::args().collect(); let s=fs::read_to_string(&a[1]).unwrap(); let p=sdocx::tapepen_pattern::rasterize(&s,a[2].parse().unwrap()).unwrap();io::stdout().lock().write_all(&p).unwrap();}')
 (harness/'Cargo.toml').write_text(f"[package]\nname = \"tapepen-control-replay\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nsdocx = {{ path = {json.dumps(str(root/'crates/sdocx'))}, features = [\"render\", \"pdf\", \"serde\"] }}\nserde_json = \"1\"\nsha2 = \"0.10.9\"\nlopdf = {{ version = \"=0.42.0\", default-features = false }}\n")
 (harness/'Cargo.lock').write_bytes((root/'tools/tapepen-public-drawing/pdf-control-Cargo.lock').read_bytes())
 subprocess.run(['cargo','build','--offline','--locked','--manifest-path',str(harness/'Cargo.toml'),'--bins'],check=True,env=env,stdout=subprocess.DEVNULL)
 pattern=subprocess.check_output(['/tmp/sdocx-agent-target/debug/asset',str(asset),'64'])
 for size in (2,8,16):
  expected=json.loads((root/'tools/tapepen-public-drawing/v249-size-receipt.json').read_text())['cases'][str(size)]; native=zlib.decompress((root/f'tools/tapepen-public-drawing/v249-size{size}-frame.rgba.zlib').read_bytes()); assert hashlib.sha256(native).hexdigest()==expected['native_sha256']
  env.update(SDOCX_TAPE_V249_MASK_OUT=str(tmp/'mask'),SDOCX_TAPE_V249_SIZE=str(size)); subprocess.run(['cargo','test','-p','sdocx','--lib','--features','render','--offline','v249_size_source_mask'],check=True,env=env,stdout=subprocess.DEVNULL)
  mask=(tmp/'mask').read_bytes(); native_mask=zlib.decompress((root/f'tools/tapepen-public-drawing/v249-size{size}-mask.alpha.zlib').read_bytes()); assert hashlib.sha256(native_mask).hexdigest()==expected['native_mask_alpha_sha256']; mask_metric=metric(mask,native_mask); rect=[]
  for y in range(256):
   for x in range(256):
    a=mask[y*256+x]/255; sh=.2*sample(mask,256,256,x-size*.0225,y-size*.025,1,False)[0]
    if not a and not sh:continue
    tex=sample(pattern,64,64,x-13,y-21,4,True); stroke=tex[3]*(128/255)*a; alpha=stroke+sh*(1-stroke); q=round(alpha*255)
    if not q: continue
    rgb=[round(tex[c]*stroke/max(alpha,1e-5)*255) for c in range(3)];rect.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}" fill-opacity="{q/255:.9f}"/>')
  svg=tmp/'out.svg';pdf=tmp/'out.pdf';png=tmp/'out.png';svg.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">'+''.join(rect)+'</svg>')
  subprocess.run(['/tmp/sdocx-agent-target/debug/svg_pdf',str(svg),str(pdf),str(tmp/'pdf.json'),'256','256'],check=True,stdout=subprocess.DEVNULL); subprocess.run(['resvg',str(svg),str(png)],check=True)
  data=Image.open(png).convert('RGBA').tobytes();rgba=bytes(round(v*data[i-i%4+3]/255) if i%4<3 else v for i,v in enumerate(data));white=bytes(min(255,native[i+c]+255-native[i+3]) for i in range(0,len(native),4) for c in range(3))
  with pymupdf.open(pdf) as doc: mu=doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=True).samples
  with pypdfium2.PdfDocument(pdf) as doc: pi=doc[0].render(scale=4/3).to_pil().convert('RGB').tobytes()
  results[str(size)]={'native_sha256':expected['native_sha256'],'native_log_sha256':expected['native_log_sha256'],'mask_sha256':hashlib.sha256(mask).hexdigest(),'mask':mask_metric,'svg':metric(rgba,native),'mupdf':metric(mu,native),'pdfium_white':metric(pi,white)}
 for size, result in results.items():
  assert all(result[k] == expected_value for k,expected_value in json.loads((root/'tools/tapepen-public-drawing/v249-size-receipt.json').read_text())['cases'][size].items() if k in result)
 print(json.dumps(results,indent=2))
