import hashlib,json,re,subprocess,sys
from pathlib import Path
import numpy as np
from PIL import Image
import pymupdf
import pypdfium2 as pdfium
import pypdfium2_raw
import shutil
reader_hashes = {Path(shutil.which("resvg")): "3e9eaf857346d730761a7c58412c923859525d5f498f9badc1851abe0f540611", Path(pymupdf.__file__).parent / "libmupdf.so.28.2": "64ea1ff04304a3e90d5ae7fb38de4fb8e4057ff776434e747fe5a62d219b1cfa", Path(pymupdf.__file__).parent / "_mupdf.so": "d16a4ad71a3477f6a11dfbb0ef53c41180ec35a37b50fd39956e61c8344ec15c", Path(pypdfium2_raw.__file__).parent / "libpdfium.so": "fd4de9f91f183476d04b1bc3fd17dacd95aba349c55d2c376512d95e082b441e"}
for path, expected in reader_hashes.items():
 assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
assert subprocess.check_output(["resvg", "--version"], text=True).strip() == "0.45.1"
assert pymupdf.VersionBind == "1.28.2"
case=sys.argv[3]; scale={'phase2':2,'phase4':4,'boundary':1}[case]; n=256 if scale>1 else 128
native=np.frombuffer(Path(sys.argv[1]).read_bytes(),np.uint8).reshape(256,256,4)[:n,:n]
a=native[...,3:4].astype(np.uint16)
paper=np.minimum(native[...,:3].astype(np.uint16)+(252*(255-a)+127)//255,255).astype(np.uint8)
out=Path(sys.argv[2]); rows={}
for mode in ('baseline','profile'):
 s=(out/f'{mode}.svg').read_text();s,count=re.subn(r'<rect fill="#fcfcfc"[^>]*/>','',s,count=1);assert count==1
 (out/'transparent.svg').write_text(s);subprocess.run(['resvg','--width',str(n),str(out/'transparent.svg'),str(out/'transparent.png')],check=True)
 im=np.asarray(Image.open(out/'transparent.png').convert('RGBA')).astype(np.uint16); rgba=np.concatenate(((im[...,:3]*im[...,3:4]+127)//255,im[...,3:4]),axis=2).astype(np.uint8)
 doc=pymupdf.open(out/f'{mode}.pdf');pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(4*scale/3,4*scale/3),alpha=False); mu=np.frombuffer(pix.samples,np.uint8).reshape(n,n,3)
 pi=np.asarray(pdfium.PdfDocument(out/f'{mode}.pdf')[0].render(scale=4*scale/3).to_pil().convert('RGB'))
 def err(a,b):return int(np.abs(a.astype(np.int16)-b.astype(np.int16)).sum())
 rows[mode]={'svg_rgba_l1':err(native,rgba),'mupdf_rgb_l1':err(paper,mu),'pdfium_rgb_l1':err(paper,pi),'native_missing':int(((native[...,3]>0)&(rgba[...,3]==0)).sum())}
print(json.dumps(rows,indent=2))
receipt=json.loads(Path(sys.argv[4]).read_text())
order=sys.argv[5]
assert hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest() == receipt[order+'_sha256']
keys=('svg_rgba_l1','mupdf_rgb_l1','pdfium_rgb_l1','native_missing')
assert receipt['columns'] == ['svg_rgba_l1','mupdf_rgb_l1','pdfium_rgb_l1','svg_missing_support_pixels']
for mode,key in [('baseline','baseline'),('profile','candidate')]:
 assert [rows[mode][k] for k in keys] == receipt[order][key], (order,mode,rows[mode])
assert str(pdfium.PYPDFIUM_INFO) == '5.14.0' and str(pdfium.PDFIUM_INFO) == '156.0.8076.0'
assert np.__version__ == '2.4.2' and __import__('PIL').__version__ == '12.1.1'
