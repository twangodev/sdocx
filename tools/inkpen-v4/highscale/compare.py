import hashlib,json,re,subprocess,zlib,sys,importlib.metadata
from pathlib import Path
import numpy as np
import pymupdf
from PIL import Image
root=Path(sys.argv[1]); data=json.loads((root/'packed.json').read_text()); frames=(root/'packed.bin').read_bytes(); results=[]
for case in data['cases']:
 name=case['name']; scale=case['scale']; native=zlib.decompress(frames[case['offset']:case['offset']+case['length']]); assert hashlib.sha256(native).hexdigest()==case['rgba_sha256']; record={'name':name,'scale':scale,'rgba_sha256':case['rgba_sha256'],'stream_sha256':case['stream_sha256'],'contours':{}}
 for contour in (1,scale):
  svg=(root/'exports'/f'{name}-{contour}.svg').read_text();head,body=svg.split('>',1);body=body.rsplit('</svg>',1)[0];body,n=re.subn(r'<rect fill="#fcfcfc"[^>]*/>','',body);assert n==1
  head=head.replace('height="64"',f'height="{64*scale}"').replace('width="128"',f'width="{128*scale}"'); (root/'probe.svg').write_text(head+'>'+body+'</svg>');subprocess.run([sys.argv[2],str(root/'probe.svg'),str(root/'probe.png')],check=True)
  pixels=Image.open(root/'probe.png').convert('RGBA');assert pixels.size==(128*scale,64*scale);alpha=sum(abs(a-b) for a,b in zip(native[3::4],pixels.tobytes()[3::4]));doc=pymupdf.open(root/'exports'/f'{name}-{contour}.pdf');pix=doc[0].get_pixmap(matrix=pymupdf.Matrix(scale*4/3,scale*4/3),alpha=False);assert (pix.width,pix.height,pix.n)==(128*scale,64*scale,3);rgba=np.frombuffer(native,dtype=np.uint8).reshape(-1,4).astype(np.float64); reference=np.rint(rgba[:,:3]+252*(255-rgba[:,3,None])/255).astype(np.int16); actual=np.frombuffer(pix.samples,dtype=np.uint8).reshape(-1,3).astype(np.int16); rgb=int(np.abs(actual-reference).sum());record['contours'][str(contour)]={'svg_alpha_error':alpha,'mupdf_rgb_error':rgb,'svg_sha256':hashlib.sha256((root/'exports'/f'{name}-{contour}.svg').read_bytes()).hexdigest(),'pdf_sha256':hashlib.sha256((root/'exports'/f'{name}-{contour}.pdf').read_bytes()).hexdigest()}
 assert record['contours'][str(scale)]['svg_alpha_error'] < record['contours']['1']['svg_alpha_error']
 assert record['contours'][str(scale)]['mupdf_rgb_error'] < record['contours']['1']['mupdf_rgb_error']
 results.append(record);print(name,record['contours'],flush=True)
(root/'comparison.json').write_text(json.dumps({'scope':'Synthetic host Mesa capture, SVG resvg alpha and MuPDF PDF RGB on #fcfcfc; not device or saved-note parity','frames_sha256':data['frames_sha256'],'fixture_sha256':hashlib.sha256((root/'packed.json').read_bytes()).hexdigest(),'renderer_sha256':hashlib.sha256(Path(sys.argv[2]).read_bytes()).hexdigest(),'render_rs_sha256':hashlib.sha256(Path('crates/sdocx/src/render.rs').read_bytes()).hexdigest(),'versions':{name:importlib.metadata.version(name) for name in ('Pillow','PyMuPDF','numpy')},'cases':results},indent=2)+'\n')
