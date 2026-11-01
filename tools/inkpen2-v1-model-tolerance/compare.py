"""Compare public SVG/direct PDF outputs to retained host native frames."""
import subprocess, zlib
from pathlib import Path
import pymupdf, pypdfium2
from PIL import Image
root=Path(__file__).resolve().parents[2]; tmp=Path('/tmp/inkpen-tolerance-probe')
expected={'note29':{'resvg':(2032,52162),'mupdf':(13492,61664),'pdfium':(6043,53415)},'scene7':{'resvg':(5191,167171),'mupdf':(23177,177863),'pdfium':(15965,168960)},'tool3':{'resvg':(5143,144152),'mupdf':(49253,178507),'pdfium':(17261,147042)}}
for case, prefix, scale in [('note29','',1),('scene7','scene7-',1),('tool3','tool3-',2)]:
 size=64*scale; data=zlib.decompress((root/'tools/inkpen2-v1-model-tolerance'/f'{case}.rgba.zlib').read_bytes()); native=Image.frombytes('RGBA',(size,size),data); ref=Image.new('RGBA',(size,size),(252,252,252,255));ref.alpha_composite(native); ref=ref.convert('RGB').tobytes()
 for reader in ('resvg','mupdf','pdfium'):
  sums=[]
  for name in ('positive-profile','positive-fallback'):
   src=tmp/(prefix+name)
   if reader=='resvg':
    png=tmp/'comparison.png';subprocess.run(['resvg','-z',str(scale),str(src.with_suffix('.svg')),str(png)],check=True); image=Image.open(png).convert('RGB')
   elif reader=='mupdf':
    doc=pymupdf.open(str(src.with_suffix('.pdf'))); pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(4*scale/3,4*scale/3),alpha=False);image=Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
   else:
    doc=pypdfium2.PdfDocument(str(src.with_suffix('.pdf')));image=doc[0].render(scale=4*scale/3).to_pil().convert('RGB')
   assert image.size==(size,size);sums.append(sum(abs(a-b) for a,b in zip(ref,image.tobytes())))
  print(case,reader,*sums);assert tuple(sums)==expected[case][reader]
