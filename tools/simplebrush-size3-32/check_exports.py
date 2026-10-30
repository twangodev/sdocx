#!/usr/bin/env python3
"""Regress public exports against retained native endpoint frames."""
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

import pymupdf
import pypdfium2

ROOT = Path(__file__).resolve().parents[2]
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == '0.45.1'
assert (pymupdf.VersionBind, pymupdf.VersionFitz) == ('1.28.2', '1.28.2')
assert str(pypdfium2.PYPDFIUM_INFO) == '5.9.0'
assert str(pypdfium2.PDFIUM_INFO) == '150.0.7869.0'
for count in (16, 64):
    receipt = json.loads((ROOT / f'conformance/simplebrush-size3-{count}-native.json').read_text())
    native = zlib.decompress((ROOT / f'tools/simplebrush-size3-32/frame-{count}.rgba.zlib').read_bytes())
    frame_hash = receipt['frame_sha256']
    assert hashlib.sha256(native).hexdigest() == frame_hash
    with tempfile.TemporaryDirectory(prefix='simplebrush-size3-check-') as path:
        work = Path(path)
        (work / 'captures').mkdir()
        (work / 'captures' / f'simplebrush-native-draw-rgba-{frame_hash}.bin').write_bytes(native)
        native_json = work / 'native.json'
        native_json.write_text(json.dumps({'drawing_vector': receipt['geometry'], 'native_draw_target_readback': {'sha256': frame_hash}}))
        env = os.environ.copy()
        env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_POINTS_COUNT=str(count), PROBE_SIZE='3', SDOCX_TOOL='0', SDOCX_V62_NATIVE=str(native_json), SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(work))
        subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', '--lib', 'two_pixel_native_export_fixture'], cwd=ROOT, env=env, check=True)
        def compare(script):
            return subprocess.check_output([os.sys.executable, str(ROOT / script), str(work)], text=True)
        lines = compare('tools/simplebrush-size3-32/compare.py').splitlines()
        results = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1]) for line in lines}
        generic, candidate = results['generic'], results['two']
        assert candidate['svg_support'][0] == candidate['svg_support'][1]
        assert candidate['svg_alpha_error'] < generic['svg_alpha_error'] * 0.1
        assert candidate['pdf_rgb_error'] < generic['pdf_rgb_error'] * 0.8
        pdfium = {line.split()[0]: int(line.split()[1]) for line in compare('tools/simplebrush-size3-32/pdfium.py').splitlines()}
        assert pdfium['two'] < pdfium['generic'] * 0.4
        print(f'{count} points: {results}; PDFium {pdfium}', flush=True)
