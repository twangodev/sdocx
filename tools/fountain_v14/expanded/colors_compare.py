import json,base64,zlib,hashlib,subprocess,sys
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
root=Path(__file__).resolve().parents[3]; native=json.loads((root/'conformance/fountain-v14-drawing-paint-native.json').read_bytes()); results={}
for version in ['base','head']:
 for scale in [1,2]:
  for name,color in [('black','ff000000'),('blue','8044aaee'),('orange','00c04020')]:
   case=native['cases'][f'drawing_center_top_{scale}_{color}_white']; c=case['output']; raw=zlib.decompress(base64.b64decode(c['rgba_zlib_base64'])); assert hashlib.sha256(raw).hexdigest()==c['sha256']; expected=b''.join(raw[y*256:(y+1)*256] for y in reversed(range(64)))
   stem=Path(sys.argv[1]) / version / f'{name}_{scale}'; subprocess.run(['resvg','-z',str(scale),str(stem.with_suffix('.svg')),str(stem.with_suffix('.png'))],check=True,stdout=subprocess.DEVNULL)
   images={'resvg':Image.open(stem.with_suffix('.png')).convert('RGBA')}
   with pymupdf.open(stem.with_suffix('.pdf')) as doc:
    assert not doc[0].get_images(); pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(scale*4/3,scale*4/3),alpha=False); images['mupdf']=Image.frombytes('RGB',(pix.width,pix.height),pix.samples).convert('RGBA')
   with pypdfium2.PdfDocument(str(stem.with_suffix('.pdf'))) as doc:
    page=doc[0]; bitmap=page.render(scale=scale*4/3,rev_byteorder=True,fill_color=(255,255,255,255)); images['pdfium']=bitmap.to_pil().convert('RGBA'); bitmap.close();page.close()
   x,y=case['source_origin']; box=(x*scale,y*scale,x*scale+64,y*scale+64)
   for label,img in images.items():
    actual=img.crop(box).tobytes(); assert len(actual)==len(expected); results[f'{version}/{name}/{scale}/{label}']=sum(abs(a-b) for a,b in zip(actual,expected))
print(json.dumps(results,indent=2))
