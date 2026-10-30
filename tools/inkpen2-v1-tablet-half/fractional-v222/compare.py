import json,zlib,hashlib,subprocess,tempfile,sys
from pathlib import Path
if sys.flags.optimize:
 raise SystemExit("Optimized Python disables required integrity checks")
import pymupdf,pypdfium2
from PIL import Image, __version__ as pillow_version
here=Path(__file__).resolve().parent
r=json.loads((here/'receipt.json').read_text()); a=zlib.decompress((here/'frame.rgba.zlib').read_bytes()); w,h=63,47
assert len(a)==w*h*4 and hashlib.sha256(a).hexdigest()==r['capture']['rgba_sha256']
versions={'resvg':subprocess.check_output(['resvg','--version'],text=True).strip().removeprefix('resvg '),'pymupdf':pymupdf.VersionBind,'pypdfium2':str(pypdfium2.PYPDFIUM_INFO),'pdfium':str(pypdfium2.PDFIUM_INFO),'pillow':pillow_version}
assert versions == r['readers'], (versions, r['readers'])
assert r['capture']['output_size']==[w,h]
out=Path(sys.argv[1]); result={}
with tempfile.TemporaryDirectory() as t:
 for v in ('candidate','ordinary'):
  svg=out/(v+'.svg'); pdf=out/(v+'.pdf')
  subprocess.run(['resvg','--width',str(w),'--height',str(h),str(svg),t+'/a.png'],check=True)
  svg_image=Image.open(t+'/a.png').convert('RGB')
  pixmap=pymupdf.open(pdf)[0].get_pixmap(matrix=pymupdf.Matrix(2/3,2/3),alpha=False)
  pdfium_image=pypdfium2.PdfDocument(pdf)[0].render(scale=2/3).to_pil().convert('RGB')
  assert svg_image.size==(w,h) and (pixmap.width,pixmap.height)==(w,h) and pdfium_image.size==(w,h)
  images={'resvg':svg_image.tobytes(),'mupdf':pixmap.samples,'pdfium':pdfium_image.tobytes()}
  assert all(len(x)==w*h*3 for x in images.values())
  result[v]={'svg_sha256':hashlib.sha256(svg.read_bytes()).hexdigest(),'pdf_sha256':hashlib.sha256(pdf.read_bytes()).hexdigest(),'metrics':{}}
  for orientation in ('direct','flipped'):
   rows=range(h) if orientation=='direct' else reversed(range(h)); indices=[(y*w+x)*4 for y in rows for x in range(w)]
   expected=bytes(min(255,a[i+c]+(252*(255-a[i+3])+127)//255) for i in indices for c in range(3))
   ink=[j for j,i in enumerate(indices) if a[i+3]]
   result[v]['metrics'][orientation]={k:{'whole_rgb_l1':sum(abs(x-y) for x,y in zip(data,expected)),'ink_rgb_l1':sum(abs(data[3*i+c]-expected[3*i+c]) for i in ink for c in range(3)),'ink_pixels':len(ink)} for k,data in images.items()}
assert result == r['comparisons'], 'Export hashes or pixel measurements differ from receipt'
print(json.dumps(result,indent=2))
