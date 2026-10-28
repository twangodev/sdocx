#!/usr/bin/env python3
"""Call the pinned native getRepeatEff directly at width thresholds."""
import hashlib
import json
from pathlib import Path
import struct
import sys

root = Path(__file__).resolve().parents[3]
source = root / 'tmp/vector-parity-plan/montblanc-rtv1-mesh-capture-root-v2/montblanc_state_controls.py'
raw = source.read_bytes()
if hashlib.sha256(raw).hexdigest() != 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8':
    raise SystemExit('native harness hash mismatch')
s = raw.decode()
s = s[:s.index('def string(value):')]
sys.argv = [str(source), '--case', 'baseline']
ns = {'__file__': str(source), '__name__': '__main__'}
exec(compile(s, str(source), 'exec'), ns)
p, call, mbbase = ns['p'], ns['call'], ns['mbbase']
p.syms['repeat_probe'] = mbbase + 0x1f3ec

def bits(value):
    return struct.unpack('<I', struct.pack('<f', value))[0]

def val(word):
    return struct.unpack('<f', struct.pack('<I', word))[0]

cases = []
for width in [2, 3, 5, 10, 15, 20, 30, 40, 55, 80]:
    for width_bits in [bits(width), bits(width) + 1]:
        for length in [201.234, 180.0]:
            length_bits = bits(length)
            count = call('repeat_probe', [0], [val(length_bits), val(width_bits)])
            cases.append({'length_bits': length_bits, 'width_bits': width_bits, 'repeat': count})
print(json.dumps({'dso_sha256': hashlib.sha256((root / 'scratch/apk-analysis-native/arm64-v8a/libSPenMontblancFountainPen.so').read_bytes()).hexdigest(), 'harness_sha256': hashlib.sha256(raw).hexdigest(), 'entry_rva': '0x1f3ec', 'capture_script': 'tools/montblanc/extended-streams/repeat-probe.py', 'boundary': 'Direct ARM64 getRepeatEff calls under the pinned Unicorn host harness; not full stroke, paint, or device parity.', 'cases': cases}, indent=2))
