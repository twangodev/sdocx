"""Verify private/local native inputs without copying or printing note content."""
import hashlib
import json
from pathlib import Path
import struct
import sys

root = Path(__file__).resolve().parents[2]
frames = Path(sys.argv[1])
receipt = json.loads(Path(sys.argv[2]).read_text())
expected = {
    'first': '6c8e9466ca9e473812a5d5cf87b448a359b3008d9861c2c404c223c536bba642',
    'second': '1576d32a0d7592c3cc82da4ac60791ba5fd59e44f8806239488eba69924237a5',
    'geometry': '7d88c4f7e2f8fa148de58e515194d59736ceda859abd6297a0de7b330cc987cf',
}
for name, digest in expected.items():
    assert receipt[name + '_sha256'] == digest, name
assert [v[3] for v in receipt['uniforms'] if v[:3] == ['glUniform2fv', 4, 1]][-1] == [
    170.24111938476562, 145.2706298828125]
for name in ('first', 'second'):
    raw = (frames / f'{name}.rgba').read_bytes()
    assert len(raw) == 192 * 192 * 4, name
    assert hashlib.sha256(raw).hexdigest() == expected[name], name
native = (frames / 'second.vbo').read_bytes()
assert hashlib.sha256(native).hexdigest() == expected['geometry']
records = list(struct.iter_unpack('<14f', native))
for name, digest in (
    ('grain.r8', 'dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9'),
    ('point.r8', '475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f'),
):
    raw = (root / 'crates/sdocx/src/ink/pencil2/assets' / name).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == digest, name
print('Native frames, geometry, grain uniform and texture hashes verified.')
