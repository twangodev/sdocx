#!/usr/bin/env python3
import json
import os
import subprocess
import tempfile
from pathlib import Path

here = Path(__file__).resolve().parent
repo = here.parents[2]
revision = 'f7666dd0004173f49c93e7e5115c358dfe52cff2'
env = os.environ | {'CARGO_TARGET_DIR': '/tmp/sdocx-agent-target', 'CARGO_INCREMENTAL': '0',
                    'CARGO_PROFILE_DEV_DEBUG': '0', 'CARGO_PROFILE_TEST_DEBUG': '0', 'CARGO_BUILD_JOBS': '2'}
with tempfile.TemporaryDirectory(prefix='mb-v275-replay-') as td:
    td = Path(td)
    clean = td / 'clean'
    clean.mkdir()
    archive = subprocess.check_output(['git', '-C', str(repo), 'archive', revision])
    subprocess.run(['tar', '-x', '-C', str(clean)], input=archive, check=True)
    test = td / 'test'
    subprocess.run(['cp', '-a', str(clean), str(test)], check=True)
    subprocess.run(['patch', '-p1', '-i', str(here / 'width.patch')], cwd=test, check=True)
    patch = (clean / 'tools/montblanc/redraw-window/irregular-candidate.patch').read_text()
    test_code = patch[patch.index('+#[test]'):]
    assert all(line.startswith('+') for line in test_code.splitlines())
    p = test / 'crates/sdocx/src/ink/montblanc_many.rs'
    p.write_text(p.read_text() + '\n' + '\n'.join(line[1:] for line in test_code.splitlines()) + '\n')
    env['MONTBLANC_WINDOW_NATIVE'] = str(here / 'native.json')
    subprocess.run(['cargo', 'test', '--offline', '-p', 'sdocx', '--features', 'render', '--lib', 'irregular_window_candidate', '--', '--nocapture'], cwd=test, env=env, check=True)
    work = td / 'exports'
    work.mkdir()
    native = json.loads((here / 'native.json').read_text())
    for scale in (1, 2):
        key = f'zigzag-2.4-{scale}'
        (work / f'{key}.json').write_bytes((here / f'capture-{scale}.json').read_bytes())
        (work / f'{key}.input.json').write_text(json.dumps(native['input'], sort_keys=True) + '\n')
        (work / f'{key}.stream.json').write_text(json.dumps(native['stream']))
    subprocess.run([os.environ.get('PYTHON', 'python3'), str(here / 'export.py'), 'rebuild-verify'], cwd=repo,
                   env=env | {'MB_CLEAN_ROOT': str(clean), 'MB_EXPORT_WORK': str(work)}, check=True)
