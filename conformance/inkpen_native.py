"""Execute the selected saved InkPen V4 curve redraw against the pinned APK.

This follows the InkPen V4 ObjectStroke::RedrawPen path and actual Base SmPath.
The host supplies ObjectStroke channel access and the render-thread callback
container; the actual native RTV1 AddPoint method writes its (x, y, radius + 0.5)
triples into a preallocated host vector. This is geometry evidence, not GPU pixels.
"""
import argparse
import hashlib
import json
import math
import shutil
import struct
from pathlib import Path

import fountain_native
from fountain_native import NativeFountain

HASHES = {
    'InkPen': '939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401',
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
}
REDRAW = 'SPen::InkPenStrokeDrawableGLV4::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)'
ADD_POINT = 'SPen::InkPenStrokeDrawableRTV1::AddPoint(SPen::Vector2<float>, float)'
SET_RESOLUTION = 'SPen::Pen::SetScreenResolution(int, int)'
GET_SETTING_DATA = 'SPen::Pen::getSettingData()'
SET_SIZE = 'SPen::Pen::SetSize(float)'
SET_CURVE = 'SPen::InkPen::SetCurveEnabled(bool)'
SET_FIXED_WIDTH_ENABLED = 'SPen::InkPen::SetFixedWidthEnabled(bool)'
SET_FIXED_WIDTH = 'SPen::InkPen::SetFixedWidth(float)'


class NativeInkPen(NativeFountain):
    def __init__(self, library_dir, objdump):
        self.dots = []
        fountain_native.HASHES['InkPen'] = HASHES['InkPen']
        super().__init__(library_dir, objdump, ('InkPen',))
        for name, expected in HASHES.items():
            actual = hashlib.sha256((library_dir / ('libSPen' + name + '.so')).read_bytes()).hexdigest()
            if actual != expected:
                raise ValueError('unverified library: libSPen' + name + '.so')
        self.add_point_address = self.syms[ADD_POINT]
        self.redraw_address = self.syms[REDRAW]
        self.orientation_address = self.syms['SPen::MotionEvent::GetOrientation(int) const']
        self.callbacks.update({
            'pthread_mutexattr_init': lambda: self.x(0, 0),
            'pthread_mutexattr_settype': lambda: self.x(0, 0),
            'pthread_mutexattr_destroy': lambda: self.x(0, 0),
            'pthread_mutex_init': lambda: self.x(0, 0),
            'pthread_mutex_lock': lambda: self.x(0, 0),
            'pthread_mutex_unlock': lambda: self.x(0, 0),
            'pthread_mutex_destroy': lambda: self.x(0, 0),
            'operator new(unsigned long, std::nothrow_t const&)': lambda: self.x(0, self.alloc(self.x(0))),
            '__cxa_atexit': lambda: self.x(0, 0),
            '__cxa_guard_acquire': lambda: self.x(0, 1),
            '__cxa_guard_release': lambda: None,
            '__cxa_guard_abort': lambda: None,
            'SPen::PenStrokeDataManager::GetStrokeData(int)': lambda: self.x(0, 0),
            'SPen::ObjectBase::ClearRedrawingCacheDirtyFlag()': lambda: None,
            'SPen::ObjectBase::SetOriginalRuntimeHandle(int)': lambda: None,
            'SPen::PenGLDataManager::GetMsgQueue()': lambda: self.x(0, self.queue),
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)': lambda: None,
            'SPen::PenReturnCallback::GetBuffer()': lambda: self.x(0, self.vector),
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)': self.set_buffer,
            'SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)': lambda: None,
            'SPen::PenReturnCallback::~PenReturnCallback()': lambda: None,
            'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)': lambda: None,
            'SPen::MotionEvent::~MotionEvent()': lambda: None,
        })
        for name, key in [('GetX(int) const', 'x'), ('GetY(int) const', 'y')]:
            self.callbacks['SPen::MotionEvent::' + name] = lambda k=key: self.d(0, self.end[k])
        for name, key in [('GetToolType(int) const', 'tool'), ('GetSource() const', 'source'), ('GetEventTime() const', 'time')]:
            self.callbacks['SPen::MotionEvent::' + name] = lambda k=key: self.x(0, self.end[k])
        for name, key in [('GetPressure(int) const', 'pressure'), ('GetTilt(int) const', 'tilt')]:
            self.callbacks['SPen::MotionEvent::' + name] = lambda k=key: self.f(0, self.end[k])

    def hook(self, u, address, size, data):
        # RedrawPen's loop sets x20 to the number of saved transitions
        # remaining and x25 to the current timestamp. 0x31090 calls
        # reDrawLine and 0x31094 is its return boundary. These hooks capture
        # native vector lengths at executed instruction boundaries.
        loop = self.redraw_address + 0x290
        line_call = self.redraw_address + 0x2d0
        line_return = self.redraw_address + 0x2d4
        terminal_call = self.redraw_address + 0x3a4
        terminal_return = self.redraw_address + 0x3a8
        no_curve_loop = self.redraw_address + 0x844
        no_curve_line_return = self.redraw_address + 0x88c
        no_curve_terminal_call = self.redraw_address + 0x93c
        no_curve_terminal_return = self.redraw_address + 0x940
        from unicorn import arm64_const as R
        if address == self.orientation_address:
            self.motion_event_get_orientation_calls += 1
        if self.curve_enabled and address == loop:
            remaining = u.reg_read(R.UC_ARM64_REG_X20)
            index = len(self.timestamps) - remaining
            timestamp_ptr = u.reg_read(R.UC_ARM64_REG_X25)
            native_timestamp = int.from_bytes(u.mem_read(timestamp_ptr, 4), 'little', signed=True)
            if index < 1 or index >= len(self.timestamps) or native_timestamp != self.timestamps[index]:
                raise AssertionError('RedrawPen loop sample index/timestamp did not match the fixture')
            if index != self.next_sample:
                raise AssertionError('RedrawPen loop sample order changed')
            if self.sample_ends[0] is None:
                self.sample_ends[0] = self.vector_float_count()
            self.pending_sample = index
        elif self.curve_enabled and address == line_call:
            if self.pending_sample is None:
                raise AssertionError('reDrawLine called without a traced saved sample')
            index = self.pending_sample
            actual_timestamp = u.reg_read(R.UC_ARM64_REG_X1)
            actual_timestamp = actual_timestamp - (1 << 64) if actual_timestamp & (1 << 63) else actual_timestamp
            if actual_timestamp != self.timestamps[index]:
                raise AssertionError('reDrawLine timestamp argument did not match traced saved sample')
        elif self.curve_enabled and address == line_return:
            if self.pending_sample is None:
                raise AssertionError('reDrawLine returned without a traced saved sample')
            self.sample_ends[self.pending_sample] = self.vector_float_count()
            self.next_sample += 1
            self.pending_sample = None
        elif self.curve_enabled and address == terminal_call:
            if self.pending_sample is not None or self.next_sample != len(self.timestamps):
                raise AssertionError('terminal endPen reached before all saved transitions returned')
            count = self.vector_float_count()
            self.terminal_start = count
            if len(self.sample_ends) == 1:
                self.sample_ends[0] = count
        elif self.curve_enabled and address == terminal_return:
            self.terminal_end = self.vector_float_count()
        elif not self.curve_enabled and address == no_curve_loop:
            remaining = u.reg_read(R.UC_ARM64_REG_X27)
            index = len(self.timestamps) - remaining
            if index < 1 or index >= len(self.timestamps) or index != self.next_sample:
                raise AssertionError('no-curve loop sample index/order differs from saved channels')
            if self.sample_ends[0] is None:
                self.sample_ends[0] = self.vector_float_count()
            self.pending_sample = index
        elif not self.curve_enabled and address == no_curve_line_return:
            if self.pending_sample is None:
                raise AssertionError('no-curve drawLine_NoCurve returned without a traced sample')
            self.sample_ends[self.pending_sample] = self.vector_float_count()
            self.next_sample += 1
            self.pending_sample = None
        elif not self.curve_enabled and address == no_curve_terminal_call:
            if self.pending_sample is not None or self.next_sample != len(self.timestamps):
                raise AssertionError('no-curve endPen_NoCurve reached before saved samples returned')
            self.terminal_start = self.vector_float_count()
            if len(self.sample_ends) == 1:
                self.sample_ends[0] = self.terminal_start
        elif not self.curve_enabled and address == no_curve_terminal_return:
            self.terminal_end = self.vector_float_count()
        if address == getattr(self, 'queue_submit_address', None):
            self.x(0, 1)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        super().hook(u, address, size, data)

    def set_buffer(self):
        rt, vector = self.x(0), self.x(1)
        self.u.mem_write(rt + 80, vector.to_bytes(8, 'little'))

    def vector_float_count(self):
        start, end, capacity = self.get(self.vector, 'QQQ')
        if end < start or end > capacity or (end - start) % 4:
            raise AssertionError('native vector bounds are invalid at sample boundary')
        return (end - start) // 4


    def render(self, stroke, screen_resolution, curve_enabled):
        r = stroke['rendering']
        pts = stroke['points']
        n = len(pts)
        if r['pen_name'] != 'com.samsung.android.sdk.pen.pen.preload.InkPen' or r['advanced_settings'] != '4;':
            raise ValueError('oracle requires explicit InkPen V4 saved settings 4;')
        if r['tool_type_raw'] not in (1, 2, 3) or not r['properties'].get('stylus_channels', False):
            raise ValueError('oracle requires traced saved tool type 1, 2 or 3')
        if not n or any(len(stroke[k]) != n for k in ('pressures', 'timestamps')):
            raise ValueError('point, pressure and timestamp arrays must be complete')
        if any(stroke[k] and len(stroke[k]) != n for k in ('tilts', 'orientations')):
            raise ValueError('optional arrays must be complete when present')
        fixed_width_enabled = r['properties'].get('fixed_width', False)
        fixed_width_value = r.get('style', {}).get('fixed_width')
        if not isinstance(fixed_width_enabled, bool):
            raise ValueError('fixed_width enabled state must be explicit boolean')
        if fixed_width_enabled and fixed_width_value is None:
            raise ValueError('enabled fixed width requires a saved fixed_width style value')
        if fixed_width_value is None:
            fixed_width_value = 0.0
        fixed_width_value = float(struct.unpack('<f', struct.pack('<f', float(fixed_width_value)))[0])
        if (not math.isfinite(float(fixed_width_value))
                or not math.isfinite(float(stroke['pen_width']))):
            raise ValueError('pen size and fixed width must be finite')
        if (not isinstance(screen_resolution, list) or len(screen_resolution) != 2
                or any(not isinstance(v, int) or v < 0 or v > 0x7fffffff for v in screen_resolution)):
            raise ValueError('explicit nonnegative signed-int screen_resolution [width, height] required')
        if curve_enabled is not True and curve_enabled is not False:
            raise ValueError('explicit curve_enabled boolean is required')
        self.curve_enabled = curve_enabled

        self.heap = 0x10000000
        self.dots = []
        self.motion_event_get_orientation_calls = 0
        self.timestamps = [int(value) for value in stroke['timestamps']]
        self.sample_ends = [None] * n
        self.next_sample = 1
        self.pending_sample = None
        self.terminal_start = None
        self.terminal_end = None
        drawable, rt, manager, canvas = (self.alloc(512) for _ in range(4))
        data, vector = self.alloc(128), self.alloc(24)
        # Pen itself is host-allocated/zeroed, but the resolution setter and
        # settings accessor are the pinned native PenCommon implementations.
        pen = self.alloc(128)
        self.put(pen + 120, 'Q', data)
        self.call(SET_RESOLUTION, [pen, screen_resolution[0], screen_resolution[1]])
        self.call(SET_SIZE, [pen], [stroke['pen_width']])
        self.call(GET_SETTING_DATA, [pen])
        settings = self.x(0)
        native_resolution = list(self.get(settings + 32, 'ii')) if settings == pen + 24 else None
        if settings != pen + 24 or native_resolution != screen_resolution:
            raise AssertionError('native Pen resolution setter/accessor did not populate setting data')
        native_size = self.get(settings, 'f')[0]
        self.call(SET_CURVE, [pen, int(curve_enabled)])
        self.call(SET_FIXED_WIDTH_ENABLED, [pen, int(fixed_width_enabled)])
        self.call(SET_FIXED_WIDTH, [pen], [float(fixed_width_value)])
        if (self.get(data + 8, 'B')[0] != int(curve_enabled)
                or self.get(data + 9, 'B')[0] != int(fixed_width_enabled)
                or self.get(data + 12, 'f')[0] != fixed_width_value):
            raise AssertionError('native InkPen setters did not populate InkPenData state')
        queue = self.alloc(16)
        self.queue, self.vector, self.rt = queue, vector, rt
        point_storage = self.alloc(4096 * 4)
        self.put(vector, 'QQQ', point_storage, point_storage, point_storage + 4096 * 4)
        self.queue_submit_address = 0x20001000
        queue_vtable = self.alloc(24)
        self.put(queue, 'Q', queue_vtable)
        self.put(queue_vtable + 16, 'Q', self.queue_submit_address)
        # The native redraw reads runtime object pointers at drawable +16/+24/+32,
        # InkPenData at +72, data.settings at +0 and curve at byte +8.
        self.put(drawable + 16, 'Q', rt)
        self.put(drawable + 24, 'Q', manager)
        self.put(drawable + 32, 'Q', canvas)
        self.put(drawable + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(data + 8, 'B', int(curve_enabled))
        # The native redraw reaches RTV1 through virtual slot +64 bytes.
        vtable = self.alloc(9 * 8)
        self.put(rt, 'Q', vtable)
        self.put(vtable + 64, 'Q', self.add_point_address)
        self.call('SPen::SmPath::SmPath()', [drawable + 112])

        self.channel('GetPoint', 'f', [v for p in pts for v in (p['x'], p['y'])])
        self.channel('GetPressure', 'f', stroke['pressures'])
        self.channel('GetTimeStamp', 'i', stroke['timestamps'])
        self.channel('GetTilt', 'f', stroke['tilts'])
        self.channel('GetOrientation', 'f', stroke['orientations'])
        self.callbacks['SPen::ObjectStroke::GetPointCount() const'] = lambda: self.x(0, n)
        self.callbacks['SPen::ObjectStroke::GetToolType() const'] = lambda: self.x(0, r['tool_type_raw'])
        self.callbacks['SPen::ObjectStroke::IsShape() const'] = lambda: self.x(0, 0)
        self.callbacks['SPen::ObjectBase::GetOriginalRuntimeHandle() const'] = lambda: self.x(0, -1)
        self.callbacks['SPen::ObjectBase::GetRuntimeHandle() const'] = lambda: self.x(0, 0)
        self.end = {**pts[-1], 'pressure': stroke['pressures'][-1],
                    'tilt': stroke['tilts'][-1] if stroke['tilts'] else 0.,
                    'time': int(stroke['timestamps'][-1]) - int(stroke['timestamps'][0]),
                    'tool': r['tool_type_raw'], 'source': 0}
        rect = self.alloc(32)
        self.call(REDRAW, [drawable, self.alloc(512), rect], instruction_limit=20000000)
        result = self.x(0) & 0xffffffff
        bounds = list(self.get(rect, '4f'))
        start, end, capacity = self.get(vector, 'QQQ')
        if end > capacity:
            raise AssertionError('native RTV1 output exceeded the bounded host vector capacity')
        values = self.get(start, f'{(end - start) // 4}f') if end > start else ()
        if len(values) % 3:
            raise AssertionError('native RTV1 buffer is not a sequence of triples')
        triples = [list(values[i:i + 3]) for i in range(0, len(values), 3)]
        if any(value is None for value in self.sample_ends):
            raise AssertionError('did not capture every saved sample boundary')
        if self.terminal_start is None or self.terminal_end != (end - start) // 4:
            raise AssertionError('did not capture terminal vector boundaries')
        output = {'callback_triples': triples, 'bounds': bounds, 'redraw_result': result,
                'motion_event_get_orientation_calls': self.motion_event_get_orientation_calls,
                'screen_resolution': screen_resolution,
                'setting_data_screen_resolution': native_resolution,
                'requested_pen_size': float(stroke['pen_width']),
                'setting_data_pen_size': native_size,
                'fixed_width_enabled': fixed_width_enabled,
                'fixed_width_value': float(fixed_width_value),
                'configured_min_dimension': min(native_resolution)}
        output.update(sample_ends=self.sample_ends, terminal_start=self.terminal_start,
                      terminal_end=self.terminal_end)
        output['sample_mapping'] = (
            'curve path: reDrawLine return boundaries at RedrawPen+0x2d4 and terminal endPen at +0x3a8'
            if curve_enabled else
            'no-curve path: drawLine_NoCurve return boundaries at RedrawPen+0x88c and terminal endPen_NoCurve at +0x940'
        )
        return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=Path('scratch/apk-analysis-native/arm64-v8a'))
    parser.add_argument('--objdump', type=Path, default=Path(shutil.which('aarch64-linux-gnu-objdump') or 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true', help='execute and replace stored native result records')
    mode.add_argument('--check-fixture', action='store_true', help='execute and compare with stored results (default)')
    parser.add_argument('--fixture', type=Path, default=Path('conformance/inkpen-v4.json'))
    args = parser.parse_args()
    cases = json.loads(args.fixture.read_text())
    failures = []
    for case in cases['cases']:
        native = NativeInkPen(args.library_dir, args.objdump)
        stroke = json.loads(json.dumps(cases['stroke']))
        stroke.update(case['stroke'])
        if 'tool_type_raw' in case:
            stroke['rendering']['tool_type_raw'] = case['tool_type_raw']
        if 'screen_resolution' not in case:
            failures.append(case['name'] + ': explicit screen_resolution is required')
            continue
        if 'curve_enabled' not in case:
            failures.append(case['name'] + ': explicit curve_enabled is required')
            continue
        result = native.render(stroke, case['screen_resolution'], case['curve_enabled'])
        if args.write_fixture:
            case['native'] = result
        else:
            expected = case.get('native')
            if expected is None:
                failures.append(case['name'] + ': no recorded native output; run with --write-fixture')
            elif result != expected:
                failures.append(case['name'] + ': native output differs from recorded output')
        print(f"{case['name']}: {len(result['callback_triples'])} triples; bounds={result['bounds']}; redraw={result['redraw_result']}")
    if args.write_fixture:
        args.fixture.write_text(json.dumps(cases, indent=2) + '\n')
    if failures:
        raise SystemExit('\n'.join(failures))


if __name__ == '__main__':
    main()
