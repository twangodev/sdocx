"""Ablate V8 masks and compare the same SVG in resvg, MuPDF, and PDFium.

Requires the runner documented in pencil2_v14_svg_pdf_control.py and the retained
prototype and native capture. All rendered files are temporary.
"""
import re,base64,tempfile,subprocess,json
from pathlib import Path
import pymupdf
import pypdfium2
D=Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-svg-paint-v8-stable-bounds-full84'); s=(D/'all84-alpha-prototype.svg').read_text()
for name in set(re.findall(r'(?:href|xlink:href)="([^"]+\.png)"',s)): s=s.replace(name,'data:image/png;base64,'+base64.b64encode((D/name).read_bytes()).decode())
cap=json.loads(Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json').read_text()); b=bytes.fromhex(cap['mesa']['readback_rgba_hex']); native=bytes(b[((191-y)*192+x)*4+3] for y in range(192) for x in range(192))
with tempfile.TemporaryDirectory() as td:
 p=Path(td)
 for kind in ['original','no_filter','no_point','no_width','no_dir','no_perp','no_grain','no_masks']:
  v=s
  patterns={'no_filter':r' filter="url\(#sat\d+\)"','no_point':r' mask="url\(#pointRow\d+\)"','no_width':r' mask="url\(#widthm\)"','no_dir':r' mask="url\(#dirm\d+\)"','no_perp':r' mask="url\(#perpm\d+\)"','no_grain':r' mask="url\(#grainAlpha\)"','no_masks':r' mask="url\(#[^"]+\)"'}
  if kind in patterns: v=re.sub(patterns[kind],'',v)
  (p/'in.svg').write_text(v); subprocess.run(['/tmp/sdocx-agent-target/debug/sdocx-svg-pdf-brush',str(p/'in.svg'),str(p/'out.pdf')],check=True); subprocess.run(['resvg',str(p/'in.svg'),str(p/'out.png')],check=True)
  rgb=subprocess.check_output(['ffmpeg','-v','error','-i',str(p/'out.png'),'-f','rawvideo','-pix_fmt','rgba','-']); a=rgb[3::4]; m=pymupdf.open(p/'out.pdf')[0].get_pixmap(alpha=True,colorspace=pymupdf.csRGB).samples[3::4]; q=bytes(pypdfium2.PdfDocument(str(p/'out.pdf'))[0].render(scale=1,rev_byteorder=True,fill_color=(0,0,0,0)).buffer)[3::4]
  def stat(x):
   diff=[abs(c-d) for c,d in zip(a,x)];return dict(support=sum(c>0 for c in x),sum_abs=sum(diff),max_abs=max(diff),different=sum(v>0 for v in diff))
  print(kind, 'resvg_support',sum(c>0 for c in a),'mupdf',stat(m),'pdfium',stat(q), 'resvg_vs_native_sum',sum(abs(x-y) for x,y in zip(a,native)), 'mupdf_vs_native_sum',sum(abs(x-y) for x,y in zip(m,native)), 'pdfium_vs_native_sum',sum(abs(x-y) for x,y in zip(q,native)),flush=True)
