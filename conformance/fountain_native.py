"""Independent numeric oracle for the saved FountainPen V16 geometry.

Requires Unicorn, nm, ARM64 objdump and the three hash-pinned APK libraries.
No APK code is distributed. Execute from the repository root, for example:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_native.py

The oracle runs native saved redraw, tolerance, SmPath, width history, drawLine,
endPen and drawPoint. Object initialization and the two-pass wrapper are
reconstructed from the saved ObjectStroke path. The host supplies ObjectStroke
channels, endpoint MotionEvent access and bounded allocation/memory operations.
Final GPU submission is replaced by a collector of positions and radii.
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
    "Marker4": "23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff",
    "Drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
    "Graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
    "Engine": "79a8024586ce58ceeca613ddfce159e92274c597c5a863dd0aaf3e8e32e1b505",
    "FountainPen": "1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec",
    "PenCommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
    "Base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
    "Renderer": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
}

class NativeFountain:

    def __init__(self, library_dir, objdump, additional_libraries=()):
        self.v16_recording = False
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
        self.u.mem_unmap(0, 0x1000)
        self.u.hook_add(UC_HOOK_CODE, self.hook)
        self.callbacks['SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)'] = self.dot
        self.callbacks['SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)'] = lambda: None
        self.callbacks['SPen::MotionEvent::~MotionEvent()'] = lambda: None
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
        if self.v16_recording:
            if a == self.syms['SPen::FountainPenStrokeDrawableGLV16::drawLine(float, float, float, float, long long, int, SPen::RectF*, bool)']:
                self.ends[self.sample] = len(self.dots)
                self.sample += 1
            elif a == self.syms['SPen::FountainPenStrokeDrawableGLV16::endPen(SPen::MotionEvent const*, SPen::RectF*, bool)']:
                self.ends[self.sample] = len(self.dots)
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

    def call(self, name, x=(), f=(), instruction_limit=2000000):
        for i, v in enumerate(x):
            self.x(i, v)
        for i, v in enumerate(f):
            self.f(i, v)
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(self.syms[name], 0x200f1000, count=instruction_limit)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('instruction limit')

    def dot(self):
        self.dots.append([self.f(i) for i in range(3)])

    def channel(self, name, fmt, values):
        pointer = self.alloc(8 * len(values)) if values else 0
        if values:
            self.put(pointer, fmt * len(values), *values)
        self.callbacks['SPen::ObjectStroke::' + name + '() const'] = lambda: self.x(0, pointer)

    def render(self, s):
        count = len(s['points'])
        if not count or any(len(s[k]) != count for k in ('pressures', 'timestamps')):
            raise ValueError('incomplete input channels')
        if any(s[k] and len(s[k]) != count for k in ('tilts', 'orientations')):
            raise ValueError('incomplete optional channels')
        self.heap = 0x10000000
        obj = self.alloc(512)
        data = self.alloc(512)
        settings = self.alloc(128)
        tol = self.alloc(64)
        wm = self.alloc(64)
        rect = self.alloc(32)
        source = self.alloc(16)
        self.put(obj + 16, 'Q', self.alloc(512))
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(data + 24, 'QQ', wm, tol)
        self.put(data + 56, 'BB', 1, 0)
        self.put(settings, 'f', s['pen_width'])
        self.put(obj + 96, 'f', 0.82)
        self.put(wm + 40, 'f', 0.15)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        rendering = s['rendering']
        if rendering['tool_type_raw'] not in (1, 2, 3):
            raise ValueError('oracle requires saved tool type 1, 2 or 3')
        fixed_width = rendering['style']['fixed_width']
        if rendering['properties']['fixed_width']:
            if fixed_width is None:
                raise ValueError('fixed-width redraw requires the saved width')
            self.put(data + 57, 'B', 1)
            self.put(data + 60, 'f', fixed_width)
        points = s['points']
        tool = rendering['tool_type_raw']
        self.channel('GetPoint', 'f', [v for p in points for v in (p['x'], p['y'])])
        self.channel('GetPressure', 'f', s['pressures'])
        self.channel('GetTimeStamp', 'i', s['timestamps'])
        self.channel('GetTilt', 'f', s['tilts'])
        self.channel('GetOrientation', 'f', s['orientations'])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, count)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, tool)
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, 0)
        self.end = {**points[-1], 'pressure': s['pressures'][-1],
                    'tilt': s['tilts'][-1] if s['tilts'] else 0.,
                    'time': s['timestamps'][-1], 'tool': tool, 'source': 0}
        self.call('SPen::WidthSmoothManager::resetSmoothing(SPen::MotionEvent::Tooltype)', [wm, tool])
        for mode in (1, 2):
            self.dots, self.ends, self.sample = [], [0] * count, 0
            self.call('SPen::WidthSmoothManager::setMode(SPen::WidthSmoothManager::WIDTH_MODE)', [wm, mode])
            self.call('SPen::PenTolerance::SetTolerance(float, float)', [tol], [rendering['style']['initial_tolerance'] or 0.0, 0.0])
            self.v16_recording = True
            try:
                self.call('SPen::FountainPenStrokeDrawableGLV16::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)', [obj, source, rect, 1])
            finally:
                self.v16_recording = False
            if self.x(0) != 1 or self.sample != max(0, count - 2):
                raise AssertionError('native redraw did not complete all samples')
            self.ends[-1] = len(self.dots)
            if mode == 1:
                self.call('SPen::WidthSmoothManager::completeSmoothingArray()', [wm])
        return {'dots': self.dots, 'sample_ends': self.ends}

def check(actual, expected, label):
    if actual['sample_ends'] != expected['sample_ends']:
        raise AssertionError(f'{label}: sample mapping differs')
    if len(actual['dots']) != len(expected['dots']):
        raise AssertionError(f'{label}: generated dot count differs')
    maximum = 0.0
    for a, b in zip(actual['dots'], expected['dots']):
        if len(a) != len(b):
            raise AssertionError(f'{label}: stamp attribute count differs')
        if not all(math.isfinite(v) for v in (*a, *b)):
            raise AssertionError(f'{label}: non-finite stamp attribute')
        maximum = max(maximum, *(abs(x - y) for x, y in zip(a, b)))
    if maximum > 0.0001:
        raise AssertionError(f'{label}: geometry differs by {maximum}')
    return maximum

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--prepared', type=Path, help='JSON from ink_geometry; compare reconstructed V16 fountain strokes')
    parser.add_argument('--reference', type=Path, default=Path('conformance/fountain-v16.json'))
    args = parser.parse_args()
    native = NativeFountain(args.library_dir, args.objdump)
    inputs = []
    if args.prepared:
        for i, row in enumerate(json.loads(args.prepared.read_text())):
            g = row['geometry']
            rendering = row['stroke']['rendering']
            if (g['support'] != 'reconstructed' or not rendering
                    or rendering['pen_name'] != 'com.samsung.android.sdk.pen.pen.preload.FountainPen'
                    or rendering['advanced_settings'] != '18;0;100;'):
                continue
            if len(g['points']) != len(g['dot_radii']):
                raise ValueError(f'{i}: incomplete prepared stamp attributes')
            inputs.append((str(i), row['stroke'], {'sample_ends': g['sample_ends'], 'dots': [[p['x'], p['y'], r] for p, r in zip(g['points'], g['dot_radii'])]}))
    else:
        reference = json.loads(args.reference.read_text())
        for case in reference['cases']:
            stroke = copy.deepcopy(reference['stroke'])
            stroke['pen_width'] = case['size']
            stroke['rendering']['style']['initial_tolerance'] = case['tolerance']
            if 'tool_type_raw' in case:
                stroke['rendering']['tool_type_raw'] = case['tool_type_raw']
            if 'fixed_width' in case:
                stroke['rendering']['properties']['fixed_width'] = True
                stroke['rendering']['style']['fixed_width'] = case['fixed_width']
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
