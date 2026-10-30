import json,base64,zlib,hashlib,io
import pymupdf,pypdfium2
import PIL
from PIL import Image
assert pymupdf.VersionBind == "1.28.2"
assert str(pypdfium2.PDFIUM_INFO) == "150.0.7869.0"
assert PIL.__version__ == "12.1.1"
from pathlib import Path
receipt=json.loads((Path(__file__).parent / 'receipt.json').read_text()); frame=receipt['frame']; capture=receipt; raw=zlib.decompress(base64.b64decode(frame['data_base64'])); assert hashlib.sha256(raw).hexdigest()==frame['sha256']; native=b''.join(reversed([raw[i:i+2048] for i in range(0,len(raw),2048)])); v=bytes.fromhex(capture['producer_events'][-1]['after_bytes']); actual=open('/tmp/airbrush2x-vector.bin','rb').read(); assert v == actual; print('geometry',len(v),len(actual),v==actual)
def err(a,b): return sum(abs(x-y) for x,y in zip(a,b,strict=True))
def premul(im):
 b=bytearray(im.convert('RGBA').tobytes())
 for i in range(0,len(b),4):
  for c in range(3):b[i+c]=(b[i+c]*b[i+3]+127)//255
 return bytes(b)
def measure(pdf):
 with pymupdf.open(stream=pdf,filetype='pdf') as d:
  p=d[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=True); assert (p.width,p.height)==(512,512);m=err(p.samples,native)
 with pypdfium2.PdfDocument(pdf) as d:
  p=d[0].render(scale=4/3,rev_byteorder=True,fill_color=(0,0,0,0)).to_pil().convert('RGBA'); alpha=err(p.tobytes()[3::4],native[3::4]);p=d[0].render(scale=4/3,rev_byteorder=True,fill_color=(255,255,255,255)).to_pil().convert('RGB'); target=bytes(min(255,native[i+c]+255-native[i+3]) for i in range(0,len(native),4) for c in range(3)); white=err(p.tobytes(),target)
 return m,alpha,white
expected={'candidate':(0,(5421,2216,3646)),'generic':(8718,(8977,3788,12219))}
for name in ('candidate','generic'):
 png=f'/tmp/airbrush2x-{name}.png'; result=(err(premul(Image.open(png)),native),measure(open(f'/tmp/airbrush2x-{name}.pdf','rb').read())); assert result == expected[name], (name,result); print(name,'svg rgba and pdf',result)
 if name=='candidate':
  d=pymupdf.open();p=d.new_page(width=384,height=384);p.insert_image(p.rect,filename=png);pdf=d.tobytes();d.close(); transport=measure(pdf); assert transport == (377,0,297), transport; print('source PNG transport',transport)
print('native frame',frame['sha256'],'alpha sum',sum(native[3::4]))
