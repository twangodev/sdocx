#!/usr/bin/env python3
"""Verify the synthetic frame and payload relationship and pinned native outputs."""
import hashlib
import json
import struct
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
fixtures = root / 'crates/sdocx/tests/assets/brushpen-third'
control = root / 'tools/brushpen-third'
base_frame = (root / 'conformance/brushpen-serialized/v10-frame.bin').read_bytes()
base_payload = (root / 'conformance/brushpen-serialized/v10-payload.bin').read_bytes()
assert hashlib.sha256(base_frame).hexdigest() == '9b495ebc360c03d2b064e21bf97ce5a45d6f02290aa04410ba87316ae59874f3'
assert hashlib.sha256(base_payload).hexdigest() == '28b0848cb3aa9dadc700bf425fd390281c8de0d222e46f7126d0bcbcd527b60f'
assert len(base_payload) == 137 + len(base_frame) and base_payload[137:] == base_frame
frame = bytearray(base_frame)
struct.pack_into('<f', frame, 20 + 16 + 203 * 4, 0.70)
struct.pack_into('<H', frame, 20 + 16 + 4 * 40, 48)
assert bytes(frame) == (control / 'frame.bin').read_bytes()
assert base_payload[:137] + frame == (fixtures / 'payload.bin').read_bytes()
receipt = json.loads((control / 'receipt.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
assert sha(bytes(frame)) == receipt['inputs']['frame.bin']
assert sha((fixtures / 'payload.bin').read_bytes()) == receipt['inputs']['payload.bin']
assert sha((fixtures / 'geometry.bin').read_bytes()) == receipt['callback']['vector_sha256']
assert sha(zlib.decompress((fixtures / 'frame.rgba.zlib').read_bytes())) == receipt['frame_sha256']
for name, expected in receipt['native_getter_sha256'].items():
    assert sha((fixtures / name).read_bytes()) == expected
print('Serialized frame, payload, native geometry and RGBA hashes verified')
