#!/usr/bin/env python3
"""Replay the paired SimpleBrush controls using the retained local scratch harness.

Requires the pinned native libraries, Unicorn, AArch64 binutils and Mesa assets
at the scratch/tmp paths used by the harness. Run from the repository root.
The harness writes full frame captures under its scratch capture directory.
"""
import json
import os
from pathlib import Path
import struct
import subprocess
import sys

root = Path(__file__).resolve().parent.parent
receipt = json.loads((root / 'conformance/simplebrush-tool0-terminal-native.json').read_text())
harness = root / 'scratch/simplebrush-tool3-multi/simplebrush-5-paint-capture.py'
if not harness.is_file():
    raise SystemExit(f'Local capture harness is missing: {harness}')

def decode(value):
    return struct.unpack('<f', bytes.fromhex(value))[0]

for name, control in receipt['controls'].items():
    for tool in (0, 2):
        source = control['paired_native'][str(tool)]['input']
        env = os.environ.copy()
        env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
        env['SIMPLEBRUSH_POINTS'] = json.dumps([[decode(v) for v in xy] for xy in source['points_f32_bits']])
        env['SIMPLEBRUSH_PRESSURES'] = json.dumps([decode(v) for v in source['pressure_f32_bits']])
        command = [sys.executable, str(harness), '--case', 'simplebrush_pressure_first075', '--tool-type', str(tool)]
        for sequence in range(8):
            command.extend(('--dispatch-task-seq', str(sequence)))
        result = json.loads(subprocess.check_output(command, cwd=root, env=env))
        expected = control['paired_native'][str(tool)]
        assert result['drawing_vector']['sha256'] == expected['geometry_sha256'], (name, tool, 'geometry')
        assert result['native_draw_target_readback']['sha256'] == expected['frame_sha256'], (name, tool, 'frame')
        assert result['native_gl_error_verdict']['paint_clean'], (name, tool, 'GL error')
        print(f'{name}: tool {tool} geometry and frame match', flush=True)
