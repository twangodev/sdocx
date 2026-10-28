"""Capture synthetic V1 shape controls into a compact host-Mesa receipt."""
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
cases = {'zero': {'pressure': [0] * 4}, 'full': {'pressure': [1] * 4}, 'alternating': {'pressure': [0, 1, 0, 1]}, 'diagonal': {'points': [[0, 0], [8, 8], [16, 16], [24, 24]]}}
env = os.environ.copy()
env['PYTHONPATH'] = ':'.join(str(root / p) for p in ('scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python', 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
out = {}
with tempfile.TemporaryDirectory() as tmp:
    path = Path(tmp) / 'input.json'
    for name, changes in cases.items():
        source = base | changes
        path.write_text(json.dumps(source))
        result = json.loads(subprocess.check_output([sys.executable, str(root / 'conformance/inkpen2_v1_paint_native.py'), '--print-capture', '--matrix-translate-x', '32', '--matrix-translate-y', '32', '--input-json', str(path)], cwd=root, env=env))
        out[name] = {'input': source, **{key: result[key] for key in ('geometry_sha256', 'rgba_sha256', 'rgba_zlib_base64', 'output_size', 'nonzero_pixels')}}
print(json.dumps({'schema': 'sdocx-inkpen2-null-v1-pressure-paint-native-v1', 'scope': 'Synthetic ObjectStroke native Drawing and RTV1 host Mesa, transparent 128x96 target; not device or app canvas.', 'capture_script': 'conformance/inkpen2_v1_pressure_paint_capture.py', 'cases': out}, indent=2))
