import hashlib
import os
import tempfile
from pathlib import Path
import subprocess
import pymupdf as f
import pypdfium2 as pdfium
from PIL import Image
p=Path(os.environ['PENCIL2_PROBE_DIR']);temp=tempfile.TemporaryDirectory(prefix='pencil2-export-');out=Path(temp.name);w=h=192
for scale,case in [(1,'baseline'),(2,'scale2')]:
 native=bytearray(w*h);crop=Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes(); assert hashlib.sha256(crop).hexdigest() == {'baseline':'fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b','scale2':'e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4'}[case]
 for y in range(22):native[(170+y)*w:(170+y)*w+62]=crop[y*62:(y+1)*62]
 s=(p/'generic.svg').read_text()
 if scale==2: s=s.replace('<line ', '<g transform="matrix(2 0 0 2 0 -192)"><line ',1).replace('</svg>','</g></svg>')
 (out/'g.svg').write_text(s); subprocess.run(['resvg',str(out/'g.svg'),str(out/'g.png')],check=True)
 if scale==2:
  d=f.open();pg=d.new_page(width=192,height=192);src=f.open(p/'generic.pdf');pg.show_pdf_page(f.Rect(0,-192,384,192),src,0);d.save(out/'g.pdf');d.close();src.close()
 else: (out/'g.pdf').write_bytes((p/'generic.pdf').read_bytes())
 svg=Image.open(out/'g.png').convert('RGB').tobytes()[::3]
 with f.open(out/'g.pdf') as d: mu=d[0].get_pixmap(alpha=False,colorspace=f.csRGB).samples[::3]
 with pdfium.PdfDocument(out/'g.pdf') as d: pi=d[0].render(scale=1,rev_byteorder=True,fill_color=(255,255,255,255)).to_pil().convert('RGB').tobytes()[::3]
 for name,actual in [('svg',svg),('mu',mu),('pdfium',pi)]:
  e=[abs(255-a-b) for a,b in zip(actual,native)]; print(case,name,'sum',sum(e),'max',max(e),'changed',sum(bool(x) for x in e))
