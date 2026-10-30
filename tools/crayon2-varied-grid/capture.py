#!/usr/bin/env python3
"""Run the pinned Crayon2 native capture with an independent source and target."""
import os
import hashlib
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
base = root / 'tools/crayon2-varied-path/capture.py'
source = base.read_text()
expected_base = "a306d1524adb90f198882cb68ee53194a53938db2e35eb0e3140928634db5e4d"
if hashlib.sha256(base.read_bytes()).hexdigest() != expected_base:
    raise SystemExit("base capture changed; review substitutions before updating its hash")
changes = {
    'bbox[2]<255 and bbox[3]<255': 'bbox[2]<191 and bbox[3]<191',
    "p.put(pts,'6f',64,64,72,70,82,60)": "p.put(pts,'6f',33,58,57,39,82,72)",
    "'42800000','42800000','42900000','428c0000','42a40000','42700000'": "'42040000','42680000','42640000','421c0000','42a40000','42900000'",
    "[[64,64],[72,70],[82,60]]": "[[33,58],[57,39],[82,72]]",
    "'color_argb':'0x80336699'": "'color_argb':'0x997bd247'",
    'width=256,height=256': 'width=192,height=192',
    '[rt,256,256,queue]': '[rt,192,192,queue]',
    '256*256': '192*192',
    '[queue,256,256,fmt,zero,1,1024,1]': '[queue,192,192,fmt,zero,1,768,1]',
    'read(0,0,256,256,': 'read(0,0,192,192,',
    'i%256,i//256': 'i%192,i//192',
    'y*256+x': 'y*192+x',
    "'target_size':[256,256]": "'target_size':[192,192]",
    "'width':256,'height':256": "'width':192,'height':192",
}
counts = {'256*256': 4, 'y*256+x': 4, "'width':256,'height':256": 2}
for old, new in changes.items():
    count = source.count(old)
    expected = counts.get(old, 1)
    if count != expected:
        raise SystemExit(f'capture anchor {old!r}: expected {expected}, found {count}')
    source = source.replace(old, new)
source = source.replace('translation=(0.0,0.0) if CLI_ARGS.translated_source else (64.0,64.0)', 'translation=(7.0,21.0) if CLI_ARGS.translated_source else (64.0,64.0)')
# Keep the native texture-scale uniform alongside the paint calls.
old = "capture_calls={'glBufferData'"
assert source.count(old) == 1
source = source.replace(old, "capture_calls={'glUniform2fv','glBufferData'")
os.environ['CRAYON2_ARGB'] = '0x997bd247'
sys.argv = [str(base), '--_capture-background', 'opaque', '--translated-source']
exec(compile(source, str(base), 'exec'), {'__name__': '__main__', '__file__': str(base)})
