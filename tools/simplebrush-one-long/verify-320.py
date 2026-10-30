import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
receipt = json.loads((here / '320-receipt.json').read_text())
for path, expected in receipt['replay_source_sha256'].items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, path
assert subprocess.check_output(['rustc', '--version'], text=True).strip() == receipt['rustc']
vector = zlib.decompress((here / '320.vector.zlib').read_bytes())
frame = zlib.decompress((here / '320.rgba.zlib').read_bytes())
assert hashlib.sha256(vector).hexdigest() == receipt['geometry_sha256']
assert hashlib.sha256(frame).hexdigest() == receipt['frame_sha256']
assert vector == (root / 'crates/sdocx/src/ink/simplebrush_assets/320.bin').read_bytes()
with tempfile.TemporaryDirectory(prefix='simplebrush-320-') as tmp:
    output = Path(tmp)
    captures = output / 'captures'
    captures.mkdir()
    (captures / f"simplebrush-native-draw-rgba-{receipt['frame_sha256']}.bin").write_bytes(frame)
    (output / 'native.json').write_text(json.dumps({
        'drawing_vector': {'vector_bytes': vector.hex()},
        'native_draw_target_readback': {'sha256': receipt['frame_sha256']},
    }))
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0',
               CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2',
               SDOCX_POINTS_COUNT='320', PROBE_SIZE='8', VARIANT='reverse',
               SDOCX_V62_NATIVE=str(output / 'native.json'), SDOCX_SIMPLEBRUSH_TWO_EXPORT=tmp)
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', '--lib',
                    'two_pixel_native_export_fixture'], cwd=root, env=env, check=True)
    for name, expected in receipt['exports_sha256'].items():
        assert hashlib.sha256((output / name).read_bytes()).hexdigest() == expected, name
    for name, filename in (('generic_svg', 'generic.svg'), ('profile_svg', 'one.svg'),
                           ('generic_pdf', 'generic.pdf'), ('profile_pdf', 'one.pdf')):
        assert (output / filename).stat().st_size == receipt['export_bytes'][name], name
    comparison = subprocess.check_output([os.sys.executable, str(here / 'compare.py'), tmp],
                                         cwd=root, text=True)
    print(comparison, end='')
    rows = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1])
            for line in comparison.splitlines()}
    for mode, label in (('generic', 'generic'), ('one', 'profile')):
        assert rows[mode]['svg_alpha_error'] == receipt['svg_alpha_error'][label]
        assert rows[mode]['svg_support'] == receipt['svg_support_intersection_union'][label]
        assert rows[mode]['matched'] == receipt['direct_pdf_mupdf_matched_paper_rgb_error'][label]
    comparison = subprocess.check_output([os.sys.executable, str(here / 'pdfium.py'), tmp],
                                         cwd=root, text=True)
    print(comparison, end='')
    rows = {mode: int(error) for mode, error in (line.split() for line in comparison.splitlines())}
    for mode, label in (('generic', 'generic'), ('one', 'profile')):
        assert rows[mode] == receipt['direct_pdf_pdfium_rgb_error'][label]
