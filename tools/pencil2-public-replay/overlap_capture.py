"""Capture two independently sampled public streams on host RTV4 over an opaque prior."""
import hashlib, json, struct, zlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py'
stream=(ROOT/'crates/sdocx/tests/assets/pencil2-translation/ramp.bin').read_bytes()
second=(ROOT/'crates/sdocx/tests/assets/pencil2-overlap-black/flat.bin').read_bytes()
assert len(stream)==len(second)
prior=bytes(c for y in range(192) for x in range(192) for c in ((x*3+y)%256,(x+y*5)%256,(x*7+y*3)%256,255))
bottom=b''.join(prior[y*768:(y+1)*768] for y in reversed(range(192)))
cases={'overlap':[1,1,6,9]}
injection='''    get_string=p._cfunc('glGetString',C.c_char_p,[C.c_uint])
    GL_INFO.update({key:get_string(enum).decode() for key,enum in [('vendor',0x1f00),('renderer',0x1f01),('version',0x1f02),('shading_language',0x8b8c)]})
    bind_texture=p._cfunc('glBindTexture',None,[C.c_uint,C.c_uint])
    upload=p._cfunc('glTexSubImage2D',None,[C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
    for name, (sx,sy,tx,ty) in CASES.items():
        p.u.mem_write(vector_begin,STREAM)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        p.u.mem_write(matrix,struct.pack('<16f',sx,0,0,0,0,sy,0,0,0,0,1,0,tx,ty,0,1))
        prior_buffer=C.create_string_buffer(PRIOR)
        bind_texture(0x0de1,target_texture)
        upload(0x0de1,0,0,0,width,height,0x1908,0x1401,prior_buffer)
        f['glBindFramebuffer'](0x8d40,draw_target_fbo)
        f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
        if pixels.raw!=PRIOR: raise RuntimeError('prior upload mismatch')
        f['glBindFramebuffer'](0x8d40,0)
        p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
        p.u.mem_write(vector_begin,SECOND)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        p.u.mem_write(matrix,struct.pack('<16f',1,0,0,0,0,1,0,0,0,0,1,0,9,11,0,1))
        p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
        f['glBindFramebuffer'](0x8d40,draw_target_fbo)
        f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
        if f['glGetError'](): raise RuntimeError('GL error')
        raw=pixels.raw
        RESULTS[name]=b''.join(raw[y*768:(y+1)*768] for y in reversed(range(192)))
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''
source=SOURCE.read_text();needle='    graphics=p.get(rt+200,"Q")[0]';assert source.count(needle)==1
ns={'__name__':'translation_capture','__file__':str(SOURCE),'CASES':cases,'STREAM':stream,'SECOND':second,'PRIOR':bottom,'RESULTS':{},'GL_INFO':{}}
exec(compile(source.replace(needle,injection+needle),str(SOURCE),'exec'),ns)
baseline=ns['capture'](ROOT/'scratch/pencil2-mixed-rgb')
metadata={'gl':ns['GL_INFO'],'dso_pins':ns['DSO_PINS'],'source_pins':ns['SOURCE_PINS'],'baseline_sha256':baseline['mesa']['readback_sha256']}
(ROOT/'tools/pencil2-public-replay/overlap-capture-info.json').write_text(json.dumps(metadata,indent=2)+'\n')
for name,raw in ns['RESULTS'].items():
 (ROOT/f'tools/pencil2-public-replay/{name}-prior.rgba.zlib').write_bytes(zlib.compress(raw,9))
 print(name,hashlib.sha256(raw).hexdigest(),sum(raw[i:i+4]!=prior[i:i+4] for i in range(0,len(raw),4)))
