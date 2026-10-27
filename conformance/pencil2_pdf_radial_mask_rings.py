import tempfile,subprocess
from pathlib import Path
import pymupdf,pypdfium2
h='''<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192"><defs><radialGradient id="g"><stop stop-color="white"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient><mask id="m" maskContentUnits="objectBoundingBox" mask-type="{}">{}</mask></defs><rect x="{}" y="{}" width="{}" height="{}" fill="black" mask="url(#m)"/></svg>'''
def render(p,s):
 (p/'in.svg').write_text(s);subprocess.run(['/tmp/sdocx-agent-target/debug/sdocx-svg-pdf-brush',str(p/'in.svg'),str(p/'out.pdf')],check=True,stdout=subprocess.DEVNULL);subprocess.run(['resvg',str(p/'in.svg'),str(p/'out.png')],check=True)
 a=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/'out.png'),'-f','rawvideo','-pix_fmt','rgba','-'])[3::4];m=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples[3::4];q=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).buffer)[3::4];return a,m,q
with tempfile.TemporaryDirectory() as d:
 p=Path(d)
 for sz in [2,8,32,64]:
  for pos in [40,40.3]:
   base=render(p,h.format('alpha','<rect width="1" height="1" fill="url(#g)"/>',pos,pos,sz,sz))[0]
   for n in [4,16,64]:
    circles=''.join(f'<circle cx=".5" cy=".5" r="{i/(2*n)}" fill="rgb({round(255*(1-(i-.5)/n))},{round(255*(1-(i-.5)/n))},{round(255*(1-(i-.5)/n))})"/>' for i in range(n,0,-1))
    rs=render(p,h.format('luminance',circles,pos,pos,sz,sz))
    print(sz,pos,n,[(sum(abs(x-y) for x,y in zip(base,z)),max(abs(x-y) for x,y in zip(base,z))) for z in rs],flush=True)
