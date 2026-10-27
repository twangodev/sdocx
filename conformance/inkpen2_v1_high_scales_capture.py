"""Capture six synthetic V1 strokes at 1440x2560 and 0.5x/2x output."""
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
env['PYTHONPATH'] = os.pathsep.join(str(root / p) for p in ('scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python', 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
def capture(job):
    name, scale = job
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / 'input.json'
        path.write_text(json.dumps(baseline[name]['input']))
        result = json.loads(subprocess.check_output([
            sys.executable, str(root / 'conformance/inkpen2_v1_paint_native.py'),
            '--print-capture', '--width', '1440', '--height', '2560',
            '--matrix-scale', str(scale), '--matrix-translate-x', str(32 * scale),
            '--matrix-translate-y', str(32 * scale), '--output-width', str(int(128 * scale)),
            '--output-height', str(int(96 * scale)), '--input-json', str(path)], cwd=root, env=env))
        return name, str(scale), {key: result[key] for key in ('geometry_hex', 'geometry_sha256', 'rgba_sha256', 'rgba_zlib_base64', 'output_size', 'nonzero_pixels')}
with ThreadPoolExecutor(max_workers=3) as pool:
    results = list(pool.map(capture, [(name, scale) for name in baseline for scale in (.5, 2)]))
cases = {}
for name, scale, result in results:
    cases.setdefault(name, {})[scale] = result
receipt = {'schema': 'sdocx-inkpen2-v1-high-scales-native-v1', 'scope': 'Six synthetic null-setting strokes, native Drawing and RTV1 on transparent host Mesa at 0.5x and 2x. 1440x2560 screen profile; no device, app canvas or serialized note.', 'capture_script': 'conformance/inkpen2_v1_high_scales_capture.py', 'baseline_receipt': 'conformance/inkpen2-null-v1-shapes-paint-native.json', 'cases': cases}
(root / 'conformance/inkpen2-v1-high-scales-native.json').write_text(json.dumps(receipt, indent=2) + '\n')
