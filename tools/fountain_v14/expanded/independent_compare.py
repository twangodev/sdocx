import json,base64,zlib,hashlib,subprocess,sys
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
root=Path(__file__).resolve().parents[3]
native=json.loads((root/'conformance/fountain-v14-independent-native.json').read_bytes())
seq=[3,6,7,18,19,28,29,30]; results={}
for version in ['base','head']:
 for i,s in enumerate(seq):
  c=native['cases'][f'scene_top_1_white_{i}_{s}']['output']; raw=zlib.decompress(base64.b64decode(c['rgba_zlib_base64'])); assert hashlib.sha256(raw).hexdigest()==c['sha256']; expected=b''.join(raw[y*512:(y+1)*512] for y in reversed(range(256)))
  stem=Path(sys.argv[1]) / version / f'scale_{i}'; subprocess.run(['resvg',str(stem.with_suffix('.svg')),str(stem.with_suffix('.png'))],check=True)
  images={'resvg':Image.open(stem.with_suffix('.png')).convert('RGBA')}
  for route in ['svg','direct']:
   path=Path(str(stem)+f'.{route}.pdf')
   with pymupdf.open(path) as doc:
    assert not doc[0].get_images(); pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=False); images[route+'_mupdf']=Image.frombytes('RGB',(pix.width,pix.height),pix.samples).convert('RGBA')
   with pypdfium2.PdfDocument(str(path)) as doc:
    page=doc[0]; bitmap=page.render(scale=4/3,rev_byteorder=True,fill_color=(255,255,255,255)); images[route+'_pdfium']=bitmap.to_pil().convert('RGBA'); bitmap.close();page.close()
  for label,img in images.items():
   actual=img.crop((80,72,208,328)).tobytes(); assert len(actual)==len(expected); results[f'{version}/{s}/{label}']=sum(abs(a-b) for a,b in zip(actual,expected))
print(json.dumps(results,indent=2))
