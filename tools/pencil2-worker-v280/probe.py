"""Exercise the renderer's native AGLL intrusive queue in single-threaded emulation."""
import hashlib
from pathlib import Path
import struct
import sys

if sys.flags.optimize:
    raise SystemExit('Run without Python optimization')

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tools/pencil2-page-v219/native/scratch/apk-analysis-runtime/python'))
from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
from unicorn.arm64_const import UC_ARM64_REG_PC, UC_ARM64_REG_LR, UC_ARM64_REG_SP, UC_ARM64_REG_TPIDR_EL0, UC_ARM64_REG_X0, UC_ARM64_REG_X1

binary = (root / 'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so').read_bytes()
assert hashlib.sha256(binary).hexdigest() == 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
u = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
u.mem_map(0x10000, 0x70000)
u.mem_write(0x10000, binary[0x10000:0x65c10])
u.mem_map(0x100000, 0x20000)
u.mem_map(0x200000, 0x10000)
u.reg_write(UC_ARM64_REG_SP, 0x20f000)
u.reg_write(UC_ARM64_REG_TPIDR_EL0, 0x110000)
def put(a, v): u.mem_write(a, struct.pack('<Q', v))
def get(a): return struct.unpack('<Q', u.mem_read(a, 8))[0]
def ret(): u.reg_write(UC_ARM64_REG_PC, u.reg_read(UC_ARM64_REG_LR))
obj, head, vt = 0x100000, 0x100020, 0x101000
put(head, 0); put(head+8, head); put(head+16, head)
tasks = [0x102000, 0x102100]
for task in tasks: put(task, vt)
put(vt+8, 0x111000); put(vt+16, 0x111010)
events = []
def hook(u, addr, size, data):
    if addr in (0x63fc0, 0x63fe0, 0x63fb0):
        ret()
    elif addr == 0x64250:
        dst, value = u.reg_read(UC_ARM64_REG_X0), u.reg_read(UC_ARM64_REG_X1)
        old = get(dst); put(dst, value); u.reg_write(UC_ARM64_REG_X0, old); ret()
    elif addr in (0x111000, 0x111010):
        events.append(('slot8' if addr == 0x111000 else 'run', u.reg_read(UC_ARM64_REG_X0))); ret()
u.hook_add(UC_HOOK_CODE, hook)
def call(addr, *args):
    for reg, value in zip((UC_ARM64_REG_X0, UC_ARM64_REG_X1), args): u.reg_write(reg, value)
    u.reg_write(UC_ARM64_REG_LR, 0x112000)
    u.emu_start(addr, 0x112000, count=100000)
    assert u.reg_read(UC_ARM64_REG_PC) == 0x112000
    return u.reg_read(UC_ARM64_REG_X0)
first = call(0x4fbf0, obj, tasks[0])
second = call(0x4fbf0, obj, tasks[1])
assert first == second == 1
assert get(head) == tasks[0]+8 and get(tasks[0]+8) == tasks[1]+8
call(0x4fac8, obj, head)
assert events == [('run',tasks[0]),('slot8',tasks[0]),('run',tasks[1]),('slot8',tasks[1])], events
assert get(head) == 0 and get(head+8) == head and get(head+16) == head
call(0x4fac8, obj, head)
assert len(events) == 4
print('Native AGLL enqueue and RunTasks: two FIFO tasks, slot 16 then slot 8 each; queue empty on second drain.')
