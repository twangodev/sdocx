#!/usr/bin/env python3
"""Replay retained native controls against the pinned public export fixture."""
import ast
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

import numpy
import PIL
import pymupdf
import pypdfium2

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
receipt = json.loads((HERE / 'receipt.json').read_text())
expected = receipt['reader_versions']
actual = {
    'resvg': subprocess.check_output(['resvg', '--version'], text=True).strip().removeprefix('resvg '),
    'pymupdf': pymupdf.VersionBind,
    'pdfium': str(pypdfium2.PDFIUM_INFO),
    'pypdfium2': str(pypdfium2.PYPDFIUM_INFO.version),
    'pillow': PIL.__version__,
    'numpy': numpy.__version__,
}
if actual != expected:
    raise RuntimeError(f'reader version mismatch: expected {expected}, got {actual}')
with tempfile.TemporaryDirectory(prefix='simplebrush-128-') as tmp:
    base = Path(tmp)
    source = base / 'source'
    source.mkdir()
    archive = base / 'source.tar'
    with archive.open('wb') as output:
        subprocess.run(['git', 'archive', receipt['source_revision']], cwd=ROOT, stdout=output, check=True)
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    for patch in ('candidate.patch', 'fixture.patch'):
        subprocess.run(['patch', '-p1', '-i', str(HERE / patch)], cwd=source, check=True)
    env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2')
    for name in ('baseline', 'alternate'):
        case = receipt['cases'][name]
        output = base / name
        output.mkdir()
        native = json.loads((HERE / f'{name}-native.json').read_text())
        assert native['native_gl_error_verdict'] == case['native_gl_verdict']
        (output / 'native.json').write_text(json.dumps(native))
        frame = gzip.decompress((HERE / f'{name}-frame.rgba.gz').read_bytes())
        assert hashlib.sha256(frame).hexdigest() == case['frame_sha256']
        captures = output / 'captures'
        captures.mkdir()
        (captures / f"simplebrush-native-draw-rgba-{case['frame_sha256']}.bin").write_bytes(frame)
        run_env = dict(env, SDOCX_POINTS_COUNT='128', PROBE_SIZE='3', SDOCX_TOOL='0', SDOCX_V62_NATIVE=str(output / 'native.json'), SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(output))
        run_env.pop('VARIANT', None)
        if name == 'alternate':
            run_env['VARIANT'] = 'alternate'
        subprocess.run(['cargo', 'test', '--locked', '--offline', '-p', 'sdocx', '--features', 'pdf,serde', 'two_pixel_native_export_fixture', '--lib'], cwd=source, env=run_env, check=True)
        for filename, digest in case['export_sha256'].items():
            assert hashlib.sha256((output / filename).read_bytes()).hexdigest() == digest
        text = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-size3-32/compare.py'), str(output)], text=True)
        rows = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1]) for line in text.splitlines()}
        for metric, field in [('svg_alpha', 'svg_alpha_error'), ('mupdf_rgb', 'matched')]:
            assert [rows[k][field] for k in ('generic', 'two')] == case['errors_generic_candidate'][metric]
        text = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-v209/pdfium.py'), str(output)], text=True)
        values = {line.split()[0]: int(line.split()[1]) for line in text.splitlines()}
        assert [values[k] for k in ('generic', 'two')] == case['errors_generic_candidate']['pdfium_rgb']
        print(f'{name}: native geometry, export hashes, and reader metrics match')
