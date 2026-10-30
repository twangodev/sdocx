import hashlib
import os
import tempfile
import base64,io,subprocess
from pathlib import Path
import pymupdf as f
import pypdfium2 as pdfium
from PIL import Image
p=Path(os.environ['PENCIL2_PROBE_DIR']); temp=tempfile.TemporaryDirectory(prefix='pencil2-export-'); out=Path(temp.name); w=h=192
for scale,case in [(1,'baseline'),(2,'scale2')]:
 raw=(p/f'layers{scale}.raw').read_bytes(); layers=[Image.frombytes('RGBA',(w,h),raw[i:i+w*h*4]) for i in range(0,len(raw),w*h*4)]
 native=bytearray(w*h); crop=Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes(); assert hashlib.sha256(crop).hexdigest() == {'baseline':'fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b','scale2':'e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4'}[case]
 for y in range(22): native[(170+y)*w:(170+y)*w+62]=crop[y*62:(y+1)*62]
 svg=['<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192">']; doc=f.open(); page=doc.new_page(width=w,height=h)
 for layer in layers:
  box=layer.getbbox()
  if not box: continue
  im=layer.crop(box); b=io.BytesIO(); im.save(b,format='PNG'); png=b.getvalue()
  svg.append(f'<image x="{box[0]}" y="{box[1]}" width="{box[2]-box[0]}" height="{box[3]-box[1]}" href="data:image/png;base64,{base64.b64encode(png).decode()}"/>')
  page.insert_image(f.Rect(*box),stream=png)
 svg.append('</svg>'); (out/'test.svg').write_text(''.join(svg)); doc.save(out/'test.pdf');doc.close()
 subprocess.run(['resvg',str(out/'test.svg'),str(out/'test.png')],check=True)
 svg_alpha=Image.open(out/'test.png').convert('RGBA').tobytes()[3::4]
 with f.open(out/'test.pdf') as d: mu=d[0].get_pixmap(alpha=True).samples[3::4]
 with pdfium.PdfDocument(out/'test.pdf') as d: pi=d[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).to_pil().convert('RGBA').tobytes()[3::4]
 for name,actual in [('svg',svg_alpha),('mu',mu),('pdfium',pi)]:
  error=[abs(a-b) for a,b in zip(actual,native)];print(case,name,'sum',sum(error),'max',max(error),'changed',sum(bool(x) for x in error))
