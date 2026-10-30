exec((__import__('pathlib').Path(__file__).with_name('analyze.py')).read_text().split('for idx in [1,100,250,478]:')[0])
import numpy as np
f=np.float32
v=zlib.decompress(Path('tools/crayon2-independent-control/native-vector.z').read_bytes())
scale=f(1.7066665887832642); mat=f(0.010416666977107525)
for idx in [1,100,250,478]:
 xx,yy,r,_,bx,by=struct.unpack_from('<6f',v,24*idx);arrays={}
 for typ in ['xuv','xsample','yuv','ysample']:
  j=json.loads((p/f'{typ}{idx}.json').read_text())['paint'];arrays[typ]=zlib.decompress(base64.b64decode(j['readback']['rgba8_zlib_base64']))
 def vertices(c,off):
  out=[]
  for sg in [-1,1]:
   coord=f(f(c)+f(f(sg)*f(r))); bg=f(coord+f(off)); clip=f(f(mat*coord)-f(1));bgclip=f(f(mat*bg)-f(1));screen=f(f(f(clip+f(1))*f(.5))*f(192)); uv=f(f(bgclip+f(1))*f(.5));out.append((screen,uv))
  return out
 verts=[vertices(xx,bx),vertices(yy,by)];errors=[[],[]]; mul_err=[]
 for y in range(192):
  for x in range(192):
   off=((191-y)*192+x)*4
   if arrays['xuv'][off:off+3]==bytes([64,128,192]): continue
   for axis,(pixel,typ,styp) in enumerate([(x+.5,'xuv','xsample'),(191-y+.5,'yuv','ysample')]):
    expected=(pixel+(bx if axis==0 else by))/192
    u=value(arrays[typ][off:off+3],expected); sampled=value(arrays[styp][off:off+3],u*scale)
    (s0,u0),(s1,u1)=verts[axis];pred=f(u0+f(f(f(pixel)-s0)*f(f(u1-u0)/f(s1-s0))))
    errors[axis].append((float(pred)-u)*2**24);mul_err.append((float(f(f(u)*scale))-sampled)*2**24)
 print(idx,'uv model error times 2^24',errors,'scaled multiply errors',mul_err)
