"""Pixel-center diagnostic only; compares sampling hypothesis with native capture."""
import math, zlib, json, hashlib
from PIL import Image
from scene import BASE, GRAIN, POINT, records

def go(scale, backdrop):
    key=f'{backdrop}-{scale}'; receipt=json.loads((BASE/f'{key}-receipt.json').read_text())
    imgs=[]
    for prefix in ('first','second'):
        raw=zlib.decompress((BASE/f'{key}-{prefix}.zlib').read_bytes())
        assert hashlib.sha256(raw).hexdigest()==receipt[prefix+'_sha256']
        imgs.append(Image.frombytes('RGBA',(192,192),raw))
    bg,target=imgs; output=bg.copy(); pixel=output.load()
    gx,gy=12,(13.999998 if scale==1 else 16.0000019)
    for cx,cy,rx,ry,opacity,tilt,atlas,dx,dy,edge,smooth,red,green,blue in records:
        if opacity<=0: continue
        x0=(cx-rx)*scale+12; x1=(cx+rx)*scale+12; y0=(cy-ry)*scale+12; y1=(cy+ry)*scale+12
        for y in range(max(0,math.ceil(y0-.5)), min(192,math.ceil(y1-.5))):
            for x in range(max(0,math.ceil(x0-.5)),min(192,math.ceil(x1-.5))):
                xx=x+.5; yy=y+.5; u=(xx-x0)/(x1-x0); v=(yy-y0)/(y1-y0)
                px=min(99,int(u*100)); py=min(299,int((atlas+v)*100))
                if POINT[py*100+px]==0: continue
                ix=math.floor((xx-gx)/(scale*.5))%150; iy=math.floor((yy-gy)/(scale*.5))%150
                a=u-.5;b=v-.5;dot=a*dx+b*dy;p=math.sqrt(max(0,a*a+b*b-dot*dot)); factor=1/(9*tilt+1)
                if p>factor:continue
                alpha=max(0,min(1,GRAIN[iy*150+ix]/255*opacity*(.5-dot)*((factor-p)/factor)))
                dist=math.hypot(a,b);alpha*=smooth*(1-(dist-edge)/(.5-edge) if dist>edge else 1)
                dst=pixel[x,y];pixel[x,y]=tuple(round((c*255)*alpha+d*(1-alpha)) for c,d in zip((red,green,blue),dst[:3]))+(255,)
    e=[abs(a-b) for a,b in zip(output.convert('RGB').tobytes(),target.convert('RGB').tobytes())]
    print(key,'center sample approximate float:',sum(e),max(e),sum(v!=0 for v in e))
for backdrop,scale in [('white',1),('white',2),('blue',2)]:go(scale,backdrop)
