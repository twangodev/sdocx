#!/usr/bin/env python3
"""Apply one checked intervention to the disposable v173 source test."""
from pathlib import Path
import sys

path = Path(sys.argv[1])
changes = {
    'aa': ('tape_size_level(12.0, 256, 256)', 'tape_size_level(12.0, 512, 512)'),
    'quad': ('(1.6800001_f32, 6.7200003_f32)', '(1.7754546_f32, 7.1018186_f32)'),
}
old, new = changes[sys.argv[2]]
text = path.read_text()
marker = 'fn v173_source_mask() {'
assert text.count(marker) == 1
prefix, body = text.split(marker)
assert body.count(old) == 1, (sys.argv[2], body.count(old))
path.write_text(prefix + marker + body.replace(old, new, 1))
