import hashlib,json,re,subprocess,sys
from pathlib import Path
import numpy as np
from PIL import Image
import pymupdf
import pypdfium2 as pdfium
assert subprocess.check_output(["resvg", "--version"], text=True).strip() == "0.45.1"
assert pymupdf.VersionBind == "1.28.2"
assert str(pdfium.PYPDFIUM_INFO) == "5.14.0"
assert str(pdfium.PDFIUM_INFO) == "156.0.8076.0"
assert np.__version__ == "2.4.2"
assert __import__("PIL").__version__ == "12.1.1"
native=np.frombuffer(Path(sys.argv[1]).read_bytes(),np.uint8).reshape(512,512,4)
a=native[...,3:4].astype(np.uint16)
paper=np.minimum(native[...,:3].astype(np.uint16)+(252*(255-a)+127)//255,255).astype(np.uint8)
out=Path(sys.argv[2]); rows={}
for mode in ('baseline','profile'):
 s=(out/f'{mode}.svg').read_text();s,n=re.subn(r'<rect fill="#fcfcfc"[^>]*/>','',s,count=1);assert n==1
 (out/'transparent.svg').write_text(s);subprocess.run(['resvg','--width','512',str(out/'transparent.svg'),str(out/'transparent.png')],check=True)
 im=np.asarray(Image.open(out/'transparent.png').convert('RGBA')).astype(np.uint16); rgba=np.concatenate(((im[...,:3]*im[...,3:4]+127)//255,im[...,3:4]),axis=2).astype(np.uint8)
 doc=pymupdf.open(out/f'{mode}.pdf');pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(16/3,16/3),alpha=False); mu=np.frombuffer(pix.samples,np.uint8).reshape(512,512,3)
 pi=np.asarray(pdfium.PdfDocument(out/f'{mode}.pdf')[0].render(scale=16/3).to_pil().convert('RGB'))
 def err(a,b):return int(np.abs(a.astype(np.int16)-b.astype(np.int16)).sum())
 rows[mode]={'svg_rgba_l1':err(native,rgba),'mupdf_rgb_l1':err(paper,mu),'pdfium_rgb_l1':err(paper,pi),'native_missing':int(((native[...,3]>0)&(rgba[...,3]==0)).sum())}
print(json.dumps(rows,indent=2))
assert rows["profile"]["native_missing"] == 0
for key in ("svg_rgba_l1", "mupdf_rgb_l1", "pdfium_rgb_l1"):
 assert rows["profile"][key] < rows["baseline"][key], key

Path(sys.argv[3]).write_text(json.dumps({'scope':'Synthetic native Drawing tool-1 vector re-fed to RTV1, host Mesa RGBA8, 4x, 512x512 full frame; phase is recorded by the enclosing case receipt. Public SVG and direct PDF compared to native transparent frame and mathematical #fcfcfc composition; not saved-note, device, opaque native backdrop, or general transform parity.','versions':{'resvg':subprocess.check_output(['resvg','--version'],text=True).strip(),'pymupdf':pymupdf.VersionBind,'pypdfium2':str(pdfium.PYPDFIUM_INFO),'pdfium':str(pdfium.PDFIUM_INFO),'numpy':np.__version__,'pillow':__import__('PIL').__version__},'export_sha256':{f'{mode}.{ext}':hashlib.sha256((out/f'{mode}.{ext}').read_bytes()).hexdigest() for mode in ('baseline','profile') for ext in ('svg','pdf')},'frame_sha256':hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest(),'results':rows},indent=2)+'\n')
