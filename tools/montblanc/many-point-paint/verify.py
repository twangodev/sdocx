#!/usr/bin/env python3
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import sys
import tempfile
import zlib

import pymupdf
from PIL import Image
import PIL

repository = Path(__file__).resolve().parents[3]
receipt = json.loads(subprocess.check_output(['git', 'show', ':conformance/montblanc-many-point-paint-host.json'], cwd=repository))
revision = receipt['source_revision']
subprocess.run(['git', 'cat-file', '-e', f'{revision}^{{commit}}'], cwd=repository, check=True)
with tempfile.TemporaryDirectory(prefix='montblanc-many-point-') as directory:
    tmp = Path(directory)
    root = tmp / 'source'
    root.mkdir()
    archive = tmp / 'source.tar'
    subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), revision], cwd=repository, check=True)
    with tarfile.open(archive) as source:
        source.extractall(root, filter='data')
    for name, digest in receipt['source_sha256'].items():
        data = subprocess.check_output(['git', 'show', f':{name}'], cwd=repository)
        assert hashlib.sha256(data).hexdigest() == digest, name
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    assert hashlib.sha256(Path(__file__).read_bytes()).hexdigest() == receipt['source_sha256']['tools/montblanc/many-point-paint/verify.py']
    (root / 'scratch').symlink_to(repository / 'scratch', target_is_directory=True)
    package = root / 'tools/montblanc/many-point-paint'
    sys.path.insert(0, str(root / 'tools/montblanc'))
    from public_fallback_compare import composite_native_frame, metrics
    fixture_path = root / 'conformance/montblanc-many-point-independent-streams.json'
    assert hashlib.sha256(fixture_path.read_bytes()).hexdigest() == receipt['geometry_fixture_sha256']
    fixture = json.loads(fixture_path.read_text())
    assert subprocess.check_output(['resvg', '--version'], text=True).strip() == receipt['renderers']['resvg']
    assert pymupdf.VersionBind == receipt['renderers']['pymupdf'] and PIL.__version__ == receipt['renderers']['pillow']
    for scale in (1, 2):
        assert hashlib.sha256((package / f'capture-{scale}x.py').read_bytes()).hexdigest() == receipt['capture_script_sha256'][f'{scale}x']
    env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0', 'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0', 'SDOCX_CAPTURE_ROOT': str(root), 'PYTHONPATH': str(root / 'scratch/apk-analysis-runtime/python') + os.pathsep + str(root / 'tools/montblanc/extended-paint/conformance')}
    env.pop('SDOCX_MONTBLANC_DOUBLE', None)
    for scale in (1, 2):
        output = tmp / str(scale)
        output.mkdir()
        subprocess.run(['cargo', 'run', '--locked', '--offline', '--quiet', '--manifest-path', str(root / 'tools/montblanc/many-point-public/Cargo.toml'), '--', str(output)], cwd=root, env=env | ({'SDOCX_MONTBLANC_DOUBLE': '1'} if scale == 2 else {}), check=True)
        for name, case in receipt['cases'].items():
            expected = case[f'{scale}x']
            capture = tmp / f'{scale}-{name}.json'
            subprocess.run([sys.executable, str(package / f'capture-{scale}x.py'), '--case', 'baseline', '--output', str(capture)], cwd=root, env=env | {'MB_CASE': json.dumps(fixture['cases'][name]['input'])}, check=True)
            native = json.loads(capture.read_text())
            for key in ('input_pins', 'host_gl_sha256', 'matrix_f32'):
                assert native[key] == expected[key], (name, scale, key)
            assert [v['sha256'] for v in native['native_add_vectors']] == expected['vector_sha256']
            frame = zlib.decompress(base64.b64decode(native['post_readback']['rgba8_zlib_base64']))
            assert hashlib.sha256(frame).hexdigest() == expected['frame_sha256']
            reference = composite_native_frame(frame, (256, 256)).tobytes()
            for mode, comparison in expected['metrics'].items():
                path = output / f'{name}-{mode}'
                png = tmp / 'output.png'
                subprocess.run(['resvg', '-w', '256', str(path) + '.svg', str(png)], check=True)
                with Image.open(png) as image:
                    assert image.size == (256, 256)
                    rgb = image.convert('RGB').tobytes()
                assert len(rgb) == len(reference) == 256 * 256 * 3
                svg = metrics(reference, rgb)
                with pymupdf.open(str(path) + '.pdf') as doc:
                    pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4*scale/3, 4*scale/3), alpha=False)
                    assert (pix.width, pix.height) == (256, 256)
                    assert len(pix.samples) == len(reference)
                    pdf = metrics(reference, pix.samples)
                assert svg == comparison['svg'] and pdf == comparison['pdf'], (name, scale, mode)
                assert Path(str(path) + '.svg').stat().st_size == comparison['svg_bytes']
                assert Path(str(path) + '.pdf').stat().st_size == comparison['pdf_bytes']
            print(scale, name, 'verified', flush=True)
