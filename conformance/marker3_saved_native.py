"""Hash-pinned Marker3 V5 saved ObjectStroke geometry oracle.

Executes native saved redraw, SmPath, drawLine, endPen, the Base MotionEvent
constructor and the native terminal GetX/GetY methods. It supplies synthetic
ObjectStroke channels and drawable state and reads native vector writes back.
Shader rasterization and the application's real pen admission remain outside
this bounded oracle.
"""
import hashlib
import json
import math
import os
import argparse
import shutil
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path('conformance').resolve()))
from fountain_native import NativeFountain, HASHES

HASHES['Marker3'] = 'd9e63380088cadd4128f05d904bba109a3adff343c048d005beba4d83f5f15c5'

class Probe(NativeFountain):
    def __init__(self, libdir, objdump):
        self.points = []
        self.motion_trace = []
        self.sample_ends = []
        self.spacing_trace = []
        super().__init__(libdir, objdump, ('Marker3',))
        self.callbacks['SPen::Marker3StrokeDrawableRTV2::AddPoint(SPen::Vector2<float>, SPen::Vector3<float>)'] = self.add
        self.callbacks['SPen::PenUtil::ConvertToRGBA(unsigned int)'] = self.rgba
        self.callbacks['sincosf'] = self.sincos
        self.callbacks['tanf'] = lambda: self.f(0, math.tan(self.f(0)))
        self.callbacks['fmodf'] = lambda: self.f(0, math.fmod(self.f(0), self.f(1)))
        # Let Base build the event from the V5 wrapper's native PointerCoords,
        # then let its getters supply the terminal sample.
        for name in [
            'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)',
            'SPen::MotionEvent::~MotionEvent()',
            'SPen::MotionEvent::GetX(int) const', 'SPen::MotionEvent::GetY(int) const',
            'SPen::MotionEvent::GetToolType(int) const', 'SPen::MotionEvent::GetSource() const',
            'SPen::MotionEvent::GetEventTime() const', 'SPen::MotionEvent::GetPressure(int) const',
            'SPen::MotionEvent::GetTilt(int) const', 'SPen::MotionEvent::GetHistorySize() const',
        ]:
            self.callbacks.pop(name, None)

    def add(self):
        self.points.append([self.f(i) for i in range(5)])

    def hook(self, u, a, size, _):
        if a == 0x303df08 or a == 0x303d2c8:
            obj = self.x(0)
            vec = self.get(obj + 80, 'Q')[0]
            begin, finish, _ = self.get(vec, '3Q')
            self.sample_ends.append((finish - begin) // 12 if finish >= begin else 0)
            if a == 0x303df08:
                self.spacing_trace.append({'spacing': self.get(obj+88,'f')[0], 'residual': self.get(obj+92,'f')[0], 'threshold':self.get(obj+44,'f')[0], 'coords':self.get(obj+116,'2f')})
        for name in ('SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)', 'SPen::MotionEvent::GetX(int) const', 'SPen::MotionEvent::GetY(int) const', 'SPen::MotionEvent::GetPressure(int) const', 'SPen::MotionEvent::GetToolType(int) const', 'SPen::MotionEvent::GetEventTime() const', 'SPen::MotionEvent::GetHistorySize() const'):
            if self.syms.get(name) == a: self.motion_trace.append(name)
        return super().hook(u, a, size, _)

    def rgba(self):
        c = self.x(0) & 0xffffffff
        for i, shift in enumerate((16, 8, 0)):
            self.f(i, ((c >> shift) & 255) / 255.0)
        self.f(3, ((c >> 24) & 255) / 255.0)

    def sincos(self):
        x = self.f(0)
        a = self.x(1)
        b = self.x(2)
        self.put(a, 'f', math.sin(x))
        self.put(b, 'f', math.cos(x))

    def sample(self, p0, p1, width=8.0, spacing=1.0, remainder=0.0, version=5):
        self.heap = 0x10000000
        obj = self.alloc(512)
        data = self.alloc(128)
        settings = self.alloc(128)
        points_vec = self.alloc(24)
        point_storage = self.alloc(4096)
        self.put(points_vec, '3Q', point_storage, point_storage, point_storage + 4096)
        self.put(obj + 16, 'Q', 0x10100000)  # non-null RT marker; no GL method is called
        self.put(obj + 80, 'Q', points_vec)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(settings, 'fI', width, 0xff336699)
        self.put(data + 56, 'B', 0)
        self.put(obj + 88, 'f', spacing)
        self.put(obj + 92, 'f', remainder)
        self.put(obj + 44, 'f', 0.0)
        self.put(obj + 96, 'f', 0.0)
        self.put(obj + 116, '2f', *p0)
        self.put(obj + 124, '2f', *p0)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        self.points.clear()
        self.call(f'SPen::Marker3StrokeDrawableGLV{version}::drawLine(float, float, bool, bool)',
                  [obj, 0, 0], [p1[0], p1[1]], instruction_limit=5000000)
        vec = self.get(obj + 80, 'Q')[0]
        begin, end, cap = self.get(vec, '3Q')
        if begin and end >= begin and end - begin <= 0x100000:
            staged = [list(self.get(begin + offset, '3f')) for offset in range(0, end - begin, 12)]
        else:
            staged = []
        return {'version': version, 'input': [p0, p1], 'width': width, 'spacing': spacing,
                'remainder_in': remainder, 'points': self.points, 'staged': staged,
                'remainder_out': self.get(obj + 92, 'f')[0],
                'previous_point_out': list(self.get(obj + 116, '2f'))}

    def saved_redraw(self, samples, width=8.0, pressures=None, tool=2, is_shape=False, fixed_width=False, wrapper_flag=False):
        self.heap = 0x10000000
        count = len(samples)
        obj, stroke = self.alloc(512), self.alloc(256)
        data, settings = self.alloc(128), self.alloc(128)
        points_vec, = [self.alloc(24)]
        point_storage = self.alloc(8192)
        self.put(points_vec, '3Q', point_storage, point_storage, point_storage + 8192)
        self.put(obj + 16, 'Q', 0x10100000)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(settings, 'fI', width, 0xff336699)
        self.put(data + 56, 'B', 0)
        if fixed_width:
            self.put(data + 57, 'B', 1)
            self.put(data + 60, 'f', width)
        self.put(obj + 80, 'Q', points_vec)
        self.put(obj + 88, 'f', 1.0)
        self.put(obj + 44, 'f', 2.0)
        self.put(obj + 96, 'f', 20.0)
        self.call('SPen::SmPath::SmPath()', [obj + 136])
        self.channel('GetPoint', 'f', [v for p in samples for v in p])
        self.channel('GetTimeStamp', 'i', [i * 8 for i in range(count)])
        self.channel('GetPressure', 'f', pressures if pressures is not None else [1.0] * count)
        self.channel('GetTilt', 'f', [])
        self.channel('GetOrientation', 'f', [])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, count)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, tool)
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, int(is_shape))
        self.end = {'x': samples[-1][0], 'y': samples[-1][1], 'tool': tool, 'source': 0, 'time': (count - 1) * 8}
        self.motion_trace.clear()
        self.sample_ends.clear()
        self.spacing_trace.clear()
        rect = self.alloc(32)
        self.call('SPen::Marker3StrokeDrawableGLV5::redraw(SPen::ObjectStroke const*, SPen::RectF*, bool)',
                  [obj, stroke, rect, int(wrapper_flag)], instruction_limit=10000000)
        begin, end, cap = self.get(points_vec, '3Q')
        staged = [list(self.get(begin + off, '3f')) for off in range(0, end - begin, 12)]
        return {'samples': samples, 'width': width, 'pressures': pressures, 'tool_type_raw': tool,
                'is_shape': is_shape, 'fixed_width': fixed_width, 'wrapper_flag': wrapper_flag,
                'sample_ends': [1, *self.sample_ends[1:], len(staged)], 'spacing_trace':self.spacing_trace, 'stamps': staged, 'rect': list(self.get(rect, '4f')), 'native_motion_calls': self.motion_trace}

def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    parser.add_argument('--fixture', type=Path, default=root / 'conformance/marker3-saved-native.json')
    args = parser.parse_args()
    libdir = root / 'scratch/apk-analysis-native/arm64-v8a'
    objdump = Path(shutil.which('aarch64-linux-gnu-objdump') or root / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump')
    p = Probe(libdir, objdump)
    saved = [p.saved_redraw([(0, 0), (20, 0)], pressures=[0.05, 1.0]),
             p.saved_redraw([(0, 0), (20, 0)], pressures=[1.0, 0.05]),
             p.saved_redraw([(0, 0), (10, 0), (20, 10)], pressures=[1.0, 0.05, 1.0]),
             *[p.saved_redraw([(0, 0), (6, 0), (20, 0)], pressures=[0.1, 0.5, 1], tool=t) for t in (1, 2, 3)],
             p.saved_redraw([(0, 0), (10, 0), (20, 10)], is_shape=True),
             p.saved_redraw([(0, 0), (10, 0), (20, 10)], fixed_width=True),
             p.saved_redraw([(0, 0), (10, 0), (20, 10)], wrapper_flag=True),
             *[p.saved_redraw([(3, 4)], width=w) for w in (1.0, 4.0, 12.0)]]
    source = {name.lower() + '_sha256': HASHES[name] for name in ('FountainPen', 'PenCommon', 'Base', 'Marker3')}
    saved_cases = []
    for i, item in enumerate(saved, 1):
        saved_cases.append({
            'id': f'marker3-v5-saved-{i}',
            'input': {key: item[key] for key in ('samples', 'width', 'pressures', 'tool_type_raw', 'is_shape', 'fixed_width', 'wrapper_flag')},
            'native': {'sample_ends': item['sample_ends'], 'stamps': item['stamps']},
        })
    fixture = {'schema':'sdocx-marker3-saved-native-v1', 'source':source, 'evidence_scope': 'Native V5 saved redraw(ObjectStroke), drawLine, endPen, SmPath, MotionEvent constructor and terminal getters execute; synthetic ObjectStroke channels and drawable state are supplied; native point-vector writes are read back; shader raster/composition and real app admission are outside this probe.', 'cases': saved_cases}
    encoded = json.dumps(fixture, indent=2) + '\n'
    if args.write_fixture:
        args.fixture.write_text(encoded)
    elif args.fixture.read_text() != encoded:
        raise SystemExit('native fixture differs; rerun with --write-fixture to replace it')

if __name__ == '__main__':
    main()
