#!/usr/bin/env python3
"""Replay the isolated screen-space edge shift against retained native frames."""
import base64
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import zlib

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import metrics

RECEIPT = ROOT / 'conformance/montblanc-edge-shift-experiment.json'
SOURCE = Path('crates/sdocx/src/render/montblanc_candidate.rs')
PATCH = ROOT / 'tools/montblanc/public-candidate/edge_shift_experiment.patch'
CROP = ROOT / 'conformance/montblanc-scale-2x.rgba'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    receipt = json.loads(RECEIPT.read_text())
    captures = json.loads((ROOT / 'conformance/montblanc-public-candidate-host-comparison.json').read_text())
    scale_receipt = json.loads((ROOT / 'conformance/montblanc-scene-scale-2x-comparison.json').read_text())
    assert sha((ROOT / SOURCE).read_bytes()) == receipt['source_sha256']
    assert sha(PATCH.read_bytes()) == receipt['patch_sha256']
    assert sha(CROP.read_bytes()) == scale_receipt['native_crop_sha256']
    assert pymupdf.VersionBind == scale_receipt['renderers']['pymupdf']
    seen = set()
    with tempfile.TemporaryDirectory(prefix='montblanc-edge-shift-') as directory:
        checkout = Path(directory)
        archive = checkout / 'source.tar'
        subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), 'HEAD'], cwd=ROOT, check=True)
        with tarfile.open(archive) as source_archive:
            source_archive.extractall(checkout, filter='data')
        archive.unlink()
        assert sha((checkout / SOURCE).read_bytes()) == receipt['source_sha256']
        subprocess.run(['patch', '-p1', '-i', str(PATCH)], cwd=checkout, check=True)
        base_env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0',
                                 'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0'}
        for scale in (1, 2):
            for shift in (0, .25, .5, 1):
                with tempfile.TemporaryDirectory(prefix='montblanc-edge-output-') as output:
                    output = Path(output)
                    env = base_env | {'SDOCX_MONTBLANC_CAPTURE_DIR': str(output),
                                      'MB_SCALE': str(scale), 'MB_SHIFT': str(shift)}
                    subprocess.run(['cargo', 'test', '--offline', '-q', '-p', 'sdocx', '--lib', '--features',
                                    'pdf', 'captured_controls_shared_scene_exports'], cwd=checkout, env=env,
                                   stdout=subprocess.DEVNULL, check=True)
                    for name in (('baseline', 'pressure', 'size', 'far_endpoint') if scale == 1 else ('baseline',)):
                        native = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
                        if scale == 1:
                            raw = gzip.decompress((ROOT / f'tools/montblanc/captures/{name}.json.gz').read_bytes())
                            assert sha(raw) == captures['cases'][name]['capture_sha256']
                            capture = json.loads(raw)
                            frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
                            assert sha(frame) == capture['post_readback']['sha256']
                            image = Image.frombytes('RGBA', (256, 256), frame)
                        else:
                            image = Image.new('RGBA', (256, 256))
                            image.paste(Image.frombytes('RGBA', (47, 14), CROP.read_bytes()), (125, 124))
                        native.alpha_composite(image.transpose(Image.Transpose.FLIP_TOP_BOTTOM))
                        reference = native.convert('RGB').tobytes()
                        raster = output / 'raster.png'
                        subprocess.run(['resvg', str(output / f'{name}.svg'), str(raster)], check=True)
                        with Image.open(raster) as image:
                            assert image.size == (256, 256)
                            svg = metrics(reference, image.convert('RGB').tobytes())
                        with pymupdf.open(output / f'{name}.pdf') as document:
                            pix = document[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
                            assert (pix.width, pix.height, pix.n) == (256, 256, 3)
                            pdf = metrics(reference, pix.samples)
                        key = f'{scale}x_{name}_shift_{shift}'
                        if key in receipt['cases']:
                            assert {'svg': svg, 'pdf': pdf} == receipt['cases'][key], key
                            seen.add(key)
                        print(scale, shift, name, svg, pdf, flush=True)
    assert seen == set(receipt['cases'])
    print(f'Verified {len(seen)} receipt cases')


if __name__ == '__main__':
    main()
