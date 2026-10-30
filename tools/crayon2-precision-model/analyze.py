import json,base64,zlib,struct,re,math
from pathlib import Path
import sys
p=Path(sys.argv[1]);s=Path(sys.argv[2]).read_text();bg=re.search(r'<image id="native-bg-mask" x="0" y="0" width="([^"]+)" height="([^"]+)',s);w,h=map(float,bg.groups())
def value(rgb,ideal):
 lo=rgb[0]<<16|rgb[1]<<8|rgb[2]
 candidates=[struct.unpack('<f',struct.pack('<I',(a<<24)|lo))[0] for a in range(0x3c,0x41)]
 return min(candidates,key=lambda a:abs(a-ideal))
for idx in [1,100,250,478]:
 origin=re.search(r'<mask id="background-mask-'+str(idx)+r'".*?<use href="#native-bg-mask" x="([^"]+)" y="([^"]+)"',s); ox,oy=map(float,origin.groups()); arrays={}
 for typ in ['xuv','xsample','yuv','ysample']:
  j=json.loads((p/f'{typ}{idx}.json').read_text())['paint'];arrays[typ]=zlib.decompress(base64.b64decode(j['readback']['rgba8_zlib_base64']))
 rows=[]
 for y in range(192):
  for x in range(192):
   off=((191-y)*192+x)*4
   if arrays['xuv'][off:off+3]==bytes([64,128,192]): continue
   ix=(x+.5-ox)/w; iy=1-(y+.5-oy)/h
   xs=value(arrays['xsample'][off:off+3],ix + (1 if idx==478 else 0));ys=value(arrays['ysample'][off:off+3],iy)
   if idx==478: ix += 1
   xu=value(arrays['xuv'][off:off+3],xs/1.7066665887832642);yu=value(arrays['yuv'][off:off+3],ys/1.7066665887832642)
   rows.append((x,y,xs,ys,xu,yu,150*(xs-ix),150*(ys-iy)))
 print(idx,'pixels',len(rows),'max texel delta',max((abs(t[-2]) for t in rows),default=0),max((abs(t[-1]) for t in rows),default=0))
 for row in sorted(rows,key=lambda t:max(abs(t[-1]),abs(t[-2])),reverse=True)[:2]:print(row)
 if idx==1: print('pixel',next((r for r in rows if r[:2]==(36,143)),None))
