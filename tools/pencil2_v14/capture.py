"""Native RTV4 replay; host substitutes records only at the upload boundary.

Run from the repository root with PYTHONPATH=scratch/apk-analysis-runtime/python.
The native shader, textures, Update and Draw are unmodified. No RT fields or
queue state are patched. Results are sparse, bottom-origin premultiplied RGBA8.
"""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'conformance/pencil2-v14-mixed-rgb-native.json'
SOURCE = ROOT / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py'
colored = (ROOT / 'conformance/pencil2-v14-color-80336699-native.bin').read_bytes()[:4704]
black = (ROOT / 'conformance/pencil2-v14-native.bin').read_bytes()[:4704]
all_black = (ROOT / 'conformance/pencil2-v14-native.bin').read_bytes()
second = (ROOT / 'conformance/pencil2-v14-color-ffdd4411-native.bin').read_bytes()
assert hashlib.sha256(second).hexdigest() == '5a1a5c3a88b3b20d67043d76be1410878cb2093f4029a67635be2be50df29483'
sources = [colored, second, all_black[14112:18816], all_black[18816:23520], all_black[23520:28224]]
mixed = b''.join(sources[i % 5][i*56:(i+1)*56] for i in range(84))
streams = {'colored': colored, 'mixed': mixed, 'mixed_reversed': b''.join(reversed([mixed[i:i+56] for i in range(0,4704,56)]))}
blank = bytearray(colored[:56])
struct.pack_into('<f', blank, 16, 0.)
batch = []
for name, stream in streams.items():
    batch.append((name, stream, [0,0,0,0]))
    batch.append((name+'_background', stream, [64,32,96,128]))
    batch.append((name+'_white', stream, [255,255,255,255]))
    batch.append((name+'_paper', stream, [64,32,96,255]))
    if name != 'mixed_reversed':
        for i in range(84):
            batch.append((f'{name}_single_{i}', stream[i*56:(i+1)*56]+bytes(blank)*83, [0,0,0,0]))
            if name == 'mixed':
                batch.append((f'{name}_white_single_{i}', stream[i*56:(i+1)*56]+bytes(blank)*83, [255,255,255,255]))

injection = '''    for batch_name, batch_stream, background in BATCH_INPUTS:
        p.u.mem_write(vector_begin,batch_stream)
        p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
        p.u.mem_write(vector_begin,actual_geometry)
        p.call(ACTIVATE_SUBBITMAP_RT,[subbitmap,0],instruction_limit=5_000_000)
        f["glViewport"](0,0,width,height)
        f["glClearColor"](*[v/255 for v in background]); f["glClear"](0x00004000)
        f["glBindFramebuffer"](0x8d40,0)
        p.call(RT_DRAW,[rt,subbitmap,matrix],instruction_limit=8_000_000)
        f["glBindFramebuffer"](0x8d40,draw_target_fbo)
        f["glReadPixels"](0,0,width,height,0x1908,0x1401,pixels)
        if f["glGetError"](): raise RuntimeError('batch GL error: '+batch_name)
        raw_batch=pixels.raw
        sparse=[[i//4%width,i//4//width,*raw_batch[i:i+4]] for i in range(0,len(raw_batch),4) if list(raw_batch[i:i+4])!=background]
        BATCH_RESULTS[batch_name]={'sha256':hashlib.sha256(raw_batch).hexdigest(),'background':background,'pixels':sparse}
        print(batch_name,len(sparse),flush=True)
    p.call(RT_UPDATE,[rt,0],instruction_limit=5_000_000)
'''
original = SOURCE.read_text()
assert hashlib.sha256(original.encode()).hexdigest() == '34d5874cb86e27b619c01e9902b0aacf743619be560d8a4ca5ee65789ca8193d'
needle = '    graphics=p.get(rt+200,"Q")[0]'
assert original.count(needle) == 1
code = original.replace(needle, injection + needle)
ns = {'__name__':'mixed_rgb_capture','__file__':str(SOURCE),'BATCH_INPUTS':batch,'BATCH_RESULTS':{}}
exec(compile(code,str(SOURCE),'exec'),ns)
native = ns['capture'](ROOT / 'scratch/pencil2-mixed-rgb')
receipt = {
    'schema':'sdocx-pencil2-v14-mixed-rgb-native-v1',
    'frame':[192,192], 'coordinates':'bottom-origin; premultiplied RGBA8',
    'inputs':{'colored_sha256':hashlib.sha256(colored).hexdigest(),'black_sha256':hashlib.sha256(black).hexdigest(),
              'second_color_sha256':hashlib.sha256(second).hexdigest(),
              'mixed_sha256':hashlib.sha256(mixed).hexdigest(),'mixed_rule':'native ordinal i from [size2 color80336699, size2 colorffdd4411, tilt_all_079, orientation_all_075, size_8][i % 5]; reversed reverses all 84 records'},
    'capture':{'script':'tools/pencil2_v14/capture.py','command':'PYTHONPATH=scratch/apk-analysis-runtime/python python3 tools/pencil2_v14/capture.py',
               'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               'base_source_sha256':hashlib.sha256(original.encode()).hexdigest(),'adapted_source_sha256':hashlib.sha256(code.encode()).hexdigest(),
               'dso_pins':ns['DSO_PINS'],'baseline_sha256':native['mesa']['readback_sha256']},
    'scope':'Host-Mesa replay of native synthetic records, mixed across two colors, tilt, orientation and size controls or reversed at GPU upload. Each single isolates one record and pads 83 with zero record alpha. Native Update and Draw, shader and textures are unchanged. Not full Drawing, saved-note, Android, device or production output.',
    'results':ns['BATCH_RESULTS']}
OUT.write_text(json.dumps(receipt,separators=(',',':'))+'\n')
print(OUT, OUT.stat().st_size)
