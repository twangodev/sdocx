"""Compare native host paint for six shapes across explicit V1 profiles."""
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
baseline = json.loads((root / 'conformance/inkpen2-null-v1-shapes-paint-native.json').read_text())['cases']
env = os.environ.copy()
env['PYTHONPATH'] = os.pathsep.join(str(root / p) for p in (
    'scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python',
    'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
def capture(job):
    name, width, height = job
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / 'input.json'
        path.write_text(json.dumps(baseline[name]['input']))
        result = json.loads(subprocess.check_output([
            sys.executable, str(root / 'conformance/inkpen2_v1_paint_native.py'),
            '--print-capture', '--matrix-translate-x', '32', '--matrix-translate-y', '32',
            '--input-json', str(path), '--width', str(width), '--height', str(height)],
            cwd=root, env=env))
        return name, width, height, {key: result[key] for key in (
            'geometry_hex', 'geometry_sha256', 'rgba_sha256', 'rgba_zlib_base64', 'output_size', 'nonzero_pixels')}
jobs = [(name, width, height) for name in baseline for width, height in ((1080, 1920), (1440, 2560))]
with ThreadPoolExecutor(max_workers=3) as pool:
    results = list(pool.map(capture, jobs))
cases = {}
for name, width, height, result in results:
    original = baseline[name]
    result['matches_1920x1080_geometry'] = result['geometry_sha256'] == original['geometry_sha256']
    result['matches_1920x1080_rgba'] = result['rgba_sha256'] == original['rgba_sha256']
    cases.setdefault(name, {})[f'{width}x{height}'] = result
receipt = {'schema': 'sdocx-inkpen2-v1-screen-paint-native-v1',
           'scope': 'Six synthetic null-setting strokes, native Drawing and RTV1 on transparent host Mesa at 1x. Explicit screen profiles, identity model matrix and translated output. No device, app canvas or serialized note.',
           'capture_script': 'conformance/inkpen2_v1_screen_paint_capture.py',
           'baseline_receipt': 'conformance/inkpen2-null-v1-shapes-paint-native.json', 'cases': cases}
(root / 'conformance/inkpen2-v1-screen-paint-native.json').write_text(json.dumps(receipt, indent=2) + '\n')
