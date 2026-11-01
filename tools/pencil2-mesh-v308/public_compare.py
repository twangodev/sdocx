"""Compare current public complete-scene SVG and PDF to retained native frames."""
import hashlib,json,os,subprocess,tempfile,zlib
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
from scene import BASE
root=Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory() as t:
 for backdrop,scale in [('white',1),('white',2),('blue',2)]:
  p=Path(t); name=f'{backdrop}-{scale}'; env=os.environ|{'PENCIL198_DIR':t,'SCALE':str(scale),'BACKDROP':backdrop}
  subprocess.run(['/tmp/sdocx-agent-target/debug/pencil2-mesh-public-control'],cwd=root,env=env,check=True)
  target=zlib.decompress((BASE/f'{name}-second.zlib').read_bytes()); receipt=json.loads((BASE/f'{name}-receipt.json').read_text()); assert hashlib.sha256(target).hexdigest()==receipt['second_sha256']
  target=Image.frombytes('RGBA',(192,192),target).transpose(Image.Transpose.FLIP_TOP_BOTTOM).convert('RGB').tobytes()
  subprocess.run(['resvg','--width','192','--height','192',str(p/'generic1.svg'),str(p/'render.png')],check=True)
  images=[('SVG resvg',Image.open(p/'render.png').convert('RGB'))]
  with pymupdf.open(p/'generic1.pdf') as d:
   pix=d[0].get_pixmap(matrix=pymupdf.Matrix(scale,scale),alpha=False);images.append(('PDF MuPDF',Image.frombytes('RGB',(pix.width,pix.height),pix.samples)))
  with pypdfium2.PdfDocument(p/'generic1.pdf') as d: images.append(('PDF PDFium',d[0].render(scale=scale).to_pil().convert('RGB')))
  print(name,'bytes SVG/PDF',(p/'generic1.svg').stat().st_size,(p/'generic1.pdf').stat().st_size)
  for label,im in images:
   assert im.size==(192,192);e=[abs(a-b) for a,b in zip(im.tobytes(),target)];print(label,sum(e),max(e),sum(v!=0 for v in e))
  for path in p.iterdir():path.unlink()
