#!/usr/bin/env python3
import hashlib
import json
import os
import subprocess
import tarfile
import tempfile
import zlib
from pathlib import Path

root = Path(__file__).resolve().parents[2]
here = Path(__file__).resolve().parent
receipt = json.loads((here / 'receipt.json').read_text())
def sha(data):
    return hashlib.sha256(data).hexdigest()
for name in ('capture.py', 'compare.py', 'export-test.rs', 'replay.py'):
    assert sha((here / name).read_bytes()) == receipt['scripts_sha256'][name]
for name in ('frame', 'stream'):
    raw = zlib.decompress((here / f'{name}.zlib').read_bytes())
    assert sha(raw) == receipt[f'{name}_sha256']
    if name == 'stream':
        assert len(raw) == receipt['stream_records'] * 44
        stream = raw
with tempfile.TemporaryDirectory(prefix='brushpen-clean-head-') as tmp:
    archive = subprocess.check_output(['git', '-C', str(root), 'archive', receipt['source_commit']])
    import io
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(tmp, filter='data')
    checkout = Path(tmp)
    stream_path = checkout / 'variant-stream.bin'
    stream_path.write_bytes(stream)
    export_dir = checkout / 'exports'
    export_dir.mkdir()
    for part in ('frame', 'payload', 'table'):
        data = (checkout / f'conformance/brushpen-serialized/v10-{part}.bin').read_bytes()
        assert sha(data) == receipt[f'source_{part}_sha256']
    source = checkout / 'crates/sdocx/src/ink/brushpen/paint.rs'
    assert sha(source.read_bytes()) == receipt['clean_paint_sha256']
    text = source.read_text()
    pos = text.index('    fn independent_serialized_brushpen_native()')
    pos = text.rfind('    #[cfg(feature = "render")]', 0, pos)
    assert pos >= 0
    text = text[:pos] + '    #[cfg(feature = "pdf")]\n    #[test]\n' + (here / 'export-test.rs').read_text() + text[pos:]
    source.write_text(text)
    env = os.environ.copy()
    env.update(CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', BRUSHPEN_VARIANT_STREAM=str(stream_path), BRUSHPEN_VARIANT_EXPORT=str(export_dir))
    result = subprocess.run(['cargo', 'test', '--manifest-path', str(checkout / 'Cargo.toml'), '-p', 'sdocx', '--features', 'pdf,serde', '--lib', 'variant_serialized_brushpen_native', '--offline'], check=True, env=env, text=True, capture_output=True)
    print(result.stdout)
    assert 'running 1 test\n' in result.stdout and '1 passed; 0 failed' in result.stdout
    subprocess.run([os.environ.get('PYTHON', 'python3'), str(here / 'compare.py'), str(export_dir)], check=True)
