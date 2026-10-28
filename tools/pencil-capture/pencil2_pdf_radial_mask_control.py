"""Render a vector radial alpha mask at multiple sizes in three readers.

Requires the same runner and Python dependencies as pencil2_v14_svg_pdf_control.py.
"""
import tempfile,subprocess
from pathlib import Path
import pymupdf,pypdfium2
h='''<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192"><defs><radialGradient id="g"><stop stop-color="white"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient><mask id="m" maskContentUnits="objectBoundingBox" mask-type="alpha"><rect width="1" height="1" fill="url(#g)"/></mask></defs>{}</svg>'''
with tempfile.TemporaryDirectory() as d:
 p=Path(d)
 for sz in [2,8,32,64]:
  for pos in [40,40.3]:
   s=h.format(f'<rect x="{pos}" y="{pos}" width="{sz}" height="{sz}" fill="black" mask="url(#m)"/>');(p/'in.svg').write_text(s);subprocess.run(['/tmp/sdocx-agent-target/debug/sdocx-svg-pdf-brush',str(p/'in.svg'),str(p/'out.pdf')],check=True);subprocess.run(['resvg',str(p/'in.svg'),str(p/'out.png')],check=True)
   a=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/'out.png'),'-f','rawvideo','-pix_fmt','rgba','-'])[3::4]; m=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples[3::4]; q=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).buffer)[3::4]
   if sz==2: print("pixels",[(x,y,a[y*192+x],m[y*192+x],q[y*192+x]) for y in range(39,44) for x in range(39,44) if a[y*192+x] or m[y*192+x] or q[y*192+x]])
   def stat(x):
    e=[abs(c-d) for c,d in zip(a,x)];return (sum(c>0 for c in x),sum(e),max(e))
   print(sz,pos,'resvg',sum(c>0 for c in a),'mupdf',stat(m),'pdfium',stat(q),flush=True)
