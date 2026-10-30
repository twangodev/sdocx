import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
parent = root / 'tools/pencil2-worker-v273/probe.py'
assert hashlib.sha256(parent.read_bytes()).hexdigest() == '710fec935daf60a20df04ca38ce0d9c8f796ab014985dad9977de7108921bf66'
source = parent.read_text()
def replace(old, new, count=1):
    global source
    assert source.count(old) == count, (old, source.count(old))
    source = source.replace(old, new)
replace('root = Path(__file__).resolve().parents[2]', f'root = Path({str(root)!r})')
replace('p.put(0x200ef000,bytes([81]).decode(),0); trace=[]', 'p.put(0x200ef000,bytes([81]).decode(),2); trace=[]')
replace('[manager,p.handoff_queue,name,192,192,0,0,0,0]', '[manager,p.handoff_queue,name,192,192,0,0,6,2]')
replace("[3553,0,33321,192,192,0,6403,5121,'0x0'],[3553,0,33321,192,192,0,6403,5121,'0x0']", "[3553,0,33321,192,192,0,6403,5121,'0x0'],[3553,0,32856,192,192,0,6408,5121,'0x0']")
replace('5574536817c27b5cbfa5536e0235b9a5dd0a3e2705ba0dde2a1c79a5d7e879ee', '8123c216413f82bbaa0339c27a43d9822c2a043e20662b27c97874429b996e9a')
old = "else 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051'), snapshots"
replace(old, "else 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'), snapshots")
replace("else 72), message", "else 322), message")
replace("root / 'tools/pencil2-worker-v273/target.rgba.zlib'", "root / 'tools/pencil2-worker-v256/rgba8-frame.zlib'")
replace("hashlib.sha256(captured).hexdigest() == 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051'", "hashlib.sha256(captured).hexdigest() == 'a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e'")
with tempfile.TemporaryDirectory(prefix='pencil274-') as tmp:
    script = Path(tmp) / 'probe.py'
    script.write_text(source)
    subprocess.run([sys.executable, str(script)], cwd=root, env=os.environ.copy(), check=True)
