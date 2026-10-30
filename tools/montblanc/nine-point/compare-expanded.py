#!/usr/bin/env python3
import base64, gzip, hashlib, json, subprocess, sys, tempfile, zlib
from pathlib import Path
import pymupdf, pypdfium2
from PIL import Image
root = Path(__file__).resolve().parents[3]
here = Path(__file__).resolve().parent
sys.path.insert(0, str(root / 'tools/montblanc'))
from public_fallback_compare import composite_native_frame, metrics
sha = lambda data: hashlib.sha256(data).hexdigest()
manifest = json.loads((here / 'manifest.json').read_text())
for path, digest in manifest['sha256'].items():
 assert sha((root / path).read_bytes()) == digest, path
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == '0.45.1'
assert pymupdf.VersionBind == '1.28.2'
assert pypdfium2.PDFIUM_INFO.version == '150.0.7869.0'
assert pypdfium2.PYPDFIUM_INFO.version == '5.9.0'
results = {}
with tempfile.TemporaryDirectory() as tmp:
 for scale, out in [('1x', Path(sys.argv[1])), ('2x', Path(sys.argv[2]))]:
  for case in ('loop', 'reverse'):
   key = f'{case}-{scale}'
   capture = json.loads(gzip.decompress((here / f'{key}.json.gz').read_bytes()))
   frame = zlib.decompress((here / f'{key}.rgba.zlib').read_bytes())
   assert len(frame) == 256*256*4
   assert sha(frame) == capture['post_readback']['sha256']
   assert frame == zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
   native = composite_native_frame(frame, (256,256)).tobytes()
   for variant in ('candidate' if scale == '1x' else 'aware', 'fallback'):
    path = out / f'{case}-{variant}'
    png = Path(tmp) / 'out.png'
    subprocess.run(['resvg','-w','256',str(path)+'.svg',str(png)],check=True)
    svg_bytes = Image.open(png).convert('RGB').tobytes()
    assert len(svg_bytes) == len(native)
    svg = metrics(native, svg_bytes)
    with pymupdf.open(str(path)+'.pdf') as d:
     mu_bytes = d[0].get_pixmap(matrix=pymupdf.Matrix((4 if scale=='1x' else 8)/3,(4 if scale=='1x' else 8)/3),alpha=False).samples
     assert len(mu_bytes) == len(native)
     mu = metrics(native, mu_bytes)
    with pypdfium2.PdfDocument(str(path)+'.pdf') as d:
     pdf_bytes = d[0].render(scale=(4 if scale=='1x' else 8)/3).to_pil().convert('RGB').tobytes()
     assert len(pdf_bytes) == len(native)
     pdf = metrics(native, pdf_bytes)
    results[f'{key}-{variant}'] = {'svg':svg,'mupdf':mu,'pdfium':pdf,'svg_sha256':sha(Path(str(path)+'.svg').read_bytes()),'pdf_sha256':sha(Path(str(path)+'.pdf').read_bytes()),'frame_sha256':sha(frame),'capture_sha256':sha((here / f'{key}.json.gz').read_bytes())}
print(json.dumps(results,indent=2))
