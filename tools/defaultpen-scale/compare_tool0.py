#!/usr/bin/env python3
"""Compare retained tool-0/1 host frames with public SVG and direct PDF exports.

Requires cargo, rsync, PyMuPDF and pypdfium2. Uses an isolated temporary source
copy, the shared agent Cargo target, and deletes generated files on exit.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

import pymupdf
import pypdfium2 as pdfium

root = Path(__file__).resolve().parents[2]
fixture_path = root / 'conformance/defaultpen-tool0-scale-native.json'
source_path = root / 'crates/sdocx/src/render.rs'
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

with tempfile.TemporaryDirectory(prefix='sdocx-defaultpen-tool0-') as temp:
    copy = Path(temp) / 'source'
    copy.mkdir()
    for entry in ('Cargo.toml', 'Cargo.lock', 'crates', 'conformance'):
        subprocess.run(['rsync', '-a', str(root / entry), str(copy) + '/'], check=True)
    for entry in ('tools/brushpen-transform/setting11-receipt.json', 'tools/brushpen-transform/curveoff_capture.py', 'tools/montblanc/narrow-probe/diverse-v8-receipt.json', 'tools/pen-capture/airbrush_v1_matrix_receipt.json', 'tools/inkpen2-v1-tool3/receipt.json', 'tools/inkpen2-v1-tool3/extended-receipt.json'):
        destination = copy / entry
        destination.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(['cp', str(root / entry), str(destination)], check=True)
    fixture = json.loads(fixture_path.read_text())
    original = json.loads((copy / 'conformance/defaultpen-scale-native.json').read_text())
    template = next(iter(original['cases'].values()))
    cases = {}
    for name, case in fixture['cases'].items():
        case = dict(case)
        case.update(stream_hex=case['native_stream_hex'], stream_sha256=case['native_stream_sha256'], width=case['framebuffer']['width'], height=case['framebuffer']['height'], paint_argb='ff336699', native_uniform_color_f32_bits=template['native_uniform_color_f32_bits'], projection=template['projection'])
        cases[name.removesuffix('x')] = case
    original['cases'] = cases
    (copy / 'conformance/defaultpen-scale-native.json').write_text(json.dumps(original))
    path = copy / 'crates/sdocx/src/render.rs'
    source = path.read_text()
    def replace(old, new):
        global source
        if source.count(old) != 1:
            raise RuntimeError(f'expected one occurrence: {old}')
        source = source.replace(old, new)
    replace('assert!((1..=4).contains(&tool));', 'assert!((0..=4).contains(&tool));')
    replace('assert_eq!(checked, 29);', 'assert_eq!(checked, 6);')
    replace('let after = measure(&candidate.svg);', 'let after = measure(&candidate.svg);\n            println!("SVG {name} {before} {after}");')
    replace('std::fs::write(dir.join(format!("{name}-baseline.pdf")), baseline_pdf).unwrap();', 'std::fs::write(dir.join(format!("{name}-baseline.svg")), &baseline.svg).unwrap();\n                std::fs::write(dir.join(format!("{name}-profile.svg")), &candidate.svg).unwrap();\n                std::fs::write(dir.join(format!("{name}-baseline.pdf")), baseline_pdf).unwrap();')
    start = source.index('    fn defaultpen_scale_profile_matches_native_controls_and_preserves_replay()')
    end = source.index('    #[test]', start)
    adapted_test_sha256 = hashlib.sha256(source[start:end].encode()).hexdigest()
    path.write_text(source)
    output = Path(temp) / 'exports'
    output.mkdir()
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_DEFAULTPEN_SCALE_EXPORT=str(output))
    run = subprocess.run(['cargo', 'test', '--locked', '-p', 'sdocx', '--features', 'pdf,serde', '--lib', 'defaultpen_scale_profile_matches_native_controls_and_preserves_replay', '--', '--nocapture'], cwd=copy, env=env, check=True, text=True, stdout=subprocess.PIPE)
    svg = {}
    for line in run.stdout.splitlines():
        if line.startswith('SVG '):
            _, name, before, after = line.split()
            svg[name] = [int(before), int(after)]
    assert set(svg) == set(cases)
    results = {}
    for name, case in cases.items():
        scale = int(name.rsplit('-', 1)[1])
        raw = zlib.decompress(base64.b64decode(case['rgba_zlib_base64'], validate=True))
        assert hashlib.sha256(raw).hexdigest() == case['rgba_sha256']
        width, height = 64 * scale, 32 * scale
        expected = bytearray()
        for y in range(height):
            for x in range(width):
                i = (y * case['width'] + x) * 4
                alpha = raw[i + 3]
                expected.extend((raw[i + c] * alpha + 252 * (255 - alpha) + 127) // 255 for c in range(3))
        readers = {'mupdf': [], 'pdfium': []}
        hashes = {}
        for variant in ('baseline', 'profile'):
            pdf = output / f'{name}-{variant}.pdf'
            hashes[variant] = {'pdf': digest(pdf), 'svg': digest(output / f'{name}-{variant}.svg')}
            page = pymupdf.open(pdf)[0]
            pix = page.get_pixmap(matrix=pymupdf.Matrix(4 / 3 * scale, 4 / 3 * scale), alpha=False)
            assert (pix.width, pix.height, pix.n) == (width, height, 3)
            readers['mupdf'].append(sum(abs(a-b) for a,b in zip(pix.samples, expected)))
            image = pdfium.PdfDocument(pdf)[0].render(scale=4 / 3 * scale).to_pil().convert('RGB')
            assert image.size == (width, height)
            readers['pdfium'].append(sum(abs(a-b) for a,b in zip(image.tobytes(), expected)))
        assert all(v[1] < v[0] for v in [svg[name], *readers.values()])
        results[name] = {'svg_rgb_l1': svg[name], 'pdf_rgb_l1': readers, 'output_sha256': hashes, 'native_rgba_sha256': case['rgba_sha256'], 'native_stream_sha256': case['native_stream_sha256']}
    print(json.dumps({'scope': 'Synthetic native Model/Drawing with explicit host vector bridge and Mesa RGBA8, compared over #fcfcfc paper; no serialized-note or device parity.', 'inputs': {'fixture_sha256': digest(fixture_path), 'render_rs_sha256': digest(source_path), 'base_fixture_sha256': digest(root / 'conformance/defaultpen-scale-native.json'), 'script_sha256': digest(Path(__file__)), 'cargo_lock_sha256': digest(root / 'Cargo.lock'), 'adapted_test_sha256': adapted_test_sha256}, 'readers': {'mupdf': pymupdf.VersionBind, 'pypdfium2': str(pdfium.PYPDFIUM_INFO), 'pdfium': str(pdfium.PDFIUM_INFO)}, 'results': results}, indent=2))
