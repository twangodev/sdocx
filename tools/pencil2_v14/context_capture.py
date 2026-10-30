"""Capture unmodified Pencil2 RTV4 paint with a page-style negative-Y matrix.

PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/context_capture.py
"""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py'
colored = (ROOT / 'conformance/pencil2-v14-color-80336699-native.bin').read_bytes()[:4704]
second = (ROOT / 'conformance/pencil2-v14-color-ffdd4411-native.bin').read_bytes()
black = (ROOT / 'conformance/pencil2-v14-native.bin').read_bytes()
sources = [colored, second, black[14112:18816], black[18816:23520], black[23520:28224]]
mixed = b''.join(sources[i % 5][i*56:(i+1)*56] for i in range(84))
streams = {'colored':colored,'mixed':mixed,'mixed_reversed':b''.join(reversed([mixed[i:i+56] for i in range(0,4704,56)]))}
streams['inverse_half'] = (ROOT / 'conformance/pencil2-v14-inverse-half-native.bin').read_bytes()
batch = []
for origin in ['top','bottom']:
    for scale in [1,2]:
        matrix = [scale,0,0,0, 0,-scale if origin=='top' else scale,0,0, 0,0,1,0, 0,192 if origin=='top' else 0,0,1]
        for name, stream in streams.items():
            if origin=='bottom' and scale==1 and name!='inverse_half':
                continue
            for suffix, background in [('',[0,0,0,0]),('_background',[64,32,96,128]),('_white',[255]*4),('_paper',[64,32,96,255])]:
                batch.append((f'{name}_{origin}_scale{scale}{suffix}',stream,background,matrix,0.5 if name=='inverse_half' else 1.0))

injection = '''    inverse_control=p.alloc(8)
    for name, stream, background, transform, inverse in BATCH_INPUTS:
        p.u.mem_write(inverse_control,struct.pack('<2f',inverse,inverse))
        p.call(RT_SET_INVERSE_SCALE,[rt,inverse_control],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        p.u.mem_write(matrix,struct.pack('<16f',*transform))
        p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
        f['glViewport'](0,0,width,height)
        f['glClearColor'](*[v/255 for v in background]); f['glClear'](0x00004000)
        f['glBindFramebuffer'](0x8d40,0)
        event_start=len(p.gl_events)
        p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
        uniforms=[row for row in p.gl_events[event_start:] if row.get('name','').startswith('glUniform') and 'values' in row]
        assert BATCH_UNIFORMS.setdefault(str(transform[5])+':'+str(inverse),uniforms)==uniforms
        f['glBindFramebuffer'](0x8d40,draw_target_fbo)
        f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
        if f['glGetError'](): raise RuntimeError('GL error: '+name)
        frame=pixels.raw
        sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if list(frame[i:i+4])!=background]
        BATCH_RESULTS[name]={'sha256':hashlib.sha256(frame).hexdigest(),'background':background,'matrix':transform,'geometry_inverse_scale':inverse,'pixels':sparse}
        print(name,len(sparse),flush=True)
    p.u.mem_write(inverse_control,struct.pack('<2f',1.,1.))
    p.call(RT_SET_INVERSE_SCALE,[rt,inverse_control],instruction_limit=5_000_000)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''
original = SOURCE.read_text()
assert hashlib.sha256(original.encode()).hexdigest() == '34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d'
needle = '    graphics=p.get(rt+200,"Q")[0]'
assert original.count(needle) == 1
ns = {'__name__':'context_capture','__file__':str(SOURCE),'BATCH_INPUTS':batch,'BATCH_RESULTS':{},'BATCH_UNIFORMS':{}}
exec(compile(original.replace(needle,injection+needle),str(SOURCE),'exec'),ns)
baseline = ns['capture'](ROOT / 'scratch/pencil2-mixed-rgb')
receipt = {'schema':'sdocx-pencil2-v14-page-context-native-v1','frame':[192,192],
           'coordinates':'Sparse bottom-origin premultiplied RGBA8; source coordinates transformed by the recorded native Draw matrix.',
           'inputs':{name:hashlib.sha256(stream).hexdigest() for name,stream in streams.items()},
           'capture':{'script':'tools/pencil2_v14/context_capture.py','script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                      'base_source_sha256':hashlib.sha256(original.encode()).hexdigest(),'dso_pins':ns['DSO_PINS'],
                      'baseline_sha256':baseline['mesa']['readback_sha256']},
           'scope':'Native synthetic records with positive-Y and negative-Y page-style Draw matrices at scales 1 and 2. Bottom-origin scale-1 color/mixed controls already exist in the earlier mixed-RGB receipt and are not duplicated here. Geometry inverse-scale state is 1 for the color/mixed controls and 0.5 for inverse_half, set through the native public method before Update. Native Update, Draw, shader and textures unmodified. Host Mesa; not full Drawing, serialized-note or device evidence.',
           'uniform_locations':baseline['mesa']['uniform_locations'],'uniforms_by_scale':ns['BATCH_UNIFORMS'],
           'results':ns['BATCH_RESULTS']}
output = ROOT / 'conformance/pencil2-v14-page-context-native.json'
output.write_text(json.dumps(receipt,separators=(',',':'))+'\n')
print(output,output.stat().st_size)
