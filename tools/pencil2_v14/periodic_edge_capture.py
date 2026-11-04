"""Pair clipped stamps and a nominal grain-period translation; see README for scope."""
import hashlib
import json
import struct
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py'
SOURCE_SHA = '34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d'
colored = (ROOT / 'conformance/pencil2-v14-color-80336699-native.bin').read_bytes()[:4704]
second = (ROOT / 'conformance/pencil2-v14-color-ffdd4411-native.bin').read_bytes()
black = (ROOT / 'conformance/pencil2-v14-native.bin').read_bytes()
sources = [colored, second, black[14112:18816], black[18816:23520], black[23520:28224]]
mixed = b''.join(sources[i % 5][i*56:(i+1)*56] for i in range(84))
streams = {'mixed': mixed}
for ordinal in (17, 20, 32, 35):
    stream = bytearray(mixed)
    for i in range(84):
        if i != ordinal-1:
            struct.pack_into('<f', stream, i*56+16, 0.)
    streams[f'record_{ordinal}'] = bytes(stream)
INJECT = '''    for name, stream in EDGE_STREAMS.items():
        p.u.mem_write(vector_begin,stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        for dy in (0,-150):
            transform=[2,0,0,0, 0,-2,0,0, 0,0,1,0, 0,192+dy,0,1]
            p.u.mem_write(matrix,struct.pack('<16f',*transform))
            p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
            f['glViewport'](0,0,width,height)
            f['glClearColor'](0.,0.,0.,0.); f['glClear'](0x00004000)
            f['glBindFramebuffer'](0x8d40,0)
            p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
            f['glBindFramebuffer'](0x8d40,draw_target_fbo)
            f['glReadPixels'](0,0,width,height,0x1908,0x1401,pixels)
            if f['glGetError'](): raise RuntimeError('edge control GL error')
            frame=pixels.raw
            sparse=[[i//4%width,i//4//width,*frame[i:i+4]] for i in range(0,len(frame),4) if frame[i+3]]
            EDGE_RESULTS[name+':'+str(dy)]={'sha256':hashlib.sha256(frame).hexdigest(),'matrix':transform,'pixels':sparse}
            print(name,dy,len(sparse),flush=True)
    p.u.mem_write(vector_begin,actual_geometry)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''
source = SOURCE.read_text()
assert hashlib.sha256(source.encode()).hexdigest() == SOURCE_SHA
needle = '    graphics=p.get(rt+200,"Q")[0]'
assert source.count(needle) == 1
ns = {'__name__':'edge_interior_capture','__file__':str(SOURCE),'EDGE_STREAMS':streams,'EDGE_RESULTS':{}}
exec(compile(source.replace(needle,INJECT+needle),str(SOURCE),'exec'),ns)
with tempfile.TemporaryDirectory(prefix='sdocx-pencil2-edge-') as directory:
    baseline = ns['capture'](Path(directory))
page = json.loads((ROOT/'conformance/pencil2-v14-page-context-native.json').read_text())
assert ns['EDGE_RESULTS']['mixed:0']['sha256'] == page['results']['mixed_top_scale2']['sha256']
receipt = {'schema':'sdocx-pencil2-v14-periodic-edge-native-v1','frame':[192,192],
 'coordinates':'bottom-origin premultiplied RGBA8; transparent clear',
 'scope':'Native Update and Draw and shaders unchanged, 84-record upload layout with nonselected alpha zeroed. A 150-pixel shift is one nominal grain period at scale 2, but floating-point interpolation can change phase even at a nominal period. Differences are not proven to isolate clipping. Host Mesa synthetic records; not device or saved-note evidence.',
 'capture':{'script':'tools/pencil2_v14/periodic_edge_capture.py','script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'base_source_sha256':SOURCE_SHA,'baseline_sha256':baseline['mesa']['readback_sha256'],'dso_pins':ns['DSO_PINS']},
 'inputs':{k:hashlib.sha256(v).hexdigest() for k,v in streams.items()},'results':ns['EDGE_RESULTS']}
out=ROOT/'conformance/pencil2-v14-periodic-edge-native.json'
out.write_text(json.dumps(receipt,separators=(',',':'))+'\n')
print(out, out.stat().st_size)
