import hashlib, os, subprocess, tempfile
from pathlib import Path
import pymupdf as f,pypdfium2 as p
from PIL import Image
P=Path(os.environ['PENCIL2_PROBE_DIR']); w=h=192
for scale,case in [(1,'baseline'),(2,'scale2')]:
 raw=(P/f'layers{scale}.raw').read_bytes()
 assert len(raw) == 84*w*h*4, f'unexpected layer stream size: {len(raw)}'
 assert hashlib.sha256(raw).hexdigest() == {1: '3c239d4ba13f0fb16a18beefee23b1189524068e505b63127e0f3ba4105336d8', 2: '0c9b540784f59e781a0d966e5b75d0eed66ceac6350e95c25a45f7751e0d519f'}[scale], 'layer stream differs from pinned source-derived probe'
 layers=[raw[i:i+w*h*4] for i in range(0,len(raw),w*h*4)]
 native=bytearray(w*h);crop=Path(f'conformance/pencil2-v14-device-profile/{case}.alpha').read_bytes()
 assert hashlib.sha256(crop).hexdigest() == {'baseline': 'fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b', 'scale2': 'e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4'}[case]
 for y in range(22):native[(170+y)*w:(170+y)*w+62]=crop[y*62:(y+1)*62]
 d=f.open();q=d.new_page(width=w,height=h); xs={}
 for a in range(1,256):
  x=d.get_new_xref();d.update_object(x,f'<</Type/ExtGState/ca {a/255:.10f}>>');xs[a]=x
 commands=['0 0 0 rg 0 0 192 192 re f','1 1 1 rg']; svg=['<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192">']
 for layer in layers:
  for i in range(w*h):
   a=layer[i*4+3]
   if a:
    x=i%w;y=i//w;commands.append(f'/A{a} gs {x} {191-y} 1 1 re f');svg.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="black" fill-opacity="{a/255:.12f}"/>')
 svg.append('</svg>');form=d.get_new_xref();res=' '.join(f'/A{a} {x} 0 R' for a,x in xs.items());d.update_object(form,f'<</Type/XObject/Subtype/Form/BBox[0 0 192 192]/Group<</S/Transparency/CS/DeviceRGB/I true>>/Resources<</ExtGState<<{res}>>>>>>');d.update_stream(form,'\n'.join(commands).encode());gs=d.get_new_xref();d.update_object(gs,f'<</Type/ExtGState/SMask<</S/Luminosity/G {form} 0 R>>>>');d.xref_set_key(q.xref,'Resources',f'<</ExtGState<</G {gs} 0 R>>>>');c=d.get_new_xref();d.update_object(c,'<<>>');d.update_stream(c,b'/G gs 0 0 0 rg 0 0 192 192 re f');d.xref_set_key(q.xref,'Contents',f'{c} 0 R');data=d.tobytes();
 with tempfile.TemporaryDirectory() as td:
  t=Path(td);(t/'s.svg').write_text(''.join(svg));subprocess.run(['resvg',str(t/'s.svg'),str(t/'s.png')],check=True);svg_a=Image.open(t/'s.png').convert('RGBA').tobytes()[3::4]
  with f.open(stream=data,filetype='pdf') as doc:mu=doc[0].get_pixmap(alpha=True).samples[3::4]
  with p.PdfDocument(data) as doc:pi=doc[0].render(fill_color=(0,0,0,0),rev_byteorder=True).to_pil().convert('RGBA').tobytes()[3::4]
  for name,actual in [('resvg',svg_a),('MuPDF',mu),('PDFium',pi)]:
   e=[abs(a-b) for a,b in zip(actual,native)];print(scale,name,sum(e),max(e),sum(bool(v) for v in e),flush=True)
