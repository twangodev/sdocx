import pypdfium2, sys, hashlib, json
from pathlib import Path
print("pdfium",pypdfium2.PDFIUM_INFO)
b=Path(sys.argv[1])
j=json.loads((b / "native.json").read_text())
h=j["native_draw_target_readback"]["sha256"]
native=(b / "captures" / ("simplebrush-native-draw-rgba-" + h + ".bin")).read_bytes()
assert hashlib.sha256(native).hexdigest() == h
def raster(p):
 d=pypdfium2.PdfDocument(str(p)); return d[0].render(scale=8/3,rev_byteorder=True).to_pil().convert('RGB')
paper=raster(b/'paper.pdf')
for m in ('generic','one','two'):
 image=raster(b/f'{m}.pdf'); err=0
 for y in range(256):
  for x in range(256):
   i=(y*256+x)*4; a=native[i+3]; px=image.getpixel((x,511-y)); bg=paper.getpixel((x,511-y))
   err+=sum(abs(px[k]-(native[i+k]+(bg[k]*(255-a)+127)//255)) for k in range(3))
 print(m,err)
