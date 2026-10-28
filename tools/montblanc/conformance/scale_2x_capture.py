#!/usr/bin/env python3
"""Capture a 2x native Draw matrix using the pinned Mesa harness."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[3]
HARNESS = ROOT / 'tmp/vector-parity-plan/montblanc-rtv1-mesa-source-v12/capture_candidate.py'
BASELINE = ROOT / 'tools/montblanc/captures/baseline.json.gz'
PIN = '7378931c784f1be00527da5dd8d39dd48597b95a8c5bcc2c636fdf69d82a2fae'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def frame(capture):
    data = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
    assert len(data) == 256 * 256 * 4 and sha(data) == capture['post_readback']['sha256']
    return data

def main():
    source = HARNESS.read_bytes()
    assert sha(source) == PIN
    old = b'1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1'
    new = b'2,0,0,0,0,2,0,0,0,0,1,0,tx,ty,0,1'
    assert source.count(old) == 2
    modified = source.replace(old, new)
    baseline = json.loads(gzip.decompress(BASELINE.read_bytes()))
    with tempfile.NamedTemporaryFile(dir=HARNESS.parent, suffix='.py') as script, tempfile.TemporaryDirectory(prefix='montblanc-scale-') as tmp:
        script.write(modified)
        script.flush()
        output = Path(tmp) / 'capture.json'
        subprocess.run([sys.executable, script.name, '--case', 'baseline', '--output', str(output)], cwd=ROOT, check=True)
        capture = json.loads(output.read_text())
    assert capture['matrix_f32'] == [2,0,0,0,0,2,0,0,0,0,1,0,128,128,0,1]
    data = frame(capture)
    base = frame(baseline)
    print(json.dumps({'harness_sha256': PIN, 'modified_harness_sha256': sha(modified), 'baseline_sha256': sha(base), 'scale_sha256': sha(data), 'baseline_statistics': baseline['pixel_statistics'], 'scale_statistics': capture['pixel_statistics'], 'baseline_vectors': baseline['native_add_vectors'], 'scale_vectors': capture['native_add_vectors'], 'draw_provenance': capture['draw_provenance'], 'uniform_diagnostics': capture['uniform_diagnostics'], 'host_gles': capture['host_gles'], 'input_pins': capture['input_pins']}, indent=2))
    receipt = json.loads((ROOT / 'conformance/montblanc-native-scale-2x.json').read_text())
    assert sha(data) == receipt['scale_sha256'] and sha(base) == receipt['baseline_sha256']
    assert [v['sha256'] for v in capture['native_add_vectors']] == [v['sha256'] for v in baseline['native_add_vectors']]
    left, top, right, bottom = receipt['scale_bounds_half_open']
    crop = b''.join(data[(y * 256 + left) * 4:(y * 256 + right) * 4] for y in range(top, bottom))
    assert crop == (ROOT / 'conformance/montblanc-scale-2x.rgba').read_bytes()
    expanded = bytearray(len(data))
    for y in range(top, bottom):
        expanded[(y * 256 + left) * 4:(y * 256 + right) * 4] = crop[(y - top) * (right - left) * 4:(y - top + 1) * (right - left) * 4]
    assert bytes(expanded) == data
    print('Verified native 2x frame and compact crop')

if __name__ == '__main__':
    main()
