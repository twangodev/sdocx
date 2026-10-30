"""Verify a fresh capture against the retained native evidence."""
import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'receipt.json').read_text())
capture = json.loads(Path(sys.argv[1]).read_text())
paint = capture['paint']
assert capture['source_capture']['source_input'] == receipt['source']
for key, value in receipt['native_evidence'].items():
    assert paint[key] == value, key
assert [x for x in paint['paint_gl_calls'] if x['call'] in ['glUniform2fv', 'glUniformMatrix4fv']] == receipt['uniform_calls']
assert paint['gl_errors_final'] == 0
vector = bytes.fromhex(paint['native_vector_hex'])
frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64']))
assert hashlib.sha256(vector).hexdigest() == receipt['native_evidence']['native_vector_sha256']
assert hashlib.sha256(frame).hexdigest() == paint['readback']['sha256'] == receipt['native_frame_sha256']
assert vector == (here / 'vector.bin').read_bytes()
assert frame == zlib.decompress((here / 'frame.rgba.zlib').read_bytes())
assert hashlib.sha256((here / 'capture.py').read_bytes()).hexdigest() == receipt['capture_script_sha256']
print('native source, frame, vector, uniforms, texture allocations, blend and draw match')
