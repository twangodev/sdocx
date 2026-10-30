import sys,json,base64,zlib,hashlib,subprocess
from pathlib import Path
sys.path.insert(0,'tools/montblanc')
from public_fallback_compare import composite_native_frame,metrics
from PIL import Image
import pymupdf,pypdfium2
cap=json.loads(Path('/tmp/sdocx-montblanc-25.json').read_text()); frame=zlib.decompress(base64.b64decode(cap['post_readback']['rgba8_zlib_base64'])); assert hashlib.sha256(frame).hexdigest()==cap['post_readback']['sha256']; native=composite_native_frame(frame,(256,256)).tobytes(); p=Path('/tmp/sdocx-mb25-run/output')
for mode in ('candidate','fallback'):
 base=p/f'twentyfive-1.0-{mode}'; svg=Path(str(base)+'.svg'); pdf=Path(str(base)+'.pdf'); png=p/'temp.png'; subprocess.run(['resvg','-w','256',str(svg),str(png)],check=True); out=Image.open(png).convert('RGB').tobytes()
 with pymupdf.open(pdf) as d: pm=d[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=False); mu=pm.samples
 with pypdfium2.PdfDocument(pdf) as d: page=d[0]; bm=page.render(scale=4/3,rev_byteorder=True); fi=bm.to_pil().convert('RGB').tobytes(); bm.close(); page.close()
 print(mode,'bytes',svg.stat().st_size,pdf.stat().st_size,'svg',metrics(native,out),'mupdf',metrics(native,mu),'pdfium',metrics(native,fi))
png.unlink()
