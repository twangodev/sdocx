"""Isolated scene admission experiment; inputs are v199 source-generated SVGs."""
import os
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

if not __debug__:
    raise RuntimeError('Run without Python optimization')

NS = '{http://www.w3.org/2000/svg}'
work = Path(os.environ['PENCIL198_DIR'])
first = list(ET.parse(work / 'vec0.svg').getroot())
both = list(ET.parse(work / 'vec1.svg').getroot())
encode = lambda node: ET.tostring(node, encoding='utf-8')
assert list(map(encode, first)) == list(map(encode, both[:len(first)]))
objects = [first[1:], both[len(first):]]
assert all(objects)


def render_svg(objects):
    return (b'<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192">'
            + encode(first[0]) + b''.join(encode(rect) for obj in objects for rect in obj)
            + b'</svg>')


def preflight(objects, rectangle_limit, byte_limit):
    # Stage the entire replacement before selecting it. The caller owns the fallback.
    count = sum(len(obj) for obj in objects)
    if count > rectangle_limit:
        return None
    output = render_svg(objects)
    return output if len(output) <= byte_limit else None


count = sum(map(len, objects))
accepted = preflight(objects, count, 2_000_000)
assert accepted is not None
fallback = (work / 'generic1.svg').read_bytes()
for limit, size in [(count - 1, len(accepted)), (count, len(accepted) - 1)]:
    destination = bytearray(fallback)
    candidate = preflight(objects, limit, size)
    if candidate is not None:
        destination[:] = candidate
    assert destination == fallback
assert preflight(objects, count, len(accepted)) == accepted
assert preflight(objects, count + 1, len(accepted) + 1) == accepted
rust_accepted = (work / 'rust-accepted.svg').read_bytes()
assert rust_accepted == (work / 'vec1.svg').read_bytes()
subprocess.run(['resvg', str(work / 'rust-accepted.svg'), str(work / 'rust-accepted.png')], check=True)
from PIL import Image
with Image.open(work / 'rust-accepted.png') as a, Image.open(work / 'vec1.png') as b:
    assert a.convert('RGBA').tobytes() == b.convert('RGBA').tobytes()
print(f'SVG scene: {count} rectangles, {len(accepted)} bytes; exact aggregate boundaries and rejected fallback passed')
