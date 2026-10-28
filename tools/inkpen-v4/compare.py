"""Reproduce five V4 3x public exports and comparisons in a temporary checkout.

Requires a coordinated shared Cargo target, Pillow, PyMuPDF, pypdfium2, and the
sdocx-render-svg-047 helper. No source files in --repo or --source are edited.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile
import zlib

import pymupdf
import pypdfium2
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('--repo', type=Path, required=True, help='Git checkout used for archive baseline')
parser.add_argument('--source', type=Path, required=True, help='candidate source tree with fixture and 3x changes')
parser.add_argument('--svg-renderer', type=Path, required=True, help='sdocx-render-svg-047 executable')
parser.add_argument('--output', type=Path, required=True, help='JSON comparison receipt path')
args = parser.parse_args()
source = args.source.resolve()
repo = args.repo.resolve()
renderer = args.svg_renderer.resolve()
receipt = json.loads((source / 'conformance/inkpen-v4-3x-native.json').read_text())
frames = (source / 'conformance/inkpen-v4-3x-native.bin').read_bytes()
assert hashlib.sha256(frames).hexdigest() == receipt['frames_sha256']

probe = r'''
    #[test]
    fn v4_public_three_scale_exports() {
        let receipt: Value = serde_json::from_str(include_str!("../../../../conformance/inkpen-v4-3x-native.json")).unwrap();
        let dir = std::path::PathBuf::from(std::env::var_os("SDOCX_V4_EXPORT_DIR").unwrap());
        for case in receipt["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let mut stroke = native_caller_stroke(&case["input"]);
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = Some("4;".into());
            rendering.style.color_argb = Some(case["argb"].as_u64().unwrap() as u32);
            let page = crate::Page { uuid: "v4-3x".into(), width: 128, height: 64, content_bbox: BoundingBox::default(), background_color: None, template: None, background: Default::default(), objects: vec![stroke.into()] };
            let document = crate::Document { pages: vec![page], metadata: Default::default() };
            for scale in [1, 3] {
                let options = crate::RenderOptions { inkpen_profile: Some(crate::InkPenRenderProfile { screen_width: 1920, screen_height: 1080 }), inkpen_v4_output_scale: Some(scale), ..crate::RenderOptions::default() };
                let svg = crate::render_document_svg(&document, &options).remove(0).svg;
                let pdf = crate::render_document_pdf(&document, &options, &crate::PdfOptions::default()).unwrap();
                std::fs::write(dir.join(format!("{name}-{scale}.svg")), svg).unwrap();
                std::fs::write(dir.join(format!("{name}-{scale}.pdf")), pdf).unwrap();
            }
        }
    }
'''

with tempfile.TemporaryDirectory(prefix='sdocx-v4-3x-') as temp:
    work = Path(temp) / 'source'
    work.mkdir()
    archive = subprocess.check_output(['git', '-C', str(repo), 'archive', 'HEAD'])
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(work, filter='data')
    for path in ('crates/sdocx/src/render.rs', 'crates/sdocx/src/ink/inkpen.rs',
                 'conformance/inkpen-v4-3x-native.json', 'conformance/inkpen-v4-3x-native.bin'):
        target = work / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((source / path).read_bytes())
    ink = work / 'crates/sdocx/src/ink/inkpen.rs'
    text = ink.read_text()
    marker = '    #[test]\n    fn v4_output_scale_improves_svg_against_native_frames()'
    assert text.count(marker) == 1
    ink.write_text(text.replace(marker, probe + '\n' + marker))
    exports = Path(temp) / 'exports'
    exports.mkdir()
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0',
               CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0',
               SDOCX_V4_EXPORT_DIR=str(exports))
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf',
                    'v4_public_three_scale_exports', '--lib'], cwd=work, env=env, check=True)
    results = []
    for case in receipt['cases']:
        name = case['name']
        native = zlib.decompress(frames[case['offset']:case['offset'] + case['length']])
        assert len(native) == 384 * 192 * 4
        assert hashlib.sha256(native).hexdigest() == case['rgba_sha256']
        record = {'name': name, 'rgba_sha256': case['rgba_sha256'], 'stream_sha256': case['stream_sha256'], 'scales': {}}
        for scale in (1, 3):
            svg = (exports / f'{name}-{scale}.svg').read_text()
            head, body = svg.split('>', 1)
            body = body.rsplit('</svg>', 1)[0]
            body, count = re.subn(r'<rect fill="#fcfcfc"[^>]*/>', '', body)
            assert count == 1
            head = head.replace('height="64"', 'height="192"').replace('width="128"', 'width="384"')
            svg_path, png_path = Path(temp) / 'probe.svg', Path(temp) / 'probe.png'
            svg_path.write_text(head + '>' + body + '</svg>')
            subprocess.run([str(renderer), str(svg_path), str(png_path)], check=True)
            image = Image.open(png_path).convert('RGBA')
            assert image.size == (384, 192)
            svg_error = sum(abs(a-b) for a,b in zip(native[3::4], image.tobytes()[3::4]))
            pdf_path = exports / f'{name}-{scale}.pdf'
            doc = pymupdf.open(pdf_path)
            pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4, 4), alpha=False)
            assert (pix.width, pix.height, pix.n) == (384, 192, 3)
            pdfium = pypdfium2.PdfDocument(pdf_path)
            bitmap = pdfium[0].render(scale=4).to_pil().convert('RGB')
            assert bitmap.size == (384, 192)
            def rgb_error(rgb):
                return sum(abs(int(rgb[j+c])-round(native[i+c]+252*(255-native[i+3])/255))
                           for i,j in zip(range(0,len(native),4),range(0,len(rgb),3)) for c in range(3))
            record['scales'][str(scale)] = {'svg_alpha_error': svg_error,
                                            'mupdf_rgb_error': rgb_error(pix.samples),
                                            'pdfium_rgb_error': rgb_error(bitmap.tobytes())}
        results.append(record)
    args.output.write_text(json.dumps({'schema': 'sdocx-inkpen-v4-3x-public-comparison-v1',
        'scope': 'Synthetic host Mesa 3x; absolute alpha error and RGB error over #fcfcfc. Not device or saved-note parity.',
        'frames_sha256': receipt['frames_sha256'], 'cases': results}, indent=2) + '\n')
