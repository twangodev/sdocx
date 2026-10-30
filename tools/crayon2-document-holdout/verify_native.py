"""Verify a fresh native capture against the retained holdout evidence."""
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
evidence = receipt['native_evidence']
assert capture['source_capture']['source_input'] == receipt['source']
assert paint['target_size'] == receipt['target_size']
assert paint['view_translation'] == receipt['view_translation']
assert paint['observed_color_uniform_calls'] == evidence['color_uniform']
assert [x for x in paint['paint_gl_calls'] if x['call'] == 'glUniform2fv' and x['location'] == 6] == evidence['scale_uniform']
assert paint['texture_allocations'][:2] == evidence['texture_allocations']
assert paint['native_blend_calls'] == evidence['blend_calls']
assert paint['draw_calls'] == evidence['draw_calls']
assert paint['gl_errors_final'] == evidence['gl_errors_final'] == 0
assert paint['target_initial_background'] == evidence['background']
vector = bytes.fromhex(paint['native_vector_hex'])
frame = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64']))
assert hashlib.sha256(vector).hexdigest() == paint['native_vector_sha256'] == receipt['native_vector_sha256']
assert hashlib.sha256(frame).hexdigest() == paint['readback']['sha256'] == receipt['native_frame_sha256']
assert vector == (here / 'vector.bin').read_bytes()
assert frame == zlib.decompress((here / 'frame.rgba.zlib').read_bytes())
assert hashlib.sha256((here / 'capture.py').read_bytes()).hexdigest() == receipt['capture_script_sha256']
print('native capture matches retained source, vector, frame, uniforms, textures, blend and draw evidence')
