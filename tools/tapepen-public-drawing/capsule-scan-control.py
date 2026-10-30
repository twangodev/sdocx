import math,re,xml.etree.ElementTree as ET,gzip,json,base64,zlib,hashlib
from pathlib import Path
asset=Path('scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg')
assert hashlib.sha256(asset.read_bytes()).hexdigest() == '9f8b208a01db838235f795226acd6e68aa42e609b06c6e171a29447f8b68322b'
svg=ET.parse(asset).getroot(); paths=[]
for e in svg.iter('{http://www.w3.org/2000/svg}path'):
 d=e.attrib['d']; a=re.findall(r'[MVL]|-?\d+(?:\.\d+)?',d); x,y=float(a[1]),float(a[2]); x2,y2=(x,float(a[4])) if a[3]=='V' else (float(a[4]),float(a[5])); paths.append((x,y,x2,y2))
assert len(paths) == 136
for size in (64,128):
 s=gzip.decompress(Path(f'tools/tapepen-public-drawing/identity-screen-state-{size*4}.log.gz').read_bytes()).decode();j,_=json.JSONDecoder().raw_decode(s[s.index('\n{')+1:]);native=zlib.decompress(base64.b64decode(j['mesa_texture_uploads']['events'][6]['payload']['data_base64'])); scale=size/120
 assert hashlib.sha256(native).hexdigest() == {64:'78a0a70b418c59c460f8791d48d7f131ad7d445f3797323eb041fd9b689ba189',128:'f290e56addf62df35c7fbd4bfb6bddaeccc047ab382c52aa9766cafb38b7d292'}[size]
 for subs in (5,10,20):
  out=bytearray([200,233,241,255]*(size*size))
  for x,y,x2,y2 in paths:
   x*=scale;y*=scale;x2*=scale;y2*=scale;r=.85*scale
   for iy in range(max(0,int(min(y,y2)-r)),min(size,math.ceil(max(y,y2)+r))):
    for ix in range(max(0,int(min(x,x2)-r)),min(size,math.ceil(max(x,x2)+r))):
     coverage=0
     for k in range(subs):
      yy=iy+(k+.5)/subs;dy=max(min(y,y2)-yy,0,yy-max(y,y2));half=math.sqrt(max(0,r*r-dy*dy)) if dy<r else 0
      lo=min(x,x2)-half;hi=max(x,x2)+half
      if half: coverage+=max(0,min(ix+1,hi)-max(ix,lo))/subs
     if coverage:
      i=4*(iy*size+ix)
      for c,v in enumerate((63,84,128)):out[i+c]=round(out[i+c]*(1-coverage)+v*coverage)
  ds=[abs(a-b) for a,b in zip(out,native)];print(size,subs,sum(ds),max(ds),sum(a!=0 for a in ds),flush=True)
