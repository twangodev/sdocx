#!/usr/bin/env python3
"""Verify public 2x SVG/PDF exports against retained synthetic host frames."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

import pymupdf
from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/montblanc'))
from public_fallback_compare import composite_native_frame, metrics


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    receipt = json.loads((ROOT / 'conformance/montblanc-public-optin-2x.json').read_text())
    for path, digest in receipt['source_sha256'].items():
        assert sha((ROOT / path).read_bytes()) == digest, f'source changed: {path}'
    native_path = ROOT / 'conformance/montblanc-native-scale-2x-variants.json'
    frame_receipt_path = ROOT / 'conformance/montblanc-native-scale-2x.json'
    assert sha(frame_receipt_path.read_bytes()) == receipt['native_baseline_frame_receipt_sha256']
    baseline_path = ROOT / 'conformance/montblanc-scene-scale-2x-comparison.json'
    assert sha(native_path.read_bytes()) == receipt['native_variants_receipt_sha256']
    assert sha(baseline_path.read_bytes()) == receipt['native_baseline_receipt_sha256']
    assert sha((ROOT / 'tools/montblanc/public_fallback_compare.py').read_bytes()) == receipt['compositor_sha256']
    assert pymupdf.VersionBind == receipt['renderers']['pymupdf']
    assert pillow_version == receipt['renderers']['pillow']
    assert subprocess.check_output(['resvg', '--version'], text=True).strip() == receipt['renderers']['resvg']
    native = json.loads(native_path.read_text())
    refs = {}
    for case, info in native['cases'].items():
        raw = (ROOT / f'conformance/montblanc-scale-2x-{case}.rgba').read_bytes()
        assert sha(raw) == info['crop_sha256']
        l, t, r, b = info['bounds_half_open']
        frame = Image.new('RGBA', (256, 256))
        frame.paste(Image.frombytes('RGBA', (r-l, b-t), raw), (l, t))
        assert sha(frame.tobytes()) == info['frame_sha256']
        refs[case] = composite_native_frame(frame.tobytes(), (256, 256)).tobytes()
    raw = (ROOT / 'conformance/montblanc-scale-2x.rgba').read_bytes()
    assert sha(raw) == json.loads(baseline_path.read_text())['native_crop_sha256']
    frame = Image.new('RGBA', (256, 256))
    frame.paste(Image.frombytes('RGBA', (47, 14), raw), (125, 124))
    assert sha(frame.tobytes()) == json.loads(frame_receipt_path.read_text())['scale_sha256']
    refs['baseline'] = composite_native_frame(frame.tobytes(), (256, 256)).tobytes()
    with tempfile.TemporaryDirectory(prefix='montblanc-public-2x-') as directory:
        out = Path(directory)
        env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0',
                            'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0'}
        subprocess.run(['cargo', 'run', '--locked', '--offline', '--quiet', '--manifest-path',
                        str(ROOT / 'tools/montblanc/public-optin-2x/Cargo.toml'), '--', str(out)], cwd=ROOT, env=env, check=True)
        cases = {}
        for case, reference in refs.items():
            cases[case] = {}
            for mode in ('fallback', 'candidate', 'aware'):
                png = out / 'raster.png'
                subprocess.run(['resvg', '--width', '256', str(out / f'{case}-{mode}.svg'), str(png)], check=True)
                with Image.open(png) as image:
                    assert image.size == (256, 256)
                    svg = metrics(reference, image.convert('RGB').tobytes())
                with pymupdf.open(out / f'{case}-{mode}.pdf') as document:
                    pix = document[0].get_pixmap(matrix=pymupdf.Matrix(8/3, 8/3), alpha=False)
                    assert (pix.width, pix.height) == (256, 256)
                    pdf = metrics(reference, pix.samples)
                cases[case][mode] = {'svg': svg, 'pdf': pdf}
    assert cases == receipt['cases']
    print('Verified public 2x SVG and PDF comparisons')


if __name__ == '__main__':
    if len(sys.argv) == 2 and sys.argv[1] == '--archived':
        main()
    elif len(sys.argv) == 1:
        with tempfile.TemporaryDirectory(prefix='montblanc-public-source-') as directory:
            snapshot = Path(directory)
            archive = snapshot / 'source.tar'
            subprocess.run(['git', 'archive', '--format=tar', '--output', str(archive), 'HEAD'], cwd=ROOT, check=True)
            with tarfile.open(archive) as source:
                source.extractall(snapshot, filter='data')
            archive.unlink()
            subprocess.run([sys.executable, str(snapshot / 'tools/montblanc/public-optin-2x/compare.py'), '--archived'], cwd=snapshot, check=True)
    else:
        raise SystemExit('usage: compare.py')
