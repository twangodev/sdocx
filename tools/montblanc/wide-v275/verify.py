#!/usr/bin/env python3
import base64
import hashlib
import json
import zlib
from pathlib import Path

here = Path(__file__).resolve().parent
native = json.loads((here / 'native.json').read_text())
r = json.loads((here / 'export-receipt.json').read_text())
assert native['input']['pen_size'] == 2.4 and len(native['input']['points']) == 10
assert len(native['stream']) == 53
for scale in (1, 2):
    cap = json.loads((here / f'capture-{scale}.json').read_text())
    frame = zlib.decompress(base64.b64decode(cap['post_readback']['rgba8_zlib_base64']))
    assert hashlib.sha256(frame).hexdigest() == cap['post_readback']['sha256']
    case = r['cases'][f'zigzag-2.4-{scale}']
    assert case['input'] == native['input'] and case['frame_sha256'] == cap['post_readback']['sha256']
    for reader in ('svg', 'pdf', 'pdfium'):
        baseline = case['metrics']['baseline'][reader][-1]
        candidate = case['metrics']['candidate'][reader][-1]
        assert candidate < baseline if scale == 1 else candidate == baseline
    if scale == 2:
        assert case['metrics']['candidate']['artifacts'] == case['metrics']['baseline']['artifacts']
print('Retained frame hashes, input, stream count, reader inequalities and 2x fallback verified.')
