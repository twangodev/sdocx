"""Compare the bitwise model with uniforms from full synthetic native draws."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
base = root / 'tools/pencil2-public-replay/drawing_native_target_translated_probe.py'
independent = root / 'tools/pencil2-independent-drawing/capture.py'
libm = ctypes.CDLL('libm.so.6')
fma = libm.fmaf
fma.argtypes = [ctypes.c_float] * 3
fma.restype = ctypes.c_float
f = lambda x: ctypes.c_float(x).value

def model(b, d, point, dimensions):
    m = []
    for j in range(4):
        for i in range(4):
            a = f(b[4+i] * d[4*j+1])
            a = fma(b[i], d[0] if j == 3 and i == 2 else d[4*j], a)
            a = fma(b[8+i], d[4*j+2], a)
            m.append(fma(b[12+i], d[4*j+3], a))
    vec = []
    for i in range(4):
        a = f(m[4+i] * point[1])
        a = fma(m[i], point[0], a)
        a = fma(m[8+i], 0, a)
        vec.append(fma(m[12+i], 1, a))
    return struct.pack('<2f', *(f(f(f(f(vec[i] / vec[3]) + 1) * .5) * dim) for i, dim in enumerate(dimensions))).hex()

cases = {
 'translation': ('5,9,15,9,25,11,35,19', '1,0,0,0,0,1,0,0,0,0,1,0,12,12,0,1'),
 'scale': ('-5,-7,5,-7,15,-5,25,3', '2,0,0,0,0,0.5,0,0,0,0,1,0,12,12,0,1'),
 'rotate': ('5,9,15,9,25,11,35,19', '0,1,0,0,-1,0,0,0,0,0,1,0,12,12,0,1'),
 'shear': ('3.25,-4.5,13,5,23,7,33,15', '1,0.25,0,0,0.5,1,0,0,0,0,1,0,12,12,0,1'),
 'perspective': ('5,9,15,9,25,11,35,19', '1,0.2,0,0.001,0.3,1,0,-0.002,0,0,1,0,12,12,0,1'),
 'negative_origin': ('-80,-60,-70,-60,-60,-58,-50,-50', '1,0,0,0,0,1,0,0,0,0,1,0,-24,8,0,1'),
 'clipped_far_edge': ('185,183,195,183,205,185,215,193', '0.5,0,0,0,0,1.5,0,0,0,0,1,0,16,-32,0,1'),
}
results = {}
with tempfile.TemporaryDirectory(dir=root / 'tmp', prefix='pencil-offset-lifecycle-') as directory:
    tmp = Path(directory)
    probe = tmp / 'probe.py'
    code = base.read_text()
    marker = '    return code\n\nm.adapt_source = adapt'
    instrumentation = '''    code = code.replace("event_order=[]\\nhandoff_events=[]", """event_order=[]
offset_events=[]
matrix_due=[False]
def offset_hook(u,a,z,d):
 if a == 0x5098a88:
  offset_events.append({'kind':'set','point':struct.pack('<II',u.reg_read(R.UC_ARM64_REG_S0),u.reg_read(R.UC_ARM64_REG_S1)).hex()})
 if a == 0x5099938:
  offset_events.append({'kind':'transform','state':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X0)+240,9)).hex()})
 if a == 0x50996fc:
  matrix_due[0]=True
 if a == 0x5094674 and matrix_due[0]:
  matrix_due[0]=False
  offset_events.append({'kind':'matrix','b':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X21),64)).hex(),'d':bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_X20),64)).hex()})
p.u.hook_add(UC_HOOK_CODE,offset_hook)
handoff_events=[]""")
    code = code.replace("data=C.create_string_buffer(raw);ip.cfunc(p.mesa.gl,'glUniform2fv'", "offset_events.append({'kind':'uniform','bits':raw.hex()});data=C.create_string_buffer(raw);ip.cfunc(p.mesa.gl,'glUniform2fv'")
    code = code.replace(" result['native_target_probe']=target", " target['offset_events']=offset_events\\n result['native_target_probe']=target")
'''
    assert code.count(marker) == 1
    probe.write_text(code.replace(marker, instrumentation + marker))
    for name, (points, matrix) in cases.items():
        out = tmp / name
        out.mkdir()
        source = independent.read_text().replace('root = Path(__file__).resolve().parents[2]', f'root = Path({str(root)!r})', 1)
        source = source.replace('here = Path(__file__).resolve().parent', f'here = Path({str(out)!r})', 1)
        source = source.replace("root / 'tools/pencil2-public-replay/drawing_native_target_translated_probe.py'", f'Path({str(probe)!r})', 1)
        assert source.count('0,2,10,2,20,4,30,12') == 1
        source = source.replace('0,2,10,2,20,4,30,12', points)
        source = source.replace("'outer_outcome':target.get('outcome')", "'offset_events':target.get('offset_events'), 'outer_outcome':target.get('outcome')", 1)
        old = "p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,12,12,0,1)"
        source = source.replace('    probe.write_text(code)', f'    code = code.replace({old!r}, {("p.put(matrix,'16f'," + matrix + ")")!r})\n    probe.write_text(code)')
        old = "   target['independent_second']['stage']='draw_returned'"
        new = old + '''
   p.put(matrix,'16f',1,0,0,0,0,1,0,0,0,0,1,0,48,36,0,1)
   p.call('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt2,bitmap,matrix],limit=8000000)
   target['independent_second']['repeat_error']=f['glGetError']()
   p.f(0, -7.25); p.f(1, 4.5)
   p.call('SPen::Pencil2StrokeDrawableRTV4::SetStartPoint(float, float)',[rt2])
   p.call('SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',[rt2,bitmap,matrix],limit=8000000)
   target['independent_second']['dirty_error']=f['glGetError']()
'''
        assert source.count(old) == 1
        script = tmp / (name + '.py')
        script.write_text(source.replace(old, new))
        run = subprocess.run(['python3', str(script)], cwd=root, env=os.environ.copy(), capture_output=True, text=True)
        if run.returncode:
            raise RuntimeError(f'{name}: {run.stdout[-3000:]} {run.stderr[-3000:]}')
        observed = json.loads((out / 'receipt.json').read_text())
        assert observed['outer_outcome'] is None and observed['outer_error'] is None
        assert observed['second']['same_rt'] and observed['second']['stage'] == 'draw_returned'
        assert observed['first_gl_error'] == observed['second_gl_error'] == 0
        assert all(observed['second'][k] == 0 for k in ('repeat_error', 'dirty_error'))
        events = observed['offset_events']
        assert [e['kind'] for e in events] == ['set','set','matrix','transform','uniform','set','matrix','transform','uniform','uniform','set','matrix','transform','uniform']
        assert all(e['state'][-2:] == '01' for e in events if e['kind'] == 'transform')
        sets = [e['point'] for e in events if e['kind'] == 'set']
        assert sets[2] == struct.pack('<2f', *map(float, points.split(',')[:2])).hex()
        assert sets[3] == struct.pack('<2f', -7.25, 4.5).hex()
        matrices = [e['d'] for e in events if e['kind'] == 'matrix']
        assert matrices[1] == struct.pack('<16f', *map(float, matrix.split(','))).hex()
        current = None
        pending = None
        rows = []
        for event in events:
            if event['kind'] == 'set':
                pending = struct.unpack('<2f', bytes.fromhex(event['point']))
            elif event['kind'] == 'matrix':
                assert pending is not None
                b = struct.unpack('<16f', bytes.fromhex(event['b']))
                d = struct.unpack('<16f', bytes.fromhex(event['d']))
                current = model(b, d, pending, (192, 192))
                pending = None
            elif event['kind'] == 'uniform':
                assert current == event['bits'], (name, current, event)
                rows.append({'transition': ('initial', 'new_stroke', 'repeat', 'dirty')[len(rows)], 'expected_bits':current, 'uniform_bits':event['bits']})
        assert len(rows) == 4 and pending is None
        results[name] = {'events':events, 'comparisons':rows, 'gl_errors':[observed['first_gl_error'], observed['second_gl_error'], observed['second']['repeat_error'], observed['second']['dirty_error']]}
receipt = {'scope':'Synthetic ARM64 Drawing queue intervention and host Mesa; 192x192 bitmap. Matrices and point bits captured at native execution; actual glUniform2fv argument bytes compared with the model. Not device or saved-note parity.', 'cases':results, 'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'binary_sha256':hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenPencil2.so').read_bytes()).hexdigest(), 'dependencies': {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [base, independent, root / 'tools/pencil2-mixed-drawing/capture.patch', root / 'tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py', root / 'tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance/inkpen_paint_native.py', root / 'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so']}, 'limitations':'Finite selected cases only. libm fmaf and host floating-point behavior are required; other exceptional inputs and runtimes are not covered.'}
(root / 'tools/pencil2-offset-trace/lifecycle-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({k: v['comparisons'] for k,v in results.items()}, indent=2))
