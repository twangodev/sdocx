"""Exercise bounded, ordered transport of freshly generated Pencil2 stamp rectangles."""
import os
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image

if not __debug__:
    raise RuntimeError('Run without Python optimization')

work = Path(os.environ['PENCIL198_DIR'])
ns = '{http://www.w3.org/2000/svg}'
first = list(ET.parse(work / 'vec0.svg').getroot())
both = list(ET.parse(work / 'vec1.svg').getroot())
serialize = lambda node: ET.tostring(node, encoding='unicode')
assert [serialize(n) for n in first] == [serialize(n) for n in both[:len(first)]]
background, one = first[0], first[1:]
two = both[len(first):]
assert one and two


def transport(objects, max_rectangles, max_bytes):
    # Stage and validate the entire scene before publishing any bytes.
    count = sum(len(obj) for obj in objects)
    if count > max_rectangles:
        return None
    parts = ['<svg xmlns="http://www.w3.org/2000/svg" width="192" height="192">', serialize(background)]
    parts.extend(serialize(rect) for obj in objects for rect in obj)
    parts.append('</svg>')
    size = sum(len(part.encode()) for part in parts)
    if size > max_bytes:
        return None
    return ''.join(parts).encode()

count = len(one) + len(two)
scene = transport([one, two], count, 2_000_000)
assert scene is not None
assert transport([one, two], count - 1, 2_000_000) is None
assert transport([one, two], count, len(scene) - 1) is None
assert transport([one, two], count, len(scene)) == scene
reverse = transport([two, one], count, 2_000_000)
assert reverse is not None and reverse != scene
for name, data in [('transport', scene), ('reverse', reverse)]:
    (work / f'{name}.svg').write_bytes(data)
    subprocess.run(['resvg', str(work / f'{name}.svg'), str(work / f'{name}.png')], check=True)
with Image.open(work / 'transport.png') as a, Image.open(work / 'vec1.png') as b, Image.open(work / 'reverse.png') as c:
    assert a.convert('RGBA').tobytes() == b.convert('RGBA').tobytes()
    assert a.convert('RGBA').tobytes() != c.convert('RGBA').tobytes()
print(f'ordered transport: {count} source-derived rectangles, {len(scene)} bytes; exact rectangle and byte budget boundaries passed; reverse changes pixels')
