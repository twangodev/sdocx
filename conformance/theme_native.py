"""Verify saved color fixtures against hash-pinned SPen native theme functions.

Uses the existing ARM64 loader; only libc fmod/fmodf are host supplied.
"""
import argparse
import json
import math
import shutil
import struct
from pathlib import Path

from fountain_native import NativeFountain
from unicorn import arm64_const as R


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=Path('conformance/theme-colors.json'))
    parser.add_argument('--capture', type=Path)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    args = parser.parse_args()
    native = NativeFountain(args.library_dir, args.objdump)
    def double(i):
        return struct.unpack('<d', struct.pack('<Q', native.u.reg_read(R.UC_ARM64_REG_D0 + i)))[0]
    native.callbacks['fmod'] = lambda: native.d(0, math.fmod(double(0), double(1)))
    native.callbacks['fmodf'] = lambda: native.f(0, math.fmod(native.f(0), native.f(1)))
    reference = json.loads(args.reference.read_text())
    for case in reference['cases']:
        native.call('SPen::DarkColorTheme::GetColor(int)', [0, int(case['input'], 16)])
        actual = f'{native.x(0) & 0xffffffff:08x}'
        if args.capture:
            case['dark'] = actual
        elif actual != case['dark']:
            raise AssertionError(f'{case["input"]}: {actual} != {case["dark"]}')
        native.call('SPen::LightColorTheme::GetColor(int)', [0, int(case['input'], 16)])
        if native.x(0) & 0xffffffff != int(case['input'], 16):
            raise AssertionError('light theme modified a stored color')
    if args.capture:
        args.capture.write_text(json.dumps(reference, indent=2) + '\n')
    print(json.dumps({'cases': len(reference['cases']), 'reference_checked': not bool(args.capture)}))


if __name__ == '__main__':
    main()
