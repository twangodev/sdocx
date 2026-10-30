#!/usr/bin/env python3
"""Source-derived destination-scale diagnostic; native frames are comparison only."""
import argparse, hashlib, json, math, os, struct, subprocess, sys, tempfile, zlib
from pathlib import Path
import PIL
from PIL import Image
import pymupdf, pypdfium2

root = Path(__file__).resolve().parents[2]
parser=argparse.ArgumentParser()
parser.add_argument('--native-logs', nargs=2, metavar=('LOG_1X','LOG_2X'), help='verify fresh native replay logs before comparing')
args=parser.parse_args()
provenance=json.loads((root/'tools/tapepen-public-drawing/v247-provenance.json').read_text())
assert PIL.__version__ == provenance['versions']['pillow']
assert pymupdf.VersionBind == provenance['versions']['mupdf']
assert str(pypdfium2.PDFIUM_INFO) == provenance['versions']['pdfium']
assert subprocess.check_output(['resvg','--version'],text=True).strip() == provenance['versions']['resvg']
if args.native_logs:
    for scale, log in enumerate(args.native_logs, 1):
        verifier = 'v173-verify-native.py' if scale == 1 else 'v248-verify-native.py'
        command = [sys.executable, str(root/'tools/tapepen-public-drawing'/verifier)]
        if scale == 1: command.append('1')
        subprocess.run([*command,log],check=True,stdout=sys.stderr)
source = root / 'scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg'
assert hashlib.sha256(source.read_bytes()).hexdigest() == '9f8b208a01db838235f795226acd6e68aa42e609b06c6e171a29447f8b68322b'
env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0')
def run(*args, **kw): return subprocess.run(args, cwd=root, env=env, check=True, **kw)
def sample(a,w,h,x,y,ch,repeat):
    ix,iy=math.floor(x),math.floor(y); fx,fy=x-ix,y-iy; out=[0.]*ch
    for dx,wx in ((0,1-fx),(1,fx)):
        for dy,wy in ((0,1-fy),(1,fy)):
            xx=(ix+dx)%w if repeat else min(max(ix+dx,0),w-1)
            yy=(iy+dy)%h if repeat else min(max(iy+dy,0),h-1)
            for c in range(ch): out[c]+=a[(yy*w+xx)*ch+c]*wx*wy/255
    return out
with tempfile.TemporaryDirectory(prefix='sdocx-tape-v247-') as tmp:
    tmp=Path(tmp)
    harness=tmp/'harness'; (harness/'src/bin').mkdir(parents=True)
    (harness/'src/bin/svg_pdf.rs').write_bytes((root/'tools/tapepen-public-drawing/v248-svg-pdf.rs').read_bytes())
    (harness/'src/bin/asset.rs').write_text('''use std::{env,fs,io::{self,Write}};
fn main() { let a: Vec<_> = env::args().collect(); let source = fs::read_to_string(&a[1]).unwrap();
let pixels = sdocx::tapepen_pattern::rasterize(&source, a[2].parse().unwrap()).unwrap();
io::stdout().lock().write_all(&pixels).unwrap(); }
''')
    (harness/'Cargo.toml').write_text(f'''[package]
name = "tapepen-control-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = {{ path = {json.dumps(str(root/'crates/sdocx'))}, features = ["render", "pdf", "serde"] }}
serde_json = "1"
sha2 = "0.10.9"
lopdf = {{ version = "=0.42.0", default-features = false }}
''')
    (harness/'Cargo.lock').write_bytes((root/'tools/tapepen-public-drawing/pdf-control-Cargo.lock').read_bytes())
    run('cargo','build','--offline','--locked','--manifest-path',str(harness/'Cargo.toml'),'--bins')
    cli=Path('/tmp/sdocx-agent-target/debug/asset')
    results={}
    for scale in (2,):
        maskfile=tmp/'mask'; env.update(SDOCX_TAPE_V248_MASK_OUT=str(maskfile),SDOCX_TAPE_V248_SCALE=str(scale))
        run('cargo','test','-p','sdocx','--lib','--features','render','--offline','v248_scaled_source_mask',stdout=subprocess.DEVNULL)
        mask=maskfile.read_bytes(); w=256*scale; assert len(mask)==w*w
        native_mask=zlib.decompress((root/'tools/tapepen-public-drawing/v248-native-mask.alpha.zlib').read_bytes())
        assert hashlib.sha256(native_mask).hexdigest() == '60cdce6862739174abda6dc8d815ccb738a94016518cde9c95114c21d31d3788'
        differences=[abs(a-b) for a,b in zip(mask,native_mask)]
        assert (sum(differences),max(differences),sum(d!=0 for d in differences)) == (1,1,1)
        native=zlib.decompress((root/f'tools/tapepen-public-drawing/v173-{scale}x-frame.rgba.zlib').read_bytes())
        receipt=json.loads((root/'tools/tapepen-public-drawing/v173-receipt.json').read_text())
        assert hashlib.sha256(native).hexdigest()==receipt['native'][str(scale)]['sha256']
        white=bytes(min(255,native[i+c]+255-native[i+3]) for i in range(0,len(native),4) for c in range(3))
        for size in (256,):
            pattern=subprocess.check_output([str(cli),str(source),str(size)])
            native_pattern=zlib.decompress((root/'tools/tapepen-public-drawing/v248-native-pattern.rgba.zlib').read_bytes())
            assert hashlib.sha256(native_pattern).hexdigest() == '42abf1113cfd13c27f1e0ccb29affae62221c0888f5ffcdc2425e2169b71b13d'
            assert pattern == native_pattern
            sx,sy=(v*w for v in struct.unpack('<ff',bytes.fromhex('003d8a3a009a993a')))
            rect=[]
            for y in range(w):
                for x in range(w):
                    a=mask[y*w+x]/255; sh=.2*sample(mask,w,w,x-sx,y-sy,1,False)[0]
                    if not a and not sh: continue
                    tex=sample(pattern,size,size,(x+0.5-154)*size/256-0.5,(y+0.5-118)*size/256-0.5,4,True)
                    stroke=tex[3]*(192/255)*a; alpha=stroke+sh*(1-stroke); q=round(alpha*255)
                    if not q: continue
                    rgb=[round(tex[c]*stroke/max(alpha,1e-5)*255) for c in range(3)]
                    rect.append(f'<rect x="{x/scale}" y="{y/scale}" width="{1/scale}" height="{1/scale}" fill="#{rgb[0]:02x}{rgb[1]:02x}{rgb[2]:02x}" fill-opacity="{q/255:.9f}"/>')
            svg=tmp/'out.svg'; pdf=tmp/'out.pdf'; png=tmp/'out.png'
            svg.write_text('<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256">'+''.join(rect)+'</svg>')
            run('/tmp/sdocx-agent-target/debug/svg_pdf',str(svg),str(pdf),str(tmp/'pdf.json'),'256','256',stdout=subprocess.DEVNULL)
            run('resvg','--zoom',str(scale),str(svg),str(png))
            data=Image.open(png).convert('RGBA').tobytes(); rgba=bytes(round(v*data[i-i%4+3]/255) if i%4<3 else v for i,v in enumerate(data))
            with pymupdf.open(pdf) as doc: mupdf=doc[0].get_pixmap(matrix=pymupdf.Matrix(scale*4/3,scale*4/3),alpha=True).samples
            with pypdfium2.PdfDocument(pdf) as doc: pdfium=doc[0].render(scale=scale*4/3).to_pil().convert('RGB').tobytes()
            def metric(a,b):
                assert len(a)==len(b)
                d=[abs(x-y) for x,y in zip(a,b)]; return {'l1':sum(d),'max':max(d),'different_channels':sum(v!=0 for v in d)}
            results[f'{scale}x_texture{size}']={'mask_sha256':hashlib.sha256(mask).hexdigest(),'pattern_sha256':hashlib.sha256(pattern).hexdigest(),'svg':metric(rgba,native),'mupdf':metric(mupdf,native),'pdfium_white':metric(pdfium,white)}
    expected=json.loads((root/'tools/tapepen-public-drawing/v248-scale-results.json').read_text())
    assert results == expected, (results, expected)
    print(json.dumps(results,indent=2))
