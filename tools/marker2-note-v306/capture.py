"""Generate the saved-source Marker2 control with the existing pinned host harness."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
source_path = root / 'crates/sdocx/tests/assets/fountain-v14/saved-source-29.json'
receipt = json.loads((root / 'tools/marker2-note-v306/receipt.json').read_text())
assert hashlib.sha256(source_path.read_bytes()).hexdigest() == receipt['source_sha256']
source = json.loads(source_path.read_text())
case = {'points': [[p['x'] - 96, p['y'] - 268] for p in source['points']],
        'size': 10., 'curve_enabled': True, 'tool_type': 2, 'stroke_type': 0,
        'pressure': source['pressures'], 'timestamps': source['timestamps'],
        'tilt': source['tilts'], 'orientation': source['orientations']}
base = root / 'tools/marker2-note-v306/native_runner.py'
manifest = json.loads((base.parent / 'native_manifest.json').read_text())
assert hashlib.sha256(base.read_bytes()).hexdigest() == manifest['runner_sha256']
text = base.read_text()
def replace_once(old, new):
    global text
    assert text.count(old) == 1, old
    text = text.replace(old, new)
replace_once('REPO=Path(__file__).resolve().parents[4]', f'REPO=Path({str(root)!r})')
replace_once("MANIFEST=Path(__file__).with_name('manifest.json')", f'MANIFEST=Path({str(base.parent / "native_manifest.json")!r})')
old = " source=caller_original.decode('utf-8').replace('[[10.0,20.0],[18.0,25.0],[26.0,20.0],[34.0,25.0]]', '[[10.0,20.0],[12.0,33.0],[28.0,12.0],[34.0,25.0]]')"
new = " source=caller_original.decode('utf-8')\n old=next(line for line in source.splitlines() if line.startswith(\" 'marker2_nontrivial_saved3':\"))\n source=source.replace(old, \" 'marker2_nontrivial_saved3': \"+repr(" + repr(case) + ")+\",\")"
assert text.count(old) == 1
replace_once(old, new)
replace_once("if manifest!=expected: raise RuntimeError('frozen manifest does not exactly match runner and source pins')", "if {**manifest, 'runner_sha256':runner_hash}!=expected: raise RuntimeError('frozen manifest source pins mismatch')")
replace_once("int(u['size'])==len(vector_data)]", "int(u['size'])==len(vector_data) and u.get('dataHex')==vector_data.hex()]")
replace_once("source_case=json.loads(source_fixture.read_text())['cases'][source_case_key]", "(CLI.output_dir/'native-phases.json').write_text(source_fixture.read_text())\nsource_case=json.loads(source_fixture.read_text())['cases'][source_case_key]")
with tempfile.TemporaryDirectory(prefix='marker2-note-runner-') as tmp:
    runner = Path(tmp) / 'runner.py'
    runner.write_text(text)
    subprocess.run([sys.executable, runner, '--one-case', '--size', '2', '--phase', 'canonical', '--alpha', '128', '--output-dir', sys.argv[1]], check=True, cwd=root)

output = Path(sys.argv[1])
case_output = next(iter(json.loads((output / 'native-phases.json').read_text())['cases'].values()))
for key in ('input', 'phase_final_vectors', 'sample_boundaries_from_native_drawLine_returns'):
    assert case_output[key] == receipt[key], key
assert hashlib.sha256((output / 'w2-canonical-a128-composite-rgba.png').read_bytes()).hexdigest() == receipt['native_paint']['frame_sha256']
print('native input, phases, boundaries and frame match receipt')
