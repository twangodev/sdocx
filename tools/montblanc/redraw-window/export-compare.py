#!/usr/bin/env python3
import base64, hashlib, json, os, shutil, subprocess, sys, tempfile, zlib
from pathlib import Path
import pymupdf, pypdfium2
from PIL import Image
root = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(root/'tools/montblanc'))
from public_fallback_compare import composite_native_frame, metrics
native = json.loads((root/'tools/montblanc/redraw-window/irregular-native.json').read_text())
variants = json.loads((root/'tools/montblanc/redraw-window/irregular-variants.json').read_text())['cases']
cases = {'irregular16': native['input']} | {name: value['input'] for name, value in variants.items()}
sizes = (1.2,)

env = os.environ | {'CARGO_TARGET_DIR':'/tmp/sdocx-agent-target','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','SDOCX_CAPTURE_ROOT':str(root),'PYTHONPATH':str(root/'scratch/apk-analysis-runtime/python')+os.pathsep+str(root/'tools/montblanc/extended-paint/conformance')}
def replace_once(source, old, new):
    assert source.count(old) == 1, f'expected one source marker: {old!r}'
    return source.replace(old, new, 1)

work = Path(os.environ.get('MB_EXPORT_WORK', '/tmp/montblanc-window-export')); work.mkdir(exist_ok=True)
capture_work = Path(os.environ.get('MB_CAPTURE_WORK', str(work)))
if sys.argv[1] == 'capture':
    capture = (root/'tools/montblanc/narrow-probe/capture.py').read_text()
    host = (root/'tools/montblanc/extended-paint/capture_candidate.py').read_text()
    assert host.count("p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1)") == 1
    assert host.count("'matrix_f32':[1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1]") == 1
    capture2 = replace_once(capture, "sys.path.insert(0,str(host.parent))", "host_source = host_source.replace(\"p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1)\", \"p.put(matrix,'16f',2,0,0,0,0,2,0,0,0,0,1,0,tx,ty,0,1)\").replace(\"'matrix_f32':[1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1]\", \"'matrix_f32':[2,0,0,0,0,2,0,0,0,0,1,0,tx,ty,0,1]\")\nsys.path.insert(0,str(host.parent))")
    assert capture != capture2
    script = work/'capture-2.py'; script.write_text(capture2)
    for scale in (1,2):
      for name, case in cases.items():
       for size in sizes:
        key=f'{name}-{size}-{scale}'
        out=work/f'{key}.json'
        p=subprocess.run([sys.executable,str(root/'tools/montblanc/narrow-probe/capture.py' if scale==1 else script),'--case','baseline','--output',str(out)],cwd=root,env=env|{'MB_CASE':json.dumps(case|{'pen_size':size})},capture_output=True,text=True,check=True)
        stream=next(x.partition('=')[2] for x in p.stderr.splitlines() if x.startswith('CAPTURE_STREAM='))
        (work/f'{key}.stream.json').write_text(stream)
        print(key,flush=True)
    sys.exit()

# Copy the current crate into a temporary isolated workspace so changing the
# dispatch gate does not mutate the shared checkout.
if sys.argv[1] not in ('compare-only', 'verify'):
 for mode in ('baseline','candidate'):
   dest=work/mode
   if dest.exists(): shutil.rmtree(dest)
   (dest/'crates').mkdir(parents=True)
   shutil.copytree(root/'crates/sdocx',dest/'crates/sdocx')
   (dest/'Cargo.toml').write_text('[workspace]\nmembers=["crates/sdocx"]\nresolver="2"\n')
   (dest/'conformance').symlink_to(root/'conformance',target_is_directory=True)
   f=dest/'crates/sdocx/src/ink/montblanc.rs'; s=f.read_text()
   if mode == 'candidate':
    s=replace_once(s, '!(3..=9).contains(&stroke.points.len())','!(3..=32).contains(&stroke.points.len())')
    s=replace_once(s, '        9 => many_point::many_point(input),','        9..=32 => many_point::many_point(input),')
    m=dest/'crates/sdocx/src/ink/montblanc_many.rs'; ms=m.read_text()
    ms=replace_once(ms, '!(6..=9).contains(&n)','!(6..=32).contains(&n)')
    ms=replace_once(ms, 'if replay && !redraw {','if replay && seg_index < accepted.len().saturating_sub(7) / 2 {\n                continue;\n            }\n            if replay && !redraw {')
    m.write_text(ms)
   f.write_text(s)
   package=dest/'tools/montblanc/many-point-public'; package.mkdir(parents=True)
   shutil.copy(root/'tools/montblanc/many-point-public/Cargo.toml',package/'Cargo.toml')
   shutil.copy(root/'tools/montblanc/many-point-public/Cargo.lock',package/'Cargo.lock')
   (package/'src').mkdir()
   source=(root/'tools/montblanc/many-point-public/src/main.rs').read_text()
   start=source.index('    for (name, coords, pressure, size) in ['); end=source.index('    ] {',start)+len('    ] {')
   def fl(x): return f'{float(x):.8f}'
   rows=[]
   for name,c in cases.items():
    for size in sizes:
     points=','.join('['+','.join(map(fl,p))+']' for p in c['points'])
     pressure=','.join(map(fl,c['pressure']))
     rows.append(f'("{name}-{size}",vec![{points}],vec![{pressure}],{fl(size)}),')
   source=source[:start]+'    for (name, coords, pressure, size) in [\n'+''.join(rows)+'\n    ] {'+source[end:]
   source=replace_once(source, '"crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx"',repr(str(root/'crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx')).replace("'",'"'))
   (package/'src/main.rs').write_text(source)
   for scale in (1,2):
     out=work/f'{mode}-{scale}'; out.mkdir(exist_ok=True)
     subprocess.run(['cargo','run','--locked','--offline','--quiet','--manifest-path',str(package/'Cargo.toml'),'--',str(out)],cwd=dest,env=env|({'SDOCX_MONTBLANC_DOUBLE':'1'} if scale==2 else {}),check=True)

receipt={'renderers': {'resvg':subprocess.check_output(['resvg','--version'],text=True).strip(), 'pymupdf':pymupdf.VersionBind, 'pdfium':str(pypdfium2.PDFIUM_INFO)}, 'source_sha256': {str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [root/'crates/sdocx/src/ink/montblanc.rs',root/'crates/sdocx/src/ink/montblanc_many.rs',root/'tools/montblanc/narrow-probe/capture.py',root/'tools/montblanc/redraw-window/export-compare.py',root/'tools/montblanc/redraw-window/irregular-candidate.patch',root/'tools/montblanc/many-point-public/src/main.rs',root/'tools/montblanc/many-point-public/Cargo.toml',root/'tools/montblanc/many-point-public/Cargo.lock']}, 'scope':'Synthetic Drawing and host Mesa with partially pinned native dependencies; public SVG resvg and direct PDF MuPDF and PDFium; no device or saved-note claim. Metrics are native support, output support, intersection, union, and union RGB absolute error.', 'cases':{}}
receipt['isolated_source_sha256'] = {
 mode: {name: hashlib.sha256((work/mode/path).read_bytes()).hexdigest() for name,path in {
  'sampler_dispatch': 'crates/sdocx/src/ink/montblanc.rs',
  'sampler': 'crates/sdocx/src/ink/montblanc_many.rs',
  'generated_driver': 'tools/montblanc/many-point-public/src/main.rs',
 }.items()} for mode in ('baseline','candidate')}
for scale in (1,2):
 for name in cases:
  for size in sizes:
   key=f'{name}-{size}-{scale}'
   if not (capture_work/f'{key}.json').exists():
    continue
   cap=json.loads((capture_work/f'{key}.json').read_text())
   stream=json.loads((capture_work/f'{key}.stream.json').read_text())
   expected = native['stream'] if name == 'irregular16' else variants[name]['stream']
   assert stream == expected, f'{key} native callback stream differs'
   frame=zlib.decompress(base64.b64decode(cap['post_readback']['rgba8_zlib_base64']))
   assert hashlib.sha256(frame).hexdigest() == cap['post_readback']['sha256']
   ref=composite_native_frame(frame,(256,256)).tobytes()
   record={'input':cases[name]|{'pen_size':size},'scale':scale,'frame_sha256':hashlib.sha256(frame).hexdigest(),'reference_rgb':{'sha256':hashlib.sha256(ref).hexdigest(),'bytes':len(ref)},'stream_sha256':hashlib.sha256((capture_work/f'{key}.stream.json').read_bytes()).hexdigest(),'input_pins':cap['input_pins'],'host_gl_sha256':cap['host_gl_sha256'],'matrix_f32':cap['matrix_f32'],'metrics':{}}
   for mode in ('baseline','candidate'):
    path=work/f'{mode}-{scale}'/(f'{name}-{size}-'+('candidate' if scale==1 else 'aware'))
    png=work/'render.png'; subprocess.run(['resvg','-w','256',str(path)+'.svg',str(png)],check=True)
    svg=Image.open(png).convert('RGB').tobytes()
    with pymupdf.open(str(path)+'.pdf') as d:
      pix=d[0].get_pixmap(matrix=pymupdf.Matrix(4*scale/3,4*scale/3),alpha=False); assert (pix.width,pix.height)==(256,256); pdf=pix.samples
    with pypdfium2.PdfDocument(str(path)+'.pdf') as d:
      page=d[0]; bitmap=page.render(scale=4*scale/3, rev_byteorder=True); image=bitmap.to_pil().convert('RGB'); assert image.size==(256,256); pdfium=image.tobytes(); bitmap.close(); page.close()
    def digest(data): return {'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)}
    artifacts={'svg_document':digest(Path(str(path)+'.svg').read_bytes()), 'pdf_document':digest(Path(str(path)+'.pdf').read_bytes()), 'svg_png':digest(png.read_bytes()), 'svg_rgb':digest(svg), 'mupdf_rgb':digest(pdf), 'pdfium_rgb':digest(pdfium)}
    record['metrics'][mode]={'artifacts':artifacts,'svg':metrics(ref,svg),'pdf':metrics(ref,pdf),'pdfium':metrics(ref,pdfium),'svg_bytes':Path(str(path)+'.svg').stat().st_size,'pdf_bytes':Path(str(path)+'.pdf').stat().st_size}
   receipt['cases'][key]=record
   print(key, record['metrics'],flush=True)
receipt['complete'] = len(receipt['cases']) == len(cases)*len(sizes)*2
receipt_path=root/'tools/montblanc/redraw-window/export-receipt.json'
if sys.argv[1] in ('verify', 'rebuild-verify'):
 expected=json.loads(receipt_path.read_text())
 assert receipt['complete'] and expected['complete']
 assert receipt == expected, 'receipt mismatch'
 print('All 8 cases, document and raster hashes, lengths, metrics, native hashes and source pins verified.')
else:
 receipt_path.write_text(json.dumps(receipt,indent=2)+'\n')
