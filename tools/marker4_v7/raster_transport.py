"""Measure SVG and direct PDF raster transport at the model's fixed pixel grid."""
import argparse
import base64
import io
import json
import subprocess
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image
import pymupdf

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('alpha',type=Path)
a=p.parse_args()
alpha=Image.open(a.alpha).convert('L')
w,h=alpha.size
rgba=Image.new('RGBA',(w,h),(255,203,48,0)); rgba.putalpha(alpha)
buf=io.BytesIO(); rgba.save(buf,format='PNG'); png=buf.getvalue()
svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}"><image width="{w}" height="{h}" href="data:image/png;base64,{base64.b64encode(png).decode()}"/></svg>'
with tempfile.TemporaryDirectory() as d:
    source=Path(d)/'candidate.svg'; output=Path(d)/'candidate.png'; source.write_text(svg)
    subprocess.run(['resvg',str(source),str(output)],check=True)
    svg_a=np.array(Image.open(output).convert('RGBA').getchannel('A'),dtype=int)
    pdf=pymupdf.open(); page=pdf.new_page(width=w,height=h); page.insert_image(page.rect,stream=png)
    pdf_bytes=pdf.tobytes(); pdf.close()
    doc=pymupdf.open(stream=pdf_bytes,filetype='pdf'); pix=doc[0].get_pixmap(alpha=True)
    pdf_a=np.array(Image.frombytes('RGBA',(pix.width,pix.height),pix.samples).getchannel('A'),dtype=int)
    reference=np.array(alpha,dtype=int)
    def metrics(data):
        diff=np.abs(data-reference)
        return {'alpha_l1':int(diff.sum()),'max_error':int(diff.max()),'support':int((data>0).sum())}
    print(json.dumps({'size':[w,h],'source_support':int((reference>0).sum()),'resvg':metrics(svg_a),'direct_pdf_mupdf':metrics(pdf_a),'boundary':'Standalone SVG image and direct PyMuPDF image insertion, not sdocx public exporters; 1:1 rasterization only.'}))
