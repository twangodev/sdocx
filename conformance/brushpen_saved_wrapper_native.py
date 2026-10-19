"""Hash-pinned BrushPen saved ObjectStroke redraw wrapper oracle.

Runs the real ARM64 Samsung Notes BrushPen and Model code under Unicorn. The
ObjectStroke instance and channel getters are native; point/channel values are
explicit synthetic rows. The wrapper's MotionEvent constructor, channel getters,
and destructor execute in the pinned Base library; their arguments, pointed-to
records, decoded channels, and native getter returns are captured. No
texture upload, shader, GPU submission, or pixels are executed.
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path
from unicorn import UC_HOOK_CODE

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_saved_native as core

EXPECTED = {
    'FountainPen': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
    'BrushPen': 'acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce',
    'Graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
    'Renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
    'Model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
}
OUT = ROOT / 'conformance/brushpen-saved-wrapper-native.json'

CASES = {
    'name': 'synthetic-saved-object-brushpen-tool2',
    'tool_type': 2,
    'points': [[1, -2], [3, 1], [6, 4]],
    'pressures': [0.3, 0.6, 0.85],
    'times': [300, 315, 340],
    'tilts': [0.12, 0.32, 0.55],
    'orientations': [0.2, 0.35, -0.42],
}
STYLE_CASES = [(size, fixed) for size in (3.144, 20.0) for fixed in (False, True)]


class TracedBrush(core.NativeBrush):
    """Capture native MotionEvent construction/access without replacing it."""
    def __init__(self):
        self.motion_event_constructors = []
        self.motion_event_getters = []
        self.pending_motion_event_getters = {}
        self.draw_pattern_calls = []
        self.make_draw_point_calls = []
        self.draw_pattern_style_steps = []
        self.traced_version = None
        super().__init__()
        # GLV10's drawPattern computes its active radius in S14 and loads the
        # first source SmPoint immediately before this PC.
        self.draw_pattern_style_pc = None
        self.u.hook_add(UC_HOOK_CODE, self.capture_draw_pattern_style_step)

    def capture_draw_pattern_style_step(self, u, address, size, data):
        if self.draw_pattern_style_pc is None or address != self.draw_pattern_style_pc:
            return
        raw_s14 = u.reg_read(core.R.UC_ARM64_REG_S14)
        self.draw_pattern_style_steps.append({
            'pc': hex(address),
            'drawable': hex(self.x(19)),
            'settings': hex(self.get(self.x(19) + 72, 'Q')[0]),
            'fixed_width_enabled_byte_offset_89': self.get(
                self.get(self.x(19) + 72, 'Q')[0] + 89, 'B')[0],
            'fixed_width_f32_offset_92': self.get(
                self.get(self.x(19) + 72, 'Q')[0] + 92, 'f')[0],
            'working_opacity_w20_u32': self.x(20) & 0xffffffff,
            'working_radius_s14_f32': struct.unpack('<f', struct.pack('<I', raw_s14 & 0xffffffff))[0],
            'source_point_xy_s0_s1_f32': [self.f(0), self.f(1)],
        })

    def hook(self, u, address, size, data):
        ctor = self.syms.get(
            'SPen::MotionEvent::MotionEvent(long long, int, int, '
            'SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)')
        if address == ctor:
            sp = u.reg_read(core.R.UC_ARM64_REG_SP)
            properties = self.x(4)
            coords = self.x(5)
            props_raw = bytes(u.mem_read(properties, 16))
            coords_raw = bytes(u.mem_read(coords, 72))
            self.motion_event_constructors.append({
                'arguments_x0_x7': [hex(self.x(i)) for i in range(8)],
                'final_stack_argument_i32': self.get(sp, 'i')[0],
                'arguments_decoded': {
                    'down_time_ms_i64': self.x(1),
                    'action_i32': self.x(2),
                    'pointer_count_i32': self.x(3),
                    'meta_state_i32': self.x(6),
                    'button_state_i32': self.x(7),
                    'last_stack_i32_argument': self.get(sp, 'i')[0],
                },
                'pointer_properties_address': hex(properties),
                'pointer_properties_first_16_bytes': props_raw.hex(),
                'pointer_properties_decoded': {
                    'pointer_id_i32': struct.unpack_from('<i', props_raw, 0)[0],
                    'tool_type_i32': struct.unpack_from('<i', props_raw, 4)[0],
                },
                'pointer_coords_address': hex(coords),
                'pointer_coords_first_72_bytes': coords_raw.hex(),
                'pointer_coords_decoded_from_Base_getter_offsets': {
                    'event_time_i64_ms': struct.unpack_from('<q', coords_raw, 0)[0],
                    'orientation_f32': struct.unpack_from('<f', coords_raw, 16)[0],
                    'pressure_f32': struct.unpack_from('<f', coords_raw, 20)[0],
                    'tilt_f32': struct.unpack_from('<f', coords_raw, 24)[0],
                    'x_f64': struct.unpack_from('<d', coords_raw, 40)[0],
                    'y_f64': struct.unpack_from('<d', coords_raw, 48)[0],
                    'raw_x_f64': struct.unpack_from('<d', coords_raw, 56)[0],
                    'raw_y_f64': struct.unpack_from('<d', coords_raw, 64)[0],
                },
            })
        version = self.traced_version
        if version is not None:
            pattern_name = (
                f'SPen::BrushPenStrokeDrawableGLV{version}::drawPattern('
                'SPen::SmPoint const*, int, float, SPen::SmPoint*, SPen::RectF*, bool)')
            draw_point_name = (
                f'SPen::BrushPenStrokeDrawableGLV{version}::makeDrawPoint('
                'float, float, float, float, float, long long, SPen::RectF*, bool)')
            if address == self.syms.get(pattern_name):
                drawable, point_ptr, opacity = self.x(0), self.x(1), self.x(2)
                settings = self.get(drawable + 72, 'Q')[0]
                # The input is one eight-byte SmPoint, not an array sized by
                # x2: native drawPattern consumes x2 as integer opacity.
                raw_points = bytes(u.mem_read(point_ptr, 8))
                self.draw_pattern_calls.append({
                    'native_method': pattern_name,
                    'input_SmPoint_ptr': hex(point_ptr),
                    'input_opacity_u32': opacity & 0xffffffff,
                    'input_point_bytes_le': raw_points.hex(),
                    'input_point_xy_f32': list(struct.unpack('<2f', raw_points)),
                    'radius_argument_s0_f32': self.f(0),
                    'output_SmPoint_ptr': hex(self.x(3)),
                    'rect_ptr': hex(self.x(4)),
                    'boolean_argument_w5': self.x(5) & 0xffffffff,
                    'drawable_pen_settings': {
                        'ptr': hex(settings),
                        'fixed_width_enabled_byte_offset_89': self.get(settings + 89, 'B')[0],
                        'fixed_width_f32_offset_92': self.get(settings + 92, 'f')[0],
                    },
                })
            elif address == self.syms.get(draw_point_name):
                drawable = self.x(0)
                settings = self.get(drawable + 72, 'Q')[0]
                rect = self.x(2)
                self.make_draw_point_calls.append({
                    'native_method': draw_point_name,
                    'x0_this': hex(drawable),
                    'float_arguments_s0_s4_f32': [self.f(i) for i in range(5)],
                    'float_arguments_s0_s4_bits_le': [core.fbits(self.f(i)) for i in range(5)],
                    'timestamp_x1_i64': self.x(1),
                    'rect_x2': hex(rect),
                    'rect_f32': list(self.get(rect, '4f')) if rect else None,
                    'boolean_x3': self.x(3) & 0xffffffff,
                    'drawable_pen_settings': {
                        'ptr': hex(settings),
                        'fixed_width_enabled_byte_offset_89': self.get(settings + 89, 'B')[0],
                        'fixed_width_f32_offset_92': self.get(settings + 92, 'f')[0],
                    },
                })
        for suffix in ('GetEventTime() const', 'GetX(int) const', 'GetY(int) const',
                       'GetPressure(int) const', 'GetTilt(int) const',
                       'GetOrientation(int) const', 'GetToolType(int) const'):
            name = 'SPen::MotionEvent::' + suffix
            if address == self.syms.get(name):
                self.pending_motion_event_getters.setdefault(u.reg_read(core.R.UC_ARM64_REG_LR), []).append(
                    {'name': name, 'this': hex(self.x(0)), 'index': self.x(1) if suffix.endswith('(int) const') else None})
        if address in self.pending_motion_event_getters:
            for call in self.pending_motion_event_getters.pop(address):
                if call['name'].endswith(('GetX(int) const', 'GetY(int) const')):
                    raw = u.reg_read(core.R.UC_ARM64_REG_D0)
                    call['return_d0_f64_bits_le'] = struct.pack('<Q', raw).hex()
                    call['return_d0_f64'] = struct.unpack('<d', struct.pack('<Q', raw))[0]
                elif call['name'].endswith('GetEventTime() const') or call['name'].endswith('GetToolType(int) const'):
                    call['return_x0'] = self.x(0)
                else:
                    call['return_s0_f32_bits_le'] = struct.pack('<f', self.f(0)).hex()
                    call['return_s0_f32'] = self.f(0)
                self.motion_event_getters.append(call)
        super().hook(u, address, size, data)


def setup_pen(n, size, fixed_width_enabled, curve_enabled=True, verify_fixed_opacity_contract=False):
    pen, manager = n.alloc(256), n.alloc(128)
    queue, qvt = n.alloc(32), n.alloc(32)
    n.queue = queue
    n.put(queue, 'Q', qvt)
    n.put(qvt + 16, 'Q', 0x20001000)
    n.put(manager + 56, 'Q', queue)
    n.put(pen + 8, 'Q', manager)
    n.call(core.CTOR, [pen], instruction_limit=3_000_000)
    constructor_inputs = list(n.setting_inputs)
    n.call('SPen::Pen::SetSize(float)', [pen], [size])
    n.call('SPen::BrushPen::SetFixedWidth(float)', [pen], [size])
    n.call('SPen::BrushPen::SetFixedWidthEnabled(bool)', [pen, int(fixed_width_enabled)])
    n.call('SPen::BrushPen::SetCurveEnabled(bool)', [pen, int(curve_enabled)])
    n.call(core.GET_SIZE, [pen])
    actual_size = n.f(0)
    n.call('SPen::BrushPen::GetFixedWidth()', [pen])
    actual_fixed_width = n.f(0)
    setting_data = n.get(pen + 144, 'Q')[0]
    fixed_width_enabled_native = n.get(setting_data + 89, 'B')[0]
    fixed_opacity_contract = None
    if verify_fixed_opacity_contract:
        n.call('SPen::BrushPen::GetMorphable()', [pen])
        morphable = n.x(0)
        if morphable != pen + 96:
            raise AssertionError(f'BrushPen GetMorphable returned {morphable:#x}, expected {pen + 96:#x}')
        morphable_vptr = n.get(morphable, 'Q')[0]
        setter_target = n.get(morphable_vptr + 8 * 8, 'Q')[0]
        getter_target = n.get(morphable_vptr + 9 * 8, 'Q')[0]
        expected_setter = n.syms['SPen::IPenMorphable::SetFixedOpacityEnabled(bool)']
        expected_getter = n.syms['SPen::IPenMorphable::IsFixedOpacityEnabled()']
        brush_base = 3 * 0x1000000
        if (setter_target != expected_setter or getter_target != expected_getter
                or setter_target - brush_base != 0x566e0
                or getter_target - brush_base != 0x566e8):
            raise AssertionError('BrushPen morphable vtable does not select the pinned no-op targets')
        n.syms['__probe_virtual_fixed_opacity_setter'] = setter_target
        n.syms['__probe_virtual_fixed_opacity_getter'] = getter_target
        pen_before = bytes(n.u.mem_read(pen, 256))
        settings_before = bytes(n.u.mem_read(setting_data, 96))
        observed = []
        for enabled in (False, True):
            n.call('__probe_virtual_fixed_opacity_setter', [morphable, int(enabled)])
            setter_result = n.x(0) & 0xffffffff
            n.call('__probe_virtual_fixed_opacity_getter', [morphable])
            getter_result = n.x(0) & 0xffffffff
            pen_unchanged = bytes(n.u.mem_read(pen, 256)) == pen_before
            settings_unchanged = bytes(n.u.mem_read(setting_data, 96)) == settings_before
            if (setter_result != 1 or getter_result != 0
                    or not pen_unchanged or not settings_unchanged):
                raise AssertionError('native BrushPen fixed-opacity virtual pair changed state')
            observed.append({
                'setter_input': enabled,
                'setter_return_w0_u32': setter_result,
                'getter_return_w0_u32': getter_result,
                'pen_256_bytes_unchanged': pen_unchanged,
                'settings_96_bytes_unchanged': settings_unchanged,
            })
        fixed_opacity_contract = {
            'GetMorphable_result_is_pen_plus_96': True,
            'morphable_vptr': hex(morphable_vptr),
            'vtable_slot_8_target': hex(setter_target),
            'vtable_slot_8_brush_elf_offset': '0x566e0',
            'vtable_slot_9_target': hex(getter_target),
            'vtable_slot_9_brush_elf_offset': '0x566e8',
            'setter_symbol_target_match': True,
            'getter_symbol_target_match': True,
            'native_calls': observed,
        }
    n.call(core.GET_VERSION, [pen])
    version = n.x(0)
    n.traced_version = version
    n.put(pen + 8, 'Q', manager)
    n.call(core.GET_DRAWABLE, [pen], instruction_limit=3_000_000)
    drawable = n.x(0)

    n.end_pen_plt = next(a for a, name in n.plt.items()
                         if name == core.END_PEN.format(version=version))
    n.end_pen_address = n.syms[core.END_PEN.format(version=version)]
    n.draw_pattern_address = n.syms[
        f'SPen::BrushPenStrokeDrawableGLV{version}::drawPattern('
        'SPen::SmPoint const*, int, float, SPen::SmPoint*, SPen::RectF*, bool)']
    n.draw_pattern_style_pc = n.draw_pattern_address + 0x80

    vptr = n.get(drawable, 'Q')[0]
    expected_vptr = n.syms[f'vtable for SPen::BrushPenStrokeDrawableGLV{version}'] + 16
    rt = n.get(drawable + 16, 'Q')[0]
    rt_vptr = n.get(rt, 'Q')[0]
    expected_rt_vptr = n.syms['vtable for SPen::BrushPenStrokeDrawableRTV1'] + 16
    if vptr != expected_vptr or rt_vptr != expected_rt_vptr:
        raise AssertionError('native BrushPen drawable admission/vtable mismatch')

    vector = n.alloc(24)
    storage = n.alloc(8192 * 4)
    n.put(vector, 'QQQ', storage, storage, storage + 8192 * 4)
    n.vector, n.vector_addresses = vector, {vector}
    n.put(rt + 80, 'Q', vector)
    canvas, cvt = n.alloc(128), n.alloc(256)
    n.put(canvas, 'Q', cvt)
    n.put(cvt + 216, 'Q', n.matrix_stub)
    n.put(cvt + 184, 'Q', n.matrix_stub)
    n.put(cvt + 16, 'Q', 0x20001000)
    n.put(drawable + 32, 'Q', canvas)
    return {
        'pen': pen, 'manager': manager, 'drawable': drawable, 'rt': rt,
        'vector': vector, 'version': version, 'size': actual_size,
        'fixed_width': actual_fixed_width, 'fixed_width_enabled_input': fixed_width_enabled,
        'fixed_width_enabled_native': fixed_width_enabled_native,
        'fixed_opacity_contract': fixed_opacity_contract,
        'setting_data': setting_data,
        'constructor_inputs': constructor_inputs,
        'vptr': vptr, 'expected_vptr': expected_vptr,
        'rt_vptr': rt_vptr, 'expected_rt_vptr': expected_rt_vptr,
    }


def run_case(size, fixed_width_enabled, case=CASES):
    n = TracedBrush()
    n.import_calls = {}
    n.active_import = '<none>'
    n.stop_before_end_pen = False
    n.vector = 0
    # The real Model constructor uses recursive mutex calls. They are bounded
    # local no-ops; all ObjectStroke getters and channel reads remain native.
    for name in ('std::__ndk1::recursive_mutex::lock()',
                 'std::__ndk1::recursive_mutex::unlock()',
                 'std::__ndk1::recursive_mutex::recursive_mutex()',
                 'std::__ndk1::recursive_mutex::~recursive_mutex()'):
        n.callbacks[name] = lambda: None
    for name in ('SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)',
                 'SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, int, int const*, float const*, float const*, long long const*)',
                 'SPen::MotionEvent::~MotionEvent()',
                 'SPen::MotionEvent::GetHistorySize()',
                 'SPen::MotionEvent::GetHistoricalX(int, int) const',
                 'SPen::MotionEvent::GetHistoricalY(int, int) const',
                 'SPen::MotionEvent::GetHistoricalPressure(int) const',
                 'SPen::MotionEvent::GetHistoricalTilt(int) const',
                 'SPen::MotionEvent::GetHistoricalOrientation(int) const',
                 'SPen::MotionEvent::GetHistoricalEventTime(int) const',
                 'SPen::MotionEvent::GetEventTime() const',
                 'SPen::MotionEvent::GetX(int) const',
                 'SPen::MotionEvent::GetY(int) const',
                 'SPen::MotionEvent::GetPressure(int) const',
                 'SPen::MotionEvent::GetTilt(int) const',
                 'SPen::MotionEvent::GetOrientation(int) const',
                 'SPen::MotionEvent::GetToolType(int) const',
                 'SPen::MotionEvent::GetDownTime() const',
                 'SPen::MotionEvent::GetButtonState() const',
                 'SPen::MotionEvent::GetSource() const',
                 'SPen::MotionEvent::GetFlags() const',
                 'SPen::ObjectStroke::GetToolType() const',
                 'SPen::ObjectStroke::GetPointCount() const',
                 'SPen::ObjectStroke::GetPoint() const',
                 'SPen::ObjectStroke::GetTimeStamp() const',
                 'SPen::ObjectStroke::GetPressure() const',
                 'SPen::ObjectStroke::GetTilt() const',
                 'SPen::ObjectStroke::GetOrientation() const'):
        n.callbacks.pop(name, None)

    obj, impl, rect = n.alloc(0x100), n.alloc(0x400), n.alloc(32)
    n.call('SPen::ObjectStroke::ObjectStroke()', [obj])
    n.call('SPen::ObjectStrokeImpl::ObjectStrokeImpl(SPen::ObjectStroke*)', [impl, obj])
    n.put(obj + 32, 'Q', impl)
    n.call('SPen::ObjectStroke::SetPenSize(float)', [obj], [size])
    curve_enabled = bool(case.get('curve_enabled', True))
    fixed_opacity_enabled = bool(case.get('fixed_opacity_enabled', False))
    n.call('SPen::ObjectStroke::SetToolType(SPen::ToolType)', [obj, case['tool_type']])
    n.call('SPen::ObjectStroke::SetCurveEnabled(bool)', [obj, int(curve_enabled)])
    n.call('SPen::ObjectStroke::SetFixedOpacityEnabled(bool)', [obj, int(fixed_opacity_enabled)])
    n.call('SPen::ObjectStroke::GetPenSize() const', [obj])
    saved_object_size = n.f(0)
    n.call('SPen::ObjectStroke::GetToolType() const', [obj])
    saved_tool = n.x(0)
    n.call('SPen::ObjectStroke::IsCurveEnabled() const', [obj])
    saved_curve_enabled = bool(n.x(0))
    n.call('SPen::ObjectStroke::IsFixedOpacityEnabled() const', [obj])
    saved_fixed_opacity_enabled = bool(n.x(0))
    if (core.fbits(saved_object_size) != core.fbits(size) or saved_tool != case['tool_type']
            or saved_curve_enabled != curve_enabled
            or saved_fixed_opacity_enabled != fixed_opacity_enabled):
        raise AssertionError('native ObjectStroke setter/getter round trip failed')

    count = len(case['points'])
    channels = (
        ('points', 'f' * (count * 2), [v for point in case['points'] for v in point], 8),
        ('pressures', 'f' * count, case['pressures'], 4),
        ('times', 'i' * count, case['times'], 4),
        ('tilts', 'f' * count, case['tilts'], 4),
        ('orientations', 'f' * count, case['orientations'], 4),
    )
    pointers = {}
    for key, fmt, values, width in channels:
        ptr = n.alloc(width * count)
        n.put(ptr, fmt, *values)
        pointers[key] = ptr
    n.put(impl + 36, 'I', count)
    for offset, key, width in ((48, 'points', 8), (72, 'pressures', 4),
                               (96, 'times', 4), (120, 'tilts', 4),
                               (144, 'orientations', 4)):
        ptr = pointers[key]
        n.put(impl + offset, 'QQQ', ptr, ptr + count * width, ptr + count * width)

    state = setup_pen(
        n, size, fixed_width_enabled, curve_enabled,
        bool(case.get('verify_fixed_opacity_native_contract', False)))
    n.current_motion = {'source': 'native converted MotionEvent', 'count': count}
    n.current_object = {
        'points': case['points'], 'tool_type': saved_tool,
        'version': state['version'], 'points_ptr': pointers['points'],
        'pressures_ptr': pointers['pressures'], 'times_ptr': pointers['times'],
        'tilts_ptr': pointers['tilts'], 'orientations_ptr': pointers['orientations'],
    }
    n.current_draw_line = n.syms[core.DRAW_LINE.format(version=state['version'])]
    n.call(core.OBJECT_REDRAW.format(version=state['version']),
           [state['drawable'], obj, rect], instruction_limit=5_000_000)
    if n.active_end_pen_record is not None or len(n.end_pen_records) != 1:
        raise AssertionError('saved ObjectStroke did not complete one native endPen record')
    terminal = n.end_pen_records[0]
    if (terminal.get('path_length_call_count') != 1
            or terminal.get('path_length_bits_le') is None
            or not terminal.get('selected_path_method')
            or terminal.get('emitted_addpoint_count') is None):
        raise AssertionError('saved ObjectStroke endPen terminal trace is incomplete')
    if not n.buffer_records or any(r['count_f32'] != 11 * i
                                   for i, r in enumerate(n.buffer_records, 1)):
        raise AssertionError('saved ObjectStroke did not emit 11-float native AddPoint records')
    begin, end, _ = n.get(state['vector'], 'QQQ')
    raw = bytes(n.u.mem_read(begin, end - begin))
    expected_getters = {
        'SPen::ObjectStroke::GetPointCount() const': 1,
        'SPen::ObjectStroke::GetPoint() const': 1,
        'SPen::ObjectStroke::GetTimeStamp() const': 1,
        'SPen::ObjectStroke::GetPressure() const': 1,
        'SPen::ObjectStroke::GetTilt() const': 1,
        'SPen::ObjectStroke::GetOrientation() const': 1,
    }
    actual_getters = {name: n.import_calls.get(name, 0) for name in expected_getters}
    if actual_getters != expected_getters:
        raise AssertionError(f'ObjectStroke native channel getter coverage mismatch: {actual_getters}')
    if len(n.motion_event_constructors) != 1:
        raise AssertionError('saved ObjectStroke did not construct exactly one native MotionEvent')
    expected_draw_pattern_count = len(n.buffer_records)
    if (len(n.draw_pattern_calls) != expected_draw_pattern_count
            or len(n.draw_pattern_style_steps) != expected_draw_pattern_count):
        raise AssertionError('native drawPattern/style-step trace count mismatch')
    for call, step in zip(n.draw_pattern_calls, n.draw_pattern_style_steps):
        native_style = call['drawable_pen_settings']
        expected_radius = (size * 0.5 if fixed_width_enabled
                           else max(1.0, call['radius_argument_s0_f32']))
        if (native_style['fixed_width_enabled_byte_offset_89'] != int(fixed_width_enabled)
                or core.fbits(native_style['fixed_width_f32_offset_92']) != core.fbits(size)
                or step['fixed_width_enabled_byte_offset_89'] != int(fixed_width_enabled)
                or core.fbits(step['fixed_width_f32_offset_92']) != core.fbits(size)
                or core.fbits(step['working_radius_s14_f32']) != core.fbits(expected_radius)
                or step['working_opacity_w20_u32'] != (255 if fixed_width_enabled else call['input_opacity_u32'])):
            raise AssertionError(f'native drawPattern style transition mismatch: {call=} {step=}')
    if any(call['drawable_pen_settings']['fixed_width_enabled_byte_offset_89']
           != int(fixed_width_enabled) for call in n.make_draw_point_calls):
        raise AssertionError('makeDrawPoint did not observe configured native style state')
    motion_ctor = n.motion_event_constructors[0]
    decoded = motion_ctor['pointer_coords_decoded_from_Base_getter_offsets']
    expected_point = case['points'][-1]
    expected_down_time = case['times'][0]
    expected_event_time = case['times'][-1]
    expected_motion_getters = {
        'SPen::MotionEvent::GetX(int) const': float(expected_point[0]),
        'SPen::MotionEvent::GetY(int) const': float(expected_point[1]),
    }
    if saved_tool == 2:
        expected_motion_getters['SPen::MotionEvent::GetPressure(int) const'] = case['pressures'][-1]
    getter_values = {call['name']: call for call in n.motion_event_getters}
    if (motion_ctor['arguments_decoded']['down_time_ms_i64'] != expected_down_time
            or motion_ctor['arguments_decoded']['pointer_count_i32'] != 1
            or motion_ctor['pointer_properties_decoded']['tool_type_i32'] != saved_tool
            or decoded['event_time_i64_ms'] != expected_event_time
            or any((core.fbits(decoded[key]) != core.fbits(expected)
                    if key.endswith('_f32') else
                    struct.pack('<d', decoded[key]) != struct.pack('<d', expected))
                   for key, expected in (
                ('x_f64', expected_point[0]), ('y_f64', expected_point[1]),
                ('pressure_f32', case['pressures'][-1]),
                ('tilt_f32', case['tilts'][-1]),
                ('orientation_f32', case['orientations'][-1])))):
        raise AssertionError('native ObjectStroke-to-MotionEvent conversion differs from declared final row')
    for name, expected in expected_motion_getters.items():
        call = getter_values.get(name)
        actual = (call.get('return_d0_f64') if name.endswith(('GetX(int) const', 'GetY(int) const'))
                  else call.get('return_s0_f32')) if call else None
        if actual is None or (struct.pack('<d', actual) != struct.pack('<d', expected)
                              if name.endswith(('GetX(int) const', 'GetY(int) const')) else
                              core.fbits(actual) != core.fbits(expected)):
            raise AssertionError(f'native MotionEvent getter mismatch for {name}: {actual}')
    if getter_values.get('SPen::MotionEvent::GetEventTime() const', {}).get('return_x0') != expected_event_time - expected_down_time:
        raise AssertionError('native MotionEvent event-time offset differs from converted row/down-time')
    if getter_values.get('SPen::MotionEvent::GetToolType(int) const', {}).get('return_x0') != saved_tool:
        raise AssertionError('native MotionEvent tool type differs from saved ObjectStroke')
    expected_motion_names = set(expected_motion_getters) | {
        'SPen::MotionEvent::GetEventTime() const',
        'SPen::MotionEvent::GetToolType(int) const'}
    if (len(n.motion_event_getters) != len(expected_motion_names)
            or {call['name'] for call in n.motion_event_getters} != expected_motion_names):
        raise AssertionError('native MotionEvent getter trace does not contain the expected accesses')
    expected_motion_calls = {
        'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)': 1,
        'SPen::MotionEvent::GetX(int) const': 1,
        'SPen::MotionEvent::GetY(int) const': 1,
        'SPen::MotionEvent::GetToolType(int) const': 1,
        'SPen::MotionEvent::GetEventTime() const': 1,
        'SPen::MotionEvent::~MotionEvent()': 1,
    }
    if saved_tool == 2:
        expected_motion_calls['SPen::MotionEvent::GetPressure(int) const'] = 1
    actual_motion_calls = {name: n.import_calls.get(name, 0) for name in expected_motion_calls}
    if actual_motion_calls != expected_motion_calls:
        raise AssertionError(f'native MotionEvent method counts differ: {actual_motion_calls}')
    if any('MotionEvent' in name for name in n.host_boundary_calls):
        raise AssertionError('MotionEvent constructor/getter/destructor reached a host shim')

    return {
        'label': (f"tool-{case['tool_type']}-curve-{int(curve_enabled)}-fixed-opacity-"
                  f"{int(fixed_opacity_enabled)}-size-{size:g}-fixed-width-{int(fixed_width_enabled)}"),
        'input': dict(case),
        'style_setters': {
            'calls': ['SPen::Pen::SetSize(float)',
                      'SPen::BrushPen::SetFixedWidth(float)',
                      'SPen::BrushPen::SetFixedWidthEnabled(bool)',
                      'SPen::BrushPen::SetCurveEnabled(bool)'],
            'curve_enabled_input': curve_enabled,
            'fixed_opacity_enabled_input': fixed_opacity_enabled,
            'fixed_width_argument': size,
            'fixed_width_enabled_argument': fixed_width_enabled,
            'native_GetSize_f32': state['size'],
            'native_GetFixedWidth_f32': state['fixed_width'],
            'native_fixed_width_enabled_input': state['fixed_width_enabled_input'],
            'native_fixed_width_enabled_byte_offset_89': state['fixed_width_enabled_native'],
            'native_setting_bytes_offsets_84_to_99': bytes(n.u.mem_read(state['setting_data'] + 84, 16)).hex(),
        },
        'object_setters_and_getters': {
            'calls': ['SPen::ObjectStroke::SetPenSize(float)',
                      'SPen::ObjectStroke::SetToolType(SPen::ToolType)',
                      'SPen::ObjectStroke::SetCurveEnabled(bool)',
                      'SPen::ObjectStroke::SetFixedOpacityEnabled(bool)',
                      'SPen::ObjectStroke::GetPenSize() const',
                      'SPen::ObjectStroke::GetToolType() const',
                      'SPen::ObjectStroke::IsCurveEnabled() const',
                      'SPen::ObjectStroke::IsFixedOpacityEnabled() const'],
            'native_saved_size_f32': saved_object_size,
            'native_saved_tool_type': saved_tool,
            'native_channel_getter_import_calls': actual_getters,
            'native_curve_enabled': saved_curve_enabled,
            'native_fixed_opacity_enabled': saved_fixed_opacity_enabled,
        },
        'native_admission': {
            'constructor': core.CTOR,
            'constructor_parser_inputs': state['constructor_inputs'],
            'actual_getVersion': state['version'],
            'actual_GetSize_f32': state['size'],
            'actual_GetStrokeDrawableGL_vptr': hex(state['vptr']),
            'selected_GLVx_vtable_symbol_plus_16': hex(state['expected_vptr']),
            'actual_RT_vptr': hex(state['rt_vptr']),
            'constructed_RTV1_vtable_symbol_plus_16': hex(state['expected_rt_vptr']),
        },
        'bounds_f32': list(n.get(rect, '4f')),
        'drawLine_arguments': n.object_line_records,
        'endPen_terminal_record': terminal,
        'native_AddPoint_records': n.buffer_records,
        'native_drawPattern_calls': n.draw_pattern_calls,
        'native_drawPattern_style_transition_steps': n.draw_pattern_style_steps,
        'native_makeDrawPoint_calls': n.make_draw_point_calls,
        'native_output_buffer_f32_le_hex': raw.hex(),
        'native_output_buffer_sha256': hashlib.sha256(raw).hexdigest(),
        'native_output_f32_count': len(raw) // 4,
        'native_MotionEvent_constructor_calls': n.motion_event_constructors,
        'native_MotionEvent_getter_returns': n.motion_event_getters,
        'native_MotionEvent_method_call_counts': actual_motion_calls,
        'native_host_boundaries': n.boundaries(),
        **({'native_fixed_opacity_contract': state['fixed_opacity_contract']}
           if state['fixed_opacity_contract'] is not None else {}),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    actual_hashes = {name: hashlib.sha256(
        (core.LIB / f'libSPen{name}.so').read_bytes()).hexdigest() for name in EXPECTED}
    if actual_hashes != EXPECTED:
        raise SystemExit('pinned BrushPen/Model input hash mismatch')
    cases = [run_case(size, enabled) for size, enabled in STYLE_CASES]
    for tool_type in (1, 3):
        for curve_enabled in (False, True):
            case = dict(CASES, name=f'synthetic-tool-{tool_type}-curve-{int(curve_enabled)}',
                        tool_type=tool_type, curve_enabled=curve_enabled,
                        fixed_opacity_enabled=False, object_fixed_width_enabled=False)
            cases.append(run_case(3.144, False, case))
    fixed_opacity_case = dict(
        CASES,
        name='synthetic-tool-2-fixed-opacity-noop',
        tool_type=2,
        curve_enabled=True,
        fixed_opacity_enabled=True,
        object_fixed_width_enabled=False,
        verify_fixed_opacity_native_contract=True,
    )
    fixed_opacity_row = run_case(3.144, False, fixed_opacity_case)
    baseline = cases[0]
    if (fixed_opacity_row['native_output_buffer_f32_le_hex']
            != baseline['native_output_buffer_f32_le_hex']
            or fixed_opacity_row['native_AddPoint_records'] != baseline['native_AddPoint_records']):
        raise AssertionError('native fixed-opacity true changed the captured baseline output buffer')
    fixed_opacity_row['native_fixed_opacity_buffer_comparison'] = {
        'baseline_label': baseline['label'],
        'native_buffer_bytes_equal': True,
        'native_AddPoint_records_equal': True,
        'comparison_scope': 'this synthetic ObjectStroke row and pinned BrushPen constructor profile',
    }
    cases.append(fixed_opacity_row)
    result = {
        'schema': 'sdocx-brushpen-saved-wrapper-native-v1',
        'artifact': 'Samsung Notes 4.4.45.37 ARM64 BrushPen',
        'library_sha256': actual_hashes,
        'evidence_scope': 'Synthetic ObjectStroke channel rows execute actual Model constructor/getters, BrushPen GLV10 saved redraw, Base MotionEvent construction/getters/destruction, and native endPen. Constructor arguments, pointed-to PointerProperties/PointerCoords, decoded channel fields, and native getter returns are captured. Native texture/GPU/pixel work is not executed; no captured note or production pen-id admission is claimed.',
        'cases': cases,
    }
    encoded = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
    else:
        expected = json.loads(OUT.read_text())
        if json.dumps(result, sort_keys=True) != json.dumps(expected, sort_keys=True):
            raise SystemExit('native BrushPen saved-wrapper output differs from fixture')
        print(f'BrushPen saved-wrapper oracle passed: {len(cases)} source/style configurations')


if __name__ == '__main__':
    main()
