#!/usr/bin/env python3
"""Regenerate the native control and verify public SVG and direct PDF exports.

See airbrush_v1_curved_README.md for exact local prerequisites and limits.
Run from the recorded clean archive with this patch applied; source-tree pins
intentionally reject unrelated modifications. Coordinate the shared Cargo target.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib

import pymupdf

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'conformance/airbrush-v1-curved-export-control.json').read_text())
source_paths = [root / 'Cargo.lock', root / 'Cargo.toml', root / 'crates/sdocx/Cargo.toml', *sorted((root / 'crates/sdocx/src').rglob('*.rs'))]
source_pins = sorted((str(path.relative_to(root)), hashlib.sha256(path.read_bytes()).hexdigest()) for path in source_paths)
source_digest = hashlib.sha256(''.join(f'{path} {digest}\n' for path, digest in source_pins).encode()).hexdigest()
if source_digest != receipt['export_source_tree_sha256']:
    raise RuntimeError('export source tree hash mismatch; use the pinned clean archive and patch')
capture = subprocess.run([sys.executable, str(root / 'tools/pen-capture/airbrush_v1_curved_capture.py')], cwd=root, check=True, capture_output=True, text=True)
lines = capture.stdout.splitlines()
frame = json.loads(next(line.removeprefix('FRAME_STATE ') for line in lines if line.startswith('FRAME_STATE ')))['post_frame']
selection = json.loads(next(line for line in lines if line.startswith('{')))
assert selection['selected_profile'] == receipt['selected_profile'] == 1
assert selection['native_hashes'] == receipt['native_library_sha256']
native = zlib.decompress(base64.b64decode(frame['data_base64']))
assert hashlib.sha256(native).hexdigest() == receipt['native_frame']['sha256']
assert zlib.decompress(base64.b64decode(receipt['native_frame']['data_base64'])) == native
flipped = b''.join(native[i*1024:(i+1)*1024] for i in range(255, -1, -1))
with tempfile.TemporaryDirectory() as directory:
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', SDOCX_AIRBRUSH_CURVE_CAPTURE_DIR=directory)
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf', '--lib', 'saved_v1_curved_public_exports_match_native_control'], cwd=root, env=env, check=True)
    assert pymupdf.VersionBind == receipt['pymupdf_version']
    for name in ('candidate', 'generic'):
        doc = pymupdf.open(Path(directory) / f'{name}.pdf')
        pixels = doc[0].get_pixmap(matrix=pymupdf.Matrix(96/72, 96/72), alpha=True).samples
        expected = receipt['results'][name]['pdf']
        assert hashlib.sha256(pixels).hexdigest() == expected['raster_sha256']
        assert sum(abs(a-b) for a,b in zip(pixels, flipped)) == expected['sum_absolute_channel_error']
        print(f"{name} PDF RGBA error: {expected['sum_absolute_channel_error']}")
print('Native frame and public SVG/PDF controls verified')
