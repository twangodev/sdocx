#!/usr/bin/env python3
"""Capture pre-instruction ARM64 register snapshots for the sixth setValue call."""
from pathlib import Path

root = Path(__file__).resolve().parents[2]
source_path = root / "tools/simplebrush-note-v301/capture.py"
source = source_path.read_text()
needle = "        (work / original.name).write_text(text)"
injection = '''        hook = """
import atexit
probe_rows=[]
probe_count=0
def probe_hook(u,a,size,user):
 global probe_count
 if a==0x501c408: probe_count+=1
 if probe_count==6 and 0x501c408 <= a <= 0x501c55c:
  probe_rows.append([hex(a),[hex(u.reg_read(getattr(R,'UC_ARM64_REG_Q'+str(i)))) for i in range(28)]])
@atexit.register
def save_probe():
 if not probe_rows: raise RuntimeError('sixth setValue invocation was not captured')
 open(os.environ.get('SIMPLEBRUSH_TRACE_OUTPUT', '/tmp/simplebrush-v302-instructions.json'),'w').write(json.dumps(probe_rows))
p.u.hook_add(UC_HOOK_CODE,probe_hook)
"""
        trace_needle = 'p.u.hook_add(UC_HOOK_CODE,trace)'
        if text.count(trace_needle) != 1:
            raise RuntimeError('unexpected native trace hook insertion point')
        text = text.replace(trace_needle, hook+'\\n'+trace_needle)
'''
if source.count(needle) != 1:
    raise RuntimeError("unexpected v301 capture structure")
exec(compile(source.replace(needle, injection + needle), str(source_path), "exec"),
     {"__name__": "__main__", "__file__": str(source_path)})
