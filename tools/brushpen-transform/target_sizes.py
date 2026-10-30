"""Replay four synthetic native BrushPen targets; grain stays 321x228.

Requires the project-local native scratch checkout and host Mesa/Unicorn runtime.
This verifies retained host frames, not device parity or general page sizing.
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

ROOT = Path(__file__).resolve().parents[2]
NATIVE = ROOT / 'tmp/conformance-cleanup/native-capture-a241793/conformance'
EVIDENCE = ROOT / 'conformance/brushpen-second-native'
FRAMES = ROOT / 'crates/sdocx/tests/fixtures/brushpen-target-sizes'
RUNNER = ROOT / 'tools/brushpen-transform/second_mapdraw.py'
PROBE = ROOT / 'tools/brushpen-transform/long_probe.py'
POSITIVE = NATIVE / 'brushpen_paint_positive_native.py'
PINS = {
    RUNNER: '9e9632df53c725b6018ba6892edc3605649762f7eba6e7b7098f25af6a8d185f',
    PROBE: '77e48b4625361cd14eb5aa4637e31af5e7e3da52794547e93681f5b0e6bbeb40',
    NATIVE / 'brushpen_queue_native.py': '8d9672741385b77c02445e039e5982e0deb744a01ae51c77b1dc2c554baf5e26',
    POSITIVE: '3223d0d0d85a2d228c6e348253c6c90c7bcc1944304a8de15b0a760bf0234242',
    EVIDENCE / 'geometry-221x44.bin': '9c0e7c402e7bd064b37612b6bcaf8718f4475771cbecb98503afbabb69c61b3d',
}

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise RuntimeError(f'expected one anchor: {old}')
    return source.replace(old, new, 1)

for path, digest in PINS.items():
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f'input hash mismatch: {path}')

with tempfile.TemporaryDirectory(prefix='brushpen-target-sizes-') as directory:
    tmp = Path(directory)
    geometry = tmp / 'geometry.json'
    geometry.write_text(json.dumps({'producer': {'buffer_hex': (EVIDENCE / 'geometry-221x44.bin').read_bytes().hex()}}))
    positive = POSITIVE.read_text()
    positive = replace_once(positive, 'ROOT = Path(__file__).resolve().parents[1]', f'ROOT = Path({str(NATIVE.parent)!r})')
    positive = replace_once(positive, 'if (w, h) == (144, 144) or (w, h) == (321, 228):',
        "if (w, h) == (144, 144) or (w, h) == (321, 228) or (w, h) == tuple(map(int, os.environ['BP_TARGET'].split('x'))):")
    (tmp / POSITIVE.name).write_text(positive)
    probe = PROBE.read_text()
    probe = replace_once(probe, 'ROOT=Path(__file__).resolve().parents[2]', f'ROOT=Path({str(ROOT)!r})')
    probe = replace_once(probe, 'import brushpen_queue_native as q', f"sys.path.insert(0, {str(tmp)!r})\nimport brushpen_queue_native as q")
    (tmp / 'long_probe.py').write_text(probe)
    source = RUNNER.read_text()
    source = replace_once(source, "Path(__file__).with_name('long_probe.py')", f'Path({str(tmp / "long_probe.py")!r})')
    source = replace_once(source, "Path('/tmp/brushpen-second-geometry.json')", f'Path({str(geometry)!r})')
    source = replace_once(source, " print('TASK2_STATE',json.dumps(getattr(p,'last_task2_result',{}),sort_keys=True))", " print('CAPTURE_UPLOADS',json.dumps(p.texture_uploads_real,sort_keys=True))\n print('TASK2_STATE',json.dumps(getattr(p,'last_task2_result',{}),sort_keys=True))")
    for old, name in [("Path('/tmp/brushpen-rebound-source.py')", 'rebound.py'), ("'/tmp/brushpen-source-dump.py'", 'dump.py'), ("'/tmp/brushpen-source-after-injection.py'", 'after.py')]:
        if old in source:
            source = replace_once(source, old, f'Path({str(tmp / name)!r})' if old.startswith('Path(') else repr(str(tmp / name)))
    injection = '''w,h=map(int,__import__('os').environ['BP_TARGET'].split('x'))
for old,new in [("321 * 228 * 4",f"{w} * {h} * 4"),("[bitmap_queue, 321, 228,",f"[bitmap_queue, {w}, {h},"),("'width': 321, 'height': 228",f"'width': {w}, 'height': {h}"),("0, 0, 321, 228",f"0, 0, {w}, {h}"),("'dimensions': [321, 228]",f"'dimensions': [{w}, {h}]"),("get('width') != 321",f"get('width') != {w}"),("get('height') != 228",f"get('height') != {h}")]:
    if old not in source: raise RuntimeError(f'missing target allocation anchor: {old}')
    source=source.replace(old,new)
'''
    source = replace_once(source, "ns=q.__dict__.copy();ns['ROOT']=ROOT;", injection + "ns=q.__dict__.copy();ns['ROOT']=ROOT;")
    script = tmp / 'capture.py'
    script.write_text(source)
    for size in ('256x192', '400x300'):
        for name, matrix in [('identity','1,0,0,0,1,0,0,0,1'), ('half','0.5,0,0,0,0.5,0,25,20,1')]:
            receipt = json.loads((FRAMES / f'{size}-{name}.json').read_text())
            assert receipt['matrix'] == matrix and receipt['dimensions'] == list(map(int, size.split('x')))
            reference = zlib.decompress((FRAMES / f'{size}-{name}.rgba.zlib').read_bytes())
            observed = []
            for repeat in range(2):
                env = dict(os.environ, BP_TARGET=size, BRUSHPEN_CANVAS_MATRIX=matrix, BRUSHPEN_RUN_CREATE_CANVAS_TASK='1', BRUSHPEN_NATIVE_PEN_DATA_MANAGER='1', BRUSHPEN_BITMAP_MATRIX='1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1', BRUSHPEN_EXPLICIT_COLOR='0')
                env['PYTHONPATH'] = str(ROOT / 'scratch/apk-analysis-runtime/python') + ':' + str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin')
                result = subprocess.run([sys.executable, str(script)], cwd=ROOT, env=env, capture_output=True, text=True, check=True)
                lines = result.stdout.splitlines()
                if not any(x.startswith('SAME_DRAWING_RESULT ') for x in lines) or any(x.startswith('SAME_DRAWING_STOP ') for x in lines): raise RuntimeError('native draw failed')
                def parse(prefix): return json.loads(next(x[len(prefix):] for x in lines if x.startswith(prefix)))
                state = parse('TASK2_STATE '); target = state['native_canvas_rgba_target']
                frame = base64.b64decode(target['after_task2_base64'], validate=True)
                uniforms = parse('UNIFORM_NATIVE_IMPORT_TRACE ')
                def values(key, location): return next(row['values'] for row in uniforms if row.get(key) == location)
                width, height = map(int, size.split('x'))
                scale, tx, ty = (0.5, 25, 20) if name == 'half' else (1, 0, 0)
                projection = values('location', 0)
                for actual, expected in zip([projection[i] for i in (0, 5, 12, 13)], [2*scale/width, 2*scale/height, -1+2*tx/width, -1+2*ty/height]):
                    assert abs(actual - expected) < 0.000001
                for actual, expected in zip(values('uniform2fv_location', 5) + values('uniform2fv_location', 6), [321*scale, 228*scale, 125*scale+tx, 30*scale+ty]):
                    assert abs(actual - expected) < 0.0001
                uploads = parse('CAPTURE_UPLOADS ')
                grain = [x for x in uploads if x['kind'] == 'upload' and x['dimensions'] == [321, 228]]
                assert grain and all(x['sha256'] == 'ec10a82c17d4912c8d0611b0c87664c7f26662911ed284bbad017d6e3f444beb' for x in grain)
                assert target['before_native_task2_verified_zero'] and target['gl_error_after_readback'] == 0 and not target['feedback_alias_with_pattern_or_grain']
                assert state['target_debug']['native_canvas_target_after_task2']['sha256'] == hashlib.sha256(frame).hexdigest()
                assert parse('NATIVE_TASK2_FRAMEBUFFER_STATUS ')[0]['status_enum'] == 0x8cd5
                assert frame == reference and uniforms == receipt['uniforms']
                observed.append((frame, uniforms, grain[0]['sha256']))
            assert observed[0] == observed[1]
            print(f'PASS {size} {name}: full RGBA and uniforms repeated; grain upload {observed[0][2]}', flush=True)
