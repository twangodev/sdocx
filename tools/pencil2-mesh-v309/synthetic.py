"""Run v308 full-scene diagnostic with Rust-sampled descriptors."""
import importlib.util,os,struct
from pathlib import Path
from PIL import Image
import pymupdf,pypdfium2
root=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('mesh_v308',root/'tools/pencil2-mesh-v308/scene.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
directory=Path(os.environ['PENCIL_MESH_GEOMETRY_OUT'])
m.first_records=list(struct.iter_unpack('<14f',(directory/'synthetic0.bin').read_bytes()))
m.records=list(struct.iter_unpack('<14f',(directory/'synthetic1.bin').read_bytes()))
for backdrop,scale in [('white',1),('white',2),('blue',2)]:
 doc,target,triangles=m.render(scale,backdrop,full=True); assert not doc[0].get_images(full=True)
 data=doc.tobytes(garbage=3,deflate=True);pix=doc[0].get_pixmap(alpha=False); a=Image.frombytes('RGB',(192,192),pix.samples)
 pdf=pypdfium2.PdfDocument(data);b=pdf[0].render().to_pil().convert('RGB')
 print(backdrop,scale,'triangles',triangles,'bytes',len(data))
 for name,im in [('MuPDF',a),('PDFium',b)]:
  e=[abs(x-y) for x,y in zip(im.tobytes(),target.tobytes())];print(name,sum(e),max(e),sum(v!=0 for v in e))
 doc.close();pdf.close()
