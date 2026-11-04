#!/usr/bin/env python3
"""Compare the uniform-pressure control using the retained native readbacks."""
import json
from pathlib import Path
import tempfile

root = Path(__file__).resolve().parents[2]
tool = root / 'tools/defaultpen-scale'
fixture = json.loads((root / 'conformance/defaultpen-tool0-diverse-native.json').read_text())
receipt = json.loads((tool / 'tool0-pressure-receipt.json').read_text())
for name, case in fixture['cases'].items():
    control = receipt['cases'][name]
    assert case['native_stream_sha256'] == control['native_stream_sha256']
    assert case['rgba_sha256'] == control['rgba_sha256']
    case['input'] = control['full']['input']
with tempfile.TemporaryDirectory(prefix='sdocx-defaultpen-channels-') as temp:
    path = Path(temp) / 'fixture.json'
    path.write_text(json.dumps(fixture))
    source_path = tool / 'compare_tool0_diverse.py'
    source = source_path.read_text()
    old = "root / 'conformance/defaultpen-tool0-diverse-native.json'"
    assert source.count(old) == 1
    source = source.replace(old, f'Path({str(path)!r})')
    # Copy exact tool files referenced by the current workspace's Rust tests.
    marker = "    fixture = json.loads(fixture_path.read_text())"
    extra = r"""    import re
    for rust in (root / 'crates').rglob('*.rs'):
        for relative in re.findall(r'include(?:_str|_bytes)?!\(\s*\"([^\"]+)\"', rust.read_text()):
            referenced = (rust.parent / relative).resolve()
            try:
                entry = referenced.relative_to(root)
            except ValueError:
                continue
            if entry.parts[0] != 'tools' or not referenced.is_file():
                continue
            destination = copy / entry
            destination.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(['cp', str(referenced), str(destination)], check=True)
"""
    assert source.count(marker) == 1
    source = source.replace(marker, extra + marker)
    exec(compile(source, str(source_path), 'exec'), {'__file__': str(source_path), '__name__': '__main__'})
