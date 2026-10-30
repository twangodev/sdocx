import hashlib,json,os,zlib
from pathlib import Path
from PIL import Image, __version__
import pymupdf,pypdfium2
assert (__version__, pymupdf.VersionBind, pypdfium2.PDFIUM_INFO.version) == ('12.1.1','1.28.2','156.0.8076.0')
work=Path(os.environ['PENCIL200_DIR']); base=Path('tools/pencil2-mixed-scene-controls'); out={}
for case in ('white:1','white:2','transparent:1','blue:2'):
 name=case.replace(':','-'); receipt=json.loads((base/f'{name}-receipt.json').read_text()); rows=[]
 for i,label in enumerate(('first','second')):
  raw=zlib.decompress((base/f'{name}-{label}.zlib').read_bytes()); assert hashlib.sha256(raw).hexdigest()==receipt[f'{label}_sha256']; ref=Image.frombytes('RGBA',(192,192),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).tobytes(); path=work/case/f'direct{i}.pdf'; assert path.stat().st_size<8*1024*1024
  with pymupdf.open(path) as d: pix=d[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB); assert (pix.width,pix.height)==(192,192); mupdf=pix.samples
  with pypdfium2.PdfDocument(path) as d: im=d[0].render(fill_color=(0,0,0,0),rev_byteorder=True).to_pil().convert('RGBA'); assert im.size==(192,192); pdfium=bytearray(im.tobytes())
  for j in range(0,len(pdfium),4):
   for k in range(3): pdfium[j+k]=(pdfium[j+k]*pdfium[j+3]+127)//255
  vals=[]
  for pixels in (mupdf,pdfium):
   assert len(pixels)==len(ref); e=[abs(a-b) for a,b in zip(pixels,ref)]; vals.append([sum(e),max(e),sum(x!=0 for x in e)])
  rows.append({'mupdf':vals[0],'pdfium':vals[1],'pdf_bytes':path.stat().st_size})
 out[case]=rows
print(json.dumps(out,indent=2))
