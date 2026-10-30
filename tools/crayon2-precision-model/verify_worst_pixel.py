#!/usr/bin/env python3
"""Verify retained worst-pixel intervention against fresh capture JSON files."""
import base64
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/crayon2-precision-model/worst-pixel-receipt.json').read_text())
for path, expected in receipt['input_sha256'].items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, path
for key, expected in receipt['observations'].items():
    capture = json.loads((Path(sys.argv[1]) / f'{key}.json').read_text())['paint']
    frame = zlib.decompress(base64.b64decode(capture['readback']['rgba8_zlib_base64']))
    assert capture['target_size'] == [192, 192]
    assert capture['gl_errors_final'] == 0
    draws = capture['draw_calls']
    assert len(draws) == 1 and draws[0]['call'] == 'glDrawArraysInstanced'
    assert draws[0]['values'] == [5, 0, 4, 1]
    assert draws[0]['state']['viewport'] == [0, 0, 192, 192]
    mode = ''.join(c for c in key if not c.isdigit())
    shaders = [entry['source'][0] for entry in capture['shader_source_calls']
               if 'uniform sampler2D pointPattern;' in entry['source'][0]]
    assert len(shaders) == 1
    assert hashlib.sha256(shaders[0].encode()).hexdigest() == receipt['shader_sha256'][mode], key
    assert hashlib.sha256(frame).hexdigest() == expected['frame_sha256'], key
    assert list(frame[(43 * 192 + 75) * 4:][:4]) == expected['pixel_rgba'], key
    index = int(''.join(c for c in key if c.isdigit()))
    vector = zlib.decompress((root / 'tools/crayon2-independent-control/native-vector.z').read_bytes())
    assert bytes.fromhex(capture['exact_native_gles_buffer_uploads'][0]['dataHex']) == vector[index * 24:(index + 1) * 24]
texture = (root / 'tmp/vector-parity-plan/crayon2-native-next/native-uploads/g_Crayon2BgPtn.source.bin').read_bytes()
for coord, value in receipt['background_texture_candidates'].items():
    x, y = map(int, coord.split(','))
    assert texture[y * 150 + x] == value
for axis in ('x', 'y'):
    bits = int(receipt['coordinate_bits'][axis], 16)
    encoded = [(bits >> 16) & 255, (bits >> 8) & 255, bits & 255, 255]
    for index in (442, 443, 456, 462):
        assert receipt['observations'][f'{axis}sample{index}']['pixel_rgba'] == encoded
    value = struct.unpack('<f', struct.pack('<I', bits))[0]
    print(axis, hex(bits), value, value * 150)
print(f"verified {len(receipt['observations'])} frames and four texture candidates")
