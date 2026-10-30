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
old = "p.call(key,[p.handoff_queue,obj,0]); cv=p.x(0); err="
new = "p.callbacks.pop('SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)',None); p.syms['SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)']=0x40c1398; p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None); p.syms['SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)']=0x40c1454; p.put(0x107a758,bytes([81]).decode(),0x40c04ac); p.put(0x107a760,bytes([81]).decode(),0x40c1454); p.put(0x107a770,bytes([81]).decode(),0x40c1398); p.put(0x200ef000,bytes([81]).decode(),0); trace=[]; p.u.hook_add(UC_HOOK_CODE,lambda u,a,z,d: trace.append((hex(a),[hex(p.x(i)) for i in range(4)])) if a in (0x1049318,0x1049348,0x10493a4,0x10493fc,0x1049400,0x104941c,0x40c04ac,0x40c1454,0x40c1398) else None); manager=p.alloc(256); p.call('SPen::PenGLDataManager::PenGLDataManager()',[manager]); name=p.alloc(32); p.call('SPen::String::String()',[name]); p.call('SPen::String::Construct()',[name]); p.call('SPen::String::Set(char const*)',[name,fmt]); p.syms[key]=0x1049260; p.call(key,[manager,p.handoff_queue,name,192,192,0,0,0,0],limit=8000000); cv=p.x(0); p.call('SPen::SPPenCanvas::GetBitmap()',[cv]); manager_bitmap=p.x(0); p.syms[key]=0x4090a2c; p.call(key,[manager_bitmap,0]); manager_sub=p.x(0); p.syms[key]=0x40c04ac; err="
assert source.count(old) == 1
source = source.replace(old,new.replace(chr(39), chr(92)+chr(39)))
source = source.replace("hex(cv),hex(err)", "(hex(cv),hex(p.get(cv,bytes([81]).decode())[0]),trace,bytes(p.u.mem_read(manager,80)).hex(),bytes(p.u.mem_read(p.get(manager+32,bytes([81]).decode())[0],80)).hex(),hex(manager_bitmap),hex(manager_sub)),hex(err)")
source = source.replace("[message[1], factory[2]]", "[message[1], factory[3][6]]")
source = source.replace("[3553,0,33321,192,192,0,6403,5121,'0x0']], uploads", "[3553,0,33321,192,192,0,6403,5121,'0x0'],[3553,0,33321,192,192,0,6403,5121,'0x0']], uploads")
source = source.replace("        assert factory[0]", "        canvas, vt, trace, manager_bytes, node_bytes, bitmap, sub = factory[3]\n        assert vt == '0x40d1008' and int.from_bytes(bytes.fromhex(manager_bytes)[48:56], 'little') == 1\n        assert [row[0] for row in trace] == ['0x40c04ac','0x1049318','0x40c1454','0x1049348','0x10493a4','0x10493fc','0x40c1398','0x1049400','0x104941c'], trace\n        assert trace[1][1][0] == bitmap == trace[2][1][1] == trace[5][1][0] == trace[6][1][0]\n        assert int.from_bytes(bytes.fromhex(node_bytes)[56:64], 'little') == int(canvas, 16)\n        assert bytes.fromhex(node_bytes)[:16] == bytes(16)\n        assert bytes.fromhex(manager_bytes)[32:40] == bytes.fromhex(manager_bytes)[40:48]\n        assert trace[3][1][0] == canvas and sub != '0x0'\n        assert factory[0]")

old_paths = "        source=source.replace(','.join(map(str,old.encode())), ','.join(map(str,str(new).encode())))"
new_paths = "        encoded = ','.join(map(str,old.encode()))\n        assert source.count(encoded) == 1, old\n        source=source.replace(encoded, ','.join(map(str,str(new).encode())))"
assert source.count(old_paths) == 1
source = source.replace(old_paths, new_paths)
with tempfile.TemporaryDirectory(prefix='pencil2-v270-') as directory:
    probe = Path(directory) / 'probe.py'
    probe.write_text(source)
    result = subprocess.run([sys.executable, str(probe)], cwd=root, text=True, capture_output=True)
    assert result.returncode == 0, (result.stdout, result.stderr)
assert result.stdout.strip() == 'Verified native bitmap and canvas factories and deferred message; readback changed 72 bytes.'
print('Verified native manager, bitmap and canvas factories, bitmap release, and deferred Draw.')
