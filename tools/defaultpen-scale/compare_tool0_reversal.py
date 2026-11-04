#!/usr/bin/env python3
"""Compare retained reversal frames with public SVG and direct PDF exports."""
import argparse
import contextlib
import io
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--verify', action='store_true', help='Require exact agreement with retained receipt')
args = parser.parse_args()
root = Path(__file__).resolve().parent
source_path = root / 'compare_tool0_diverse.py'
source = source_path.read_text()
replacements = {
    "root / 'conformance/defaultpen-tool0-diverse-native.json'": "root / 'tools/defaultpen-scale/tool0-reversal-native.json'",
    "'tools/inkpen2-v1-tool3/extended-receipt.json'": "'tools/inkpen2-v1-tool3/extended-receipt.json', 'tools/inkpen2-v1-landscape/receipt.json', 'tools/inkpen2-v1-tablet-half/bounded_grid.rs', 'tools/inkpen2-v1-tablet-half/receipt.json', 'tools/inkpen2-v1-tablet-half/multi-phase-model.json', 'tools/inkpen2-v1-landscape-expanded/receipt.json'",
    "paint_argb='ff336699', native_uniform_color_f32_bits=template['native_uniform_color_f32_bits'], projection=template['projection']": "paint_argb=case['paint_argb'], native_uniform_color_f32_bits=case['native_uniform_color_f32_bits'], projection=case['projection']",
}
for old, new in replacements.items():
    assert source.count(old) == 1
    source = source.replace(old, new)
output = io.StringIO()
with contextlib.redirect_stdout(output):
    exec(compile(source, str(source_path), 'exec'), {'__file__': str(source_path), '__name__': '__main__'})
result = json.loads(output.getvalue())
if args.verify:
    expected = json.loads((root / 'tool0-reversal-export.json').read_text())
    if result != expected:
        raise SystemExit('Export receipt differs from retained inputs, versions, measurements, or output hashes')
    print('Export receipt verified')
else:
    print(json.dumps(result, indent=2))
