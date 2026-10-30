#!/usr/bin/env python3
"""Check archived sources and their pinned historical capture paths."""
import gzip
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = Path(__file__).resolve().parent / 'sources'
AUDIT = Path('tmp/vector-parity-plan/pen-family-audit')
CONF = Path('tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance')
FILES = [
    ('v12_capture_candidate.py', AUDIT / 'pencil2-v14-geometry-v12/capture_candidate.py', '996ef6db3a376619e9e5d1d05b7d5e1a940726f3feb2ef6895275a705576ee3e'),
    ('v10_capture_candidate.py', AUDIT / 'pencil2-v14-geometry-v10/capture_candidate.py', '40c99bcca08c606634430649f6a6492d76824afe1623155834a7090df67d6796'),
    ('v10_baseline_capture.json.gz', AUDIT / 'pencil2-v14-geometry-v10/captures/channel_baseline/capture.json', '469b54b231de1b6a8672c36130fda2e2617da53cfe90b0e188442610e2cd5eea'),
    ('legacy-pencils-source.json', AUDIT / 'legacy-pencils-source.json', '58dafbda19adcc5d4a22670a6c7e25a545f924ecb533bb61bc13b8a4a6c2f779'),
    ('pencil2-v14-native-capture-plan.md', AUDIT / 'pencil2-v14-native-capture-plan.md', '0e3b9e2f70f7f6a234587250354ae98b4a0cde4e405c3f50be68f290f5c357bf'),
    ('pencil3_selector_probe_native.py', CONF / 'pencil3_selector_probe_native.py', '0212de3d9c6b3b5832587601c32cea087f6fbf537fe34b101985970a3107b907'),
    ('oblique_canvas_native.py', CONF / 'oblique_canvas_native.py', '1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2'),
    ('pencil_native.py', CONF / 'pencil_native.py', 'b4d88fb86da22c732e63466e6f1fc7c829aca9d0b2623abbcb3d0fc6f981795c'),
    ('../montblanc/extended-paint/conformance/fountain_native.py', CONF / 'fountain_native.py', '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'),
    ('../montblanc/extended-paint/conformance/elf_plt.py', CONF / 'elf_plt.py', '816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b'),
]

def digest(data):
    return hashlib.sha256(data).hexdigest()

def main():
    failures = []
    for source, destination, expected in FILES:
        path = (SOURCES / source) if not source.startswith('../') else ROOT / 'tools' / source[3:]
        data = path.read_bytes()
        if source.endswith('.gz'):
            data = gzip.decompress(data)
        if digest(data) != expected:
            failures.append(f'source hash mismatch: {path}')
            continue
        target = ROOT / destination
        if target.exists():
            if digest(target.read_bytes()) != expected:
                failures.append(f'existing target hash mismatch: {target}')
        else:
            failures.append(f'missing target: {target}')
    if failures:
        raise SystemExit('\n'.join(failures))
    print('Pinned source files match. Native/runtime pins still require the original V12 preflight.')

if __name__ == '__main__':
    main()
