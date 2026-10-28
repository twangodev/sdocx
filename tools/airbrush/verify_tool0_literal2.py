#!/usr/bin/env python3
"""Verify native tool-0 setting pairs against the retained receipt."""
import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'conformance/airbrush-tool0-literal2-paired-native.json').read_text())
source = Path(sys.argv[1])
for case in receipt['cases']:
    values = []
    for setting in ('empty', '2;'):
        lines = (source / f"{case['shape']}-{setting}.json").read_text().splitlines()
        frame = json.loads(lines[0].removeprefix('FRAME_STATE '))['post_frame']
        capture = json.loads(lines[1])
        pixels = zlib.decompress(base64.b64decode(frame['data_base64']))
        assert hashlib.sha256(pixels).hexdigest() == frame['sha256'] == case['frame_sha256']
        stream = [(event.get('after_bytes'), event.get('delta_bytes')) for event in capture['producer_events']]
        digest = hashlib.sha256(json.dumps(stream, separators=(',', ':')).encode()).hexdigest()
        assert digest == case['producer_stream_sha256']
        assert capture['selected_profile'] == case['selected_profile']
        assert capture['model_state'] == case['model_empty' if setting == 'empty' else 'model_literal']
        assert capture['native_hashes'] == receipt['native_hashes']
        values.append((pixels, stream))
    assert values[0] == values[1], case['shape']
    print(f"{case['shape']}: frames and producer streams match")
