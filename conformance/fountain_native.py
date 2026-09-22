"""Independent numeric oracle for the saved FountainPen V16 geometry.

Requires Unicorn, nm, ARM64 objdump and the three hash-pinned APK libraries.
No APK code is distributed. Execute from the repository root, for example:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_native.py

The oracle runs the native tolerance, SmPath, width-history, drawLine and endPen
implementations. The object initialization and two-pass redraw orchestration
are reconstructed from the native saved ObjectStroke path. GPU calls are
replaced by a collector of drawPoint positions/radii. MotionEvent endpoint
getters and bounded host allocation/memory operations are the only other stubs.
This validates geometry at unit inverse scale, not GPU shader rasterization.
"""
import argparse
import copy
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

HASHES = {
    "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "Engine": "79a8024586ce58ceeca613ddfce159e92274c597c5a863dd0aaf3e8e32e1b505",
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Renderer": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
}

def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


class NativeFountain:

    def __init__(self, library_dir, objdump, additional_libraries=()):
        self.u = Uc(UC_ARCH_ARM64, UC_MODE_ARM)
        self.syms = {}
        self.plt = {}
        self.callbacks = {}
        self.heap = 0x10000000
        self.u.mem_map(self.heap, 0x2000000)
        self.u.mem_map(0x20000000, 0x100000)
        self.u.reg_write(R.UC_ARM64_REG_TPIDR_EL0, 0x20000000)
        env = dict(os.environ)
        runtime = Path('scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
        if runtime.is_dir():
            env['LD_LIBRARY_PATH'] = str(runtime.resolve())
        names = ('FountainPen', 'PenCommon', 'Base', *additional_libraries)
        for index, name in enumerate(names):
            base = index * 0x1000000
            path = library_dir / ('libSPen' + name + '.so')
            b = path.read_bytes()
            if hashlib.sha256(b).hexdigest() != HASHES[name]:
                raise ValueError('unverified library: ' + str(path))
            off = struct.unpack_from('<Q', b, 32)[0]
            stride, n = struct.unpack_from('<HH', b, 54)
            self.u.mem_map(base, 0x200000)
            for i in range(n):
                kind, _, fo, va, _, fs, ms, _ = struct.unpack_from('<II6Q', b, off + i * stride)
                if kind == 1:
                    self.u.mem_write(base + va, b[fo:fo + fs])
            for line in subprocess.check_output(['nm', '-D', '-C', path], text=True).splitlines():
                parts = line.strip().split(' ', 2)
                if len(parts) == 3 and parts[1] in ('T', 'W'):
                    self.syms[parts[2]] = base + int(parts[0], 16)
            dis = subprocess.check_output([str(objdump), '-d', '-C', '-j', '.plt', path], env=env, text=True)
            for a, s in re.findall('^([0-9a-f]+) <(.+)@plt>:', dis, re.M):
                self.plt[base + int(a, 16)] = s
        self.u.hook_add(UC_HOOK_CODE, self.hook)
        self.callbacks['SPen::FountainPenStrokeDrawableGLV16::drawPoint(SPen::SmPoint const*, float, SPen::RectF*, bool)'] = self.dot
        for name in ['operator new(unsigned long)', 'operator new[](unsigned long)']:
            self.callbacks[name] = lambda: self.x(0, self.alloc(self.x(0)))
        for name in ['operator delete(void*)', 'operator delete[](void*)']:
            self.callbacks[name] = lambda: None
        self.callbacks['memcpy'] = self.copy
        self.callbacks['memmove'] = self.copy
        self.callbacks['memset'] = lambda: self.u.mem_write(self.x(0), bytes([self.x(1) & 255]) * self.x(2))
        for n, key in [('GetX(int) const', 'x'), ('GetY(int) const', 'y')]:
            self.callbacks['SPen::MotionEvent::' + n] = lambda k=key: self.d(0, self.end[k])
        for n, key in [('GetToolType(int) const', 'tool'), ('GetSource() const', 'source'), ('GetEventTime() const', 'time')]:
            self.callbacks['SPen::MotionEvent::' + n] = lambda k=key: self.x(0, self.end[k])
        for n, key in [('GetPressure(int) const', 'pressure'), ('GetTilt(int) const', 'tilt')]:
            self.callbacks['SPen::MotionEvent::' + n] = lambda k=key: self.f(0, self.end[k])

    def alloc(self, n):
        a = self.heap
        self.heap += (n + 15) // 16 * 16
        if self.heap > 0x12000000:
            raise RuntimeError('oracle heap limit')
        self.u.mem_write(a, bytes(n))
        return a

    def x(self, i, v=None):
        r = R.UC_ARM64_REG_X0 + i
        if v is None:
            return self.u.reg_read(r)
        self.u.reg_write(r, v & (1 << 64) - 1)

    def f(self, i, v=None):
        r = R.UC_ARM64_REG_S0 + i
        if v is None:
            return struct.unpack('<f', struct.pack('<I', self.u.reg_read(r)))[0]
        self.u.reg_write(r, struct.unpack('<I', struct.pack('<f', v))[0])

    def d(self, i, v):
        self.u.reg_write(R.UC_ARM64_REG_D0 + i, struct.unpack('<Q', struct.pack('<d', v))[0])

    def put(self, a, fmt, *v):
        self.u.mem_write(a, struct.pack('<' + fmt, *v))

    def get(self, a, fmt):
        return struct.unpack('<' + fmt, self.u.mem_read(a, struct.calcsize('<' + fmt)))

    def copy(self):
        self.u.mem_write(self.x(0), bytes(self.u.mem_read(self.x(1), self.x(2))))

    def hook(self, u, a, size, _):
        if a not in self.plt:
            return
        name = self.plt[a]
        if name in self.callbacks:
            self.callbacks[name]()
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
        elif name in self.syms:
            u.reg_write(R.UC_ARM64_REG_PC, self.syms[name])
        else:
            raise RuntimeError('unhandled native import ' + name)

    def call(self, name, x=(), f=()):
        for i, v in enumerate(x):
            self.x(i, v)
        for i, v in enumerate(f):
            self.f(i, v)
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(self.syms[name], 0x200f1000, count=2000000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('instruction limit')

    def dot(self):
        obj, p = (self.x(0), self.x(1))
        self.put(obj + 88, 'B', 0)
        # V16 drawPoint floor; bypass only the GPU/bounds bookkeeping.
        radius = max(self.f(0), struct.unpack("<f", struct.pack("<f", 0.1))[0])
        self.dots.append([*self.get(p, 'ff'), radius])

    def render(self, s):
        self.heap = 0x10000000
        obj = self.alloc(512)
        data = self.alloc(512)
        settings = self.alloc(128)
        tol = self.alloc(64)
        wm = self.alloc(64)
        rect = self.alloc(32)
        point = self.alloc(8)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(data + 24, 'QQ', wm, tol)
        self.put(data + 56, 'BB', 1, 0)
        self.put(settings, 'f', s['pen_width'])
        self.put(obj + 96, 'f', 0.82)
        self.put(obj + 272, 'B', 1)
        self.put(obj + 276, 'f', 5.0)
        self.put(wm + 40, 'f', 0.15)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        rendering = s['rendering']
        if rendering['tool_type_raw'] != 2 or rendering['properties']['fixed_width']:
            raise ValueError('oracle requires a variable-width stylus stroke')
        points = s['points']
        press = [min(f32(v), 1.0) for v in s['pressures']]
        initial_width = f32(f32(f32(s['pen_width']) * 0.5) * press[0])
        times = s['timestamps']
        tilts = s['tilts']
        self.dots = []
        self.call('SPen::WidthSmoothManager::resetSmoothing(SPen::MotionEvent::Tooltype)', [wm, 2])
        # Saved ObjectStroke redraw: fill widths, finish smoothing, then replay.
        # The native direction ratio ring intentionally survives between passes.
        for mode in (1, 2):
            self.dots = []
            ends = []
            self.call('SPen::WidthSmoothManager::setMode(SPen::WidthSmoothManager::WIDTH_MODE)', [wm, mode])
            self.call('SPen::PenTolerance::SetTolerance(float, float)', [tol], [rendering['style']['initial_tolerance'] or 0.0, 0.0])
            first = points[0]
            p0 = [first['x'], first['y']]
            self.put(point, 'ff', *p0)
            self.call('SPen::PenTolerance::Reset(SPen::PointF const&)', [tol, point])
            self.put(obj + 88, 'BB', 1, 0)
            self.put(obj + 100, 'fff', 0.0, initial_width, press[0])
            self.put(obj + 112, 'ffffff', *p0, *p0, *p0)
            self.call('SPen::WidthSmoothManager::getSmoothedWidthFromList(float, float, long long, float, SPen::WidthSmoothManager::CALL_FROM_TYPE)', [wm, times[0], 0], [p0[0], p0[0], initial_width])
            self.put(obj + 104, 'f', self.f(0))
            ends.append(0)
            for i in range(1, len(points) - 1):
                degrees = f32(f32(f32(tilts[i] if tilts else 0) * 180) / math.pi)
                tilt = f32(f32(max(0., f32(min(75., degrees) - 15)) / 60) * 3)
                self.call('SPen::FountainPenStrokeDrawableGLV16::drawLine(float, float, float, float, long long, int, SPen::RectF*, bool)', [obj, times[i], 2, rect, 1], [points[i]['x'], points[i]['y'], press[i], tilt])
                ends.append(len(self.dots))
            last = points[-1]
            self.end = {**last, 'pressure': press[-1], 'tilt': tilts[-1] if tilts else 0.0, 'time': times[-1], 'tool': 2, 'source': 0}
            self.call('SPen::FountainPenStrokeDrawableGLV16::endPen(SPen::MotionEvent const*, SPen::RectF*, bool)', [obj, point, rect, 1])
            ends.append(len(self.dots))
            ends = ends[-len(points):]
            if mode == 1:
                self.call('SPen::WidthSmoothManager::completeSmoothingArray()', [wm])
        return {'dots': self.dots, 'sample_ends': ends}

def check(actual, expected, label):
    if actual['sample_ends'] != expected['sample_ends']:
        raise AssertionError(f'{label}: sample mapping differs')
    if len(actual['dots']) != len(expected['dots']):
        raise AssertionError(f'{label}: generated dot count differs')
    maximum = 0.0
    for a, b in zip(actual['dots'], expected['dots']):
        maximum = max(maximum, *(abs(x - y) for x, y in zip(a, b)))
    if maximum > 0.0001:
        raise AssertionError(f'{label}: geometry differs by {maximum}')
    return maximum

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--prepared', type=Path, help='JSON from the ink_geometry Rust example; compare every reconstructed stroke')
    parser.add_argument('--reference', type=Path, default=Path('conformance/fountain-v16.json'))
    args = parser.parse_args()
    native = NativeFountain(args.library_dir, args.objdump)
    inputs = []
    if args.prepared:
        for i, row in enumerate(json.loads(args.prepared.read_text())):
            g = row['geometry']
            if g['support'] != 'reconstructed':
                continue
            inputs.append((str(i), row['stroke'], {'sample_ends': g['sample_ends'], 'dots': [[p['x'], p['y'], r] for p, r in zip(g['points'], g['dot_radii'])]}))
    else:
        reference = json.loads(args.reference.read_text())
        for case in reference['cases']:
            stroke = copy.deepcopy(reference['stroke'])
            stroke['pen_width'] = case['size']
            stroke['rendering']['style']['initial_tolerance'] = case['tolerance']
            stroke['points'] = [dict(x=s[0], y=s[1]) for s in case['samples']]
            stroke['pressures'] = [s[2] for s in case['samples']]
            stroke['tilts'] = [s[3] for s in case['samples']]
            stroke['timestamps'] = [s[4] for s in case['samples']]
            inputs.append((case['name'], stroke, case))
    if not inputs:
        raise ValueError('no reconstructed strokes to check')
    maximum = 0.0
    dots = 0
    for name, stroke, expected in inputs:
        actual = native.render(stroke)
        maximum = max(maximum, check(actual, expected, name))
        dots += len(actual['dots'])
    print(f'{len(inputs)} strokes, {dots} native dots, identical sample mapping, maximum error {maximum:.9f}')
if __name__ == '__main__':
    main()
