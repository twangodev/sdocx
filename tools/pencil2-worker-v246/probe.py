#!/usr/bin/env python3
"""Probe the first two synthetic render-message execution boundaries."""
import ast
import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parents[2]
parent = root / 'tools/pencil2-public-replay/drawing_queue_exception_diagnostic.py'
assert hashlib.sha256(parent.read_bytes()).hexdigest() == '7866a4c0bd27c1471409bdf02e271b49c9650a830d5e9539d67f9ced9e15c359'
source = parent.read_text()
start = source.index('  x19=u.reg_read', source.index("hook = '''"))
end = source.index('\n if a==p.syms', start)
source = source[:start] + "  u.mem_write(0x200f2530,bytes.fromhex('c0035fd6')); u.reg_write(R.UC_ARM64_REG_X8,0x200f2530)\n" + source[end:]
hook_anchor = "hook = ''' if a==0x10467d8:"
assert source.count(hook_anchor) == 1
source = source.replace(hook_anchor, """hook = ''' if a==0x104676c:
  p.message_receiver=struct.unpack('<Q',u.mem_read(p.x(0)+32,8))[0]
 if a==0x50995f4:
  assert p.x(0)==p.message_receiver and p.x(1)==0 and u.reg_read(R.UC_ARM64_REG_LR)==0x10467f8
  p.update_receiver=p.x(0)
 if a==0x5099648:
  assert u.reg_read(R.UC_ARM64_REG_PC)==0x5099648 and p.x(0)==p.update_receiver and p.get(p.update_receiver+200,'Q')[0]==0 and p.x(8)==0
  raise RuntimeError('verified Update receiver and null graphics before load at 0x99648 V246_BOUNDARY '+json.dumps({'pc':hex(u.reg_read(R.UC_ARM64_REG_PC)),'lr':hex(u.reg_read(R.UC_ARM64_REG_LR)),'message_receiver':hex(p.message_receiver),'update_receiver':hex(p.update_receiver),'graphics_field':p.get(p.update_receiver+200,'Q')[0],'graphics_register':p.x(8)}))
 if a==0x10467d8:""")
old_output = "m.OUTPUTS = Path(tempfile.mkdtemp(prefix='pencil2-queue-intervention-', dir=root / 'tmp'))"
assert source.count(old_output) == 1
source = source.replace(old_output, "m.OUTPUTS = Path(__import__('os').environ['PENCIL_V246_OUTPUT'])")
env = __import__('os').environ.copy()
env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
with tempfile.TemporaryDirectory(prefix='pencil-v246-') as output, tempfile.NamedTemporaryFile('w', suffix='.py', dir=parent.parent) as f:
    env['PENCIL_V246_OUTPUT'] = output
    f.write(source)
    f.flush()
    run = subprocess.run([sys.executable, f.name], cwd=root, env=env, capture_output=True, text=True)
    failures = list(pathlib.Path(output).rglob('*failure*.json'))
    assert len(failures) == 1, failures
    failure = json.loads(failures[0].read_text())
assert run.returncode != 0
assert 'verified Update receiver and null graphics before load at 0x99648' in run.stderr, run.stderr[-4000:]
exception_message = ast.literal_eval(failure['exception_repr'][len('RuntimeError('):-1])
boundary = json.loads(exception_message.split(' V246_BOUNDARY ', 1)[1])
print(json.dumps({
    'scope': 'synthetic synchronous dispatcher with bitmap slot 168 return-self intervention; stopped before null read; no paint or normal scheduling claim',
    'parent_sha256': hashlib.sha256(parent.read_bytes()).hexdigest(),
    'candidate_sha256': failure['candidate_sha256'],
    'adapted_probe_source_sha256': failure['adapted_probe_source_sha256'],
    'boundary': boundary,
    'verified': 'Run receiver equals Update(false) receiver; Update entry LR=0x10467f8; graphics field and register zero at PC=0x5099648',
}, indent=2, sort_keys=True))
