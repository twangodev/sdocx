import hashlib,json,collections,os
from pathlib import Path
import PIL
from PIL import Image
import pymupdf,pypdfium2
D=Path(os.environ['SDOCX_VECTOR_TMP'])
def top(b):return b''.join(b[y*512:(y+1)*512] for y in reversed(range(256)))
before=top((D/'native-before.rgba').read_bytes());native=top((D/'native-after.rgba').read_bytes())
def hash(b):return hashlib.sha256(b).hexdigest()
def stats(actual):
 assert len(actual)==len(native);bg=collections.Counter(tuple(actual[i:i+4]) for i in range(0,len(actual),4)).most_common(1)[0][0]; rows={'ink_union':{'pixels':0,'different_pixels':0,'absolute_channel_error':0,'max_channel_error':0},'background_only':{'pixels':0,'different_pixels':0,'absolute_channel_error':0,'max_channel_error':0}}
 for i in range(0,len(actual),4):
  n=native[i:i+4];a=actual[i:i+4];category='ink_union' if n!=before[i:i+4] or tuple(a)!=bg else 'background_only';d=[abs(x-y) for x,y in zip(n,a)];r=rows[category];r['pixels']+=1;r['different_pixels']+=int(n!=a);r['absolute_channel_error']+=sum(d);r['max_channel_error']=max(r['max_channel_error'],*d)
 return {'sha256':hash(actual),'dominant_background_rgba':bg,**rows}
def premul(b):return bytes((v*b[i-i%4+3]+127)//255 if i%4!=3 else v for i,v in enumerate(b))
box=(80,145,208,401);im=Image.open(D/'selected.png').convert('RGBA').crop(box);results={'svg_resvg':stats(premul(im.tobytes()))}
for route in ('svg','direct'):
 path=D/f'selected.{route}.pdf'
 with pymupdf.open(path) as doc:
  pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=True,colorspace=pymupdf.csRGB);assert (pix.width,pix.height,pix.n)==(208,401,4)
  results[route+'_pdf_mupdf']=stats(Image.frombytes('RGBA',(208,401),pix.samples).crop(box).tobytes())
 with pypdfium2.PdfDocument(str(path)) as doc:
  page=doc[0];bitmap=page.render(scale=4/3,rev_byteorder=True,fill_color=(0,0,0,0));im=bitmap.to_pil().convert('RGBA');assert im.size==(208,401);results[route+'_pdf_pdfium']=stats(premul(im.crop(box).tobytes()));bitmap.close();page.close()
print(json.dumps({'native_topdown_sha256':hash(native),'baseline_topdown_sha256':hash(before),'readers':{'pillow':PIL.__version__,'mupdf':pymupdf.VersionBind,'pdfium':str(pypdfium2.PDFIUM_INFO)},'results':results},indent=2))
