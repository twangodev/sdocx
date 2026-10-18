"""Capture bounded Pencil3 V1 native particle and saved-redraw outputs.

The direct cases execute native drawPoint and RTV1::AddPoint with explicit
rand values. Saved cases execute the object adapter, V1 event redraw, real
drawLine/drawPoint/AddPoint, and endPen paths. libc rand/srand is modeled with
the pinned AOSP bionic TYPE_3 algorithm; this is an Android-source profile, not
execution under Samsung's runtime. Queue work is not executed and texture/GPU
appearance is outside this fixture.

Run from the repository root:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pencil_native.py
"""
import argparse
import hashlib
import json
import shutil
from pathlib import Path

import fountain_native
from fountain_native import NativeFountain
from unicorn import arm64_const as R

HASHES = {
    'Pencil3': '41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359',
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
}
DRAW_POINT = 'SPen::Pencil3StrokeDrawableGLV1::drawPoint(SPen::SmPoint*, float, float, SPen::RectF*)'
ADD_POINT = 'SPen::Pencil3StrokeDrawableRTV1::AddPoint(SPen::Vector3<float>, float, float)'
OBJECT_REDRAW = 'SPen::Pencil3StrokeDrawableGLV1::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)'
MOTION_EVENT_ARRAY = ('SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, '
                      'int, int const*, float const*, float const*, long long const*)')


class BionicRandom:
    """A local model of AOSP's TYPE_3 random.c profile used by rand/srand."""
    def seed(self, value):
        self.state = [value & 0xffffffff]
        for _ in range(1, 31):
            previous = self.state[-1]
            next_value = 16807 * (previous % 127773) - 2836 * (previous // 127773)
            if next_value <= 0:
                next_value += 0x7fffffff
            self.state.append(next_value)
        self.front, self.rear = 3, 0
        for _ in range(10 * 31):
            self.next()

    def next(self):
        total = (self.state[self.front] + self.state[self.rear]) & 0xffffffff
        self.state[self.front] = total
        value = (total >> 1) & 0x7fffffff
        self.front = (self.front + 1) % 31
        self.rear = (self.rear + 1) % 31
        return value


class NativePencil3(NativeFountain):
    def __init__(self, library_dir, objdump):
        # NativeFountain's loader consults its module registry. Scope the
        # Pencil3 entry to construction and restore it before returning.
        old_hashes = fountain_native.HASHES
        fountain_native.HASHES = {**old_hashes, 'Pencil3': HASHES['Pencil3']}
        try:
            super().__init__(library_dir, objdump, ('Pencil3',))
        finally:
            fountain_native.HASHES = old_hashes
        for name, expected in HASHES.items():
            path = library_dir / ('libSPen' + name + '.so')
            if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise ValueError('unverified library: ' + str(path))
        self.draw_point = self.syms[DRAW_POINT]
        self.callbacks['srand'] = self.seed_random
        self.callbacks['rand'] = self.next_random
        self.random_values = []
        self.random_index = 0
        self.seed = None
        self.rng_profile = 'fixture'
        self.bionic_rng = BionicRandom()
        self.random_trace = []
        self.events = {}
        self.saved_stroke = None
        self.callbacks.update({
            MOTION_EVENT_ARRAY: self.make_motion_event,
            'SPen::MotionEvent::GetHistorySize() const': self.motion_history_size,
            'SPen::MotionEvent::GetAction() const': lambda: self.x(0, 2),
            'SPen::MotionEvent::GetHistoricalX(int, int) const': lambda: self.motion_historical('x'),
            'SPen::MotionEvent::GetHistoricalY(int, int) const': lambda: self.motion_historical('y'),
            'SPen::MotionEvent::GetHistoricalPressure(int) const': lambda: self.motion_historical('pressure'),
            'SPen::MotionEvent::GetHistoricalTilt(int) const': lambda: self.motion_historical('tilt'),
            'SPen::MotionEvent::GetToolType(int) const': lambda: self.motion_current('tool', 'int'),
            'SPen::MotionEvent::GetX(int) const': lambda: self.motion_current('x', 'double'),
            'SPen::MotionEvent::GetY(int) const': lambda: self.motion_current('y', 'double'),
            'SPen::MotionEvent::GetPressure(int) const': lambda: self.motion_current('pressure', 'float'),
            'SPen::MotionEvent::GetTilt(int) const': lambda: self.motion_current('tilt', 'float'),
            'SPen::MotionEvent::GetOrientation(int) const': lambda: self.motion_current('orientation', 'float'),
            'SPen::MotionEvent::GetEventTime() const': lambda: self.motion_current('event_time', 'int'),
            'SPen::MotionEvent::GetDownTime() const': lambda: self.motion_current('down_time', 'int'),
            'SPen::MotionEvent::GetButtonState() const': lambda: self.x(0, 0),
            'SPen::MotionEvent::GetSource() const': lambda: self.x(0, 0),
            'SPen::MotionEvent::GetFlags() const': lambda: self.x(0, 0),
            'SPen::MotionEvent::SetTilt(float)': lambda: None,
            'SPen::MotionEvent::SetHistoricalTilt(int, float)': lambda: None,
            'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)': self.make_endpoint_event,
            'SPen::MotionEvent::~MotionEvent()': lambda: None,
            'SPen::PenGLDataManager::GetMsgQueue()': lambda: self.x(0, self.queue),
            'SPen::PenStrokeDrawableGL::PenStrokeDrawableGL()': lambda: None,
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)': lambda: None,
            'SPen::PenReturnCallback::SetDepthMapBitmap(SPen::ISPBitmap*)': lambda: None,
            'SPen::PenReturnCallback::GetBuffer()': lambda: self.x(0, self.vector),
            'SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)': lambda: None,
            'SPen::PenReturnCallback::~PenReturnCallback()': lambda: None,
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)': self.attach_buffer,
            'SPen::Error::SetError(unsigned long)': lambda: None,
            '__android_log_print': lambda: self.x(0, 0),
        })
        self.vector_values = []

    def seed_random(self):
        self.seed = self.x(0)
        self.random_index = 0
        if self.rng_profile == 'aosp-bionic-type3':
            self.bionic_rng.seed(self.seed)

    def next_random(self):
        if self.rng_profile == 'aosp-bionic-type3':
            value = self.bionic_rng.next()
            self.random_trace.append(value)
            return self.x(0, value)
        if self.random_index >= len(self.random_values):
            raise AssertionError('native code requested more random values than the fixture supplies')
        value = self.random_values[self.random_index]
        self.random_index += 1
        return self.x(0, value)

    def hook(self, u, address, size, data):
        if address in getattr(self, 'work_helper_entries', set()):
            # Work-item submission is an Android render-queue boundary. Supply
            # an explicit success handler so the synchronous saved geometry
            # path can continue; work-item execution itself remains unobserved.
            local = u.reg_read(R.UC_ARM64_REG_X0)
            self.put(local, 'Q', self.work_handler)
        if address == getattr(self, 'draw_line_entry', None):
            if self.current_line is not None:
                raise AssertionError('nested Pencil3 drawLine call')
            self.current_line = {'line_call_index': len(self.sample_boundaries),
                                 'start_float': self.vector_float_count(),
                                 'xy': [self.f(i) for i in range(4)],
                                 'bool_arg': bool(self.x(1)),
                                 'line_width': self.get(self.x(0) + 96, 'f')[0],
                                 'spacing': self.get(self.x(0) + 104, 'f')[0],
                                 'return_address': u.reg_read(R.UC_ARM64_REG_X30)}
        elif getattr(self, 'current_line', None) is not None and address == self.current_line['return_address']:
            if self.current_line is None:
                raise AssertionError('Pencil3 drawLine returned without a traced source index')
            self.current_line['end_float'] = self.vector_float_count()
            self.current_line.pop('return_address')
            self.sample_boundaries.append(self.current_line)
            self.current_line = None
        super().hook(u, address, size, data)

    def vector_float_count(self):
        begin, end, cap = self.get(self.vector, 'QQQ')
        if end < begin or end > cap or (end - begin) % 4:
            raise AssertionError('native Pencil3 vector bounds are invalid')
        return (end - begin) // 4

    def make_saved_event(self, obj, stroke):
        count = len(stroke['points'])
        point_bytes = [value for pt in stroke['points'] for value in pt]
        point_data = self.alloc(8 * count)
        pressure_data = self.alloc(4 * count)
        time_data = self.alloc(4 * count)
        # ObjectStroke exposes optional axes as null channels. Keep that
        # distinct from a present channel whose values happen to be zero.
        tilt_data = self.alloc(4 * count) if stroke['tilts'] else 0
        orientation_data = self.alloc(4 * count) if stroke['orientations'] else 0
        self.put(point_data, f'{2 * count}f', *point_bytes)
        self.put(pressure_data, f'{count}f', *stroke['pressures'])
        self.put(time_data, f'{count}i', *stroke['timestamps'])
        if tilt_data:
            self.put(tilt_data, f'{count}f', *stroke['tilts'])
        if orientation_data:
            self.put(orientation_data, f'{count}f', *stroke['orientations'])
        self.saved_stroke = stroke
        self.callbacks.update({
            'SPen::ObjectStroke::GetToolType() const': lambda: self.x(0, stroke['tool_type']),
            'SPen::ObjectStroke::GetPointCount() const': lambda: self.x(0, count),
            'SPen::ObjectStroke::GetPoint() const': lambda: self.x(0, point_data),
            'SPen::ObjectStroke::GetPressure() const': lambda: self.x(0, pressure_data),
            'SPen::ObjectStroke::GetTimeStamp() const': lambda: self.x(0, time_data),
            'SPen::ObjectStroke::GetTilt() const': lambda: self.x(0, tilt_data),
            'SPen::ObjectStroke::GetOrientation() const': lambda: self.x(0, orientation_data),
        })

    def render_saved(self, stroke):
        count = len(stroke['points'])
        if not count or any(len(stroke[key]) != count for key in ('pressures', 'timestamps')):
            raise ValueError('saved stroke requires aligned points, pressure and timestamps')
        if any(stroke[key] and len(stroke[key]) != count for key in ('tilts', 'orientations')):
            raise ValueError('optional saved channel length differs from point count')
        self.heap = 0x10000000
        # The ELF's .init_array includes 0x108a78, the lazy constructor for a
        # function-local static whose body stores the V1 line-width scale
        # (0.08) at Pencil3+0x1213d0. Run that pinned native initializer so
        # drawLine's native width guard sees the same initialized value as a
        # normally loaded DSO. The other .init_array entries are unrelated to
        # this path and are not executed by this bounded fixture.
        self.callbacks['__cxa_atexit'] = lambda: self.x(0, 0)
        self.syms['Pencil3::initialize-v1-line-width'] = 0x3000000 + 0x108a78
        self.call('Pencil3::initialize-v1-line-width')
        data, manager, drawable, rect, queue = (self.alloc(size) for size in (128, 64, 1024, 32, 32))
        settings = self.alloc(32)
        self.put(settings, 'f', stroke['size'])
        self.put(data, 'Q', settings)
        self.queue = queue
        self.work_handler = self.alloc(16)
        work_vtable = self.alloc(24)
        self.work_return = 0x20001000
        self.put(self.work_handler, 'Q', work_vtable)
        self.put(work_vtable + 16, 'Q', self.work_return)
        self.u.mem_write(self.work_return, (0xd2800020).to_bytes(4, 'little') + (0xd65f03c0).to_bytes(4, 'little'))  # mov x0, 1; ret
        self.call('SPen::Pencil3StrokeDrawableGLV1::Pencil3StrokeDrawableGLV1(SPen::Pencil3Data*, SPen::PenGLDataManager*)',
                  [drawable, data, manager])
        # PenStrokeDrawableGL's base constructor is a supplied host boundary;
        # install the saved redraw virtual slot after the native V1 constructor.
        vtable = self.alloc(160)
        self.put(vtable + 144, 'Q', self.syms['SPen::Pencil3StrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'])
        self.put(drawable, 'Q', vtable)
        self.put(drawable + 32, 'Q', self.alloc(64))
        rt = self.get(drawable + 16, 'Q')[0]
        self.vector = self.alloc(24)
        vector_capacity = 200000
        vector_storage = self.alloc(vector_capacity * 4)
        self.put(self.vector, 'QQQ', vector_storage, vector_storage, vector_storage + vector_capacity * 4)
        self.put(rt + 80, 'Q', self.vector)
        self.put(rect, '4f', 0., 0., 0., 0.)
        self.make_saved_event(self.alloc(512), stroke)
        event_entry = self.syms['SPen::Pencil3StrokeDrawableGLV1::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)']
        self.work_helper_entries = {event_entry + offset for offset in (0x620, 0x6b8, 0x768)}
        self.draw_line_entry = self.syms['SPen::Pencil3StrokeDrawableGLV1::drawLine(float, float, float, float, bool, SPen::RectF*)']
        self.sample_boundaries = []
        self.current_line = None
        self.seed = None
        self.random_trace = []
        self.rng_profile = 'aosp-bionic-type3'
        self.call(OBJECT_REDRAW, [drawable, self.alloc(512), rect], instruction_limit=40000000)
        if self.seed != 1:
            raise AssertionError('saved V1 redraw did not call srand(1)')
        float_count = self.vector_float_count()
        packed = list(self.get(vector_storage, f'{float_count}f')) if float_count else []
        if float_count % 5:
            raise AssertionError('RTV1 buffer output is not a five-float particle record sequence')
        return {'packed_attributes': packed, 'bounds': list(self.get(rect, '4f')),
                'sample_boundaries': self.sample_boundaries,
                'seed': self.seed, 'rand_values': self.random_trace,
                'particle_count': float_count // 5}

    def make_motion_event(self):
        ptr, tool, count, points, pressures = self.x(0), self.x(1), self.x(2), self.x(3), self.x(4)
        base_time, timestamps, tilts = self.x(5), self.x(6), self.x(7)
        orientations = self.x(0)  # overwritten below from stack argument 8
        sp = self.u.reg_read(__import__('unicorn').arm64_const.UC_ARM64_REG_SP)
        orientations = self.get(sp, 'Q')[0]
        self.events[ptr] = {
            'tool': tool, 'points': list(self.get(points, f'{2 * count}f')),
            'pressures': list(self.get(pressures, f'{count}f')),
            'times': list(self.get(timestamps, f'{count}i')) if timestamps else [base_time] * count,
            'tilts': list(self.get(tilts, f'{count}f')) if tilts else [0.0] * count,
            'orientations': list(self.get(orientations, f'{count}f')) if orientations else [0.0] * count,
        }

    def make_endpoint_event(self):
        """Register native endPen's one-pointer event from its source endpoint.

        V1 builds this temporary from the already-read final MotionEvent
        channels at 0x107f68-0x108040. The fixture uses those saved channels
        rather than inventing an Android MotionEvent ABI layout.
        """
        ptr = self.x(0)
        stroke = self.saved_stroke
        self.events[ptr] = {
            'tool': stroke['tool_type'],
            'points': list(stroke['points'][-1]),
            'pressures': [stroke['pressures'][-1]],
            'times': [stroke['timestamps'][-1]],
            'down_time': self.x(1),
            'tilts': [stroke['tilts'][-1] if stroke['tilts'] else 0.0],
            'orientations': [stroke['orientations'][-1] if stroke['orientations'] else 0.0],
        }

    def event(self):
        ptr = self.x(0)
        if ptr not in self.events:
            raise AssertionError('MotionEvent getter called for an unregistered event')
        return self.events[ptr]

    def motion_history_size(self):
        return self.x(0, len(self.event()['pressures']) - 1)

    def motion_historical(self, key):
        e = self.event()
        index = self.x(1)
        count = len(e['pressures'])
        if not 0 <= index < count - 1:
            raise AssertionError(f'historical {key} index {index} out of range')
        if key in ('x', 'y'):
            value = e['points'][2 * index + (key == 'y')]
        elif key == 'pressure':
            value = e['pressures'][index]
        else:
            value = e['tilts'][index]
        return self.d(0, value) if key in ('x', 'y') else self.f(0, value)

    def motion_current(self, key, kind):
        e = self.event()
        index = len(e['pressures']) - 1
        values = {
            'tool': e['tool'], 'x': e['points'][2 * index], 'y': e['points'][2 * index + 1],
            'pressure': e['pressures'][index], 'tilt': e['tilts'][index],
            'orientation': e['orientations'][index], 'event_time': e['times'][index],
            'down_time': e.get('down_time', e['times'][0]),
        }
        if kind == 'int':
            return self.x(0, values[key])
        if kind == 'double':
            return self.d(0, values[key])
        return self.f(0, values[key])

    def attach_buffer(self):
        rt = self.x(0)
        self.put(rt + 80, 'Q', self.x(1))

    def render_case(self, case):
        self.heap = 0x10000000
        drawable, rt, point, rect = (self.alloc(size) for size in (128, 128, 16, 32))
        vector = self.alloc(24)
        storage = self.alloc(5 * 4)
        # AddPoint appends five floats to the std::vector<float> at rt + 80.
        # Exact capacity avoids the allocator/growth branch and bounds writes.
        self.put(vector, 'QQQ', storage, storage, storage + 5 * 4)
        self.put(drawable + 16, 'Q', rt)
        self.put(rt + 80, 'Q', vector)
        self.put(point, '2f', *case['point'])
        self.put(rect, '4f', *case['initial_bounds'])
        self.random_values = list(case['rand_values'])
        self.rng_profile = case.get('rng_profile', 'fixture')
        if len(self.random_values) != 3 or any(not 0 <= n <= 0x7fffffff for n in self.random_values):
            raise ValueError('each case requires three explicit nonnegative C rand-range values')
        self.random_index = 0
        self.seed = None
        self.vector_values = []
        # drawPoint(this, SmPoint*, size, alpha, RectF*) -> x0, x1, s0, s1, x2
        self.call(DRAW_POINT, [drawable, point, rect], [case['size'], case['alpha']])
        if self.seed is not None:
            raise AssertionError('drawPoint unexpectedly reseeded RNG; fixture contract changed')
        if self.rng_profile == 'fixture' and self.random_index != len(self.random_values):
            raise AssertionError('fixture supplied unused random values')
        begin, end, capacity = self.get(vector, 'QQQ')
        if begin != storage or end - begin != 5 * 4 or capacity != storage + 5 * 4:
            raise AssertionError('native RTV1 AddPoint did not append exactly five bounded floats')
        packed = list(self.get(begin, '5f'))
        bounds = list(self.get(rect, '4f'))
        # The native AddPoint appends x, y, half-size, signed row, alpha.
        return {'packed_attributes': packed, 'bounds': bounds,
                'vector_length': (end - begin) // 4, 'rand_calls': self.random_index}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    parser.add_argument('--fixture', type=Path, default=Path('conformance/pencil-v1-native.json'))
    parser.add_argument('--write-fixture', action='store_true', help='execute and replace stored native outputs')
    args = parser.parse_args()

    fixture = json.loads(args.fixture.read_text())
    results = []
    for case in fixture['cases']:
        # Static TLS, RNG hook state and native object memory are isolated per case.
        native = NativePencil3(args.library_dir, args.objdump)
        results.append({'name': case['name'], **native.render_case(case)})
    saved_results = []
    for case in fixture.get('saved_cases', []):
        native = NativePencil3(args.library_dir, args.objdump)
        saved_results.append({'name': case['name'], **native.render_saved(case['stroke'])})
    if args.write_fixture:
        fixture['native_outputs'] = results
        if 'saved_cases' in fixture:
            fixture['saved_native_outputs'] = saved_results
        args.fixture.write_text(json.dumps(fixture, indent=2) + '\n')
        print(f'wrote {len(results)} direct and {len(saved_results)} saved native cases to {args.fixture}')
        return
    expected = fixture.get('native_outputs')
    if expected is None:
        raise SystemExit('fixture has no native_outputs; run with --write-fixture')
    if results != expected:
        raise AssertionError('Pencil3 native callback output differs from fixture')
    saved_expected = fixture.get('saved_native_outputs')
    if 'saved_cases' in fixture and saved_results != saved_expected:
        raise AssertionError('Pencil3 native saved-redraw output differs from fixture')
    if BionicRandomVector() != [55436735, 1399865117, 2032643283, 571329216]:
        raise AssertionError('Bionic TYPE_3 model differs from Android 10 CTS vector')
    print(f'{len(results)} direct and {len(saved_results)} saved Pencil3 V1 native cases match; '
          f'{sum(x["rand_calls"] for x in results)} injected RNG values; '
          f'{sum(len(x["rand_values"]) for x in saved_results)} modeled bionic RNG values')


def BionicRandomVector():
    rng = BionicRandom()
    rng.seed(0x01020304)
    return [rng.next() for _ in range(4)]


if __name__ == '__main__':
    main()
