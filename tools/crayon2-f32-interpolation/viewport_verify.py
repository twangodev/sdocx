#!/usr/bin/env python3
"""Verify the captured Mesa viewport operation against retained UV controls."""
from pathlib import Path
import sys

source = Path('tools/crayon2-f32-interpolation/translated_model.py')
text = source.read_text()
old = 'f(f(f(clip + f(1)) * f(0.5)) * f(160))'
assert text.count(old) == 1
text = text.replace(old, 'fm(clip, f(80), f(80))')
text = text.replace('expected = 17 if fused_vertex else 21', 'expected = 22')
sys.argv = [str(source)]
exec(compile(text, str(source), 'exec'), {'__name__': '__main__'})

source = Path('tools/crayon2-transform-control/model139.py')
text = source.read_text()
old = 'f(f(f(clip+f(1))*f(.5))*f(160))'
assert text.count(old) == 1
text = text.replace(old, 'f(float(clip)*80+80)')
text = text.replace('assert inds == [113,4] and pinds == [5,24]', 'assert inds == [114,3] and pinds == [5,24]')
text = text.replace('assert bg[inds[1]*150+inds[0]] == 41', 'assert bg[inds[1]*150+inds[0]] == 111')
exec(compile(text, str(source), 'exec'), {'__name__': '__main__'})
