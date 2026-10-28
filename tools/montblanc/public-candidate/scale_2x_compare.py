#!/usr/bin/env python3
"""Replay the isolated scale-aware derivative experiment and compare native paint."""
import hashlib
import json
import os
from pathlib import Path
import tarfile
import subprocess
import sys
import tempfile

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import metrics

RECEIPT = ROOT / 'conformance/montblanc-scene-scale-2x-comparison.json'
PATCH = ROOT / 'tools/montblanc/public-candidate/scale_2x_experiment.patch'
SOURCE = ROOT / 'crates/sdocx/src/render/montblanc_candidate.rs'
CROP = ROOT / 'conformance/montblanc-scale-2x.rgba'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    receipt = json.loads(RECEIPT.read_text())
    for path, key in [(SOURCE, 'source_sha256'), (PATCH, 'patch_sha256'), (CROP, 'native_crop_sha256')]:
        assert sha(path) == receipt[key], f'input changed: {path}'
    assert pymupdf.VersionBind == receipt['renderers']['pymupdf']
    native = Image.new('RGBA', (256, 256))
    native.paste(Image.frombytes('RGBA', (47, 14), CROP.read_bytes()), (125, 124))
    background = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
    background.alpha_composite(native.transpose(Image.Transpose.FLIP_TOP_BOTTOM))
    reference = background.convert('RGB').tobytes()
    with tempfile.TemporaryDirectory(prefix='sdocx-montblanc-scale-') as directory:
        tmp = Path(directory)
        archive = tmp / 'source.tar'
        subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), 'HEAD'],
                       cwd=ROOT, check=True)
        with tarfile.open(archive) as source_archive:
            source_archive.extractall(tmp, filter='data')
        archive.unlink()
        archived_source = tmp / 'crates/sdocx/src/render/montblanc_candidate.rs'
        assert sha(archived_source) == receipt['source_sha256'], 'HEAD candidate source changed'
        subprocess.run(['patch', '-p1', '-i', str(PATCH)], cwd=tmp, check=True)
        for mode in ('fixed', 'aware'):
            output = tmp / mode
            output.mkdir()
            env = os.environ.copy()
            env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0',
                       CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0',
                       SDOCX_MONTBLANC_CAPTURE_DIR=str(output))
            env.pop('MONTBLANC_SCALE_DERIVATIVE', None)
            if mode == 'aware':
                env['MONTBLANC_SCALE_DERIVATIVE'] = '1'
            subprocess.run(['cargo', 'test', '-p', 'sdocx', '--lib', '--features', 'pdf',
                            'captured_controls_shared_scene_exports'], cwd=tmp, env=env, check=True)
            png = tmp / 'raster.png'
            subprocess.run(['resvg', str(output / 'baseline.svg'), str(png)], check=True)
            with Image.open(png) as image:
                assert image.size == (256, 256)
                svg = metrics(reference, image.convert('RGB').tobytes())
            with pymupdf.open(output / 'baseline.pdf') as document:
                pix = document[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
                assert (pix.width, pix.height) == (256, 256)
                pdf = metrics(reference, pix.samples)
            assert {'svg': svg, 'pdf': pdf} == receipt['cases'][mode]
            print(mode, 'svg', svg, 'pdf', pdf, flush=True)
    print('Verified isolated native 2x Scene comparison')

if __name__ == '__main__':
    main()
