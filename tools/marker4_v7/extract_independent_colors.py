"""Verify six original full frames and extract the compact ordered-color fixture."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('capture_dir', type=Path, help='directory containing m4-independent-{a,b,ab,ba,bg,bg0}.rgba')
parser.add_argument('output', type=Path, help='new output fixture path')
args = parser.parse_args()
receipt = json.loads(Path('tools/marker4_v7/independent-colors-receipt.json').read_text())
names = ('a', 'b', 'ab', 'ba', 'bg', 'bg0')
frames = []
for name in names:
    frame = (args.capture_dir / f'm4-independent-{name}.rgba').read_bytes()
    assert len(frame) == 1024 * 512 * 4
    assert hashlib.sha256(frame).hexdigest() == receipt['frames'][name]['rgba_sha256']
    frames.append(frame)
fixture = bytearray()
for i in range(0, len(frames[0]), 4):
    if frames[0][i + 3] or frames[1][i + 3]:
        for frame in frames:
            fixture.extend(frame[i:i + 4])
assert hashlib.sha256(fixture).hexdigest() == receipt['compact_fixture']['sha256']
with args.output.open('xb') as output:
    output.write(fixture)
