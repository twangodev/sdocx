if not __debug__:
    raise RuntimeError('This evidence check requires assertions; do not use python -O')

import numpy as np,struct,hashlib
from pathlib import Path

def checked(path, expected):
 b=Path(path).read_bytes()
 assert hashlib.sha256(b).hexdigest()==expected, path
 return b
f=np.float32
r=list(struct.iter_unpack('<6f',checked('tools/crayon2-document-holdout/vector.bin','15509eda1113e0bfd007455ba4767b1cba2a22391487f94c75e4121d98ac5f8f')))[139];x,y,rad,sel,bx,by=map(f,r); coords=[];uv=[]
for sx,sy,pu,pv in [(-1,-1,0,1),(-1,1,0,0),(1,-1,1,1),(1,1,1,0)]:
 p=[];a=[]
 for coord,off,sg,t in [(x,bx,sx,f(-.8375)),(y,by,sy,f(-.8875))]:
  v=f(coord+f(sg)*rad);b=f(v+off);clip=f(f(f(.0125)*v)+t);bc=f(f(f(.0125)*b)+t);p.append(f(f(f(clip+f(1))*f(.5))*f(160)));a.append(f(f(bc+f(1))*f(.5)))
 a.extend([f(pu),f(f(1/3)*f(sel+f(pv)))]);coords.append(p);uv.append(a)
def plane(p,a):
 x0,y0=p[0];x1,y1=p[1];x2,y2=p[2];a0,a1,a2=a;dx01=f(x0-x1);dy01=f(y0-y1);dx20=f(x2-x0);dy20=f(y2-y0);inv=f(f(1)/f(f(dx01*dy20)-f(dy01*dx20)));da01=f(a0-a1);da20=f(a2-a0);dx=f(f(da01*f(dy20*inv))-f(da20*f(dy01*inv)));dy=f(f(da20*f(dx01*inv))-f(da01*f(dx20*inv)));return f(a0-f(f(dx*f(x0-f(.5)))+f(dy*f(y0-f(.5))))),dx,dy
px,py=52,57;a=(px+.5-float(coords[0][0]))/float(f(coords[2][0]-coords[0][0]));b=(py+.5-float(coords[0][1]))/float(f(coords[1][1]-coords[0][1]));ids=[0,1,2] if a+b<=1 else [2,1,3]
vals=[]
for i in range(4):
 c,dx,dy=plane([coords[j] for j in ids],[uv[j][i] for j in ids]);v=f(float(dy)*py+float(f(float(dx)*px+float(c))));vals.append(v)
ux,uy,pu,pv=vals;scale=f(1.4222222566604614); inds=[int(f(f(f(v*scale)%f(1))*f(150))) for v in [ux,uy]];pinds=[min(9,max(0,int(f(pu*f(10))))),min(29,max(0,int(f(pv*f(30)))))]
bg=checked('tmp/vector-parity-plan/crayon2-native-next/native-uploads/g_Crayon2BgPtn.source.bin','2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3');pt=checked('tmp/vector-parity-plan/crayon2-native-next/native-uploads/g_Crayon2PointPtn.source.bin','673423c6fdae1ba344c452deefeb51b7b11365fead0bfbdd4b345eed4027f405');print(vals,inds,pinds,bg[inds[1]*150+inds[0]],pt[pinds[1]*10+pinds[0]])

assert inds == [113,4] and pinds == [5,24]
assert bg[inds[1]*150+inds[0]] == 41 and pt[pinds[1]*10+pinds[0]] == 255
