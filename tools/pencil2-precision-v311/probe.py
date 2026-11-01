"""Compare ideal and float32 normalized grain selection in retained controls.

The field and blending remain the approximations in the pinned source probes.
This does not execute a GPU shader or modify the vector exporters.
"""
import contextlib
import io
import hashlib
import pathlib
import math
import os
import struct
import sys

root = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tools/pencil2-mesh-v308'))

def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]

def normalized_index(pixel, offset, scale):
    import math
    q = f32(f32(f32(pixel) - f32(offset)) / f32(75 * scale))
    q = f32(q - math.floor(q))
    return math.floor(f32(q * 150)) % 150

def run(path, expected, replacements):
    source = path.read_text()
    assert hashlib.sha256(source.encode()).hexdigest() == expected
    for old, new in replacements:
        assert source.count(old) == 1, old
        source = source.replace(old, new)
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(compile(source, str(path), 'exec'), {'normalized_index': normalized_index})
    return output.getvalue()

def check_metrics(output, expected):
    actual = [line for line in output.splitlines() if 'center sample approximate float:' in line]
    assert actual == [f'{key} center sample approximate float: {metrics}' for key, metrics in expected], actual

# Coordinate interventions belong to v309, not this normalized precision control.
assert all(float(os.environ.get(name, '0')) == 0 for name in ('PENCIL_GRAIN_DX', 'PENCIL_GRAIN_DY'))
for scale in (1, 2):
    for offset in (6, 9):
        deltas = [(normalized_index(x + .5, offset, scale) - math.floor((x + .5 - offset) / (scale * .5))) % 150 for x in range(192)]
        assert set(deltas) <= {0, 149}
        assert deltas.count(149) == (45 if scale == 1 else 0)

first = root / 'tools/pencil2-mesh-v309/first_probe.py'
second = root / 'tools/pencil2-mesh-v308/pixel_probe.py'
baseline_first = run(first, 'cf11ef731373b102b52aedbf7156f16f9c8facb0a6b0f7090838a64004ea3484', [("go(1,'white')", "go(1,'white'); go(2,'white')")])
check_metrics(baseline_first, [('white-1', '344 29 139'), ('white-2', '767 4 607')])
baseline_second = run(second, '5adebdb89b9f1e1954aee7e60bd2413a89c968bcf19591ce8b83e1a136c5d90c', [])
check_metrics(baseline_second, [('white-1', '2705 26 764'), ('white-2', '10361 24 2992'), ('blue-2', '17299 24 3866')])
print('First stroke: f32 normalized grain selection')
first_output = run(first, 'cf11ef731373b102b52aedbf7156f16f9c8facb0a6b0f7090838a64004ea3484', [
    ('math.floor((xx-gx)/(scale*.5)+float(os.environ.get("PENCIL_GRAIN_DX","0")))%150', 'normalized_index(xx,gx,scale)'),
    ('math.floor((yy-gy)/(scale*.5)+float(os.environ.get("PENCIL_GRAIN_DY","0")))%150', 'normalized_index(yy,gy,scale)'),
    ("go(1,'white')", "go(1,'white'); go(2,'white')"),
])
check_metrics(first_output, [('white-1', '170 3 133'), ('white-2', '767 4 607')])
assert 'worst-location output/target (153, 178, 205, 255) (153, 178, 205, 255)' in first_output.splitlines()
print(first_output, end='')
print('Second stroke over retained native first frame: f32 normalized grain selection')
second_output = run(second, '5adebdb89b9f1e1954aee7e60bd2413a89c968bcf19591ce8b83e1a136c5d90c', [
    ('math.floor((xx-gx)/(scale*.5))%150', 'normalized_index(xx,gx,scale)'),
    ('math.floor((yy-gy)/(scale*.5))%150', 'normalized_index(yy,gy,scale)'),
])

check_metrics(second_output, [('white-1', '2470 18 758'), ('white-2', '10361 24 2992'), ('blue-2', '17299 24 3866')])
print(second_output, end='')
