"""Probe synthetic bitmap matrices in Drawing-to-Mesa host capture.

Uses the pinned project-local native helpers and scratch libraries. The 221-record
fixture is an expected buffer only: the native Drawing path regenerates it.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

if not __debug__:
    raise SystemExit('Python optimization disables capture verification')

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / 'tools/brushpen-transform/second_mapdraw.py'
PROBE = ROOT / 'tools/brushpen-transform/long_probe.py'
EVIDENCE = ROOT / 'conformance/brushpen-second-native'
PINS = {
    RUNNER: '9e9632df53c725b6018ba6892edc3605649762f7eba6e7b7098f25af6a8d185f',
    PROBE: '77e48b4625361cd14eb5aa4637e31af5e7e3da52794547e93681f5b0e6bbeb40',
    ROOT / 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_paint_positive_native.py': '3223d0d0d85a2d228c6e348253c6c90c7bcc1944304a8de15b0a760bf0234242',
    ROOT / 'tmp/conformance-cleanup/native-capture-a241793/conformance/brushpen_queue_native.py': '8d9672741385b77c02445e039e5982e0deb744a01ae51c77b1dc2c554baf5e26',
    ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenBrushPen.so': 'acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce',
    ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
    EVIDENCE / 'geometry-221x44.bin': '9c0e7c402e7bd064b37612b6bcaf8718f4475771cbecb98503afbabb69c61b3d',
}
for path, expected in PINS.items():
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit(f'input hash mismatch: {path}')

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError(f'expected one injection anchor: {old}')
    return source.replace(old, new)

with tempfile.TemporaryDirectory(prefix='brushpen-scale-controls-') as directory:
    tmp = Path(directory)
    geometry = tmp / 'geometry.json'
    geometry.write_text(json.dumps({'producer': {'buffer_hex': (EVIDENCE / 'geometry-221x44.bin').read_bytes().hex()}}))
    probe_source = PROBE.read_text()
    probe_source = probe_source.replace('ROOT=Path(__file__).resolve().parents[2]', f'ROOT=Path({str(ROOT)!r})')
    hook = '''
import struct
from unicorn import UC_HOOK_CODE
trace = {'bitmap_callbacks': [], 'draw_entry_matrices': []}
original_bitmap_callback = p.draw_bitmap_callback
def capture_bitmap_callback(kind):
    ptr = p.x(8)
    original_bitmap_callback(kind)
    if kind == 'bitmap_matrix':
        trace['bitmap_callbacks'].append(bytes(p.u.mem_read(ptr, 64)).hex())
p.draw_bitmap_callback = capture_bitmap_callback
draw_address = p.syms['SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']
def draw_entry(u, address, size, user):
    trace['draw_entry_matrices'].append(bytes(u.mem_read(p.x(2), 64)).hex())
p.u.hook_add(UC_HOOK_CODE, draw_entry, begin=draw_address, end=draw_address)
'''
    probe_source = probe_source.replace("for n in ['SPen::ObjectDrawing", hook + "\nfor n in ['SPen::ObjectDrawing")
    local_probe = tmp / 'long_probe.py'
    local_probe.write_text(probe_source)
    source = RUNNER.read_text()
    source = replace_once(source, "Path(__file__).with_name('long_probe.py')", f'Path({str(local_probe)!r})')
    source = replace_once(source, '.read_text().split("for n in [\'SPen::ObjectDrawing")[0])',
        '.read_text().split("for n in [\'SPen::ObjectDrawing")[0].replace(\'ROOT=Path(__file__).resolve().parents[2]\', '
        + repr(f'ROOT=Path({str(ROOT)!r})') + '))')
    source = replace_once(source, " print('TASK2_STATE',json.dumps(getattr(p,'last_task2_result',{}),sort_keys=True))", " print('MATRIX_TRACE',json.dumps(trace,sort_keys=True))\n print('TASK2_STATE',json.dumps(getattr(p,'last_task2_result',{}),sort_keys=True))")
    source = replace_once(source, "Path('/tmp/brushpen-second-geometry.json')", f'Path({str(geometry)!r})')
    for old, name in [('Path(\'/tmp/brushpen-rebound-source.py\')', 'rebound.py'),
                      ("'/tmp/brushpen-source-dump.py'", 'dump.py'),
                      ("'/tmp/brushpen-source-after-injection.py'", 'after.py')]:
        if old in source:
            source = source.replace(old, f'Path({str(tmp / name)!r})' if old.startswith('Path(') else repr(str(tmp / name)))
    script = tmp / 'capture.py'
    script.write_text(source)
    for name, bitmap in [
        ('identity', '1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1'),
        ('translated', '1,0,0,0,0,1,0,0,0,0,1,0,17,29,0,1'),
        ('scaled', '2,0,0,0,0,0.5,0,0,0,0,1,0,0,0,0,1'),
    ]:
        env = dict(os.environ, BRUSHPEN_CANVAS_MATRIX='1,0,0,0,1,0,0,0,1', BRUSHPEN_RUN_CREATE_CANVAS_TASK='1', BRUSHPEN_NATIVE_PEN_DATA_MANAGER='1', BRUSHPEN_BITMAP_MATRIX=bitmap, BRUSHPEN_EXPLICIT_COLOR='0')
        env['PYTHONPATH'] = str(ROOT / 'scratch/apk-analysis-runtime/python') + ':' + str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin')
        result = subprocess.run([sys.executable, str(script)], cwd=ROOT, env=env, check=True, capture_output=True, text=True)
        lines = result.stdout.splitlines()
        assert any(line.startswith('SAME_DRAWING_RESULT ') for line in lines)
        assert not any(line.startswith('SAME_DRAWING_STOP ') for line in lines)
        def parse(prefix):
            return json.loads(next(line[len(prefix):] for line in lines if line.startswith(prefix)))
        uniforms = parse('UNIFORM_NATIVE_IMPORT_TRACE ')
        state = parse('TASK2_STATE ')
        target = state['native_canvas_rgba_target']
        frame = base64.b64decode(target['after_task2_base64'], validate=True)
        observed = {'matrix_trace': parse('MATRIX_TRACE '), 'name': name, 'bitmap_matrix': bitmap, 'uniforms': uniforms, 'frame_sha256': hashlib.sha256(frame).hexdigest(), 'nonzero_alpha': sum(x != 0 for x in frame[3::4]), 'clear': target['before_native_task2_verified_zero'], 'gl_error': target['gl_error_after_readback']}
        expected = next(json.loads(line) for line in (ROOT / 'tools/brushpen-transform/bitmap-matrix-receipt.jsonl').read_text().splitlines() if json.loads(line)['name'] == name)
        assert observed == expected
        assert observed['clear'] and observed['gl_error'] == 0
        print(f'PASS {name}: uniforms and full frame hash match receipt', flush=True)
