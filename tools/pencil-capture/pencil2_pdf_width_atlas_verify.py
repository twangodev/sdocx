"""Check the pinned local atlas diagnostic and its recorded error metrics."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

import pymupdf
import pypdfium2

if not __debug__:
    raise SystemExit('optimized Python is unsupported; run without -O')

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'conformance/pencil2-v14-pdf-width-atlas-control.json').read_text())
for name, expected in receipt['inputs_sha256'].items():
    path = (root / name).resolve()
    if not path.is_relative_to(root):
        raise SystemExit(f'input escapes checkout: {name}')
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise SystemExit(f'missing or changed input: {path}')
versions = {'resvg': subprocess.check_output(['resvg', '--version'], text=True).strip(),
            'mupdf': pymupdf.VersionBind, 'pdfium': str(pypdfium2.PDFIUM_INFO)}
if versions != receipt['readers']:
    raise SystemExit(f'reader versions differ: {versions}')
env = os.environ.copy()
env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0',
           CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0')
subprocess.run(['cargo', 'build', '--locked', '--manifest-path',
                str(root / 'tools/pencil2-vector-width/Cargo.toml'),
                '--bin', 'pencil2-mask-control-runner'], cwd=root, env=env, check=True)
env['PENCIL2_ATLAS_EXPORTER'] = '/tmp/sdocx-agent-target/debug/pencil2-mask-control-runner'
for name in ('baseline', 'scales'):
    path = root / f'tools/pencil-capture/pencil2_pdf_width_atlas_{name}.py'
    output = subprocess.check_output([sys.executable, str(path)], cwd=root, text=True, env=env)
    print(output, end='')
    lines = output.splitlines()
    if name == 'baseline':
        seen = set()
        for line in lines:
            fields = line.split()
            variant = {'0': 'original', '16': 'atlas16'}[fields[0]]
            reader = {'svg': 'resvg', 'mupdf': 'mupdf', 'pdfium': 'pdfium'}[fields[1]]
            key = (variant, reader)
            if key in seen:
                raise SystemExit(f'duplicate baseline case: {key}')
            seen.add(key)
            assert [int(fields[3]), int(fields[5])] == receipt['results']['baseline'][variant][reader], line
        assert seen == {(variant, reader) for variant in ('original', 'atlas16')
                        for reader in ('resvg', 'mupdf', 'pdfium')}, seen
    else:
        seen = set()
        for line in lines:
            fields = line.split()
            variant = {'0': 'original', '16': 'atlas16', '64': 'atlas64'}[fields[1]]
            key = (fields[0], variant)
            if key in seen:
                raise SystemExit(f'duplicate scale case: {key}')
            seen.add(key)
            assert [int(fields[2]), int(fields[3])] == receipt['results']['absolute_alpha_error_sum'][fields[0]][variant], line
        assert seen == {(case, variant) for case in ('scalehalf', 'scale4', 'translate3')
                        for variant in ('original', 'atlas16', 'atlas64')}, seen
print('All recorded error metrics match.')
