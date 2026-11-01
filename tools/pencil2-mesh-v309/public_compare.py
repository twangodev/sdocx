"""Compare generic document SVG and PDF to the recaptured note-derived scene."""
import hashlib,json,os,subprocess,tempfile
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
root=Path(__file__).resolve().parents[2]
source=Path(os.environ['PENCIL_MESH_CAPTURE_DIR'])
receipt=json.loads((source/'capture.json').read_text()); raw=(source/'second.rgba').read_bytes();assert hashlib.sha256(raw).hexdigest()==receipt['second_sha256']
expected=json.loads((root/'tools/pencil2-mesh-v309/receipt.json').read_text())['note_native'][os.environ.get('PENCIL_MESH_BACKDROP','blue')]
assert receipt['second_sha256']==expected['second_frame_sha256'] and receipt['geometry_sha256']==expected['native_descriptor_sha256']
target=Image.frombytes('RGBA',(192,192),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).convert('RGB').tobytes()
with tempfile.TemporaryDirectory(prefix='sdocx-pencil-note-public-') as t:
 p=Path(t);env=os.environ|{'PENCIL198_DIR':t,'SCALE':'1','BACKDROP':os.environ.get('PENCIL_MESH_BACKDROP','blue')}
 subprocess.run(['/tmp/sdocx-agent-target/debug/pencil2-mesh-note-control'],cwd=root,env=env,check=True)
 subprocess.run(['resvg',str(p/'generic1.svg'),str(p/'render.png')],check=True)
 images=[('SVG resvg',Image.open(p/'render.png').convert('RGB'))]
 with pymupdf.open(p/'generic1.pdf') as d:
  pix=d[0].get_pixmap(alpha=False);images.append(('PDF MuPDF',Image.frombytes('RGB',(pix.width,pix.height),pix.samples)))
 with pypdfium2.PdfDocument(p/'generic1.pdf') as d:images.append(('PDF PDFium',d[0].render().to_pil().convert('RGB')))
 print('bytes SVG/PDF',(p/'generic1.svg').stat().st_size,(p/'generic1.pdf').stat().st_size)
 for label,im in images:
  assert im.size==(192,192);e=[abs(a-b) for a,b in zip(im.tobytes(),target)];print(label,sum(e),max(e),sum(v!=0 for v in e))
