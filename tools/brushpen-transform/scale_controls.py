"""Recheck two synthetic Drawing-to-Mesa scale controls against retained frames.

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
FIXTURES = ROOT / 'crates/sdocx/tests/fixtures/brushpen-target-sizes'
PINS = {
    RUNNER: '9e9632df53c725b6018ba6892edc3605649762f7eba6e7b7098f25af6a8d185f',
    PROBE: '77e48b4625361cd14eb5aa4637e31af5e7e3da52794547e93681f5b0e6bbeb40',
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
    source = RUNNER.read_text()
    source = replace_once(source, "Path(__file__).with_name('long_probe.py')", f'Path({str(PROBE)!r})')
    source = replace_once(source, '.read_text().split("for n in [\'SPen::ObjectDrawing")[0])',
        '.read_text().split("for n in [\'SPen::ObjectDrawing")[0].replace(\'ROOT=Path(__file__).resolve().parents[2]\', '
        + repr(f'ROOT=Path({str(ROOT)!r})') + '))')
    source = replace_once(source, "Path('/tmp/brushpen-second-geometry.json')", f'Path({str(geometry)!r})')
    for old, name in [('Path(\'/tmp/brushpen-rebound-source.py\')', 'rebound.py'),
                      ("'/tmp/brushpen-source-dump.py'", 'dump.py'),
                      ("'/tmp/brushpen-source-after-injection.py'", 'after.py')]:
        if old in source:
            source = source.replace(old, f'Path({str(tmp / name)!r})' if old.startswith('Path(') else repr(str(tmp / name)))
    script = tmp / 'capture.py'
    script.write_text(source)
    for name, matrix, scale, tx, ty in [
        ('half-scale', '0.5,0,0,0,0.5,0,25,20,1', .5, 25, 20),
        ('double', '2,0,0,0,2,0,-80,-40,1', 2, -80, -40),
    ]:
        env = dict(os.environ, BRUSHPEN_CANVAS_MATRIX=matrix, BRUSHPEN_RUN_CREATE_CANVAS_TASK='1', BRUSHPEN_NATIVE_PEN_DATA_MANAGER='1', BRUSHPEN_BITMAP_MATRIX='1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1', BRUSHPEN_EXPLICIT_COLOR='0')
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
        receipt = json.loads((FIXTURES / f'{name}-receipt.json').read_text())
        assert receipt['canvas_matrix'] == [float(value) for value in matrix.split(',')]
        assert receipt['capture_runner_sha256'] == hashlib.sha256(RUNNER.read_bytes()).hexdigest()
        assert receipt['geometry_sha256'] == hashlib.sha256((EVIDENCE / 'geometry-221x44.bin').read_bytes()).hexdigest()
        assert receipt['draw_returned'] == state['returned'] is True
        assert receipt['clear_verified'] == target['before_native_task2_verified_zero'] is True
        assert receipt['feedback_alias'] == target['feedback_alias_with_pattern_or_grain'] is False
        assert receipt['read_gl_error'] == target['gl_error_after_readback'] == 0
        assert receipt['dimensions'] == target['dimensions']
        width, height = target['dimensions']
        assert len(frame) == width * height * 4
        assert receipt['nonzero_alpha_pixels'] == sum(alpha != 0 for alpha in frame[3::4])
        assert hashlib.sha256(frame).hexdigest() == receipt['frame_sha256']
        assert frame == zlib.decompress((FIXTURES / f'{name}-frame.rgba.zlib').read_bytes())
        assert uniforms == receipt['draw_time_uniform_imports']
        grain = next(row['values'] for row in uniforms if row.get('uniform2fv_location') == 5)
        offset = next(row['values'] for row in uniforms if row.get('uniform2fv_location') == 6)
        assert all(abs(a-b) < .0001 for a, b in zip(grain, [scale*321, scale*228]))
        assert all(abs(a-b) < .0001 for a, b in zip(offset, [scale*125+tx, scale*30+ty]))
        print(f'PASS {name}: native draw, transformed grain, uniforms and retained frame')
