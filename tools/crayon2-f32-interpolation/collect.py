import base64,hashlib,json,zlib,sys
from pathlib import Path
vector=Path('tools/crayon2-document-holdout/vector.bin').read_bytes(); assert hashlib.sha256(vector).hexdigest()=='15509eda1113e0bfd007455ba4767b1cba2a22391487f94c75e4121d98ac5f8f'
p=Path(sys.argv[1]); out={'scope':'Translated 13,9 160x160 recompiled shader, high byte inferred','stamps':{}}
for index in [20,100,200]:
 frames={}; hashes={}
 for mode in ['xuv','yuv']:
  j=json.loads((p/f'{mode}{index}.json').read_text())['paint']; assert bytes.fromhex(j['native_vector_hex']) == vector[24*index:24*(index+1)]; assert j['target_size']==[160,160]; assert j['view_translation']==[13,9]; assert j['draw_calls'][-1]['values']==[5,0,4,1]
  f=zlib.decompress(base64.b64decode(j['readback']['rgba8_zlib_base64']));frames[mode]=f;hashes[mode]=hashlib.sha256(f).hexdigest()
 masks={m:{i//4 for i in range(0,len(f),4) if f[i:i+4]!=bytes((64,128,192,255))} for m,f in frames.items()};assert masks['xuv']==masks['yuv']
 rows=[]
 for pos in sorted(masks['xuv']):
  y,x=divmod(pos,160); rows.append({'xy':[x,159-y], 'encoded_rgba':{m:list(f[pos*4:pos*4+4]) for m,f in frames.items()}})
 out['stamps'][str(index)]={'frame_sha256':hashes,'observations':rows}
print([(k,len(v['observations'])) for k,v in out['stamps'].items()]); assert out == json.loads(Path('tools/crayon2-f32-interpolation/translated-uv-receipt.json').read_text())
