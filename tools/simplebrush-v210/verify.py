#!/usr/bin/env python3
"""Replay both retained native captures against an archived candidate source."""
import ast
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
receipt = json.loads((HERE / 'receipt.json').read_text())

def sha(data):
    return hashlib.sha256(data).hexdigest()

for filename, digest in receipt['evidence_source_sha256'].items():
    assert sha((HERE / filename).read_bytes()) == digest, filename

with tempfile.TemporaryDirectory(prefix='sdocx-v210-') as tmp:
    base = Path(tmp)
    source = base / 'source'
    source.mkdir()
    archive = base / 'source.tar'
    with archive.open('wb') as out:
        subprocess.run(['git', 'archive', receipt['clean_source_revision']], cwd=ROOT, stdout=out, check=True)
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    for filename, digest in receipt['clean_source_sha256'].items():
        assert sha((source / filename).read_bytes()) == digest, filename
    for patch in ('candidate.patch', 'fixture.patch'):
        subprocess.run(['patch', '-p1', '-i', str(HERE / patch)], cwd=source, check=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith('SDOCX_') and key not in ('VARIANT', 'PROBE_SIZE')}
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0')
    for test in ('simplebrush',):
        subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', test, '--lib'], cwd=source, env=env, check=True)
    for name, capture, frame, variant in [('baseline','baseline-native.json.gz','d-frame.rgba.gz',None), ('visible_alternate','visible-alternate-native.json.gz','g-frame.rgba.gz','alternate')]:
        case = receipt['cases'][name]
        output = base / name
        output.mkdir()
        data = gzip.decompress((HERE / capture).read_bytes())
        assert sha(data) == case['native_json_sha256']
        native = json.loads(data)
        verdict = native['native_gl_error_verdict']
        assert verdict == case['native_gl_verdict'] and verdict['status'] == 'all_observed_draw_readback_task_errors_zero' and verdict['paint_clean'] and verdict['pixel_readback_available']
        (output / 'native.json').write_bytes(data)
        rgba = gzip.decompress((HERE / frame).read_bytes())
        assert sha(rgba) == case['native_frame_sha256'] == native['native_draw_target_readback']['sha256']
        captures = output / 'captures'
        captures.mkdir()
        (captures / f"simplebrush-native-draw-rgba-{case['native_frame_sha256']}.bin").write_bytes(rgba)
        run_env = dict(env, SDOCX_POINTS_COUNT='65', PROBE_SIZE='3', SDOCX_TOOL='0', SDOCX_V62_NATIVE=str(output / 'native.json'), SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(output))
        if variant:
            run_env['VARIANT'] = variant
        subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', 'two_pixel_native_export_fixture', '--lib'], cwd=source, env=run_env, check=True)
        for filename, expected in case['exports_sha256'].items():
            assert sha((output / filename).read_bytes()) == expected, (name, filename)
        text = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-size3-32/compare.py'), str(output)], text=True)
        rows = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1]) for line in text.splitlines()}
        for metric, field in [('svg', 'svg_alpha_error'), ('mupdf', 'matched')]:
            assert [rows[k][field] for k in ('generic','two')] == case['metrics'][metric]
        text = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-v209/pdfium.py'), str(output)], text=True)
        values = {line.split()[0]: int(line.split()[1]) for line in text.splitlines()}
        assert [values[k] for k in ('generic','two')] == case['metrics']['pdfium']
        print(f'{name}: native vectors, export hashes, and reader metrics match')
