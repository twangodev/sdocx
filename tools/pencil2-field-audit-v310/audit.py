"""Isolate piecewise-linear source-field error at fragment centers, no PDF reader."""
from fixtures import verify
verify()
import bisect, hashlib, math, os, struct, zlib
from pathlib import Path
from PIL import Image
root=Path(__file__).resolve().parents[2]
assets=root/'crates/sdocx/src/ink/pencil2/assets'
grain=(assets/'grain.r8').read_bytes(); point=(assets/'point.r8').read_bytes()
assert hashlib.sha256(grain).hexdigest()=='dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9'
assert hashlib.sha256(point).hexdigest()=='475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f'
source=(root/'tools/pencil2-worker-v259/native-geometry.vbo').read_bytes()
assert hashlib.sha256(source).hexdigest()=='be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8'
rows=list(struct.iter_unpack('<14f',source))
scale=int(os.environ.get('PENCIL_SCALE', '1')); assert scale in (1,2)
phase=float(os.environ.get('PENCIL_GRAIN_PHASE', '0')); gx=6+phase; gy=9+phase
print('grain_phase',phase)
base=root/'tools/pencil2-mixed-scene-controls'
raw=zlib.decompress((base/f'white-{scale}-first.zlib').read_bytes())
import json
assert hashlib.sha256(raw).hexdigest()==json.loads((base/f'white-{scale}-receipt.json').read_text())['first_sha256']
target=Image.frombytes('RGBA',(192,192),raw).convert('RGB').tobytes()
# Compare identical texture selection and per-stamp rounding. Only the nonlinear
# source-field evaluation changes; no PDF quantization, coverage or transport.
outs=[bytearray([255])*(192*192*3) for _ in range(3)]
max_field=0.; field_sum=0.; samples=0
for cx,cy,rx,ry,opacity,tilt,atlas,dx,dy,edge,smooth,red,green,blue in rows:
 if opacity<=0: continue
 x0=(cx-rx)*scale+6; x1=(cx+rx)*scale+6; y0=(cy-ry)*scale+9; y1=(cy+ry)*scale+9
 def grid(lo,hi,g):
  vals=[lo+(hi-lo)*i/100 for i in range(100)]+[hi]
  vals.extend(g+i*scale*.5 for i in range(math.floor((lo-g)/(scale*.5))+1,math.ceil((hi-g)/(scale*.5))) if lo<g+i*scale*.5<hi)
  return sorted(set(vals))
 xs,ys=grid(x0,x1,gx),grid(y0,y1,gy)
 def alpha(x,y,g):
  a=(x-x0)/(x1-x0)-.5; b=(y-y0)/(y1-y0)-.5; dot=a*dx+b*dy
  p=math.sqrt(max(0,a*a+b*b-dot*dot)); factor=1/(9*tilt+1)
  if p>factor:return 0.
  k=max(0,min(1,g*opacity*(.5-dot)*(factor-p)/factor)); dist=math.hypot(a,b)
  return k*smooth*(1-(dist-edge)/(.5-edge) if dist>edge else 1)
 for y in range(max(0,math.ceil(y0-.5)),min(192,math.ceil(y1-.5))):
  for x in range(max(0,math.ceil(x0-.5)),min(192,math.ceil(x1-.5))):
   xx=x+.5; yy=y+.5; u=(xx-x0)/(x1-x0); v=(yy-y0)/(y1-y0)
   px=min(99,int(u*100)); py=min(299,int((atlas+v)*100))
   if not point[py*100+px]:continue
   ix=math.floor((xx-gx)/(scale*.5))%150; iy=math.floor((yy-gy)/(scale*.5))%150
   g=grain[iy*150+ix]/255
   if not g:continue
   # Find the source-cell and horizontal run used by the mesh.
   xi=min(len(xs)-2,bisect.bisect_right(xs,xx)-1); yi=min(len(ys)-2,bisect.bisect_right(ys,yy)-1)
   if xi<0 or yi<0:continue
   ya,yb=ys[yi:yi+2]
   def cell(k):
    mx=(xs[k]+xs[k+1])/2; my=(ya+yb)/2
    px2=max(0,min(99,int((mx-x0)/(x1-x0)*100)))
    py2=max(0,min(299,int((atlas+(my-y0)/(y1-y0))*100)))
    ix2=math.floor((mx-gx)/(scale*.5))%150; iy2=math.floor((my-gy)/(scale*.5))%150
    return grain[iy2*150+ix2] if point[py2*100+px2] else 0
   # Hold the exact center texture selection fixed in all variants, including
   # at an edge, so the differences isolate interpolation and coalescing.
   cg=g
   lo=hi=xi
   while lo>0 and cell(lo-1)==cell(xi):lo-=1
   while hi+1<len(xs)-1 and cell(hi+1)==cell(xi):hi+=1
   def triangle(xa,xb):
    fx=(xx-xa)/(xb-xa); fy=(yy-ya)/(yb-ya)
    a00=alpha(xa,ya,cg); a10=alpha(xb,ya,cg); a11=alpha(xb,yb,cg); a01=alpha(xa,yb,cg)
    return (a00+(a10-a00)*fx+(a11-a10)*fy if fy<=fx else a00+(a11-a01)*fx+(a01-a00)*fy)
   interp=triangle(xs[xi],xs[xi+1]); merged=triangle(xs[lo],xs[hi+1])
   exact=alpha(xx,yy,g); err=abs(exact-interp); max_field=max(max_field,err);field_sum+=err;samples+=1
   for out,a in zip(outs,(exact,interp,merged)):
    i=(y*192+x)*3
    for c,color in enumerate((red,green,blue)):out[i+c]=max(0,min(255,round(color*255*a+out[i+c]*(1-a))))
for name,out in zip(('exact','triangles','merged'),outs):
 e=[abs(a-b) for a,b in zip(out,target)]
 print(name,'native_rgb_error',sum(e),max(e),sum(v!=0 for v in e))
e=[abs(a-b) for a,b in zip(outs[0],outs[1])]
print('triangle_vs_exact_rgb_error',sum(e),max(e),sum(v!=0 for v in e))
e=[abs(a-b) for a,b in zip(outs[0],outs[2])]
print('merged_vs_exact_rgb_error',sum(e),max(e),sum(v!=0 for v in e))
print('field_samples',samples,'sum_abs_alpha_error',field_sum,'max_alpha_error',max_field)
if phase == 0:
 expected={1:((344,29,139),(344,29,139),(349,29,140),(0,0,0),(7,3,3)),2:((767,4,607),(767,4,606),(1451,9,800),(10,1,10),(1074,8,655))}[scale]
 def metric(a,b):
  d=[abs(x-y) for x,y in zip(a,b)]; return sum(d),max(d),sum(v!=0 for v in d)
 actual=tuple(metric(out,target) for out in outs)+(metric(outs[0],outs[1]),metric(outs[0],outs[2]))
 assert actual==expected, (actual,expected)
