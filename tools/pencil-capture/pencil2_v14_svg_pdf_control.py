#!/usr/bin/env python3
"""Compare the V8 SVG prototype and its PDF with the pinned host-Mesa frame.

Build conformance/pencil2_v14_svg_pdf_runner.rs in a temporary Cargo package
with sdocx={path="<checkout>/crates/sdocx",features=["pdf"]}, package name
sdocx-svg-pdf-brush, and the shared agent target/settings from AGENTS.md.
Run from the checkout root using a Python environment with pymupdf and
pypdfium2. Inputs under tmp/vector-parity-plan must remain available.
"""
import base64,hashlib,json,re,subprocess,sys,tempfile
from pathlib import Path
import pymupdf
import pypdfium2
D=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-svg-paint-v8-stable-bounds-full84'); temporary=tempfile.TemporaryDirectory(prefix='sdocx-pencil2-pdf-'); P=Path(temporary.name); s=(D/'all84-alpha-prototype.svg').read_text()
for name in set(re.findall(r'(?:href|xlink:href)="([^"]+\.png)"',s)):
 s=s.replace(name,'data:image/png;base64,'+base64.b64encode((D/name).read_bytes()).decode())
(P/'control.svg').write_text(s)
subprocess.run(['/tmp/sdocx-agent-target/debug/sdocx-svg-pdf-brush',str(P/'control.svg'),str(P/'control.pdf')],check=True)
subprocess.run(['resvg',str(P/'control.svg'),str(P/'control.png')],check=True)
rgba=subprocess.check_output(['ffmpeg','-v','error','-i',str(P/'control.png'),'-f','rawvideo','-pix_fmt','rgba','-'])
cap=json.loads(Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text()); b=bytes.fromhex(cap['mesa']['readback_rgba_hex']); a=bytes(b[((191-y)*192+x)*4+3] for y in range(192) for x in range(192))
m=pymupdf.open(P/'control.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB); assert (m.width,m.height)==(192,192); mu=bytes(m.samples)[3::4]
p=pypdfium2.PdfDocument(str(P/'control.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)); assert(p.width,p.height,p.n_channels)==(192,192,4); pi=bytes(p.buffer)[3::4]
def stats(c):
 e=[abs(x-y) for x,y in zip(c,a)];return {'different':sum(x>0 for x in e),'sum':sum(e),'max':max(e),'support':sum(x>0 for x in c)}
print(json.dumps({'native':cap['mesa']['readback_sha256'],'svg_sha256':hashlib.sha256(s.encode()).hexdigest(),'pdf_sha256':hashlib.sha256((P/'control.pdf').read_bytes()).hexdigest(),'pdf_bytes':(P/'control.pdf').stat().st_size,'resvg':stats(rgba[3::4]),'mupdf':stats(mu),'pdfium':stats(pi),'versions':{'mupdf':pymupdf.VersionBind,'pdfium':str(pypdfium2.version.PYPDFIUM_INFO)}},indent=2))
