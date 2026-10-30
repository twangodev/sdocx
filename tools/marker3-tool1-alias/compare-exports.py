#!/usr/bin/env python3
"""Compare identical-input exports from before and after the alias admission."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zlib

parser = argparse.ArgumentParser()
parser.add_argument('exports', type=Path)
parser.add_argument('receipt', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
source = Path(__file__).resolve().parent
native = json.loads((source / 'native.json').read_text())
results = {}
with tempfile.TemporaryDirectory(prefix='marker3-alias-compare-') as temporary:
    work = Path(temporary)
    for case in ('fractional', 'crossing'):
        frame = zlib.decompress((source / f'{case}.rgba.zlib').read_bytes())
        assert hashlib.sha256(frame).hexdigest() == native['cases'][case]['rgba_sha256']
        frame_path = work / f'{case}.rgba'
        frame_path.write_bytes(frame)
        for mode in ('baseline', 'profile'):
            directory = work / f'{case}-{mode}'
            directory.mkdir()
            hashes = {}
            for revision, label in (('before', 'baseline'), ('after', 'profile')):
                for extension in ('svg', 'pdf'):
                    path = args.exports / case / revision / f'{mode}.{extension}'
                    hashes[f'{revision}.{extension}'] = hashlib.sha256(path.read_bytes()).hexdigest()
                    shutil.copyfile(path, directory / f'{label}.{extension}')
            receipt = directory / 'comparison.json'
            subprocess.run([
                sys.executable, str(root / 'tools/marker3-tool1-paint/compare-scale2.py'),
                str(frame_path), str(directory), str(receipt),
            ], check=True)
            result = json.loads(receipt.read_text())
            result['export_sha256'] = hashes
            result['options'] = 'default' if mode == 'baseline' else 'explicit 2x'
            results[f'{case}-{mode}'] = result
args.receipt.write_text(json.dumps({
    'scope': 'Same-input before/after StraightMarker tool-1 empty-setting exports; synthetic host controls, not saved-note or device parity.',
    'comparisons': results,
}, indent=2, sort_keys=True) + '\n')
