"""Compare an analytic width atlas with a radial gradient at baseline scale."""
import base64,re,subprocess,tempfile,math,struct,zlib,gzip,os
from pathlib import Path
import pymupdf,pypdfium2
src=Path('tools/pencil-capture/pencil2-width-atlas')
s=gzip.decompress((src/'prototype.svg.gz').read_bytes()).decode()
for name in set(re.findall(r'(?:href|xlink:href)="([^"]+\.png)"',s)):
 s=s.replace(name,'data:image/png;base64,'+base64.b64encode((src/name).read_bytes()).decode())
old='<rect width="1" height="1" fill="url(#widthg)" shape-rendering="crispEdges"/>'
assert s.count(old)==1
def atlas(n):
 def chunk(t,b): return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b)&0xffffffff)
 raw=b''.join(b'\0'+b''.join(bytes((255,255,255,round(255*max(0,1-2*math.hypot((x+.5)/n-.5,(y+.5)/n-.5))))) for x in range(n)) for y in range(n))
 png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',n,n,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw,9))+chunk(b'IEND',b'')
 return '<image width="1" height="1" preserveAspectRatio="none" href="data:image/png;base64,'+base64.b64encode(png).decode()+'"/>'
with tempfile.TemporaryDirectory() as d:
 p=Path(d)
 for case in ('baseline',):
  crop=Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes();ref=bytearray(192*192)
  for y in range(22): ref[(170+y)*192:(170+y)*192+62]=crop[y*62:(y+1)*62]
  for center in (0,16):
   data=s if center==0 else s.replace(old,atlas(center))
   (p/'in.svg').write_text(data);subprocess.run([os.environ['PENCIL2_ATLAS_EXPORTER'],str(p/'in.svg'),str(p/'out.pdf')],check=True)
   a=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples[3::4]
   b=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).buffer)[3::4]
   subprocess.run(['resvg',str(p/'in.svg'),str(p/'out.png')],check=True); svg=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/'out.png'),'-f','rawvideo','-pix_fmt','rgba','-'])[3::4]
   for name,z in [('svg',svg),('mupdf',a),('pdfium',b)]:
    assert len(ref)==len(z)==192*192, (name, len(ref), len(z))
    e=[abs(x-y) for x,y in zip(ref,z)];print(center,name,'sum',sum(e),'max',max(e),'support',sum(x>0 for x in z),'count',sum(x>0 for x in e),flush=True)
