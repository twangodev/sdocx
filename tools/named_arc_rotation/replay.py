import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(sys.argv[1]).resolve()
lib = Path(sys.argv[2]).resolve()
unicorn = Path(sys.argv[3]).resolve()
package = Path(__file__).resolve().parent
assert hashlib.sha256((unicorn / 'libunicorn.so.2').read_bytes()).hexdigest() == 'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c'
expected = json.loads((package / 'sources.json').read_text())
receipts = json.loads((package / 'receipts.json').read_text())
with tempfile.TemporaryDirectory(prefix='sdocx-arc-rotation-') as directory:
    tmp = Path(directory)
    (tmp / 'conformance').mkdir()
    shutil.copy(root / 'conformance/native_table.rs', tmp / 'conformance/native_table.rs')
    shutil.copytree(root / 'conformance/native_table', tmp / 'conformance/native_table', symlinks=True)
    shutil.copytree(root / 'tools', tmp / 'tools', symlinks=True, copy_function=os.symlink)
    (tmp / 'tools/named_arc_drawing/capture.rs').unlink()
    shutil.copy(root / 'tools/named_arc_cap_join/capture.rs', tmp / 'tools/named_arc_drawing/capture.rs')
    subprocess.run(['patch', '-s', str(tmp / 'tools/named_arc_drawing/capture.rs'), str(package / 'capture.patch')], check=True)
    (tmp / 'libunicorn.so').symlink_to(unicorn / 'libunicorn.so.2')
    subprocess.run(['rustc', '--edition=2024', '--emit=dep-info', 'conformance/native_table.rs', '-o', 'sources.d'], cwd=tmp, check=True)
    names = (tmp / 'sources.d').read_text().splitlines()[0].split(': ', 1)[1].split()
    assert {os.path.normpath(n) for n in names} == set(expected)
    for name, digest in expected.items():
        assert hashlib.sha256((tmp / name).read_bytes()).hexdigest() == digest, name
    subprocess.run(['rustc', '--edition=2024', 'conformance/native_table.rs', '-L', str(tmp), '-o', str(tmp / 'probe')], cwd=tmp, check=True)
    for case in ['0', '37', '-37', '90', 'alt']:
        for fill in [0, 85, 165, 255, 0]:
            env = os.environ | {'LD_LIBRARY_PATH': str(unicorn), 'ARC_OUTPUT': str(tmp / 'frame'), 'ARC_SECOND': '1', 'ARC_FILL': str(fill), 'ANGLE': '37' if case == 'alt' else case}
            if case == 'alt':
                env |= {'ARC_MOVED': '20', 'ARC_MOVED_Y': '30', 'ARC_END_X': '40', 'ARC_END_Y': '100'}
            run = subprocess.run([str(tmp / 'probe'), str(lib / 'libSPenModel.so'), '--named-arc-drawing', str(lib / 'libSPenBase.so'), str(lib / 'libSPenSkia.so')], env=env, capture_output=True, check=True)
            prefixes = ('FRAMED_SIZE', 'CONTROL ', 'SOURCE ', 'ACTIVE ', 'canvas 0 points', 'canvas 1 points', 'LEDGER ', 'HOST_CALLS ', 'reached plt ')
            receipt = '\n'.join(line for line in run.stderr.decode().splitlines() if line.startswith(prefixes)) + '\n'
            assert hashlib.sha256(receipt.encode()).hexdigest() == receipts[case]['receipt'], (case, fill)
            assert hashlib.sha256((tmp / 'frame').read_bytes()).hexdigest() == receipts[case]['frame'], (case, fill)
            if case == '37':
                assert (tmp / 'frame').read_bytes()[:604] == (root / 'crates/sdocx/tests/fixtures/native-arc-rotated-frame.bin').read_bytes()
print('Rotated Arc native roundtrip and Drawing receipts match across allocation fills.')
