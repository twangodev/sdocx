"""Check Base SmPath cubic length and getPosTan against pinned native output.

The probe constructs a fresh native SmPath for each finite cubic and executes
only its constructor, moveTo, cubicTo, getLength, and getPosTan methods plus
their direct helpers. Host code supplies bounded operator-new/delete and
memory-copy operations. No pen drawable, Android object, or rendering code is
called.
"""
import argparse
import hashlib
import json
import math
import os
import re
import shutil
import struct
import subprocess
from pathlib import Path

from unicorn import Uc, UC_ARCH_ARM64, UC_MODE_ARM, UC_HOOK_CODE
from unicorn import arm64_const as R

APK_SHA256 = 'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667'
BASE_SHA256 = 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb'
SYMBOLS = {
    'SPen::SmPath::SmPath()': 0xbd290,
    'SPen::SmPath::moveTo(float, float)': 0xbd6fc,
    'SPen::SmPath::cubicTo(float, float, float, float, float, float)': 0xbdcf0,
    'SPen::SmPath::getLength()': 0xbd370,
    'SPen::SmPath::getPosTan(float, SPen::SmPoint*, SPen::SmPoint*)': 0xbd3a0,
}
CAPTURE_BOUNDARY = (
    'Base constructor, moveTo, cubicTo, getLength, getPosTan, and their direct '
    'Base helpers execute on each supplied scalar curve. No pen, Drawing, '
    'Android, renderer, GPU, PDF, or pixel code executes. The host supplies '
    'bounded operator new/delete and memcpy/memmove/memset only.'
)
FRACTIONS = (0.0, 0.125, 0.25, 0.5, 0.75, 0.875, 1.0)


def f32(value):
    return struct.unpack('<f', struct.pack('<f', value))[0]


def f32_bits(value):
    return f"0x{struct.unpack('<I', struct.pack('<f', value))[0]:08x}"


class NativeSmPath:
    def __init__(self, library_path, objdump):
        self.u = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
        self.base = 0x02000000
        self.heap = 0x10000000
        self.u.mem_map(self.base, 0x200000)
        self.u.mem_map(self.heap, 0x02000000)
        self.u.mem_map(0x20000000, 0x100000)
        self.u.reg_write(R.UC_ARM64_REG_TPIDR_EL0, 0x20000000)
        self.callbacks = {}
        self.plt = {}
        self.symbols = {}
        self._load_elf(library_path)
        self._load_symbols(library_path, objdump)
        self.u.hook_add(UC_HOOK_CODE, self._hook)
        self.callbacks.update({
            'operator new(unsigned long)': lambda: self.x(0, self.alloc(self.x(0))),
            'operator new[](unsigned long)': lambda: self.x(0, self.alloc(self.x(0))),
            'operator delete(void*)': lambda: None,
            'operator delete[](void*)': lambda: None,
            'memcpy': self._copy,
            'memmove': self._copy,
            'memset': self._memset,
        })

    def _load_elf(self, path):
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != BASE_SHA256:
            raise ValueError('unverified libSPenBase.so: ' + str(path))
        phoff = struct.unpack_from('<Q', data, 32)[0]
        phentsize, phnum = struct.unpack_from('<HH', data, 54)
        if phentsize != 56:
            raise ValueError('unexpected ELF64 program header size')
        for index in range(phnum):
            kind, _flags, offset, vaddr, _paddr, filesz, memsz, _align = struct.unpack_from(
                '<II6Q', data, phoff + index * phentsize)
            if kind != 1:
                continue
            start = (self.base + vaddr) & ~0xfff
            end = (self.base + vaddr + memsz + 0xfff) & ~0xfff
            if not (self.base <= start < end <= self.base + 0x200000):
                raise ValueError('Base PT_LOAD segment is outside bounded mapping')
            self.u.mem_write(self.base + vaddr, data[offset:offset + filesz])

    def _load_symbols(self, path, objdump):
        output = subprocess.check_output(['nm', '-D', '-C', str(path)], text=True)
        for line in output.splitlines():
            parts = line.strip().split(' ', 2)
            if len(parts) == 3 and parts[1] in ('T', 'W'):
                self.symbols[parts[2]] = self.base + int(parts[0], 16)
        missing = [name for name, offset in SYMBOLS.items()
                   if self.symbols.get(name) != self.base + offset]
        if missing:
            raise ValueError('symbol address mismatch: ' + ', '.join(missing))
        env = dict(os.environ)
        runtime_lib = Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
        if runtime_lib.is_dir():
            env['LD_LIBRARY_PATH'] = str(runtime_lib.resolve())
        disassembly = subprocess.check_output(
            [str(objdump), '-d', '-C', '-j', '.plt', str(path)], text=True, env=env)
        for address, name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:', disassembly, re.M):
            self.plt[self.base + int(address, 16)] = name

    def alloc(self, size):
        size = max(size, 1)
        address = (self.heap + 15) & ~15
        end = address + size
        if end > 0x12000000:
            raise MemoryError('bounded SmPath host heap exhausted')
        self.heap = end
        self.u.mem_write(address, bytes(size))
        return address

    def x(self, index, value=None):
        register = R.UC_ARM64_REG_X0 + index
        if value is None:
            return self.u.reg_read(register)
        self.u.reg_write(register, value & ((1 << 64) - 1))

    def f(self, index, value=None):
        register = R.UC_ARM64_REG_S0 + index
        if value is None:
            return struct.unpack('<f', struct.pack('<I', self.u.reg_read(register)))[0]
        bits = struct.unpack('<I', struct.pack('<f', value))[0]
        self.u.reg_write(register, bits)

    def _copy(self):
        self.u.mem_write(self.x(0), bytes(self.u.mem_read(self.x(1), self.x(2))))
        self.x(0, self.x(0))

    def _memset(self):
        address, byte, size = self.x(0), self.x(1) & 255, self.x(2)
        self.u.mem_write(address, bytes([byte]) * size)
        self.x(0, address)

    def _hook(self, uc, address, _size, _user):
        name = self.plt.get(address)
        if name is None:
            return
        if name in self.symbols:
            uc.reg_write(R.UC_ARM64_REG_PC, self.symbols[name])
            return
        callback = self.callbacks.get(name)
        if callback is None:
            raise RuntimeError('unhandled Base import: ' + name)
        callback()
        uc.reg_write(R.UC_ARM64_REG_PC, uc.reg_read(R.UC_ARM64_REG_LR))

    def call(self, name, x=(), f=()):
        for index, value in enumerate(x):
            self.x(index, value)
        for index, value in enumerate(f):
            self.f(index, value)
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(self.symbols[name], 0x200f1000, count=2_000_000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('native method instruction limit exceeded: ' + name)

    def render(self, controls):
        path = self.alloc(160)
        self.call('SPen::SmPath::SmPath()', [path])
        self.call('SPen::SmPath::moveTo(float, float)', [path], controls[:2])
        self.call('SPen::SmPath::cubicTo(float, float, float, float, float, float)',
                  [path], controls[2:])
        self.call('SPen::SmPath::getLength()', [path])
        length = self.f(0)
        pos, tangent = self.alloc(8), self.alloc(8)
        samples = []
        for fraction in FRACTIONS:
            distance = f32(length * fraction)
            self.call('SPen::SmPath::getPosTan(float, SPen::SmPoint*, SPen::SmPoint*)',
                      [path, pos, tangent], [distance])
            ok = self.x(0) != 0
            point = struct.unpack('<2f', self.u.mem_read(pos, 8))
            vector = struct.unpack('<2f', self.u.mem_read(tangent, 8))
            samples.append({
                'fraction': fraction,
                'distance_bits': f32_bits(distance),
                'ok': ok,
                'position_bits': [f32_bits(value) for value in point],
                'tangent_bits': [f32_bits(value) for value in vector],
            })
        return {
            'controls_f32': controls,
            'length_bits': f32_bits(length),
            'samples': samples,
        }


def fixture_template():
    return {
        'schema_version': 1,
        'apk_version': '4.4.45.37',
        'apk_sha256': APK_SHA256,
        'base_library_sha256': BASE_SHA256,
        'capture_boundary': CAPTURE_BOUNDARY,
        'native_methods': {name: f'Base:0x{offset:x}' for name, offset in SYMBOLS.items()},
        'sample_fractions': list(FRACTIONS),
        'cases': [
            {
                'name': 'asymmetric',
                'controls': [1.25, -3.5, 18.0, 72.0, -42.0, 15.0, 96.0, 41.0],
            },
            {
                'name': 's-curve',
                'controls': [0.0, 0.0, 120.0, 220.0, -80.0, -140.0, 35.0, 55.0],
            },
            {
                'name': 'collinear',
                'controls': [0.0, 0.0, 15.0, 20.0, 30.0, 40.0, 45.0, 60.0],
            },
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library', type=Path,
                        default=Path('scratch/apk-analysis-native/arm64-v8a/libSPenBase.so'))
    parser.add_argument('--objdump', type=Path,
                        default=Path(shutil.which('aarch64-linux-gnu-objdump') or
                                     'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--fixture', type=Path, default=Path('conformance/smpath-cubic.json'))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true',
                      help='execute native methods and replace recorded output')
    mode.add_argument('--check-fixture', action='store_true',
                      help='execute native methods and compare with fixture (default)')
    args = parser.parse_args()
    if args.write_fixture or not args.fixture.exists():
        fixture = fixture_template()
    else:
        fixture = json.loads(args.fixture.read_text())
    template = fixture_template()
    for key in ('schema_version', 'apk_version', 'apk_sha256', 'base_library_sha256',
                'capture_boundary', 'native_methods', 'sample_fractions'):
        if fixture.get(key) != template[key]:
            raise SystemExit('fixture metadata mismatch: ' + key)
    expected_inputs = [(case['name'], case['controls']) for case in template['cases']]
    actual_inputs = [(case.get('name'), case.get('controls')) for case in fixture.get('cases', [])]
    if actual_inputs != expected_inputs:
        raise SystemExit('fixture case inputs do not match the canonical three curves')
    failures = []
    for case in fixture['cases']:
        controls = [f32(value) for value in case['controls']]
        if len(controls) != 8 or not all(math.isfinite(value) for value in controls):
            raise SystemExit('each case must contain eight finite f32 coordinates')
        native = NativeSmPath(args.library, args.objdump)
        result = native.render(controls)
        if args.write_fixture:
            case['native'] = result
        elif result != case.get('native'):
            failures.append(case['name'] + ': native output differs from fixture')
        print(f"{case['name']}: length={result['length_bits']}; "
              f"full-length position={result['samples'][-1]['position_bits']}")
    if args.write_fixture:
        args.fixture.write_text(json.dumps(fixture, indent=2) + '\n')
    if failures:
        raise SystemExit('\n'.join(failures))


if __name__ == '__main__':
    main()
