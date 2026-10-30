#!/usr/bin/env python3
"""Count exact constant runs in the retained Pencil2 host shader textures."""
from collections import Counter
from hashlib import sha256
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / 'crates/sdocx/src/ink/pencil2/assets'


def analyze(name, width, height, digest, binary=False):
    data = (ASSETS / f'{name}.r8').read_bytes()
    if len(data) != width * height:
        raise ValueError(f'{name}: unexpected texture length {len(data)}')
    if sha256(data).hexdigest() != digest:
        raise ValueError(f'{name}: texture SHA-256 mismatch')
    values = [int(value != 0) if binary else value for value in data]
    runs = sum(1 + sum(a != b for a, b in zip(row, row[1:]))
               for row in (values[y * width:(y + 1) * width] for y in range(height)))
    vertical_edges = sum(values[y * width + x] != values[(y - 1) * width + x]
                         for y in range(1, height) for x in range(width))
    counts = Counter(values)
    print(f'{name}: {width}x{height}, distinct={len(counts)}, '
          f'horizontal_runs={runs}, vertical_changes={vertical_edges}')


analyze('grain', 150, 150, 'dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9')
analyze('point', 100, 300, '475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f', binary=True)
