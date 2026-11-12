"""Prepare the two retained Marker4 geometry controls; native execution is opt-in."""

import argparse
from contextlib import contextmanager
import hashlib
import importlib.util
import json
from pathlib import Path
import struct
import sys
import zipfile

INPUT_SHA = 'e5d2a4c4fade709d3fb0c01c26621e2393f9cefa1810112e21d5d477da0de9db'
RUNNER_SHA = 'aa6a1ee3148c99585b95d7b3912f03dc3a8f1260f535c5ebc2cae2284e94edef'
SUPPORT_SHA = '2dda283fafee2cae85f781e2e97c682898d526f580a639a80d070463fcf36cb5'
LIBRARY_SHA = '23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff'
ARCHIVE_SHA = '8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2'
PAGE_SHA = 'b82bf068c598f68f7481e8dfec9f11936247457c70256dd27d7f6366a01324a8'
WIRE_SHA = '02631d98bd76a79cabfa35fd708ec9d1c9aaa92472fdfaa7ce619729fa749d1d'
PRESSURE_SHA = '98feab538a47c49113f689d42408bd3d7de816d53c2f8cb69e695070c773f7ba'
PAGE_NAME = 'e2c01be0-b6cf-11f1-91c8-b3fbd8ca0b23.page'


def pinned(path, expected):
    data = Path(path).read_bytes()
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError(f'Pinned input changed: {path}')
    return data


def source_geometry(source, *, pressures, pen_width):
    return {
        'rendering': {
            'pen_name': source['profile_name_resolved'],
            'advanced_settings': source['advanced_settings_resolved'],
            'tool_type_raw': source['tool_type_raw'],
        },
        'points': [{'x': p[0], 'y': p[1]} for p in source['points']],
        'pressures': pressures,
        'timestamps': source['timestamps'],
        'tilts': source['tilts'],
        'orientations': source['orientations'],
        'pen_width': pen_width,
    }


def decode_saved_pressures(page):
    if hashlib.sha256(page).hexdigest() != PAGE_SHA:
        raise ValueError('Pinned page changed')
    wire = page[89092:89600]
    if hashlib.sha256(wire).hexdigest() != WIRE_SHA:
        raise ValueError('Pinned pressure wire changed')
    value = struct.unpack_from('<f', wire)[0]
    pressures = [value]
    for n in range(252):
        word = struct.unpack_from('<H', wire, 4 + 2 * n)[0]
        value = struct.unpack('<f', struct.pack(
            '<f', value + (word & 32767) * (-1 if word & 32768 else 1) / 4096))[0]
        pressures.append(value)
    if hashlib.sha256(struct.pack('<253f', *pressures)).hexdigest() != PRESSURE_SHA:
        raise ValueError('Decoded pressure differs from native getter receipt')
    return pressures


def check_geometry(inputs, pressures, render, *, current_pressures):
    """Compare complete dots/boundaries, retaining the current-mode size control."""
    results = []
    for size in (0.4, 2.0):
        raw = source_geometry(inputs['source'], pressures=pressures, pen_width=size)
        got = render(raw)
        if got['dots'] != inputs['native']['native_dots']:
            raise ValueError(f'{size}: native dots differ')
        if got['sample_ends'] != inputs['native']['sample_ends']:
            raise ValueError(f'{size}: sample boundaries differ')
        results.append(got)
    if current_pressures and results[0] != results[1]:
        raise ValueError('Current-pressure full outputs differ across sizes')
    return results


@contextmanager
def native_module(runner, support, runtime_python):
    """Bind the exact validated support file; restore process import state."""
    old_path = sys.path[:]
    names = ('marker4_native', 'native_v8')
    previous = {name: sys.modules.get(name) for name in names}
    try:
        sys.path[:0] = [str(runner.parent.resolve()), str(support.parent.resolve()),
                        str(runtime_python.resolve())]
        for name, path in zip(names, (support, runner)):
            spec = importlib.util.spec_from_file_location(name, path)
            module = importlib.util.module_from_spec(spec)
            sys.modules[name] = module
            spec.loader.exec_module(module)
        yield module
    finally:
        sys.path[:] = old_path
        for name, module in previous.items():
            if module is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--mode', choices=('historical', 'current'), required=True)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--execute-native', action='store_true',
                        help='Explicitly run the external native emulator (never needed for preparation)')
    parser.add_argument('--native-runner', type=Path)
    parser.add_argument('--native-support', type=Path)
    parser.add_argument('--runtime-python', type=Path)
    parser.add_argument('--library-dir', type=Path)
    parser.add_argument('--objdump', type=Path)
    args = parser.parse_args()
    inputs = json.loads(pinned(args.inputs, INPUT_SHA))
    pressures = inputs['source']['pressures']
    if args.mode == 'current':
        if args.archive is None:
            parser.error('--mode current requires --archive')
        pinned(args.archive, ARCHIVE_SHA)
        with zipfile.ZipFile(args.archive) as archive:
            pressures = decode_saved_pressures(archive.read(PAGE_NAME))
    if not args.execute_native:
        print(json.dumps({'mode': args.mode, 'points': len(inputs['source']['points']),
                          'pressures': len(pressures), 'native_executed': False}))
        return
    for name in ('native_runner', 'native_support', 'runtime_python', 'library_dir', 'objdump'):
        if getattr(args, name) is None:
            parser.error(f'--execute-native requires --{name.replace("_", "-")}')
    pinned(args.native_runner, RUNNER_SHA)
    pinned(args.native_support, SUPPORT_SHA)
    pinned(args.library_dir / 'libSPenMarker4.so', LIBRARY_SHA)
    # Transitive emulator/Unicorn dependencies remain external prerequisites.
    with native_module(args.native_runner, args.native_support, args.runtime_python) as module:
        def render(raw):
            return module.NativeV8(args.library_dir, args.objdump).render_v8(raw)
        results = check_geometry(inputs, pressures, render, current_pressures=args.mode == 'current')
    print(json.dumps({'mode': args.mode, 'sizes': [0.4, 2.0],
                      'dots': [len(result['dots']) for result in results], 'native_executed': True}))


if __name__ == '__main__':
    main()
