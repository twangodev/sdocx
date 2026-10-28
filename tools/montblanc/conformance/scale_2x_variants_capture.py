#!/usr/bin/env python3
"""Capture and verify three synthetic Montblanc controls at 2x on host Mesa."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[3]
HARNESS = ROOT / 'tmp/vector-parity-plan/montblanc-rtv1-mesa-source-v12/capture_candidate.py'
PIN = '7378931c784f1be00527da5dd8d39dd48597b95a8c5bcc2c636fdf69d82a2fae'
OUT = ROOT / 'conformance'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    source = HARNESS.read_bytes()
    assert sha(source) == PIN
    old = b'1,0,0,0,0,1,0,0,0,0,1,0,tx,ty,0,1'
    new = b'2,0,0,0,0,2,0,0,0,0,1,0,tx,ty,0,1'
    assert source.count(old) == 2
    modified = source.replace(old, new)
    cases = {}
    receipt_path = OUT / 'montblanc-native-scale-2x-variants.json'
    expected = json.loads(receipt_path.read_text())
    assert expected['harness_sha256'] == PIN and expected['modified_harness_sha256'] == sha(modified)
    with tempfile.NamedTemporaryFile(dir=HARNESS.parent, suffix='.py') as script, tempfile.TemporaryDirectory(prefix='montblanc-scale-variants-') as tmp:
        script.write(modified)
        script.flush()
        for case in ('pressure', 'size', 'far_endpoint'):
            output = Path(tmp) / (case + '.json')
            subprocess.run([sys.executable, script.name, '--case', case, '--output', str(output)], cwd=ROOT, check=True)
            capture = json.loads(output.read_text())
            assert capture['matrix_f32'] == [2,0,0,0,0,2,0,0,0,0,1,0,128,128,0,1]
            data = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
            assert len(data) == 256 * 256 * 4 and sha(data) == capture['post_readback']['sha256']
            pixels = [(i % 256, i // 256) for i in range(256 * 256) if data[4*i:4*i+4] != b'\0\0\0\0']
            assert pixels
            left, top = min(x for x, y in pixels), min(y for x, y in pixels)
            right, bottom = max(x for x, y in pixels) + 1, max(y for x, y in pixels) + 1
            crop = b''.join(data[(y*256+left)*4:(y*256+right)*4] for y in range(top, bottom))
            crop_path = OUT / f'montblanc-scale-2x-{case}.rgba'
            assert crop == crop_path.read_bytes()
            saved = expected['cases'][case]
            assert sha(data) == saved['frame_sha256'] and sha(crop) == saved['crop_sha256']
            assert [left, top, right, bottom] == saved['bounds_half_open']
            assert [v['sha256'] for v in capture['native_add_vectors']] == [v['sha256'] for v in saved['native_add_vectors']]
            cases[case] = {'frame_sha256': sha(data), 'bounds_half_open': [left, top, right, bottom], 'crop_sha256': sha(crop), 'crop_bytes': len(crop), 'statistics': capture['pixel_statistics'], 'native_add_vectors': capture['native_add_vectors'], 'draw_provenance': capture['draw_provenance'], 'uniform_diagnostics': capture['uniform_diagnostics'], 'host_gles': capture['host_gles'], 'input_pins': capture['input_pins']}
    print(json.dumps({case: {key: value for key, value in result.items() if key in ('frame_sha256', 'bounds_half_open', 'crop_sha256', 'crop_bytes')} for case, result in cases.items()}, indent=2))

if __name__ == '__main__':
    main()
