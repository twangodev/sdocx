"""Finite-pixel RTV3 shader model for retained host captures; not an export renderer."""
import argparse
import hashlib
import json
import math
import zlib
from pathlib import Path

import numpy as np
from PIL import Image

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('capture', type=Path)
p.add_argument('mask', type=Path)
p.add_argument('--output', type=Path)
p.add_argument('--rotation', type=float, default=0.0, help='Document-to-raster rotation in radians')
a = p.parse_args()
m = json.loads(a.capture.read_text())
case = next(c for c in json.loads(Path('conformance/marker4-v7-finger-native.json').read_text())['cases'] if c['name'] == m['case'])
raw = a.capture.with_suffix('.rgba').read_bytes()
assert hashlib.sha256(raw).hexdigest() == m['rgba_sha256']
mask_bytes = a.mask.read_bytes()
if a.mask.suffix == '.zlib':
    mask_bytes = zlib.decompress(mask_bytes)
assert hashlib.sha256(mask_bytes).hexdigest() == '07d961c792fbd15286c3200e5358ceaff34b205db6d8cffb0e327302228b4f5b'
mask = np.frombuffer(mask_bytes, dtype=np.uint8).reshape(200, 200, 4)[:, :, 3].astype(float) / 255
w, h = m['target']; scale = m['scale']; cx, cy = m['frame_center_doc']; phase_x, phase_y = m['phase']
rc, rs = math.cos(a.rotation), math.sin(a.rotation)
expected_matrix = [scale*rc, -scale*rs, 0, 0, -scale*rs, -scale*rc, 0, 0, 0, 0, 1, 0,
                   w/2-scale*(rc*cx-rs*cy)+phase_x, h/2+scale*(rs*cx+rc*cy)-phase_y, 0, 1]
assert all(abs(actual-expected) < 0.0001 for actual, expected in zip(m['matrix_f32'], expected_matrix, strict=True)), 'capture matrix does not match model transform'
native = np.frombuffer(raw, dtype=np.uint8).reshape(h, w, 4)[::-1, :, 3].astype(int)
result = np.zeros((h, w), dtype=float)
for x, y, radius, angle in case['dots']:
    # Native AddPoint sends a rotated rectangle from MakePointRect.
    tw, th = 1.54 * (radius + .5), 2 * (radius + .5)
    c, s = math.cos(angle+a.rotation), math.sin(angle+a.rotation)
    sx, sy = w/2 + scale*(rc*(x-cx)-rs*(y-cy))+phase_x, h/2+scale*(rs*(x-cx)+rc*(y-cy))+phase_y
    extent = scale * math.hypot(tw, th)/2 + 2
    x0=max(0,math.floor(sx-extent)); x1=min(w,math.ceil(sx+extent))
    y0=max(0,math.floor(sy-extent)); y1=min(h,math.ceil(sy+extent))
    xx, yy = np.meshgrid(np.arange(x0,x1)+.5-sx, np.arange(y0,y1)+.5-sy)
    u = (xx*c+yy*s)/(scale*tw)+.5
    v = (-xx*s+yy*c)/(scale*th)+.5
    inside = (u>=0)&(u<=1)&(v>=0)&(v<=1)
    # GL_LINEAR, CLAMP_TO_EDGE; captured mask rows are GL bottom-up.
    tx=np.clip(u*200-.5,0,199); ty=np.clip(v*200-.5,0,199)
    ix=np.floor(tx).astype(int); iy=np.floor(ty).astype(int)
    fx=tx-ix; fy=ty-iy; jx=np.minimum(ix+1,199); jy=np.minimum(iy+1,199)
    alpha=(mask[iy,ix]*(1-fx)*(1-fy)+mask[iy,jx]*fx*(1-fy)+mask[jy,ix]*(1-fx)*fy+mask[jy,jx]*fx*fy)
    dx=(abs(c)+abs(s))/(scale*tw); dy=(abs(c)+abs(s))/(scale*th)
    edge_x=np.clip((.5-np.abs(u-.5))/dx,0,1)
    edge_y=np.clip((.5-np.abs(v-.5))/dy,0,1)
    pixel=np.rint(alpha*edge_x*edge_y*(128/255)*255)*inside
    result[y0:y1,x0:x1]=np.maximum(result[y0:y1,x0:x1],pixel)
out=result.astype(np.uint8)
print(json.dumps({'case':m['case'],'scale':scale,'phase':m['phase'],'support':int((out>0).sum()),'native_support':int((native>0).sum()),'alpha_l1':int(np.abs(out.astype(int)-native).sum()),'native_sum':int(native.sum()),'candidate_sum':int(out.sum()),'max_error':int(np.abs(out.astype(int)-native).max())}))
if a.output:
    Image.fromarray(out).save(a.output)
