#!/usr/bin/env python3
"""Run the pinned Drawing adapter in a disposable historical-path tree."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import importlib.util

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--native-dir', type=Path, required=True, help='directory containing pinned ARM64 libraries')
    p.add_argument('--runtime-dir', type=Path, required=True, help='directory containing python and binutils-aarch64')
    p.add_argument('--receipt', type=Path, required=True, help='new output JSON path')
    args = p.parse_args()
    spec = importlib.util.spec_from_file_location('archive', HERE / 'restore.py')
    archive = importlib.util.module_from_spec(spec); spec.loader.exec_module(archive)
    oracle_path = ROOT / 'tools/pencil2-public-replay/drawing-native-target-receipt.json'
    oracle_data = oracle_path.read_bytes()
    if sha(oracle_data) != '5bf82575777d7ca09560a860506d42e2afb22935a3e59cafcab30dfa40f65d6a':
        raise SystemExit('retained oracle hash mismatch')
    oracle = json.loads(oracle_data)
    adapter = ROOT / 'tools/pencil2-public-replay/drawing_native_target_probe.py'
    adapter_data = adapter.read_bytes()
    if sha(adapter_data) != '89ace4e896f07c60eddd250f2fdad28521cb9668c50ca32018f0a26ff3a6224b':
        raise SystemExit('Drawing adapter hash mismatch')
    helper = (HERE / 'sources/inkpen_paint_native.py').read_bytes()
    if sha(helper) != 'a3e65b2b37b2913b4bb2787b72ab2dbad0a9660cb66435b142f5acfa4d5051ae':
        raise SystemExit('GLES helper hash mismatch')
    with tempfile.TemporaryDirectory(prefix='sdocx-pencil2-', dir='/tmp') as d:
        root = Path(d)
        for source, destination, expected in archive.FILES:
            src = HERE / 'sources' / source if not source.startswith('../') else ROOT / 'tools' / source[3:]
            data = src.read_bytes()
            if source.endswith('.gz'): data = gzip.decompress(data)
            if sha(data) != expected: raise SystemExit(f'archive hash mismatch: {src}')
            dest = root / destination; dest.parent.mkdir(parents=True, exist_ok=True); dest.write_bytes(data)
        conf = root / archive.CONF
        (conf / 'inkpen_paint_native.py').write_bytes(helper)
        scratch = root / 'scratch'; scratch.mkdir()
        (scratch / 'apk-analysis-native').mkdir()
        (scratch / 'apk-analysis-native/arm64-v8a').symlink_to(args.native_dir.resolve(), target_is_directory=True)
        (scratch / 'apk-analysis-runtime').symlink_to(args.runtime_dir.resolve(), target_is_directory=True)
        (conf.parent / 'scratch').symlink_to(scratch, target_is_directory=True)
        dest = root / 'tools/pencil2-public-replay/drawing_native_target_probe.py'; dest.parent.mkdir(parents=True)
        old = '/home/jding/RustroverProjects/sdocx'
        text = adapter_data.decode()
        if text.count(old) != 5: raise SystemExit('unexpected adapter path count')
        dest.write_text(text.replace(old, str(root)))
        env = os.environ.copy(); env['PYTHONPATH'] = str(root / 'scratch/apk-analysis-runtime/python')
        run = subprocess.run(['/usr/bin/python3', str(dest)], cwd=root, env=env, text=True, capture_output=True)
        if run.returncode: raise SystemExit(f'capture failed ({run.returncode}):\n{run.stdout[-4000:]}\n{run.stderr[-8000:]}')
        captures = list((root / 'tmp').glob('pencil2-queue-intervention-*/handoff/capture.json'))
        if len(captures) != 1: raise SystemExit(f'expected one capture, found {captures}; stdout: {run.stdout[-2000:]}')
        capture = json.loads(captures[0].read_text())
        actual = {'geometry_sha256': capture['streams']['draw_object_stroke']['sha256'], 'frame_sha256': capture['native_target_probe']['readback_sha256']}
        expected = {'geometry_sha256': oracle['streams']['draw_object_stroke']['sha256'], 'frame_sha256': oracle['target']['readback_sha256']}
        result = {'scope': 'Synthetic Drawing queue intervention and native ARM64 emulation on host Mesa; no device or saved-note parity.', 'adapter_sha256': sha(adapter_data), 'gles_helper_sha256': sha(helper), 'oracle_sha256': sha(oracle_data), 'capture_sha256': sha(captures[0].read_bytes()), 'actual': actual, 'expected': expected, 'matches': actual == expected}
        with args.receipt.open('x') as f: json.dump(result, f, indent=2); f.write('\n')
        print(json.dumps(result, indent=2))
        if actual != expected: raise SystemExit('capture differs from retained oracle')

if __name__ == '__main__': main()
