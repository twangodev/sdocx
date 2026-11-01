"""Synthetic Pencil2 mesh using Rust-sampled geometry for two strokes."""
import hashlib, math, os, struct, sys, zlib
from pathlib import Path
import pymupdf, pypdfium2
from PIL import Image
ROOT=Path(__file__).resolve().parents[2]; BASE=ROOT/'tools/pencil2-mixed-scene-controls'; ASSET=ROOT/'crates/sdocx/src/ink/pencil2/assets'
GRAIN=(ASSET/'grain.r8').read_bytes(); POINT=(ASSET/'point.r8').read_bytes()
assert hashlib.sha256(GRAIN).hexdigest()=='dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9'
assert hashlib.sha256(POINT).hexdigest()=='475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f'
first_source=Path(os.environ["PENCIL_MESH_GEOMETRY_OUT"],"synthetic0.bin").read_bytes(); assert hashlib.sha256(first_source).hexdigest()=="be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8"
first_records=list(struct.iter_unpack('<14f',first_source)); assert len(first_records)==84

def render(scale,backdrop,full=False):
    import json
    directory=Path(os.environ['PENCIL_MESH_CAPTURE_DIR']); receipt=json.loads((directory/'capture.json').read_text())
    expected=json.loads((ROOT/'tools/pencil2-mesh-v309/receipt.json').read_text())['note_native'][backdrop]
    for prefix in ('first','second'):
        assert receipt[prefix+'_sha256']==expected[prefix+'_frame_sha256']
    assert receipt['geometry_sha256']==expected['native_descriptor_sha256']
    images=[]
    for part in ('first','second'):
        raw=(directory/f'{part}.rgba').read_bytes(); assert hashlib.sha256(raw).hexdigest()==receipt[part+'_sha256']
        images.append(Image.frombytes('RGBA',(192,192),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).convert('RGB'))
    bg,target=images
    first_only=os.environ.get('PENCIL_MESH_FIRST_ONLY')=='1'
    if first_only: target=bg
    native=(directory/'second.vbo').read_bytes(); assert hashlib.sha256(native).hexdigest()==receipt['geometry_sha256']
    geometry=Path(os.environ['PENCIL_MESH_GEOMETRY_OUT'],'note.bin').read_bytes()
    assert len(geometry)==len(native)
    assert [(i//14,i%14) for i,(a,b) in enumerate(zip(struct.iter_unpack('<I',native),struct.iter_unpack('<I',geometry))) if a!=b]==[(52,8),(60,8)]
    rows=list(struct.iter_unpack('<14f',geometry))
    uniform=[call[3] for call in receipt['uniforms'] if call[:3]==['glUniform2fv',4,1]][-1]
    doc=pymupdf.open(); page=doc.new_page(width=192,height=192)
    if full:
        page.draw_rect(page.rect, color=None, fill=(1,1,1) if backdrop=="white" else (64/255,128/255,191/255))
    else:
        page.insert_image(page.rect, pixmap=pymupdf.Pixmap(pymupdf.csRGB,192,192,bg.tobytes(),0))
    res=doc.xref_get_key(page.xref,'Resources')[1]; rx=int(res.split()[0]); xos=[]; gs=[]; streams=[]; triangles=0
    svg=['<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192"><rect width="192" height="192" fill="'+('#fff' if backdrop=='white' else '#4080bf')+'"/>']
    gx,gy=(12,13.999998 if scale==1 else 16.0000019)
    def new(s):
        x=doc.get_new_xref();doc.update_object(x,s);return x
    def vertex(out,x,y,v):
        out.extend(struct.pack('>BHHB',0,max(0,min(65535,round(x/192*65535))),max(0,min(65535,round(y/192*65535))),max(0,min(255,round(v*255)))))
    draws=([(first_records,6,9,6,9)] if full else [])+([] if first_only else [(rows,12,12,*uniform)])
    for rows,tx,ty,gx,gy in draws:
      for r in rows:
        cx,cy,rxs,rys,opacity,tilt,atlas,dx,dy,edge,smooth,red,green,blue=r
        if opacity<=0: continue
        x0=(cx-rxs)*scale+tx; x1=(cx+rxs)*scale+tx; y0=(cy-rys)*scale+ty; y1=(cy+rys)*scale+ty
        # Every boundary is source-derived: point atlas texels and repeating grain texels.
        def grid(lo,hi,g):
            stride=int(os.environ.get("PENCIL_MESH_POINT_STRIDE", "1")); assert stride in (1,2,4,5,10,20)
            vals=[lo+(hi-lo)*i/100 for i in range(0,100,stride)]+[hi]
            vals.extend(g+i*scale*.5 for i in range(math.floor((lo-g)/(scale*.5))+1,math.ceil((hi-g)/(scale*.5))) if lo<g+i*scale*.5<hi)
            vals=[v for v in vals if 0 <= v <= 192]
            if lo < 0 < hi: vals.append(0.)
            if lo < 192 < hi: vals.append(192.)
            return sorted(set(vals))
        xs,ys=grid(x0,x1,gx),grid(y0,y1,gy);out=bytearray()
        def alpha(x,y,grain):
            u=(x-x0)/(x1-x0); v=(y-y0)/(y1-y0); a=u-.5;b=v-.5;dot=a*dx+b*dy
            p=math.sqrt(max(0,a*a+b*b-dot*dot)); factor=1/(9*tilt+1)
            if p>factor:return 0.
            k=max(0.,min(1.,grain*opacity*(.5-dot)*((factor-p)/factor)))
            dist=math.hypot(a,b); return k*smooth*(1-(dist-edge)/(.5-edge) if dist>edge else 1)
        for ya,yb in zip(ys,ys[1:]):
            runs=[]
            for xa,xb in zip(xs,xs[1:]):
                u=(xa+xb-2*x0)/(2*(x1-x0)); px=max(0,min(99,int(u*100)))
                v=(ya+yb-2*y0)/(2*(y1-y0)); py=max(0,min(299,int((atlas+v)*100)))
                ix=math.floor(((xa+xb)/2-gx)/(75*scale)*150)%150; iy=math.floor(((ya+yb)/2-gy)/(75*scale)*150)%150
                grain=GRAIN[iy*150+ix] if POINT[py*100+px] else 0
                if runs and runs[-1][2]==grain:
                    runs[-1]=(runs[-1][0],xb,grain)
                else:
                    runs.append((xa,xb,grain))
            for xa,xb,grain in runs:
                if not grain: continue
                grain/=255
                corners=[(xa,ya),(xb,ya),(xb,yb),(xa,yb)]
                vals=[alpha(x,y,grain) for x,y in corners]
                if max(vals)<.5/255:continue
                for i in (0,1,2,0,2,3): vertex(out,*corners[i],vals[i])
                v=sum(vals)/4
                if v>0:
                    svg.append(f'<rect x="{xa:.5f}" y="{192-yb:.5f}" width="{xb-xa:.5f}" height="{yb-ya:.5f}" fill="rgb({round(red*255)},{round(green*255)},{round(blue*255)})" fill-opacity="{v:.6f}"/>')
                triangles+=2
        if not out:continue
        sh=new('<< /ShadingType 4 /ColorSpace /DeviceGray /BitsPerCoordinate 16 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 192 0 192 0 1] >>');doc.update_stream(sh,bytes(out),compress=True)
        form=new(f'<< /Type /XObject /Subtype /Form /BBox [0 0 192 192] /Group << /S /Transparency /CS /DeviceGray >> /Resources << /Shading << /S {sh} 0 R >> >> >>');doc.update_stream(form,b'/S sh',compress=True)
        ext=new(f'<< /Type /ExtGState /SMask << /S /Luminosity /G {form} 0 R >> >>')
        gs.append(f'/G{ext} {ext} 0 R');streams.append(f'q /G{ext} gs {red} {green} {blue} rg 0 0 192 192 re f Q')
    # The existing resource dictionary may contain image XObjects; add ExtGState only.
    doc.xref_set_key(rx,'ExtGState','<< '+' '.join(gs)+' >>')
    prev=page.get_contents(); content=new('<< >>'); doc.update_stream(content,('\n'.join(streams)).encode()); doc.xref_set_key(page.xref,'Contents','[ '+' '.join(f'{x} 0 R' for x in [*prev,content])+' ]')
    svg.append('</svg>')
    Path(os.environ['PENCIL_MESH_SVG']).write_text(''.join(svg))
    return doc, target,triangles
if __name__=='__main__':
    for backdrop,scale in [(os.environ.get('PENCIL_MESH_BACKDROP','blue'),1)]:
        doc,target,triangles=render(scale,backdrop,full=True)
        assert not doc[0].get_images(full=True)
        data=doc.tobytes(garbage=3,deflate=True);print(backdrop,scale,'triangles',triangles,'bytes',len(data),flush=True)
        pix=doc[0].get_pixmap(alpha=False); a=Image.frombytes('RGB',(192,192),pix.samples)
        pdf=pypdfium2.PdfDocument(data); b=pdf[0].render().to_pil().convert('RGB')
        for name,im in [('MuPDF',a),('PDFium',b)]:
            e=[abs(x-y) for x,y in zip(im.tobytes(),target.tobytes())];print(name,'sum',sum(e),'max',max(e),'changed',sum(v!=0 for v in e),flush=True)
            worst=max(range(len(e)),key=e.__getitem__); i=worst//3; x=i%192;y=i//192
            print(name,'worst pixel',x,y,'target',target.getpixel((x,y)),'actual',im.getpixel((x,y)),flush=True)
        import subprocess
        path=Path(os.environ['PENCIL_MESH_SVG']); png=path.with_suffix('.png')
        subprocess.run(['resvg',str(path),str(png)],check=True)
        im=Image.open(png).convert('RGB'); e=[abs(x-y) for x,y in zip(im.tobytes(),target.tobytes())]
        print('SVG resvg','sum',sum(e),'max',max(e),'changed',sum(v!=0 for v in e),'bytes',path.stat().st_size,flush=True)
        png.unlink();path.unlink()
        doc.close();pdf.close()
