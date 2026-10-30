"""Compare an analytic width atlas across retained host scale controls."""
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
 for case in ('scalehalf','scale4','translate3'):
  ref=zlib.decompress((src/f'{case}.alpha.zlib').read_bytes())
  assert len(ref)==192*192, (case, len(ref))
  for center in (0,16,64):
   data=s if center==0 else s.replace(old,atlas(center))
   if case=='translate3':
    data=data.replace('<g transform="translate(0 192) scale(1 -1)">','<g transform="translate(-3 192) scale(1 -1)">',1)
    data=data.replace('</defs>','</defs><g transform="translate(3 0)">',1).replace('</svg>','</g></svg>')
   elif case in ('scalehalf','scale4'):
    scale=0.5 if case=='scalehalf' else 4
    data=data.replace('</defs>',f'</defs><g transform="matrix({scale} 0 0 {scale} 0 {192*(1-scale)})">',1).replace('</svg>','</g></svg>')
   (p/'in.svg').write_text(data);subprocess.run([os.environ['PENCIL2_ATLAS_EXPORTER'],str(p/'in.svg'),str(p/'out.pdf')],check=True)
   a=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples[3::4]
   b=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).buffer)[3::4]
   assert len(ref)==len(a)==len(b)==192*192, (case, len(ref), len(a), len(b))
   print(case,center,*(sum(abs(x-y) for x,y in zip(ref,z)) for z in (a,b)),'sizes',len(data.encode()),(p/'out.pdf').stat().st_size,flush=True)
