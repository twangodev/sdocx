"""Reproduce the three GLV1 host frames using a restored pinned historical harness.

Restore the historical harness and its pinned helpers from 2ffac9f^ into a
separate project-shaped directory, symlink scratch, and pass that directory.
The baseline harness checks its own DSO and helper pins before execution.
"""
import argparse
import base64
import hashlib
import json
import subprocess
import sys
import zlib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
runner = root / 'conformance/chinesebrush_paint_native.py'
original = runner.read_text()
assert hashlib.sha256(original.encode()).hexdigest() == '0377f7de9c48e7c79fc596603128d49bff3a7427ce59e6052db14a53d8cf342a'
case = json.loads((root / 'conformance/chinesebrush-source-native.json').read_text())['cases']['curve-v1']
case_sha = hashlib.sha256(json.dumps(case, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
source = original.replace('0dd9a905a6c5b61cec192c5e8063d7c424ceec824bc1c639eef75e5c41c065d1', case_sha).replace('curve-v3', 'curve-v1').replace('GLV3', 'GLV1').replace("b'3;\\0'", "b'1;\\0'").replace("'3;'", "'1;'")
line = next(line for line in source.splitlines() if 'adv=self.alloc(32)' in line)
frames = []
try:
    for setting in ('1;', '0;', None):
        script = source if setting == '1;' else source.replace(line, line.replace("b'1;\\0'", "b'0;\\0'") if setting == '0;' else '     # Omitted setter')
        runner.write_text(script)
        subprocess.run([sys.executable, str(runner), '--write-fixture'], check=True)
        report = json.loads((root / 'conformance/chinesebrush-paint-native.json').read_text())
        frame = zlib.decompress(base64.b64decode(report['paint_readback']['full_rgba8_bottom_left_first_zlib_base64']))
        frames.append((hashlib.sha256(frame).hexdigest(), report['native_draw']['producer_vector_sha256']))
    assert len(set(frames)) == 1, frames
    print(json.dumps({'full_rgba8_sha256': frames[0][0], 'producer_sha256': frames[0][1], 'byte_count': len(frame)}))
finally:
    runner.write_text(original)
