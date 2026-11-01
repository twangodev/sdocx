import hashlib,json,re,subprocess,sys
from pathlib import Path
import numpy as np
from PIL import Image
import pymupdf
import pypdfium2 as pdfium
assert subprocess.check_output(["resvg", "--version"], text=True).strip() == "0.45.1"
assert pymupdf.VersionBind == "1.28.2"
case=sys.argv[3]; scale=int(case[-1]) if case[-1].isdigit() else 2; n=128 if scale==1 else 256
native=np.frombuffer(Path(sys.argv[1]).read_bytes(),np.uint8).reshape(256,256,4)[:n,:n]
a=native[...,3:4].astype(np.uint16)
paper=np.minimum(native[...,:3].astype(np.uint16)+(252*(255-a)+127)//255,255).astype(np.uint8)
out=Path(sys.argv[2]); rows={}
for mode in ('generic','baseline','profile'):
 s=(out/f'{mode}.svg').read_text();s,count=re.subn(r'<rect fill="#fcfcfc"[^>]*/>','',s,count=1);assert count==1
 (out/'transparent.svg').write_text(s);subprocess.run(['resvg','--width',str(n),str(out/'transparent.svg'),str(out/'transparent.png')],check=True)
 im=np.asarray(Image.open(out/'transparent.png').convert('RGBA')).astype(np.uint16); rgba=np.concatenate(((im[...,:3]*im[...,3:4]+127)//255,im[...,3:4]),axis=2).astype(np.uint8)
 doc=pymupdf.open(out/f'{mode}.pdf');pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(4*scale/3,4*scale/3),alpha=False); mu=np.frombuffer(pix.samples,np.uint8).reshape(n,n,3)
 pi=np.asarray(pdfium.PdfDocument(out/f'{mode}.pdf')[0].render(scale=4*scale/3).to_pil().convert('RGB'))
 def err(a,b):return int(np.abs(a.astype(np.int16)-b.astype(np.int16)).sum())
 rows[mode]={'svg_rgba_l1':err(native,rgba),'mupdf_rgb_l1':err(paper,mu),'pdfium_rgb_l1':err(paper,pi),'native_missing':int(((native[...,3]>0)&(rgba[...,3]==0)).sum())}
print(json.dumps(rows,indent=2))

assert str(pdfium.PYPDFIUM_INFO) == '5.14.0' and str(pdfium.PDFIUM_INFO) == '156.0.8076.0'
