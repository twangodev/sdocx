#!/usr/bin/env python3
"""Verify a fresh native capture against the retained control artifacts."""
import argparse
import base64
import hashlib
import json
import zlib
from pathlib import Path

here = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('capture', type=Path)
args = parser.parse_args()
receipt = json.loads((here / 'receipt.json').read_text())
paint = json.loads(args.capture.read_text())['paint']
vector = bytes.fromhex(paint['native_vector_hex'])
frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
for label, data, retained in (
    ('native_vector', vector, here / 'native-vector.z'),
    ('native_frame', frame, here / 'native-frame.z'),
):
    digest = hashlib.sha256(data).hexdigest()
    if digest != receipt[f'{label}_sha256'] or data != zlib.decompress(retained.read_bytes()):
        raise SystemExit(f'{label} mismatch: sha256={digest}')
    print(f'{label}: verified {len(data)} bytes sha256={digest}')
if paint['target_size'] != receipt['target'] or paint['source_argb_input'] != receipt['argb']:
    raise SystemExit('native target or color mismatch')
