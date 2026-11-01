"""Replay the two pinned source revisions and check the retained metrics."""
import json
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

here = Path(__file__).resolve().parent
root = here.parents[2]
receipt = json.loads((here / 'receipt.json').read_text())
for path, digest in receipt['sha256'].items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == digest, path
if not __debug__:
    raise RuntimeError('run without Python optimization')
env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0',
    'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0', 'CARGO_BUILD_JOBS': '2'}
with tempfile.TemporaryDirectory(prefix='sdocx-fountain-expanded-') as tmp:
    work = Path(tmp)
    for version, revision in [('head', '9c73bed1'), ('base', '9c73bed1^')]:
        checkout = work / version
        checkout.mkdir()
        archive = work / f'{version}.tar'
        with archive.open('wb') as stream:
            subprocess.run(['git', 'archive', revision], cwd=root, stdout=stream, check=True)
        with tarfile.open(archive) as stream:
            stream.extractall(checkout, filter='data')
        examples = checkout / 'crates/sdocx/examples'
        shutil.copy(here / 'independent_probe.rs', examples / 'fountain_independent_vector_probe.rs')
        shutil.copy(here / 'colors_probe.rs', examples / 'fountain_v14_colors_probe.rs')
        shutil.copy(root / 'conformance/fountain-v14-saved-scene-inputs.json', checkout / 'conformance')
        # Archives preserve commit times; invalidate cross-checkout Cargo artifacts.
        for path in (checkout / 'crates/sdocx/src').rglob('*'):
            if path.is_file():
                os.utime(path, None)
        for name in ['independent', 'colors']:
            probe = 'fountain_independent_vector_probe' if name == 'independent' else 'fountain_v14_colors_probe'
            subprocess.run(['cargo', 'run', '-q', '-p', 'sdocx', '--features', 'pdf,serde',
                '--example', probe, '--', str(work / name / version)], cwd=checkout, env=env, check=True)
    for name in ['independent', 'colors']:
        output = subprocess.check_output([sys.executable, str(here / f'{name}_compare.py'), str(work / name)], cwd=root, env=env, text=True)
        assert json.loads(output) == receipt['results'][name], name
print('Expanded Fountain native-frame comparisons reproduced.')
