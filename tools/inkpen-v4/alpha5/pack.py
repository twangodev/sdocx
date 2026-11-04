"""Pack one capture to a temporary directory for comparison with retained evidence."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

capture = Path(sys.argv[1]).read_text()
data = json.loads(capture[capture.index('{\n'):])
assert data['requested_argb_u32'] == 0x80402010
readback = data['target']['readback']
raw = zlib.decompress(base64.b64decode(readback['fbo4_rgba8_zlib_base64']))
assert len(raw) == 640 * 320 * 4
assert hashlib.sha256(raw).hexdigest() == readback['fbo4_rgba8_sha256']
frame = zlib.compress(raw, 9)
stream = data['source_caller']['streams']['draw_object_stroke']
case = {'name': 'alpha5', 'scale': 5, 'argb': data['requested_argb_u32'],
        'input': data['source_caller']['input'], 'stream': stream['values_f32_bits'],
        'stream_sha256': stream['sha256'], 'rgba_sha256': readback['fbo4_rgba8_sha256'],
        'offset': 0, 'length': len(frame)}
receipt = {'schema': 'sdocx-inkpen-v4-alpha5-native-v1',
           'scope': 'One synthetic translucent color native host Mesa capture, no device or saved-note parity.',
           'frames_sha256': hashlib.sha256(frame).hexdigest(), 'cases': [case]}
out = Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
(out / 'packed.json').write_text(json.dumps(receipt, indent=2) + '\n')
(out / 'packed.bin').write_bytes(frame)
