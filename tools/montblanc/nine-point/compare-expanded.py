#!/usr/bin/env python3
"""Verify retained native captures; external reader comparisons are historical."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import zlib

root = Path(__file__).resolve().parents[3]
here = Path(__file__).resolve().parent
manifest = json.loads((here / 'manifest.json').read_text())


def checked_bytes(path):
    raw = path.read_bytes()
    key = str(path.relative_to(root))
    assert hashlib.sha256(raw).hexdigest() == manifest['sha256'][key], key
    return raw


# The manifest also retains historical export/build pins, which are not live
# dependencies of this native-only verifier.
for name in ('capture.py', 'capture-expanded-1x.py', 'capture-expanded-2x.py',
             'expanded-streams.json'):
    checked_bytes(here / name)
for name in ('conformance/montblanc-nine-point-native.json',
             'crates/sdocx/src/ink/montblanc_nine_streams.json'):
    checked_bytes(root / name)

pairs = [('capture.json.gz', 'frame.rgba.zlib')]
pairs += [(f'{case}-{scale}.json.gz', f'{case}-{scale}.rgba.zlib')
          for scale in ('1x', '2x') for case in ('loop', 'reverse')]
for capture_name, frame_name in pairs:
    capture = json.loads(gzip.decompress(checked_bytes(here / capture_name)))
    frame = zlib.decompress(checked_bytes(here / frame_name))
    assert len(frame) == 256 * 256 * 4, frame_name
    assert hashlib.sha256(frame).hexdigest() == capture['post_readback']['sha256'], frame_name
    assert frame == zlib.decompress(base64.b64decode(
        capture['post_readback']['rgba8_zlib_base64'])), frame_name
    print(f'{frame_name}: native capture and frame verified')
