import json, math, subprocess
from pathlib import Path
from PIL import Image
rows=json.loads(Path('conformance/marker4-v7-finger-native.json').read_text())['cases']; dots=next(x for x in rows if x['name']=='turn-9.5')['dots']; meta=json.loads(Path('/tmp/m4v7-stroke30.json').read_text()); cx,cy=meta['frame_center_doc']; native=Image.frombytes('RGBA',(1024,512),Path('/tmp/m4v7-stroke30.rgba').read_bytes()).transpose(Image.Transpose.FLIP_TOP_BOTTOM).getchannel('A').tobytes()
w=.77*10.5*.99;h=10.5*.99;rx=w*25/99;ry=h*25/99;a=dots[0][3];c=math.cos(a);s=math.sin(a);chunks=[]
for x,y,_,_ in dots:
 xx=x*c+y*s-w/2; yy=-x*s+y*c-h/2
 chunks.append(f'M{xx+rx:.4f},{yy:.4f} h{w-2*rx:.4f} a{rx:.4f},{ry:.4f},0,0,1,{rx:.4f},{ry:.4f} v{h-2*ry:.4f} a{rx:.4f},{ry:.4f},0,0,1,{-rx:.4f},{ry:.4f} h{2*rx-w:.4f} a{rx:.4f},{ry:.4f},0,0,1,{-rx:.4f},{-ry:.4f} v{2*ry-h:.4f} a{rx:.4f},{ry:.4f},0,0,1,{rx:.4f},{-ry:.4f} z')
svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="512" viewBox="{cx-512} {cy-256} 1024 512"><path d="{" ".join(chunks)}" fill="black" fill-opacity="0.5019607843" transform="rotate({math.degrees(a)})"/></svg>'
Path('/tmp/m4v7.svg').write_text(svg);subprocess.run(['resvg','/tmp/m4v7.svg','/tmp/m4v7.png'],check=True)
candidate=Image.open('/tmp/m4v7.png').convert('RGBA').getchannel('A').tobytes(); print('native/candidate support',sum(x>0 for x in native),sum(x>0 for x in candidate),'alpha l1',sum(abs(a-b) for a,b in zip(native,candidate)))
import pymupdf
source = pymupdf.open('/tmp/m4v7.svg')
pdf = pymupdf.open('pdf', source.convert_to_pdf())
pixmap = pdf[0].get_pixmap(alpha=True)
pdf_alpha = Image.frombytes('RGBA', (pixmap.width, pixmap.height), pixmap.samples).getchannel('A').tobytes()
print('MuPDF support', sum(x > 0 for x in pdf_alpha), 'alpha L1', sum(abs(a - b) for a, b in zip(native, pdf_alpha)))
