#!/usr/bin/env python3
import json, os, subprocess, sys, tempfile
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parents[2]
native = json.loads((here / 'native.json').read_text())
with tempfile.TemporaryDirectory(prefix='mb-v276-verify-') as temp:
    temp = Path(temp)
    capture = temp / 'capture'
    capture.mkdir()
    key = 'zigzag-2.4-1'
    (capture / f'{key}.json').write_bytes((here / 'capture-1.json').read_bytes())
    (capture / f'{key}.input.json').write_text(json.dumps(native['input'], sort_keys=True) + '\n')
    (capture / f'{key}.stream.json').write_text(json.dumps(native['stream']))
    env = os.environ | {'MB_CLEAN_ROOT': str(root), 'MB_CAPTURE_WORK': str(capture),
                        'MB_EXPORT_WORK': str(temp / 'export'), 'CARGO_BUILD_JOBS': '2'}
    subprocess.run([sys.executable, str(here / 'export.py'), 'rebuild-verify'], cwd=root, env=env, check=True)
