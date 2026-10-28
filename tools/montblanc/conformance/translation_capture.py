#!/usr/bin/env python3
"""Replay the pinned native Mesa translation control and verify its receipt."""
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
RECEIPT = ROOT / 'conformance/montblanc-native-translation.json'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def frame(capture):
    result = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
    assert len(result) == 256 * 256 * 4
    assert digest(result) == capture['post_readback']['sha256']
    return result


def shift(source, dx, dy):
    result = bytearray(len(source))
    for y in range(256 - dy):
        start = y * 256 * 4
        dest = ((y + dy) * 256 + dx) * 4
        result[dest:dest + (256 - dx) * 4] = source[start:start + (256 - dx) * 4]
    return bytes(result)


def verify_crop(data, name, left, top):
    crop = (ROOT / f'conformance/montblanc-translation-{name}.rgba').read_bytes()
    assert len(crop) == 23 * 7 * 4
    expanded = bytearray(len(data))
    for y in range(7):
        dest = ((top + y) * 256 + left) * 4
        expanded[dest:dest + 23 * 4] = crop[y * 23 * 4:(y + 1) * 23 * 4]
    assert bytes(expanded) == data


receipt = json.loads(RECEIPT.read_text())
assert digest(HARNESS.read_bytes()) == receipt['harness_sha256']
assert digest(BASELINE.read_bytes()) == receipt['baseline_capture_gzip_sha256']
baseline = json.loads(gzip.decompress(BASELINE.read_bytes()))
with tempfile.TemporaryDirectory(prefix='montblanc-translation-') as tmp:
    output = Path(tmp) / 'capture.json'
    subprocess.run([sys.executable, str(HARNESS), '--case', 'baseline', '--translation',
                    '144', '136', '--output', str(output)], cwd=ROOT, check=True)
    translated = json.loads(output.read_text())
    baseline_bytes, translated_bytes = frame(baseline), frame(translated)
    assert digest(baseline_bytes) == receipt['baseline_rgba8_sha256']
    assert digest(translated_bytes) == receipt['translated_rgba8_sha256']
    assert baseline['matrix_f32'] == receipt['baseline_matrix']
    assert translated['matrix_f32'] == receipt['translated_matrix']
    assert shift(baseline_bytes, 16, 8) == translated_bytes
    verify_crop(baseline_bytes, 'baseline', 127, 126)
    verify_crop(translated_bytes, 'translated', 143, 134)
print('Verified exact native host RGBA8 translation by (16, 8) pixels')
