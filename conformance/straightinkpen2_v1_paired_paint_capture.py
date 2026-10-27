"""Compare literal pen names through synthetic Drawing and RTV1 host Mesa."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
base = {'points': [[0, 0], [8, 0], [16, 3], [24, 3]], 'size': 4,
        'curve_enabled': True, 'tool_type': 2, 'stroke_type': 0,
        'pressure': [.2, .4, .6, .8], 'timestamps': [0, 10, 20, 30],
        'tilt': [0] * 4, 'orientation': [0] * 4}
cases = {
    'baseline': ({}, '0xff336699', 1.0),
    'turn_half_red': ({'points': [[0, 0], [8, 0], [8, 8], [0, 8]]}, '0x80ff0000', .5),
    'short_double': ({'points': [[0, 0], [.1, 0], [.2, .1], [.3, .1]]}, '0x00336699', 2.0),
    'large_reverse': ({'points': [[0, 0], [8, 0], [0, 0], [-8, 0]], 'size': 12}, '0xff336699', 1.0),
}
env = os.environ.copy()
env['PYTHONPATH'] = os.pathsep.join(str(root / p) for p in ('scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python', 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
out = {}
with tempfile.TemporaryDirectory() as tmp:
    path = Path(tmp) / 'input.json'
    for name, (changes, color, scale) in cases.items():
        source = base | changes
        path.write_text(json.dumps(source))
        captures = []
        for pen in ('InkPen2', 'StraightInkPen2'):
            cmd = [sys.executable, str(root / 'conformance/inkpen2_v1_paint_native.py'), '--print-capture', '--matrix-translate-x', '32', '--matrix-translate-y', '32', '--matrix-scale', str(scale), '--color-argb', color, '--input-json', str(path), '--pen-name', pen]
            captures.append(json.loads(subprocess.check_output(cmd, cwd=root, env=env)))
        a, b = captures
        for key in ('geometry_hex', 'geometry_sha256', 'rgba_zlib_base64', 'rgba_sha256', 'draw_calls', 'shader_programs', 'output_size', 'draw_matrix'):
            assert a[key] == b[key], (name, key)
        out[name] = {'input': source, 'color_argb': color, 'scale': scale,
                     'geometry_sha256': a['geometry_sha256'], 'rgba_sha256': a['rgba_sha256'],
                     'rgba_zlib_base64': a['rgba_zlib_base64'], 'output_size': a['output_size'],
                     'nonzero_pixels': a['nonzero_pixels'], 'selected_versions': [a['selected_version'], b['selected_version']]}
print(json.dumps({'schema': 'sdocx-straightinkpen2-null-v1-paired-paint-native-v1',
                  'scope': 'Paired synthetic ObjectStroke Drawing and RTV1 host Mesa, transparent target; no serialized notes or device output.',
                  'capture_script': 'conformance/straightinkpen2_v1_paired_paint_capture.py',
                  'adapter_sha256': hashlib.sha256((root / 'conformance/inkpen2_v1_paint_native.py').read_bytes()).hexdigest(),
                  'compared': ['geometry bytes', 'RGBA bytes', 'draw calls', 'shader programs', 'output size', 'draw matrix'], 'cases': out}, indent=2))
