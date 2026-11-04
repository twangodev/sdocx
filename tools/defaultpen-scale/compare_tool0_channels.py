#!/usr/bin/env python3
"""Compare the nonzero-channel control using the retained native readbacks."""
import json
from pathlib import Path
import tempfile

root = Path(__file__).resolve().parents[2]
tool = root / 'tools/defaultpen-scale'
fixture = json.loads((root / 'conformance/defaultpen-tool0-diverse-native.json').read_text())
receipt = json.loads((tool / 'tool0-channels-receipt.json').read_text())
for name, case in fixture['cases'].items():
    control = receipt['cases'][name]
    assert case['native_stream_sha256'] == control['native_stream_sha256']
    assert case['rgba_sha256'] == control['rgba_sha256']
    case['input'] = control['input']
with tempfile.TemporaryDirectory(prefix='sdocx-defaultpen-channels-') as temp:
    path = Path(temp) / 'fixture.json'
    path.write_text(json.dumps(fixture))
    source_path = tool / 'compare_tool0_diverse.py'
    source = source_path.read_text()
    old = "root / 'conformance/defaultpen-tool0-diverse-native.json'"
    assert source.count(old) == 1
    source = source.replace(old, f'Path({str(path)!r})')
    exec(compile(source, str(source_path), 'exec'), {'__file__': str(source_path), '__name__': '__main__'})
