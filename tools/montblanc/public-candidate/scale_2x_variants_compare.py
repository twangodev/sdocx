#!/usr/bin/env python3
"""Compare additional 2x native controls to isolated shared Scene exports."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

import pymupdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import metrics

NATIVE = ROOT / 'conformance/montblanc-native-scale-2x-variants.json'
BASE = ROOT / 'conformance/montblanc-scene-scale-2x-comparison.json'
PATCH = ROOT / 'tools/montblanc/public-candidate/scale_2x_experiment.patch'
OUT = ROOT / 'conformance/montblanc-scene-scale-2x-variants-comparison.json'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    native = json.loads(NATIVE.read_text())
    base = json.loads(BASE.read_text())
    source = ROOT / 'crates/sdocx/src/render/montblanc_candidate.rs'
    assert sha(source) == base['source_sha256'] and sha(PATCH) == base['patch_sha256']
    assert pymupdf.VersionBind == base['renderers']['pymupdf']
    references = {}
    for case, info in native['cases'].items():
        crop = ROOT / f'conformance/montblanc-scale-2x-{case}.rgba'
        assert sha(crop) == info['crop_sha256']
        l, t, r, b = info['bounds_half_open']
        frame = Image.new('RGBA', (256, 256))
        frame.paste(Image.frombytes('RGBA', (r-l, b-t), crop.read_bytes()), (l, t))
        assert hashlib.sha256(frame.tobytes()).hexdigest() == info['frame_sha256']
        background = Image.new('RGBA', (256, 256), (252, 252, 252, 255))
        background.alpha_composite(frame.transpose(Image.Transpose.FLIP_TOP_BOTTOM))
        references[case] = background.convert('RGB').tobytes()
    cases = {}
    with tempfile.TemporaryDirectory(prefix='sdocx-montblanc-scale-variants-') as directory:
        tmp = Path(directory)
        archive = tmp / 'source.tar'
        subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), 'HEAD'], cwd=ROOT, check=True)
        with tarfile.open(archive) as source_archive:
            source_archive.extractall(tmp, filter='data')
        archive.unlink()
        assert sha(tmp / source.relative_to(ROOT)) == base['source_sha256']
        subprocess.run(['patch', '-p1', '-i', str(PATCH)], cwd=tmp, check=True)
        for mode in ('fixed', 'aware'):
            output = tmp / mode
            output.mkdir()
            env = os.environ.copy()
            env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', SDOCX_MONTBLANC_CAPTURE_DIR=str(output))
            env.pop('MONTBLANC_SCALE_DERIVATIVE', None)
            if mode == 'aware':
                env['MONTBLANC_SCALE_DERIVATIVE'] = '1'
            subprocess.run(['cargo', 'test', '-p', 'sdocx', '--lib', '--features', 'pdf', 'captured_controls_shared_scene_exports'], cwd=tmp, env=env, check=True)
            cases[mode] = {}
            for case, reference in references.items():
                png = tmp / 'raster.png'
                subprocess.run(['resvg', str(output / f'{case}.svg'), str(png)], check=True)
                with Image.open(png) as image:
                    assert image.size == (256, 256)
                    svg = metrics(reference, image.convert('RGB').tobytes())
                with pymupdf.open(output / f'{case}.pdf') as document:
                    pix = document[0].get_pixmap(matrix=pymupdf.Matrix(4/3, 4/3), alpha=False)
                    assert (pix.width, pix.height) == (256, 256)
                    pdf = metrics(reference, pix.samples)
                cases[mode][case] = {'svg': svg, 'pdf': pdf}
    result = {'schema': 'montblanc-scene-scale-2x-variants-comparison-v1', 'source_sha256': sha(source), 'patch_sha256': sha(PATCH), 'native_receipt_sha256': sha(NATIVE), 'renderers': base['renderers'], 'cases': cases, 'boundary': 'Synthetic host Mesa controls compared to test-only reconstructed shared Scene exports; no device, saved-note, or production parity implied.', 'reproduce': '.venv/bin/python tools/montblanc/public-candidate/scale_2x_variants_compare.py'}
    if OUT.exists():
        assert json.loads(OUT.read_text()) == result
        print('Verified retained comparison')
    else:
        OUT.write_text(json.dumps(result, indent=2) + '\n')
        print('Recorded comparison')
    print(json.dumps(cases, indent=2))

if __name__ == '__main__':
    main()
