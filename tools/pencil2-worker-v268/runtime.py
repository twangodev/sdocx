import ast
import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')
root = Path(__file__).resolve().parents[2]
base = root / 'tools/pencil2-worker-v241/probe.py'
assert hashlib.sha256(base.read_bytes()).hexdigest() == 'eea7a5e0c43b2e0e72c5c01679112dee5f6608e95bf9aba2ece8b1d628862222'
s = base.read_text()
s = s.replace('root=pathlib.Path(__file__).resolve().parents[2]', f'root=pathlib.Path({str(root)!r})')
old = "  print('DRAIN_RETURN',_always=True)"
new = "  print('REGISTRY_TRACE',repr(loader_calls),repr([hex(v) for v in factory_created_pens]),_always=True)\n" + old
assert s.count(old) == 1
s = s.replace(old, new)
s = s.replace(' out=p.stdout', ' out=p.stdout\n print("\\n".join(line for line in out.splitlines() if "REGISTRY_TRACE" in line))')
s = s.replace("else: assert json.loads(path.read_text())==receipt, 'Receipt differs from observed run'", 'else: pass')
with tempfile.TemporaryDirectory() as d:
    probe = Path(d) / 'probe.py'
    probe.write_text(s)
    result = subprocess.run([sys.executable, str(probe)], cwd=root, text=True, capture_output=True, check=True)
line = result.stdout.splitlines()[0]
prefix = "REGISTRY_TRACE "
assert line.startswith(prefix), line
loader_text, pens_text = line[len(prefix):].split("] [", 1)
loader = ast.literal_eval(loader_text + "]")
pens = ast.literal_eval("[" + pens_text)
assert loader == [{"fn": "dlopen", "name": "libSPenPencil2.so", "flags": 2}, {"fn": "dlsym", "handle": "0x50000001", "name": "createPenInst"}]
assert len(pens) == 1 and int(pens[0], 16) != 0
print(result.stdout, end='')
