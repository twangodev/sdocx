"""Verify the native literal-setting capture against the retained control."""
import hashlib
import json
from pathlib import Path
import sys

capture = Path(sys.argv[1]).read_text()
data = json.loads(capture[capture.index('{\n'):])
case = json.loads((Path(__file__).parent / 'packed.json').read_text())['cases'][0]
caller = data['source_caller']
assert caller['input']['advanced_getter_utf8'] == '4;'
assert caller['route']['selected_drawable_vtable_id'] == 'v4'
assert caller['streams']['draw_object_stroke']['values_f32_bits'] == case['stream']
assert caller['streams']['draw_object_stroke']['sha256'] == case['stream_sha256']
assert data['target']['readback']['fbo4_rgba8_sha256'] == case['rgba_sha256']
print('Literal 4; selected V4 and matched the retained stream and full framebuffer SHA-256.')
