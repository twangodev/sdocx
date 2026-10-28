"""Reproduce paired InkPen V1 3x SVG and PDF comparisons in an isolated checkout.

Run from an isolated checkout with the 3x patch and candidate receipt installed:
  python3 tools/inkpen-v1/compare_3x.py
Requires cargo, PyMuPDF and pypdfium2. The temporary source instrumentation is
restored even on failure; never run concurrently with edits to inkpen.rs.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import zlib

import pymupdf
import pypdfium2

root = Path(__file__).resolve().parents[2]
if not root.is_relative_to(Path('/tmp')):
    raise SystemExit('Run only in an isolated checkout under /tmp')
if subprocess.check_output(['git', 'status', '--porcelain', '--', 'crates/sdocx/src/ink/inkpen.rs'], cwd=root).strip():
    raise SystemExit('Refusing to instrument a modified inkpen.rs')
source = root / 'crates/sdocx/src/ink/inkpen.rs'
receipt = json.loads((root / 'conformance/inkpen-v1-output-scale-native.json').read_text())
frames = (root / 'conformance/inkpen-v1-output-scale-native.bin').read_bytes()
assert hashlib.sha256(frames).hexdigest() == receipt['frames_sha256']
original = source.read_text()
anchor = '                let svg = crate::render_document_svg(&document, &options)\n'
test_start = original.index('    fn v1_output_scale_improves_svg_against_native_frames()')
test_end = original.index('    fn v4_vector_candidate_improves_on_generic_against_host_frames()', test_start)
section = original[test_start:test_end]
assert section.count(anchor) == 1
instrument = '''                let pdf = crate::render_document_pdf(&document, &options, &crate::PdfOptions::default()).unwrap();
                std::fs::write(format!("/tmp/inkpen-probe-{shape}-{}.pdf", output_scale.unwrap_or(1)), pdf).unwrap();
'''
env = os.environ.copy()
env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0')
instrumented = original[:test_start] + section.replace(anchor, instrument + anchor) + original[test_end:]
try:
    source.write_text(instrumented)
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf', 'v1_output_scale_improves_svg_against_native_frames', '--lib'], cwd=root, env=env, check=True)
finally:
    if source.read_text() != instrumented:
        raise RuntimeError('inkpen.rs changed during comparison; refusing to overwrite it')
    source.write_text(original)

for case in receipt['cases']:
    if case['scale'] != 3:
        continue
    shape = case['shape']
    offset, length = case['zlib_offset'], case['zlib_length']
    rgba = zlib.decompress(frames[offset:offset + length])
    assert hashlib.sha256(rgba).hexdigest() == case['rgba_sha256']
    native = bytes(round(252 * (255 - alpha) / 255) for alpha in rgba[3::4])
    results = {'mupdf': [], 'pdfium': []}
    for scale in (1, 3):
        path = f'/tmp/inkpen-probe-{shape}-{scale}.pdf'
        doc = pymupdf.open(path)
        pix = doc[0].get_pixmap(matrix=pymupdf.Matrix(4, 4), alpha=False, colorspace=pymupdf.csRGB)
        mu = pix.samples[::3]
        bitmap = pypdfium2.PdfDocument(path)[0].render(scale=4, rev_byteorder=True)
        pd = bytes(bitmap.buffer)[::bitmap.n_channels]
        for engine, pixels in (('mupdf', mu), ('pdfium', pd)):
            assert len(pixels) == len(native)
            results[engine].append(sum(abs(a - b) for a, b in zip(pixels, native)))
    for engine, errors in results.items():
        assert errors == receipt['public_pdf_rgb_error'][shape]['3'][engine], (shape, engine, errors)
        assert errors[1] < errors[0]
    print(shape, results)
