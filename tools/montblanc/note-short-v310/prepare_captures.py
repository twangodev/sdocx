#!/usr/bin/env python3
"""Expand retained capture metadata into a new directory for export_compare.py."""
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent
native = json.loads((root / (sys.argv[2] if len(sys.argv) > 2 else 'native.json')).read_text())
out = Path(sys.argv[1])
out.mkdir(exist_ok=False)
for name, case in native.items():
    for scale, frame in case['frames'].items():
        stem = out / f"{name}-{case['input']['pen_size']}-{scale}"
        payload = {'post_readback': frame['frame'], 'matrix_f32': frame['matrix'],
                   'style': frame['style'], 'target': frame['target'],
                   'host_gl_sha256': frame['host_gl_sha256'], 'input_pins': frame['input_pins']}
        Path(str(stem) + '.json').write_text(json.dumps(payload))
        Path(str(stem) + '.input.json').write_text(json.dumps(case['input']))
        Path(str(stem) + '.stream.json').write_text(json.dumps(case['stream']))
