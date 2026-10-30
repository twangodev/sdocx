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
new = "p.callbacks.pop('SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)',None); p.syms['SPen::SPGraphicsFactory::ReleaseBitmap(SPen::ISPBitmap*)']=0x40c1398; p.callbacks.pop('SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)',None); p.syms['SPen::SPGraphicsFactory::CreatePenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*, int)']=0x40c1454; p.put(0x107a758,bytes([81]).decode(),0x40c04ac); p.put(0x107a760,bytes([81]).decode(),0x40c1454); p.put(0x107a770,bytes([81]).decode(),0x40c1398); p.put(0x200ef000,bytes([81]).decode(),0); trace=[]; p.u.hook_add(UC_HOOK_CODE,lambda u,a,z,d: trace.append((hex(a),[hex(p.x(i)) for i in range(4)])) if a in (0x1049318,0x1049348,0x10493a4,0x10493fc,0x1049400,0x104941c,0x40c04ac,0x40c1454,0x40c1398) else None); manager=p.alloc(256); p.call('SPen::PenGLDataManager::PenGLDataManager()',[manager]); name=p.alloc(32); p.call('SPen::String::String()',[name]); p.call('SPen::String::Construct()',[name]); p.call('SPen::String::Set(char const*)',[name,fmt]); p.syms[key]=0x1049260; p.call(key,[manager,p.handoff_queue,name,192,192,0,0,0,0],limit=8000000); cv=p.x(0); p.call('SPen::SPPenCanvas::GetBitmap()',[cv]); manager_bitmap=p.x(0); p.syms[key]=0x4090a2c; p.call(key,[manager_bitmap,0]); manager_sub=p.x(0); p.syms[key]=p.get(p.get(manager_sub,bytes([81]).decode())[0]+120,bytes([81]).decode())[0]; p.call(key,[manager_sub]); manager_texture=p.x(0); manager_texture_fields=p.get(manager_texture+40,bytes([50,73]).decode()); p.syms[key]=0x40c04ac; err="
assert source.count(old) == 1
source = source.replace(old,new.replace(chr(39), chr(92)+chr(39)))
source = source.replace("hex(cv),hex(err)", "(hex(cv),hex(p.get(cv,bytes([81]).decode())[0]),trace,bytes(p.u.mem_read(manager,80)).hex(),bytes(p.u.mem_read(p.get(manager+32,bytes([81]).decode())[0],80)).hex(),hex(manager_bitmap),hex(manager_sub),hex(manager_texture),manager_texture_fields),hex(err)")
source = source.replace("[message[1], factory[2]]", "[message[1], factory[3][6]]")
source = source.replace("[3553,0,33321,192,192,0,6403,5121,'0x0']], uploads", "[3553,0,33321,192,192,0,6403,5121,'0x0'],[3553,0,33321,192,192,0,6403,5121,'0x0']], uploads")
source = source.replace("        assert factory[0]", "        canvas, vt, trace, manager_bytes, node_bytes, bitmap, sub, texture, texture_fields = factory[3]\n        assert texture != '0x0' and texture_fields == (3553,5)\n        assert vt == '0x40d1008' and int.from_bytes(bytes.fromhex(manager_bytes)[48:56], 'little') == 1\n        assert [row[0] for row in trace] == ['0x40c04ac','0x1049318','0x40c1454','0x1049348','0x10493a4','0x10493fc','0x40c1398','0x1049400','0x104941c'], trace\n        assert trace[1][1][0] == bitmap == trace[2][1][1] == trace[5][1][0] == trace[6][1][0]\n        assert int.from_bytes(bytes.fromhex(node_bytes)[56:64], 'little') == int(canvas, 16)\n        assert bytes.fromhex(node_bytes)[:16] == bytes(16)\n        assert bytes.fromhex(manager_bytes)[32:40] == bytes.fromhex(manager_bytes)[40:48]\n        assert trace[3][1][0] == canvas and sub != '0x0'\n        assert factory[0]")

instrument = """  def snapshot(label):
   get=ip.cfunc(p.mesa.gl,'glGetIntegerv',None,[C.c_uint,C.POINTER(C.c_int)])
   attachment=ip.cfunc(p.mesa.gl,'glGetFramebufferAttachmentParameteriv',None,[C.c_uint,C.c_uint,C.c_uint,C.POINTER(C.c_int)])
   values=[]
   for enum in (0x8ca6,0x8caa,0x8069):
    value=C.c_int(); get(enum,C.byref(value)); values.append(value.value)
   typ=C.c_int(); name=C.c_int()
   attachment(0x8d40,0x8ce0,0x8cd0,C.byref(typ)); attachment(0x8d40,0x8ce0,0x8cd1,C.byref(name))
   buf=(C.c_ubyte*(192*192*4))(); ip.cfunc(p.mesa.gl,'glReadPixels',None,[C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])(0,0,192,192,0x1908,0x1401,buf)
   raw=bytes(buf)
   return [label,values,typ.value,name.value,sum(x!=0 for x in raw),__import__('hashlib').sha256(raw).hexdigest()]
  snapshots=[snapshot('before')]
"""
patch = "new_draw = new_draw.replace(\"  prior=(C.c_ubyte\", " + repr(instrument + "  prior=(C.c_ubyte") + ")\n"
patch += "new_draw = new_draw.replace(\"[task],limit=8000000)\\n\", \"[task],limit=8000000)\\n   snapshots.append(snapshot('after'))\\n\")\n"
patch += "new_draw = new_draw.replace(\"result=repr((observed,hex(selected_rt),\", \"result=repr((snapshots, [row for row in gl_simple if row[0] in ('glGenTextures','glBindTexture','glTexImage2D','glGenFramebuffers','glBindFramebuffer','glFramebufferTexture2D')], observed,hex(selected_rt),\")\n"
patch += "new_draw = new_draw.replace(\"  p.u.hook_add(UC_HOOK_CODE,observe_run)\", " + repr("  p.u.hook_add(UC_HOOK_CODE,observe_run)\n  if __import__('os').environ.get('PENCIL_SKIP_DRAW'):\n   def skip_draw(u,a,z,d):\n    if a==p.syms['SPen::Pencil2StrokeDrawableRTV4::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)']: u.reg_write(R.UC_ARM64_REG_PC,u.reg_read(R.UC_ARM64_REG_LR))\n   p.u.hook_add(UC_HOOK_CODE,skip_draw)\n") + ")\n"
anchor = r"\nassert p.count(old_draw)==1\n"
assert source.count(anchor) == 1
encoded_patch = patch.replace("\\", "\\\\").replace("'", "\\'").replace("\n", r"\n")
source = source.replace(anchor, r"\n" + encoded_patch + r"assert p.count(old_draw)==1\n")
source = source.replace("        assert [event[0] for event in message[0]]", "        snapshots,calls=message[:2]; message=message[2:]\n        before,after=snapshots\n        assert before[1][:2] == after[1][:2] == [2,2] and before[2:4] == after[2:4] == [5890,5], snapshots\n        assert ['glBindTexture',3553,5,None] in calls and ['glFramebufferTexture2D',36160,36064,3553,5,0,None] in calls\n        assert before[5] == '5574536817c27b5cbfa5536e0235b9a5dd0a3e2705ba0dde2a1c79a5d7e879ee'\n        assert after[5] == (before[5] if __import__('os').environ.get('PENCIL_SKIP_DRAW') else 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051'), snapshots\n        assert [event[0] for event in message[0]]")
source = source.replace("assert message[2] == 0 and message[4] == 72 and message[5] == 'bf8ab3ad30331118e6f325e4cee8c2d0bca878cceab13fd096e573443640a051', message", "assert message[2] == 0 and message[4] == (0 if __import__('os').environ.get('PENCIL_SKIP_DRAW') else 72), message")
old_paths = "        source=source.replace(','.join(map(str,old.encode())), ','.join(map(str,str(new).encode())))"
new_paths = "        encoded = ','.join(map(str,old.encode()))\n        assert source.count(encoded) == 1, old\n        source=source.replace(encoded, ','.join(map(str,str(new).encode())))"
assert source.count(old_paths) == 1
source = source.replace(old_paths, new_paths)
with tempfile.TemporaryDirectory(prefix='pencil2-v270-') as directory:
    probe = Path(directory) / 'probe.py'
    probe.write_text(source)
    result = subprocess.run([sys.executable, str(probe)], cwd=root, text=True, capture_output=True)
    assert result.returncode == 0, (result.stdout, result.stderr)
assert 'Verified native bitmap and canvas factories' in result.stdout
print('Verified manager attachment and controlled Draw readback.')
