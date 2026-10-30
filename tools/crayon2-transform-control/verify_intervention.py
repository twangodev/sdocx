"""Verify all seven fresh isolated-record captures against the retained receipt."""
if not __debug__:
    raise RuntimeError('This evidence check requires assertions; do not use python -O')

import base64
import hashlib
import json
import sys
import zlib
from pathlib import Path

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'intervention-receipt.json').read_text())
work = Path(sys.argv[1])
sha = lambda b: hashlib.sha256(b).hexdigest()
assert sha((here / 'intervention.py').read_bytes()) == receipt['intervention_script_sha256']
assert sha((here / 'model139.py').read_bytes()) == receipt['model_script_sha256']
vector = (here.parent / 'crayon2-document-holdout/vector.bin').read_bytes()
assert sha(vector) == '15509eda1113e0bfd007455ba4767b1cba2a22391487f94c75e4121d98ac5f8f'
for relative, expected_hash in receipt['source_dependencies_sha256'].items():
    assert sha((here.parents[1] / relative).read_bytes()) == expected_hash, relative
index = receipt['input_record_index']
for mode, expected in receipt['observations'].items():
    capture = json.loads((work / f'{mode}.json').read_text())
    paint = capture['paint']
    assert capture['source_capture']['source_input'] == receipt['source']
    assert paint['execution_closure'] == receipt['execution_closure']
    assert paint['target_size'] == receipt['target_size']
    assert paint['view_translation'] == receipt['view_translation']
    assert paint['texture_allocations'][:2] == receipt['texture_allocations']
    assert [x for x in paint['paint_gl_calls'] if x['call'] == 'glUniformMatrix4fv'] == receipt['matrix_calls']
    assert [x for x in paint['paint_gl_calls'] if x['call'] == 'glUniform2fv'] == receipt['texture_uniforms']
    assert paint['native_blend_calls'] == receipt['blend_calls']
    assert paint['draw_calls'] == expected['draw_calls']
    assert paint['gl_errors_final'] == 0
    record = bytes.fromhex(paint['native_vector_hex'])
    assert record == vector[index * 24:(index + 1) * 24]
    assert sha(record) == paint['native_vector_sha256'] == expected['vector_sha256']
    raw = zlib.decompress(base64.b64decode(paint['readback']['rgba8_zlib_base64'], validate=True))
    assert len(raw) == 160 * 160 * 4
    assert sha(raw) == paint['readback']['sha256'] == expected['frame_sha256']
    assert list(raw[(57 * 160 + 52) * 4:(57 * 160 + 53) * 4]) == expected['pixel_rgba']
    assert [sha(''.join(s['source']).encode()) for s in paint['shader_source_calls']] == expected['shader_sources_sha256']
print('all seven isolated native capture records match source, input and DSO closure, draw state, shader, frame and pixel receipts')
