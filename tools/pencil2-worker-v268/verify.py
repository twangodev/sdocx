from hashlib import sha256
from pathlib import Path
import re
import subprocess
import sys

if sys.flags.optimize:
    raise SystemExit("Run without Python optimization")

path = Path('scratch/apk-analysis-native/arm64-v8a/libSPenPenCommon.so')
data = path.read_bytes()
assert sha256(data).hexdigest() == 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d'
segments = subprocess.check_output(['readelf', '-lW', str(path)], text=True)
assert re.search(r'^\s*LOAD\s+0x0+\s+0x0+\s+0x0+\s+0x072890\s+0x072890\s+R E', segments, re.M)
relocations = subprocess.check_output(['readelf', '-rW', str(path)], text=True)
for address, expected in [(0x78ae8, b'com.samsung.android.sdk.pen.pen.preload.Pencil2'), (0x78af0, b'Pencil2')]:
    match = re.search(rf'^{address:016x}\s+\S+\s+R_AARCH64_RELATIVE\s+([0-9a-f]+)$', relocations, re.M)
    assert match
    offset = int(match.group(1), 16)
    assert offset < 0x72890
    actual = data[offset:data.index(b'\0', offset)]
    assert actual == expected
    print(f'{address:#x}: {actual.decode()}')
pencil = Path('scratch/apk-analysis-native/arm64-v8a/libSPenPencil2.so')
assert sha256(pencil.read_bytes()).hexdigest() == '3e7d7118ea0fea370d491a5582ac81c09dc063ac75dbcc396abdc32976a42b0f'
