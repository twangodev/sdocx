import hashlib, json, pathlib, subprocess, sys, tempfile, zlib
import pymupdf, pypdfium2
from PIL import Image
p=pathlib.Path(__file__).resolve().parent
out=pathlib.Path(sys.argv[1])
a=zlib.decompress((p/'mixed-scene-order-final.rgba.zlib').read_bytes())
b=zlib.decompress((p/'mixed-scene-order-before.rgba.zlib').read_bytes())
mid=zlib.decompress((p/'mixed-scene-order-line.rgba.zlib').read_bytes())
assert hashlib.sha256(mid).hexdigest()=='5dcb9735e677eafac26f0c4a03c1fab2e2d0f29b602ea98fe2f2da7ccac5bf8a'
assert len(a)==len(b)==len(mid)==64*48*4
assert hashlib.sha256(a).hexdigest()=='77fb3c431261e44d9608c744fe2a09caf0267bb80dda26bd40725d721b7d2234'
assert hashlib.sha256(b).hexdigest()=='ef2d94b172caedab5d4047969a2e74fc71c95a8351e718c1c62cf6d22020576a'
assert subprocess.check_output(['resvg','--version'],text=True).strip() in ('resvg 0.45.1','0.45.1')
assert pymupdf.VersionBind=='1.28.2' and str(pypdfium2.PYPDFIUM_INFO)=='5.9.0' and str(pypdfium2.PDFIUM_INFO)=='150.0.7869.0'
result={}
with tempfile.TemporaryDirectory() as t:
 for variant in ('ordinary','candidate'):
  svg=out/(variant+'.svg'); pdf=out/(variant+'.pdf')
  subprocess.run(['resvg','--width','64','--height','48',str(svg),t+'/image.png'],check=True)
  images={'resvg':Image.open(t+'/image.png').convert('RGB').tobytes(),'mupdf':pymupdf.open(pdf)[0].get_pixmap(matrix=pymupdf.Matrix(2/3,2/3),alpha=False).samples,'pdfium':pypdfium2.PdfDocument(pdf)[0].render(scale=2/3).to_pil().convert('RGB').tobytes()}
  result[variant]={'svg_sha256':hashlib.sha256(svg.read_bytes()).hexdigest(),'pdf_sha256':hashlib.sha256(pdf.read_bytes()).hexdigest(),'metrics':{}}
  assert all(len(data)==64*48*3 for data in images.values())
  for orientation in ('direct','flipped'):
   rows=range(48) if orientation=='direct' else reversed(range(48))
   indices=[(y*64+x)*4 for y in rows for x in range(64)]
   expected=bytes(min(255,a[i+c]+(252*(255-a[i+3])+127)//255) for i in indices for c in range(3))
   ink=[j for j,i in enumerate(indices) if b[i+3] and a[i:i+4]==b[i:i+4]]
   overlap=[j for j,i in enumerate(indices) if b[i:i+4]!=mid[i:i+4] and mid[i:i+4]!=a[i:i+4]]
   result[variant]['metrics'][orientation]={k:{'whole':sum(abs(x-y) for x,y in zip(data,expected)),'ink_only':sum(abs(data[3*i+c]-expected[3*i+c]) for i in ink for c in range(3)),'ink_pixels':len(ink),'overlap':sum(abs(data[3*i+c]-expected[3*i+c]) for i in overlap for c in range(3)),'overlap_pixels':len(overlap)} for k,data in images.items()}
print(json.dumps(result,indent=2))
