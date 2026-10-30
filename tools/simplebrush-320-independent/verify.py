import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import zlib

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
receipt = json.loads((here / 'receipt.json').read_text())
assert subprocess.check_output(['rustc', '--version'], text=True).strip() == receipt['rustc']
vector = zlib.decompress((here / 'vector.zlib').read_bytes())
frame = zlib.decompress((here / 'rgba.zlib').read_bytes())
assert hashlib.sha256(vector).hexdigest() == receipt['native_geometry_sha256']
assert hashlib.sha256(frame).hexdigest() == receipt['native_frame_sha256']
with tempfile.TemporaryDirectory(prefix='simplebrush-independent-replay-') as tmp:
    base = Path(tmp)
    source = base / 'source'
    source.mkdir()
    archive = base / 'source.tar'
    with archive.open('wb') as out:
        subprocess.run(['git', 'archive', receipt['source_revision']], cwd=root, stdout=out, check=True)
    with tarfile.open(archive) as tar:
        tar.extractall(source, filter='data')
    for name, expected in receipt['replay_source_sha256'].items():
        path = here / 'capture.py' if name == 'tools/simplebrush-320-independent/capture.py' else source / name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, name
    output = base / 'output'
    captures = output / 'captures'
    captures.mkdir(parents=True)
    (captures / f"simplebrush-native-draw-rgba-{receipt['native_frame_sha256']}.bin").write_bytes(frame)
    (output / 'native.json').write_text(json.dumps({'drawing_vector': {'vector_bytes': vector.hex()}, 'native_draw_target_readback': {'sha256': receipt['native_frame_sha256']}}))
    env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_POINTS_COUNT='320', PROBE_SIZE='8', VARIANT='alternate', SDOCX_V62_NATIVE=str(output / 'native.json'), SDOCX_SIMPLEBRUSH_TWO_EXPORT=str(output))
    for name in ('SDOCX_REVERSE', 'SDOCX_ALT_PRESSURE', 'SDOCX_TOOL', 'SDOCX_SIMPLEBRUSH_OVERLAP'):
        env.pop(name, None)
    subprocess.run(['cargo', 'test', '-p', 'sdocx', '--features', 'pdf,serde', '--lib', 'two_pixel_native_export_fixture'], cwd=source, env=env, check=True)
    for mode in ('generic', 'one'):
        for ext in ('svg', 'pdf'):
            name = f'{mode}.{ext}'
            assert (output / name).stat().st_size == receipt['export_bytes'][name]
            assert hashlib.sha256((output / name).read_bytes()).hexdigest() == receipt['export_sha256'][name]
    result = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-one-long/compare.py'), str(output)], text=True)
    print(result, end='')
    rows = {line.split(' ', 1)[0]: ast.literal_eval(line.split(' ', 1)[1]) for line in result.splitlines()}
    assert rows['two'] == rows['generic']
    for mode, label in (('generic', 'generic'), ('one', 'profile')):
        assert rows[mode]['svg_alpha_error'] == receipt['svg_alpha_error'][label]
        assert rows[mode]['svg_support'] == receipt['svg_support_intersection_union'][label]
        assert rows[mode]['matched'] == receipt['direct_pdf_mupdf_matched_paper_rgb_error'][label]
    result = subprocess.check_output([os.sys.executable, str(source / 'tools/simplebrush-one-long/pdfium.py'), str(output)], text=True)
    print(result, end='')
    rows = {mode: int(error) for mode, error in (line.split() for line in result.splitlines())}
    assert rows['two'] == rows['generic']
    for mode, label in (('generic', 'generic'), ('one', 'profile')):
        assert rows[mode] == receipt['direct_pdf_pdfium_rgb_error'][label]
