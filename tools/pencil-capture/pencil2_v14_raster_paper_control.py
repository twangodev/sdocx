import base64, json, struct, subprocess, tempfile, zlib
from pathlib import Path
import pymupdf, pypdfium2
root=Path('tmp/vector-parity-plan/pen-family-audit')
pixels=json.loads(subprocess.check_output(['python3',str(root/'pencil2-v14-full-frame-scalar-model/analyze.py'),'--pixels']))
a=bytearray(192*192)
for x,y,v in pixels:a[y*192+x]=v
raw=b''.join(b'\0'+b''.join(bytes((0,0,0,a[y*192+x])) for x in range(192)) for y in range(192))
def chunk(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d)&0xffffffff)
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',192,192,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw,9))+chunk(b'IEND',b'')
uri=base64.b64encode(png).decode()
svg=f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="192" height="192"><rect width="192" height="192" fill="white"/><image width="192" height="192" image-rendering="pixelated" xlink:href="data:image/png;base64,{uri}"/></svg>'
cap=json.loads((root/'pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text());b=bytes.fromhex(cap['mesa']['readback_rgba_hex']); native=bytes(255-b[((191-y)*192+x)*4+3] for y in range(192) for x in range(192))
with tempfile.TemporaryDirectory() as td:
 p=Path(td);(p/'in.svg').write_text(svg);subprocess.run(['/tmp/sdocx-agent-target/debug/sdocx-svg-pdf-brush',str(p/'in.svg'),str(p/'out.pdf')],check=True);subprocess.run(['resvg',str(p/'in.svg'),str(p/'out.png')],check=True)
 rgba=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/'out.png'),'-f','rawvideo','-pix_fmt','rgba','-']); s=rgba[::4];m=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=False,colorspace=pymupdf.csRGB).samples[::3]; q=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True).buffer)[::3]
 for name,v in [('resvg',s),('mupdf',m),('pdfium',q)]:
  err=[abs(x-y) for x,y in zip(v,native)];print(name,len(v),sum(x!=0 for x in err),sum(err),max(err),sum(x!=255 for x in v))
