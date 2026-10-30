"""Reconstruct the historical second-stroke scripts from the pinned first control."""
import hashlib
import json
import sys
from pathlib import Path

if not __debug__:
    raise SystemExit('optimized Python is unsupported: replay validation requires assertions')

base = Path('tools/marker4_v7/saved-v197')
root = Path('tools/marker4_v7/saved-v198')
old = json.loads((base / 'receipt.json').read_bytes())['script_sha256']
new = json.loads((root / 'receipt.json').read_bytes())['script_sha256']
kind = sys.argv[1] if len(sys.argv) > 1 else ''
names = {'capture': 'capture.py', 'verify': 'verify_source.py', 'compare': 'compare.py', 'export': 'export.rs'}
if kind not in names:
    raise SystemExit('usage: replay.py {capture|verify|compare|export} [arguments]; export prints Rust source')
name = names[kind]
raw = (base / name).read_bytes()
assert hashlib.sha256(raw).hexdigest() == old[name], 'historical v197 script changed'
source = raw.decode().replace('tools/marker4_v7/saved-v197', 'tools/marker4_v7/saved-v198')
if kind == 'capture':
    source = source.replace('6f14596ccc02d2436b8f4404caebf62b11692f406d01d9635a72696bc0628733', 'ee19044b6cff590f4fd2ec3af21e7846da8800a1957524c6a499b33067189974').replace('quiz.sdocx stroke 69 native', 'quiz.sdocx stroke 70 native')
elif kind == 'verify':
    source = source.replace("[69]['stroke']", "[70]['stroke']").replace("assert stroke == source['stroke']", "stroke['rendering']['metadata'] = None\nassert stroke == source['stroke']").replace("'stroke_index': 69", "'stroke_index': 70")
elif kind == 'compare':
    source = source.replace('print(json.dumps(results))', "assert results == receipt['rgb_l1'], (results, receipt['rgb_l1'])\nprint(json.dumps(results))")
assert hashlib.sha256(source.encode()).hexdigest() == new[name], 'historical v198 script reconstruction changed'
if kind == 'export':
    assert len(sys.argv) == 2
    sys.stdout.write(source)
else:
    sys.argv = [str(root / name), *sys.argv[2:]]
    exec(compile(source, str(root / name), 'exec'), {'__name__': '__main__', '__file__': str(root / name)})
