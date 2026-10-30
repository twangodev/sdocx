#!/usr/bin/env python3
"""Prepare the prior V16 generator for the retained changed-path control."""
import gzip
import hashlib
import json
import shutil
import tempfile
from pathlib import Path
root = Path('/home/jding/RustroverProjects/sdocx')
here = Path(__file__).resolve().parent
work = Path(tempfile.mkdtemp(prefix='sdocx-crayon2-'))
out = work / 'generator'
out.mkdir()
(out / 'src').mkdir(exist_ok=True)
old = root / 'tmp/vector-parity-plan/crayon2-svg-material-root-v16-direct-rgba'
archive = root / 'tmp/conformance-cleanup/native-capture-a241793/conformance/crayon2-drawing-admission-native.json'
capture = json.loads(archive.read_bytes())
witness = json.loads(gzip.decompress((here / 'transparent.json.gz').read_bytes()))
paint = witness['paint']
case = capture['paint_cases'][2]
for key in ('draw_calls', 'native_rt_rgba_f32', 'native_vector_bytes', 'native_vector_hex', 'native_vector_sha256', 'readback'):
    case[key] = paint[key]
case['source_argb'] = paint['source_argb_input']
case['background'] = paint['target_initial_background']
capture['paint_cases'] = [case]
capture['source_inputs_by_argb']['0xff336699'] = {'cases': [witness['source_v2']]}
raw = json.dumps(capture).encode()
capture_path = work / 'capture.json'
capture_path.write_bytes(raw)
s = (old / 'src/main.rs').read_text()
for a, b in [
 ('tmp/conformance-cleanup/native-capture-a241793/conformance/crayon2-drawing-admission-native.json', str(capture_path)),
 ('177c69e32bcd7c10efa07acb44ed6237375e8c6b40fd6e6606ed5fddacf4504e', hashlib.sha256(raw).hexdigest()),
 ('6a9f78b87ce3a5a6d0054dc918274780a2a3f6ec617adcf69f9ec5fb6f387416', paint['native_vector_sha256']),
 ('const RECORD_COUNT: usize = 268;', 'const RECORD_COUNT: usize = 303;'),
 ('map_or(0, Vec::len) != 6', 'map_or(0, Vec::len) != 1'),
 ('json!([5, 0, 4, 268])', 'json!([5, 0, 4, 303])'),
]:
    assert a in s
    s = s.replace(a, b)
(out / 'src/main.rs').write_text(s)
shutil.copy(old / 'Cargo.toml', out / 'Cargo.toml')

print(work)
