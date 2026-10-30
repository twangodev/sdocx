import hashlib, json, os, zlib
from pathlib import Path
from PIL import Image, __version__
assert __version__ == "12.1.1"
work=Path(os.environ['PENCIL200_DIR'])
base=Path('tools/pencil2-mixed-scene-controls')
results={}
for case in ('white:1','white:2','transparent:1','blue:2'):
 name=case.replace(':','-'); receipt=json.loads((base/f'{name}-receipt.json').read_text()); rows=[]
 for i, label in enumerate(('first','second')):
  blob=(base/f'{name}-{label}.zlib').read_bytes()
  key='first_sha256' if i == 0 else None
  assert hashlib.sha256(zlib.decompress(blob)).hexdigest() == (receipt[key] if key else receipt['second_sha256'])
  raw=zlib.decompress(blob); assert len(raw)==192*192*4
  native=Image.frombytes('RGBA',(192,192),raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).tobytes()
  im=Image.open(work/case/f'vec{i}.png').convert('RGBA'); assert im.size==(192,192)
  data=bytearray(im.tobytes())
  if case.startswith('transparent'):
   for j in range(0,len(data),4):
    for k in range(3): data[j+k]=(data[j+k]*data[j+3]+127)//255
  errors=[abs(a-b) for a,b in zip(native,data)]
  rows.append({'sum':sum(errors),'max':max(errors),'channels':sum(x!=0 for x in errors),'svg_bytes':(work/case/f'vec{i}.svg').stat().st_size})
 results[case]=rows
print(json.dumps(results,indent=2))
