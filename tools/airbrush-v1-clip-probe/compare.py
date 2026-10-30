import json,base64,zlib,hashlib
import PIL
from pathlib import Path
import pymupdf,pypdfium2
from PIL import Image
root=Path(__file__).resolve().parents[2]
assert PIL.__version__ == '12.1.1'
assert pymupdf.VersionBind == '1.28.2'
assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
p1=root/'tools/pen-capture/airbrush_v1_tool1_receipt.json'; p2=root/'tools/airbrush-v1-tool1-scale2/receipt.json'
assert hashlib.sha256(p1.read_bytes()).hexdigest() == '4802a26da96ddf3e2f9a6656aff5becae7dea7ab8ac19bfdc03585dbd308699b'
assert hashlib.sha256(p2.read_bytes()).hexdigest() == 'dc7adeedcd4ab73070e7048189b4af01456eec4e8ff881811876b91bc347869c'
for filename, digest in [('scale2.patch','68d126012c19f7a2ff9aabbbe5e3b2dd82f3108c823e42e25ebe90d1ced93d75'),('clip.patch','21220e6aa2d154998d871e8999ed017acf34c1cd8428d5d3e1be8454e980de71')]:
 assert hashlib.sha256((Path(__file__).parent/filename).read_bytes()).hexdigest() == digest
r=json.loads(p1.read_text());r2=json.loads(p2.read_text())
vector=bytes.fromhex(r2['producer_events'][-1]['after_bytes'])
assert Path('/tmp/airbrush2x-vector.bin').read_bytes() == vector
assert hashlib.sha256(vector).hexdigest() == '2de8630de46ffcbb6ff5a20702dd53ddda83c836adac1da258bbb3381c673a53'
outputs={'zigzag':('f7b6fa0a112298634b58f5a1bf01df558498347a7662e8027d70fa5e65605497','97b07280453c124b7baeec6cc99686ff343b1ea7123db8a1e2712f74a28c5b23'),'reverse':('07728df59bcda19b42f5f6f882a7733a54ac34d5012032b2d0ff6e516eb5c1c6','bafe2cb811f9b4f97eaf15dc199dba0fa0c321c35f44e060db222c2a39d8a971'),'scale2':('88a8c3ac7b629ee22ab4e0d4933b8b5c1a1dc570e6722d2bbed69a0d23684c19','d12d7072ed8fd331baed69d30b709e35b282ff45870ae42a66d3f6c4c1dcd0d1')}
expected={'zigzag':(541,7393,2490,4305),'reverse':(259,2429,826,1325),'scale2':(579,6166,2241,3543)}
def err(a,b):return sum(abs(x-y) for x,y in zip(a,b,strict=True))
for name,frame,n,pdf,png in [('zigzag',r['cases']['zigzag-1']['frame'],256,'/tmp/airclip-pdfs/zigzag-1-candidate.pdf','/tmp/airclip-zigzag-candidate.png'),('reverse',r['cases']['reverse-1']['frame'],256,'/tmp/airclip-pdfs/reverse-1-candidate.pdf','/tmp/airclip-reverse-candidate.png'),('scale2',r2['frame'],512,'/tmp/airbrush2x-candidate.pdf','/tmp/airbrush2x-candidate.png')]:
 assert tuple(hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in (pdf,png)) == outputs[name]
 raw=zlib.decompress(base64.b64decode(frame['data_base64'])); assert hashlib.sha256(raw).hexdigest()==frame['sha256'];native=b''.join(reversed([raw[i:i+n*4] for i in range(0,len(raw),n*4)])); im=Image.open(png).convert('RGBA'); q=bytearray(im.tobytes());
 for i in range(0,len(q),4):
  for c in range(3):q[i+c]=(q[i+c]*q[i+3]+127)//255
 with pymupdf.open(pdf) as d: m=err(d[0].get_pixmap(matrix=pymupdf.Matrix(4/3,4/3),alpha=True).samples,native)
 with pypdfium2.PdfDocument(pdf) as d:
  a=d[0].render(scale=4/3,rev_byteorder=True,fill_color=(0,0,0,0)).to_pil().convert('RGBA'); w=d[0].render(scale=4/3,rev_byteorder=True,fill_color=(255,255,255,255)).to_pil().convert('RGB'); target=bytes(min(255,native[i+c]+255-native[i+3]) for i in range(0,len(native),4) for c in range(3)); alpha=err(a.tobytes()[3::4],native[3::4]);white=err(w.tobytes(),target)
 result=(err(q,native),m,alpha,white); assert result == expected[name], (name,result,expected[name]); print(name,'svg, MuPDF, PDFium alpha/white:',result)
