#!/usr/bin/env python3
"""Verify the three retained native host frames; export metrics are historical."""
import hashlib
import json
from pathlib import Path
import zlib

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'native-receipt.json').read_text())
for scale in ('1', '2', '4'):
    frame = zlib.decompress((here / f'note{scale}.rgba.zlib').read_bytes())
    assert hashlib.sha256(frame).hexdigest() == receipt[scale]['rgba_sha256'], scale
    print(f'note{scale}: native frame verified')
