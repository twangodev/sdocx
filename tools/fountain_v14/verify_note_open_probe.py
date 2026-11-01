"""Verify retained native note-open and Drawing-direction receipts offline."""
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[2]
receipt = json.loads((ROOT / 'tools/fountain_v14/note-open-probe-receipt.json').read_bytes())
for path, expected in receipt['dependency_sha256'].items():
    actual = hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
    if actual != expected:
        raise RuntimeError(f'dependency hash mismatch: {path}')
host_zlib = receipt['host_environment']
if hashlib.sha256(Path(host_zlib['host_libz_path']).read_bytes()).hexdigest() != host_zlib['host_libz_sha256']:
    raise RuntimeError('host libz hash mismatch')
archive = ROOT / 'tmp/stroke-conformance/handwritten.sdocx'
with zipfile.ZipFile(archive) as z:
    for row in receipt['verified_members']:
        data = z.read(row['name'])
        if len(data) != row['size'] or hashlib.sha256(data).hexdigest() != row['sha256']:
            raise RuntimeError(f'archive member mismatch: {row["name"]}')
observed = receipt['observations']
signature = next(row for row in observed if row['step'] == 'EndTag signature compare')
if signature['actual_hex'] != signature['expected_hex']:
    raise RuntimeError('type 2 signature does not match')
error = next(row for row in observed if row['step'] == 'native Error::SetError')
if (error['code'], error['line'], error['lr']) != (11, 776, '0x728a6d8'):
    raise RuntimeError('native failure boundary differs')
reads = [row for row in observed if row['step'] == 'cache fread']
if (reads[-1]['offset'], reads[-1]['size'], reads[-1]['actual']) != (8, 234, 123):
    raise RuntimeError('native media short-read differs')
direction = json.loads((ROOT / 'tools/fountain_v14/direction-write-trace.json').read_bytes())
for path, expected in direction['dependency_sha256'].items():
    if hashlib.sha256((ROOT / path).read_bytes()).hexdigest() != expected:
        raise RuntimeError(f'direction dependency hash mismatch: {path}')
script = ROOT / 'tools/fountain_v14/direction_trace.py'
if hashlib.sha256(script.read_bytes()).hexdigest() != direction['capture_script_sha256']:
    raise RuntimeError('direction trace script hash mismatch')
print('Note open input pins, six archive members, native failure and direction script verified.')
