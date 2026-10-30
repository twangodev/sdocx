#!/usr/bin/env python3
"""Replay the retained capture against an archived clean source revision."""
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zlib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
receipt = json.loads((HERE / 'receipt.json').read_text())
revision = receipt['clean_source_revision']

def sha(data):
    return hashlib.sha256(data).hexdigest()

with tempfile.TemporaryDirectory(prefix='sdocx-v209-') as tmp:
    base = Path(tmp)
    source = base / 'source'
    source.mkdir()
    archive = base / 'source.tar'
    with archive.open('wb') as out:
        subprocess.run(['git', 'archive', revision], cwd=ROOT, stdout=out, check=True)
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    output = base / 'output'
    output.mkdir()
    data = zlib.decompress((HERE / 'native-full.json.zlib').read_bytes())
    assert sha(data) == receipt['native_full_sha256']
    verdict = json.loads(data)['native_gl_error_verdict']
    assert verdict['status'] == 'all_observed_draw_readback_task_errors_zero'
    assert verdict['paint_clean'] and verdict['pixel_readback_available']
    (output / 'native.json').write_bytes(data)
    frame = zlib.decompress((HERE / 'frame.rgba.zlib').read_bytes())
    h = receipt['native_frame_sha256']
    assert sha(frame) == h
    captures = output / 'captures'
    captures.mkdir()
    (captures / f'simplebrush-native-draw-rgba-{h}.bin').write_bytes(frame)
    env = dict({key: value for key, value in os.environ.items() if not key.startswith('SDOCX_')}, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', SDOCX_POINTS_COUNT='64', VARIANT='alternate', PROBE_SIZE='3', SDOCX_TOOL='0', SDOCX_V62_NATIVE=str(output / 'native.json'), SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(output))
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', 'two_pixel_native_export_fixture', '--lib'], cwd=source, env=env, check=True)
    for name, expected in receipt['export_sha256'].items():
        assert sha((output / name).read_bytes()) == expected, name
    text = subprocess.check_output([sys.executable, str(source / 'tools/simplebrush-size3-32/compare.py'), str(output)], text=True)
    rows = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1]) for line in text.splitlines()}
    metrics = receipt['metrics']
    for label, field in [('resvg_0.45.1_alpha_absolute_error', 'svg_alpha_error'), ('mupdf_1.28.2_matched_paper_rgb_absolute_error', 'matched')]:
        assert {k: rows[k][field] for k in ('generic', 'two')} == metrics[label]
    wrapper = source / 'tools/simplebrush-v209/pdfium.py'
    wrapper.parent.mkdir()
    wrapper.write_bytes((HERE / 'pdfium.py').read_bytes())
    text = subprocess.check_output([sys.executable, str(wrapper), str(output)], text=True)
    values = {line.split()[0]: int(line.split()[1]) for line in text.splitlines()}
    assert {k: values[k] for k in ('generic', 'two')} == metrics['pdfium_150.0.7869.0_unshifted_rgb_absolute_error']
    print('Clean source vectors, export hashes, and reader metrics match.')
