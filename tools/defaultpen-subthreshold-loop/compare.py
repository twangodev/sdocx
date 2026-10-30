#!/usr/bin/env python3
"""Compare retained subthreshold-loop frames with public SVG and direct PDF exports."""
import argparse
import contextlib
import io
import hashlib
import json
import sys
from pathlib import Path

if sys.flags.optimize:
    raise SystemExit('Optimized Python is unsupported')

parser = argparse.ArgumentParser()
parser.add_argument('--verify', action='store_true', help='Require exact agreement with retained receipt')
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent / 'defaultpen-scale'
control = Path(__file__).resolve().parent
source_path = root / 'compare_tool0_diverse.py'
pins = {
    'tool0_crossing_capture.py': '60198d57005d8cf45a1f33fcf5953cb16a55baf7a9bfc93d3d949a3c141e006f',
    'tool0_diverse_capture.py': 'c27b22bedd06a4d5634d49b120b25111bc55760529e584ae6d2b9992040d5a68',
    'retain_tool0_crossing.py': 'a9377ff3b86871a4a483f38fccf006981d48fb4326cd7a15f87e3782a0772161',
    'compare_tool0_diverse.py': 'fd9a7182b0b8239210cd1268a5df50adb60ccbbb79fb3f1557bd0527433eed6b',
}
for name, expected in pins.items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != expected:
        raise SystemExit(f'Source hash mismatch: {name}')
fixture = json.loads((control / 'native.json').read_text())
for case in fixture['cases'].values():
    if case['native_drawing_phase_stream_sha256'] != case['native_stream_sha256']:
        raise SystemExit('Native Drawing callback and bridged stream differ')
source = source_path.read_text()
replacements = {
    "root / 'conformance/defaultpen-tool0-diverse-native.json'": "root / 'tools/defaultpen-subthreshold-loop/native.json'",
    "paint_argb='ff336699', native_uniform_color_f32_bits=template['native_uniform_color_f32_bits'], projection=template['projection']": "paint_argb=case['paint_argb'], native_uniform_color_f32_bits=case['native_uniform_color_f32_bits'], projection=case['projection']",
    "digest(root / 'Cargo.lock')": "digest(copy / 'Cargo.lock')",
    "digest(root / 'conformance/defaultpen-scale-native.json')": "base_fixture_sha256",
}
for old, new in replacements.items():
    if source.count(old) != 1:
        raise SystemExit(f'Unexpected comparator source: {old}')
    source = source.replace(old, new)
start = source.index("    for entry in ('Cargo.toml'")
end = source.index('    fixture = json.loads(fixture_path.read_text())', start)
revision = '8ef491537f96154099e7c0c4066c25bba0f4aa0a'
source = source[:start] + f"""    import io, re, tarfile
    archive = subprocess.check_output(['git', 'archive', '{revision}', 'Cargo.toml', 'Cargo.lock', 'crates', 'conformance'], cwd=root)
    with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
        bundle.extractall(copy, filter='data')
    dependencies = {{}}
    for rust in (copy / 'crates').rglob('*.rs'):
        for rel in re.findall(r'include(?:_str|_bytes)?!\\s*\\(\\s*"([^"]+)"', rust.read_text()):
            target = (rust.parent / rel).resolve()
            if target.is_relative_to(copy / 'tools'):
                name = str(target.relative_to(copy))
                data = subprocess.check_output(['git', 'show', '{revision}:' + name], cwd=root)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                dependencies[name] = hashlib.sha256(data).hexdigest()
    source_path = copy / 'crates/sdocx/src/render.rs'
    render_source_sha256 = digest(source_path)
    base_fixture_sha256 = digest(copy / 'conformance/defaultpen-scale-native.json')
""" + source[end:]
source = source.replace("'render_rs_sha256': digest(source_path)", "'render_rs_sha256': render_source_sha256")
source = source.replace("'fixture_sha256': digest(fixture_path)", f"'source_revision': '{revision}', 'tool_dependencies_sha256': dependencies, 'fixture_sha256': digest(fixture_path)")
result_capture_sha256 = hashlib.sha256((control / 'capture.py').read_bytes()).hexdigest()
output = io.StringIO()
with contextlib.redirect_stdout(output):
    exec(compile(source, str(source_path), 'exec'), {'__file__': str(source_path), '__name__': '__main__'})
result = json.loads(output.getvalue())
result['inputs']['subthreshold_loop_wrapper_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
result['inputs']['source_pins'] = pins
result['inputs']['capture_sha256'] = result_capture_sha256
if args.verify:
    expected = json.loads((control / 'export.json').read_text())
    if result != expected:
        raise SystemExit('Export receipt differs from retained inputs, versions, measurements, or output hashes')
    print('Export receipt verified')
else:
    print(json.dumps(result, indent=2))
