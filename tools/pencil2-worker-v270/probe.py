import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
parent = root / 'tools/pencil2-worker-v254/probe.py'
assert hashlib.sha256(parent.read_bytes()).hexdigest() == '1a32b28eb890a4c77724f481ea42e5ab01ec415cc7d5bbc56860d2ed295547e3'
source = parent.read_text()
old_root = 'root = pathlib.Path(__file__).resolve().parents[2]'
old_canvas = 'p.call(key,[p.handoff_queue,obj,1])'
assert source.count(old_root) == source.count(old_canvas) == 1
source = source.replace(old_root, f'root = pathlib.Path({str(root)!r})')
source = source.replace(old_canvas, 'p.call(key,[p.handoff_queue,obj,0])')
old_paths = "        source=source.replace(','.join(map(str,old.encode())), ','.join(map(str,str(new).encode())))"
new_paths = "        encoded = ','.join(map(str,old.encode()))\n        assert source.count(encoded) == 1, old\n        source=source.replace(encoded, ','.join(map(str,str(new).encode())))"
assert source.count(old_paths) == 1
source = source.replace(old_paths, new_paths)
with tempfile.TemporaryDirectory(prefix='pencil2-v270-') as directory:
    probe = Path(directory) / 'probe.py'
    probe.write_text(source)
    result = subprocess.run([sys.executable, str(probe)], cwd=root, text=True, capture_output=True, check=True)
assert result.stdout.strip() == 'Verified native bitmap and canvas factories and deferred message; readback changed 72 bytes.'
print('Verified RT canvas factory and deferred draw: 72 changed bytes, v254 readback hash.')
