"""Hash-pinned Samsung Notes BrushPen GLV10/RTV1 CPU-side redraw oracle.

The inputs in this oracle are explicit synthetic MotionEvent/ObjectStroke
channels, not captured device samples. Native BrushPen constructor, version
selection, redraw, and RTV1::AddPoint execute under Unicorn. Renderer queue,
bitmap/GPU work, and Samsung String teardown remain named host boundaries;
this is geometry/attribute evidence, not pixel parity.
"""
import argparse
import hashlib
import json
import math
import os
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
sys.path.insert(0, str(ROOT / 'conformance'))
import fountain_native
from fountain_native import NativeFountain
from unicorn import arm64_const as R

HASHES = {
    'BrushPen': 'acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce',
    'Graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
    'Renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
    'Model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
}
CTOR = 'SPen::BrushPen::BrushPen()'
GET_DRAWABLE = 'SPen::BrushPen::GetStrokeDrawableGL()'
GET_VERSION = 'SPen::Pen::getVersion()'
GET_SIZE = 'SPen::Pen::GetSize()'
MOTION_CTOR = ('SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, '
               'int, int const*, float const*, float const*, long long const*)')
MOTION_REDRAW = 'SPen::BrushPenStrokeDrawableGLV{version}::RedrawPen(SPen::MotionEvent const*, SPen::RectF*)'
OBJECT_REDRAW = 'SPen::BrushPenStrokeDrawableGLV{version}::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)'
ADD_POINT = ('SPen::BrushPenStrokeDrawableRTV1::AddPoint(SPen::Vector4<float>, float, '
             'SPen::Vector2<float>, SPen::Vector2<float>, SPen::Vector2<float>, SPen::Vector3<float>)')
DRAW_LINE = ('SPen::BrushPenStrokeDrawableGLV{version}::drawLine(float, float, float, long long, '
             'SPen::RectF*, int, bool)')
END_PEN = 'SPen::BrushPenStrokeDrawableGLV{version}::endPen(SPen::MotionEvent const*, SPen::RectF*, bool)'
RETURN = 0x200f1000
CALL_SP = 0x200f0000


def fbits(value):
    return struct.pack('<f', float(value)).hex()


def elf_relocations(blob):
    shoff = struct.unpack_from('<Q', blob, 40)[0]
    entsize, count, stridx = struct.unpack_from('<HHH', blob, 58)
    sections = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * entsize)
                for i in range(count)]
    strings_sec = sections[stridx]
    section_names = blob[strings_sec[4]:strings_sec[4] + strings_sec[5]]
    by_name = {}
    for section in sections:
        end = section_names.find(b'\0', section[0])
        by_name[section_names[section[0]:end].decode()] = section
    dynsym, dynstr = by_name['.dynsym'], by_name['.dynstr']
    strings = blob[dynstr[4]:dynstr[4] + dynstr[5]]
    symbols = []
    for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
        nameoff, _info, _other, shndx, value, _size = struct.unpack_from('<IBBHQQ', blob, off)
        end = strings.find(b'\0', nameoff)
        symbols.append((strings[nameoff:end].decode() if nameoff else '', shndx, value))
    reloc = by_name.get('.rela.dyn')
    entries = []
    if reloc:
        for off in range(reloc[4], reloc[4] + reloc[5], reloc[9]):
            where, info, addend = struct.unpack_from('<QQq', blob, off)
            entries.append((where, info & 0xffffffff, info >> 32, addend))
    return entries, symbols


def elf_string(path, address, limit=16384):
    blob = path.read_bytes()
    phoff = struct.unpack_from('<Q', blob, 32)[0]
    phentsize, phnum = struct.unpack_from('<HH', blob, 54)
    for index in range(phnum):
        kind, _flags, fileoff, vaddr, _paddr, filesz, _memsz, _align = struct.unpack_from(
            '<II6Q', blob, phoff + index * phentsize)
        if kind == 1 and vaddr <= address < vaddr + filesz:
            end = min(fileoff + filesz, fileoff + address - vaddr + limit)
            return blob[fileoff + address - vaddr:end].split(b'\0', 1)[0].decode()
    raise ValueError(f'ELF address {address:#x} is outside file-backed PT_LOAD segments: {path}')


def numeric_constants():
    """Retain exact native storage, including widened float32 constants."""
    blob = (LIB / 'libSPenBrushPen.so').read_bytes()
    if hashlib.sha256(blob).hexdigest() != HASHES['BrushPen']:
        raise AssertionError('BrushPen numeric-constant source hash changed')
    phoff = struct.unpack_from('<Q', blob, 32)[0]
    stride, count = struct.unpack_from('<HH', blob, 54)
    result = {}
    for name, address, fmt in (
            ('radius_smoothing', 0x2800c, '<f'),
            ('radius_prior_weight', 0x28034, '<f'),
            ('initial_width', 0x28028, '<f'),
            ('radius_bias', 0x280b0, '<d'),
            ('radius_seed_scale', 0x280c8, '<d')):
        size = struct.calcsize(fmt)
        for index in range(count):
            kind, _, offset, base, _, length, _, _ = struct.unpack_from(
                '<II6Q', blob, phoff + index * stride)
            if kind == 1 and base <= address and address + size <= base + length:
                raw = blob[offset + address - base:offset + address - base + size]
                result[name] = {'elf_address': hex(address), 'bytes_le': raw.hex(),
                                'storage_type': 'f32' if size == 4 else 'f64',
                                'value': struct.unpack(fmt, raw)[0]}
                break
        else:
            raise AssertionError(f'numeric constant {name} is not file-backed')
    return result


class NativeBrush(NativeFountain):
    def __init__(self):
        self.host_boundary_calls = {}
        self.callback_records = []
        self.buffer_records = []
        self.draw_line_gate_records = []
        self.active_draw_line_gate_record = None
        self.active_draw_line_drawable = 0
        self.capture_draw_line_path_calls = False
        self.last_vector_bytes = {}
        self.object_line_records = []
        self.pending_returns = {}
        self.vector_addresses = set()
        self.tls_regions = {}
        self.acquired_guards = set()
        self.initialized_guards = set()
        self.current_motion = None
        self.current_object = None
        self.current_draw_line = 0
        self.queue = 0
        old = fountain_native.HASHES
        fountain_native.HASHES = {**old, **HASHES}
        try:
            super().__init__(LIB, OBJDUMP, ('BrushPen', 'Graphics', 'Renderer'))
        finally:
            fountain_native.HASHES = old
        self.load_model()
        for name, expected in HASHES.items():
            path = LIB / ('libSPen' + name + '.so')
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            if actual != expected:
                raise ValueError(f'unverified native input {path}: {actual}')
        # NativeFountain's minimal loader maps function segments and PLTs. These
        # DSOs also need their data relocations applied for real vtables/GOTs.
        self.relocate_data()
        self.add_address = self.syms[ADD_POINT]
        self.end_pen_plt = 0
        self.setting_inputs = []
        self.callbacks.update({
            'SPen::PenGLDataManager::GetMsgQueue()': self.queue_return,
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)': self.host_noop,
            'SPen::PenReturnCallback::GetBuffer()': lambda: self.x(0, self.vector),
            'SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)': self.host_noop,
            'SPen::PenReturnCallback::~PenReturnCallback()': self.host_noop,
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)': self.host_noop,
            'SPen::String::~String()': self.host_noop,
            'SPen::MotionEvent::MotionEvent(long long, int, int, SPen::MotionEvent::PointerProperties*, SPen::MotionEvent::PointerCoords*, int, int, int)': self.host_noop,
            'SPen::MotionEvent::~MotionEvent()': self.host_noop,
            'SPen::Error::SetError(unsigned long)': self.host_noop,
            '__android_log_print': lambda: self.x(0, 0),
            '__cxa_atexit': lambda: self.x(0, 0),
            '__cxa_guard_acquire': self.guard_acquire,
            '__cxa_guard_release': self.guard_release,
            '__cxa_guard_abort': self.guard_abort,
            '__emutls_get_address': self.emutls_get_address,
            'atoi': self.atoi,
            'atof': self.atof,
            'tanf': self.tanf,
            'sincosf': self.sincosf,
            'fmodf': self.fmodf,
            'expf': self.expf,
            'operator new(unsigned long, std::nothrow_t const&)': self.new_nothrow,
            'operator new[](unsigned long, std::nothrow_t const&)': self.new_nothrow,
            'pthread_mutexattr_init': lambda: self.x(0, 0),
            'pthread_mutexattr_settype': lambda: self.x(0, 0),
            'pthread_mutexattr_destroy': lambda: self.x(0, 0),
            'pthread_mutex_init': lambda: self.x(0, 0),
            'pthread_mutex_lock': lambda: self.x(0, 0),
            'pthread_mutex_unlock': lambda: self.x(0, 0),
            'pthread_mutex_destroy': lambda: self.x(0, 0),
            'memcmp': self.memcmp,
            'strcmp': self.strcmp,
            'strlen': self.strlen,
            'memcpy': self.copy,
            'memmove': self.copy,
            'memset': self.memset,
            'SPen::MotionEvent::GetHistorySize() const': self.motion_history_size,
            'SPen::MotionEvent::GetHistoricalX(int, int) const': lambda: self.motion_historical('x', 'double'),
            'SPen::MotionEvent::GetHistoricalY(int, int) const': lambda: self.motion_historical('y', 'double'),
            'SPen::MotionEvent::GetHistoricalPressure(int) const': lambda: self.motion_historical('pressure', 'float'),
            'SPen::MotionEvent::GetHistoricalTilt(int) const': lambda: self.motion_historical('tilt', 'float'),
            'SPen::MotionEvent::GetHistoricalOrientation(int) const': lambda: self.motion_historical('orientation', 'float'),
            'SPen::MotionEvent::GetHistoricalEventTime(int) const': lambda: self.motion_historical('time', 'int'),
            'SPen::MotionEvent::GetX(int) const': lambda: self.motion_current_value('x', 'double'),
            'SPen::MotionEvent::GetY(int) const': lambda: self.motion_current_value('y', 'double'),
            'SPen::MotionEvent::GetPressure(int) const': lambda: self.motion_current_value('pressure', 'float'),
            'SPen::MotionEvent::GetTilt(int) const': lambda: self.motion_current_value('tilt', 'float'),
            'SPen::MotionEvent::GetOrientation(int) const': lambda: self.motion_current_value('orientation', 'float'),
            'SPen::MotionEvent::GetToolType(int) const': lambda: self.motion_current_value('tool_type', 'int'),
            'SPen::MotionEvent::GetEventTime() const': lambda: self.x(0, self.current_motion['times'][-1]),
            'SPen::MotionEvent::GetDownTime() const': lambda: self.x(0, self.current_motion['times'][0]),
            'SPen::MotionEvent::GetButtonState() const': lambda: self.x(0, 0),
            'SPen::MotionEvent::GetSource() const': lambda: self.x(0, 0),
            'SPen::MotionEvent::GetFlags() const': lambda: self.x(0, 0),
            'SPen::PenStrokeDataManager::GetInstance()': lambda: self.x(0, 0),
            'SPen::PenStrokeDataManager::GetStrokeData(int)': lambda: self.x(0, 0),
            'SPen::ObjectBase::GetOriginalRuntimeHandle() const': lambda: self.x(0, 0xffffffff),
            'SPen::ObjectBase::GetRuntimeHandle() const': lambda: self.x(0, 1),
            'SPen::ObjectStroke::GetToolType() const': lambda: self.x(0, self.current_object['tool_type']),
            'SPen::ObjectStroke::GetPointCount() const': lambda: self.x(0, len(self.current_object['points'])),
            'SPen::ObjectStroke::GetPoint() const': lambda: self.x(0, self.current_object['points_ptr']),
            'SPen::ObjectStroke::GetTimeStamp() const': lambda: self.x(0, self.current_object['times_ptr']),
            'SPen::ObjectStroke::GetPressure() const': lambda: self.x(0, self.current_object['pressures_ptr']),
            'SPen::ObjectStroke::GetTilt() const': lambda: self.x(0, self.current_object['tilts_ptr']),
            'SPen::ObjectStroke::GetOrientation() const': lambda: self.x(0, self.current_object['orientations_ptr']),
        })
        self.u.mem_write(0x20001000, (0xd2800020).to_bytes(4, 'little') + (0xd65f03c0).to_bytes(4, 'little'))
        self.u.mem_write(0x20001020, (0xd65f03c0).to_bytes(4, 'little'))
        self.matrix_stub = 0x20001020

    def load_model(self):
        # NativeFountain's generic loader reserves 2 MiB per DSO; Model has a
        # larger PT_LOAD segment, so map that pinned dependency explicitly.
        name, base = 'Model', 6 * 0x1000000
        path = LIB / 'libSPenModel.so'
        blob = path.read_bytes()
        off = struct.unpack_from('<Q', blob, 32)[0]
        stride, count = struct.unpack_from('<HH', blob, 54)
        self.u.mem_map(base, 0x1000000)
        for i in range(count):
            kind, _, fileoff, va, _, filesz, _memsz, _ = struct.unpack_from('<II6Q', blob, off + i * stride)
            if kind == 1:
                self.u.mem_write(base + va, blob[fileoff:fileoff + filesz])
        for line in subprocess.check_output(['nm', '-D', '-C', path], text=True).splitlines():
            parts = line.strip().split(' ', 2)
            if len(parts) == 3 and parts[1] in ('T', 'W'):
                self.syms[parts[2]] = base + int(parts[0], 16)
        env = dict(os.environ)
        runtime = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'
        env['LD_LIBRARY_PATH'] = str(runtime)
        dis = subprocess.check_output([str(OBJDUMP), '-d', '-C', '-j', '.plt', path], env=env, text=True)
        for address, symbol in __import__('re').findall(r'^([0-9a-f]+) <(.+)@plt>:', dis, __import__('re').M):
            self.plt[base + int(address, 16)] = symbol

    def relocate_data(self):
        loaded = ('FountainPen', 'PenCommon', 'Base', 'BrushPen', 'Graphics', 'Renderer', 'Model')
        raw_symbols = {}
        relocations = []
        for index, name in enumerate(loaded):
            base = index * 0x1000000
            if name != 'Model':
                self.u.mem_map(base + 0x200000, 0x600000)
            blob = (LIB / ('libSPen' + name + '.so')).read_bytes()
            entries, symbols = elf_relocations(blob)
            for line in subprocess.check_output(['nm', '-D', str(LIB / ('libSPen' + name + '.so'))], text=True).splitlines():
                parts = line.strip().split(' ', 2)
                if len(parts) == 3 and parts[1] in ('T', 'W', 'D', 'B', 'V', 'R'):
                    raw_symbols[parts[2]] = base + int(parts[0], 16)
            for line in subprocess.check_output(['nm', '-D', '-C', str(LIB / ('libSPen' + name + '.so'))], text=True).splitlines():
                parts = line.strip().split(' ', 2)
                if len(parts) == 3 and parts[1] in ('T', 'W', 'D', 'B', 'V', 'R'):
                    self.syms[parts[2]] = base + int(parts[0], 16)
            relocations.append((base, entries, symbols))
        for base, entries, symbols in relocations:
            for where, typ, symidx, addend in entries:
                value = None
                if typ == 0x403:
                    value = base + addend
                elif typ in (0x101, 0x401):
                    symname, defined, symval = symbols[symidx]
                    if defined and symval:
                        value = base + symval + addend
                    elif symname in raw_symbols:
                        value = raw_symbols[symname] + addend
                if value is not None:
                    self.u.mem_write(base + where, struct.pack('<Q', value & 0xffffffffffffffff))

    def new_nothrow(self):
        size = self.x(0)
        if size > 0x100000:
            raise RuntimeError(f'nothrow allocation exceeds oracle bound: {size:#x} at {self.active_import}')
        self.x(0, self.alloc(size))

    def host_noop(self):
        name = self.active_import
        self.host_boundary_calls[name] = self.host_boundary_calls.get(name, 0) + 1

    def guard_acquire(self):
        guard = self.x(0)
        self.host_boundary_calls['__cxa_guard_acquire'] = self.host_boundary_calls.get('__cxa_guard_acquire', 0) + 1
        self.x(0, int(guard not in self.initialized_guards and guard not in self.acquired_guards))
        if self.x(0):
            self.acquired_guards.add(guard)

    def guard_release(self):
        guard = self.x(0)
        if guard not in self.acquired_guards:
            raise RuntimeError(f'__cxa_guard_release without successful acquire for {guard:#x}')
        self.acquired_guards.remove(guard)
        self.initialized_guards.add(guard)
        self.u.mem_write(guard, b'\x01')
        self.host_boundary_calls['__cxa_guard_release'] = self.host_boundary_calls.get('__cxa_guard_release', 0) + 1

    def guard_abort(self):
        guard = self.x(0)
        self.acquired_guards.discard(guard)
        self.host_boundary_calls['__cxa_guard_abort'] = self.host_boundary_calls.get('__cxa_guard_abort', 0) + 1

    def emutls_get_address(self):
        control = self.x(0)
        size, align, existing = self.get(control, 'QQQ')
        if not 0 < size <= 0x100000 or not 0 < align <= 0x10000 or align & (align - 1):
            raise RuntimeError(f'invalid __emutls_control: size={size:#x}, align={align:#x}, ptr={existing:#x}')
        if not existing:
            raw = self.alloc(size + align)
            existing = (raw + align - 1) & -align
            self.put(control + 16, 'Q', existing)
            self.tls_regions[control] = {'size': size, 'align': align, 'address': existing}
        elif control not in self.tls_regions:
            raise RuntimeError(f'unowned preinitialized emutls pointer {existing:#x}')
        self.x(0, existing)

    def queue_return(self):
        self.host_boundary_calls['SPen::PenGLDataManager::GetMsgQueue()'] = self.host_boundary_calls.get('SPen::PenGLDataManager::GetMsgQueue()', 0) + 1
        self.x(0, self.queue)

    def atoi(self):
        raw = self.cstr(self.x(0))
        self.setting_inputs.append({'kind': 'atoi', 'text': raw})
        import re
        match = re.match(r'\s*[-+]?\d+', raw)
        self.x(0, int(match.group(0)) if match else 0)

    def atof(self):
        raw = self.cstr(self.x(0))
        self.setting_inputs.append({'kind': 'atof', 'text': raw})
        self.d(0, float(raw or '0'))

    def cstr(self, address):
        out = bytearray()
        for i in range(256):
            byte = self.u.mem_read(address + i, 1)[0]
            if byte == 0:
                break
            out.append(byte)
        return out.decode(errors='replace')

    def tanf(self):
        self.f(0, math.tan(self.f(0)))

    def sincosf(self):
        # Bionic sincosf(float, float *sin, float *cos).
        angle, sin_ptr, cos_ptr = self.f(0), self.x(0), self.x(1)
        self.put(sin_ptr, 'f', math.sin(angle))
        self.put(cos_ptr, 'f', math.cos(angle))

    def fmodf(self):
        self.f(0, math.fmod(self.f(0), self.f(1)))

    def expf(self):
        self.f(0, math.exp(self.f(0)))

    def memcmp(self):
        a, b, n = self.x(0), self.x(1), self.x(2)
        aa, bb = bytes(self.u.mem_read(a, n)), bytes(self.u.mem_read(b, n))
        self.x(0, next((x - y for x, y in zip(aa, bb) if x != y), 0))

    def copy(self):
        self.u.mem_write(self.x(0), bytes(self.u.mem_read(self.x(1), self.x(2))))

    def memset(self):
        self.u.mem_write(self.x(0), bytes([self.x(1) & 255]) * self.x(2))

    def strcmp(self):
        a, b = self.cstr(self.x(0)).encode(), self.cstr(self.x(1)).encode()
        aa, bb = a + b'\0', b + b'\0'
        self.x(0, next((x - y for x, y in zip(aa, bb) if x != y), 0))

    def strlen(self):
        self.x(0, len(self.cstr(self.x(0)).encode()))

    def motion_history_size(self):
        self.x(0, len(self.current_motion['points']) - 1)

    def motion_historical(self, key, result_type):
        index = self.x(1)
        channel = self.motion_channel(key)
        value = channel[index]
        if result_type == 'double':
            self.d(0, value)
        elif result_type == 'float':
            self.f(0, value)
        else:
            self.x(0, value)

    def motion_current_value(self, key, result_type):
        channel = self.motion_channel(key)
        value = channel if result_type == 'int' and isinstance(channel, int) else channel[-1]
        if result_type == 'double':
            self.d(0, value)
        elif result_type == 'float':
            self.f(0, value)
        else:
            self.x(0, value)

    def motion_channel(self, key):
        if key in ('x', 'y'):
            return [point[0 if key == 'x' else 1] for point in self.current_motion['points']]
        aliases = {'pressure': 'pressures', 'tilt': 'tilts', 'orientation': 'orientations', 'time': 'times'}
        return self.current_motion[aliases.get(key, key)]

    def hook(self, u, address, size, data):
        if self.current_draw_line and self.current_motion is not None:
            draw_pc = address - 3 * 0x1000000
            if address == self.current_draw_line:
                drawable = self.x(0)
                self.active_draw_line_drawable = drawable
                state_fields = self.get(drawable + 232, '9f')
                flags = [self.get(drawable + offset, 'B')[0] for offset in (225, 226, 227)]
                self.active_draw_line_gate_record = {
                    'index': len(self.draw_line_gate_records),
                    'input_xy_bits_le': [fbits(self.f(0)), fbits(self.f(1))],
                    'stored_time_before_i64': self.get(drawable + 288, 'q')[0],
                    'fallback_counter_before_bits_le': fbits(self.get(drawable + 308, 'f')[0]),
                    'state_before': {
                        'minimum_distance_44_bits_le': fbits(self.get(drawable + 44, 'f')[0]),
                        'alternating_distance_336_bits_le': fbits(self.get(drawable + 336, 'f')[0]),
                        'flags_225_226_227': flags,
                        'point_state_232_264_bits_le': [fbits(value) for value in state_fields],
                    },
                    'gate_steps': [],
                    'outcome': None,
                    'state_after': None,
                }
                self.capture_draw_line_path_calls = True
                self.draw_line_gate_records.append(self.active_draw_line_gate_record)
            record = self.active_draw_line_gate_record
            if record is not None and draw_pc == 0x5c420:
                drawable = self.active_draw_line_drawable
                record['accepted_timing'] = {
                    'delta_microseconds_i64': u.reg_read(R.UC_ARM64_REG_X10),
                    'fallback_counter_bits_le': fbits(self.get(drawable + 308, 'f')[0]),
                    'distance_bits_le': fbits(self.f(8)),
                    'previous_speed_bits_le': fbits(self.get(drawable + 296, 'f')[0]),
                }
            if record is not None and draw_pc == 0x5c4c4:
                record['accepted_timing']['next_speed_bits_le'] = fbits(self.f(1))
                record['accepted_timing']['smoothed_speed_bits_le'] = fbits(self.f(9))
            if record is not None and draw_pc in (0x5c380, 0x5c39c, 0x5c3a8, 0x5c3ac, 0x5c3b0, 0x5c3d8, 0x5c3dc, 0x5c3e0, 0x5c640, 0x5cfb4):
                drawable = self.active_draw_line_drawable
                step = {'pc': hex(draw_pc),
                        's8_bits_le': fbits(self.f(8)),
                        'minimum_distance_44_bits_le': fbits(self.get(drawable + 44, 'f')[0]),
                        'alternating_distance_336_bits_le': fbits(self.get(drawable + 336, 'f')[0]),
                        'flags_225_226_227': [self.get(drawable + offset, 'B')[0] for offset in (225, 226, 227)],
                        'nzcv': (u.reg_read(R.UC_ARM64_REG_PSTATE) >> 28) & 0xf}
                if draw_pc <= 0x5c3e0:
                    step['distance_bits_le'] = step['s8_bits_le']
                if draw_pc == 0x5c380:
                    step['first_gate_threshold_bits_le'] = step['minimum_distance_44_bits_le']
                elif draw_pc == 0x5c39c:
                    step['second_gate_threshold_bits_le'] = fbits(self.f(0))
                if draw_pc in (0x5c3a8, 0x5c3ac):
                    step['old_flag_226'] = self.x(10)
                    step['toggled_flag_226'] = self.x(11)
                record['gate_steps'].append(step)
                if draw_pc == 0x5c380:
                    record['first_gate_passed'] = bool(step['nzcv'] & 0x4)
                elif draw_pc == 0x5c39c:
                    record['second_gate_pl_taken'] = not bool(step['nzcv'] & 0x8)
                elif draw_pc == 0x5c3ac:
                    record['second_gate_passed'] = self.x(10) == 0
                elif draw_pc == 0x5c3b0:
                    record['outcome'] = 'rejected-first-gate' if record.get('first_gate_passed') is False else 'rejected-alternating-gate'
                    record['state_after'] = self._draw_line_state(drawable)
                    self.capture_draw_line_path_calls = False
                    self.active_draw_line_gate_record = None
                elif draw_pc == 0x5c3d8:
                    record['second_gate_passed'] = True
                elif draw_pc == 0x5c3e0:
                    record['outcome'] = 'accepted'
                elif draw_pc == 0x5c640:
                    record['state_before_sampling'] = self._draw_line_state(drawable)
                elif draw_pc == 0x5cfb4:
                    if self.x(19) != drawable:
                        raise AssertionError('makeDrawPoint final state belongs to another drawable')
                    record['state_after'] = self._draw_line_state(drawable)
                    self.capture_draw_line_path_calls = False
                    self.active_draw_line_gate_record = None
            if record is not None and draw_pc == 0x5cd28:
                record.setdefault('make_draw_point_path_lengths_bits_le', []).append(fbits(self.f(0)))
            if record is not None and draw_pc == 0x5c664:
                self.capture_draw_line_path_calls = False
        if self.current_draw_line and self.current_motion is not None and address in self.plt:
            record = self.active_draw_line_gate_record
            name = self.plt[address]
            if record is not None and self.capture_draw_line_path_calls and name in ('SPen::SmPath::moveTo(float, float)',
                                               'SPen::SmPath::quadTo(float, float, float, float)',
                                               'SPen::SmPath::lineTo(float, float)'):
                arity = 4 if 'quadTo' in name else 2
                record.setdefault('path_calls', []).append({
                    'method': name.rsplit('::', 1)[1].split('(')[0],
                    'arguments_bits_le': [fbits(self.f(i)) for i in range(arity)],
                })
        if address == self.matrix_stub:
            u.mem_write(u.reg_read(R.UC_ARM64_REG_X8), struct.pack('<9f', 1, 0, 0, 0, 1, 0, 0, 0, 1))
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if self.current_object is not None and address == self.current_draw_line:
            self.object_line_records.append({
                'callee': DRAW_LINE.format(version=self.current_object['version']),
                'x0_x7': [hex(u.reg_read(getattr(R, f'UC_ARM64_REG_X{i}'))) for i in range(8)],
                's0_s7_f32': [self.f(i) for i in range(8)],
                'stack64_hex': bytes(u.mem_read(u.reg_read(R.UC_ARM64_REG_SP), 64)).hex(),
            })
        if address == self.add_address:
            drawable = self.x(0)
            vec = self.get(drawable + 80, 'Q')[0]
            if not vec:
                raise RuntimeError('native AddPoint reached without explicit output vector')
            begin, end, cap = self.get(vec, 'QQQ')
            if not cap:
                storage = self.alloc(8192 * 4)
                self.put(vec, 'QQQ', storage, storage, storage + 8192 * 4)
                begin = end = storage
            ret = u.reg_read(R.UC_ARM64_REG_LR)
            self.pending_returns.setdefault(ret, []).append(vec)
            vptr = self.x(1)
            rawv4 = bytes(u.mem_read(vptr, 16))
            sp = u.reg_read(R.UC_ARM64_REG_SP)
            self.callback_records.append({
                'lr': hex(ret), 'rt_this': hex(drawable), 'vector4_address': hex(vptr),
                'vector4_bits_le': [rawv4[i:i + 4].hex() for i in range(0, 16, 4)],
                'vector4_f32': list(struct.unpack('<4f', rawv4)),
                'x0_x7': [hex(u.reg_read(getattr(R, f'UC_ARM64_REG_X{i}'))) for i in range(8)],
                's0_s7_bits': [struct.pack('<f', self.f(i)).hex() for i in range(8)],
                'stack64_hex': bytes(u.mem_read(sp, 64)).hex(),
            })
        if address in self.pending_returns:
            for vec in self.pending_returns.pop(address):
                begin, end, cap = self.get(vec, 'QQQ')
                if not begin or end < begin or end > cap or (end - begin) % 4:
                    raise RuntimeError('native std::vector<float> bounds invalid')
                raw = bytes(u.mem_read(begin, end - begin))
                words = (end - begin) // 4
                previous = self.last_vector_bytes.get(vec, b'')
                if not raw.startswith(previous):
                    raise RuntimeError('native AddPoint changed a previously emitted vector prefix')
                appended = raw[len(previous):]
                if len(appended) != 11 * 4 or words != len(previous) // 4 + 11:
                    raise RuntimeError(f'native AddPoint growth is not one 11-f32 instance: +{len(appended)} bytes, total {words} f32')
                self.last_vector_bytes[vec] = raw
                self.buffer_records.append({
                    'append_index': len(previous) // (11 * 4),
                    'count_f32': words,
                    'cumulative_sha256': hashlib.sha256(raw).hexdigest(),
                    'instance_bits_le': [appended[i:i + 4].hex() for i in range(0, len(appended), 4)],
                })
        if address == self.end_pen_plt and self.stop_before_end_pen:
            u.reg_write(R.UC_ARM64_REG_PC, RETURN)
            u.emu_stop()
            return
        if address in self.plt:
            self.active_import = self.plt[address]
            self.import_calls[self.active_import] = self.import_calls.get(self.active_import, 0) + 1
        super().hook(u, address, size, data)

    def _draw_line_state(self, drawable):
        return {
            'minimum_distance_44_bits_le': fbits(self.get(drawable + 44, 'f')[0]),
            'alternating_distance_336_bits_le': fbits(self.get(drawable + 336, 'f')[0]),
            'flags_225_226_227': [self.get(drawable + offset, 'B')[0] for offset in (225, 226, 227)],
            'point_state_232_264_bits_le': [fbits(value) for value in self.get(drawable + 232, '9f')],
        }

    def setup_native_pen(self):
        pen, manager = self.alloc(256), self.alloc(128)
        queue, qvt = self.alloc(32), self.alloc(32)
        self.queue = queue
        self.put(queue, 'Q', qvt)
        self.put(qvt + 16, 'Q', 0x20001000)
        self.put(manager + 56, 'Q', queue)
        self.put(pen + 8, 'Q', manager)
        self.call(CTOR, [pen], instruction_limit=3_000_000)
        self.call(GET_VERSION, [pen])
        version = self.x(0)
        self.call(GET_SIZE, [pen])
        native_size = self.f(0)
        self.put(pen + 8, 'Q', manager)
        self.call(GET_DRAWABLE, [pen], instruction_limit=3_000_000)
        drawable = self.x(0)
        vptr = self.get(drawable, 'Q')[0]
        expected_vptr = self.syms[f'vtable for SPen::BrushPenStrokeDrawableGLV{version}'] + 16
        parsed_version = int(next(item['text'] for item in self.setting_inputs if item['kind'] == 'atoi'))
        if version != parsed_version or vptr != expected_vptr:
            raise AssertionError(f'fresh constructor admission mismatch: getVersion={version}, vptr={vptr:#x}')
        self.end_pen_plt = next(a for a, n in self.plt.items()
                                if n == END_PEN.format(version=version))
        rt = self.get(drawable + 16, 'Q')[0]
        rt_vptr = self.get(rt, 'Q')[0]
        expected_rt_vptr = self.syms['vtable for SPen::BrushPenStrokeDrawableRTV1'] + 16
        if rt_vptr != expected_rt_vptr:
            raise AssertionError(f'GLV{version} did not construct RTV1: rt_vptr={rt_vptr:#x}')
        vec = self.alloc(24)
        storage = self.alloc(8192 * 4)
        self.put(vec, 'QQQ', storage, storage, storage + 8192 * 4)
        self.vector, self.vector_addresses = vec, {vec}
        self.put(rt + 80, 'Q', vec)
        canvas, cvt = self.alloc(128), self.alloc(256)
        self.put(canvas, 'Q', cvt)
        self.put(cvt + 216, 'Q', self.matrix_stub)
        self.put(cvt + 184, 'Q', self.matrix_stub)
        self.put(cvt + 16, 'Q', 0x20001000)
        self.put(drawable + 32, 'Q', canvas)
        return {'pen': pen, 'manager': manager, 'drawable': drawable, 'rt': rt,
                'vector': vec, 'vptr': vptr, 'rt_vptr': rt_vptr,
                'native_version': version, 'native_size': native_size}

    def make_motion(self, case):
        self.current_motion = case
        count = len(case['points'])
        points = self.alloc(count * 8)
        self.put(points, 'f' * (count * 2), *(v for xy in case['points'] for v in xy))
        pressures, times = self.alloc(count * 4), self.alloc(count * 4)
        tilts, orientations = self.alloc(count * 4), self.alloc(count * 4)
        self.put(pressures, 'f' * count, *case['pressures'])
        self.put(times, 'i' * count, *case['times'])
        self.put(tilts, 'f' * count, *case['tilts'])
        self.put(orientations, 'f' * count, *case['orientations'])
        self.put(CALL_SP, 'Q', orientations)
        event, rect = self.alloc(64), self.alloc(32)
        self.call(MOTION_CTOR, [event, case['tool_type'], count, points, pressures,
                                case['times'][0], times, tilts], instruction_limit=3_000_000)
        return event, rect

    def run_motion(self, case):
        state = self.setup_native_pen()
        self.current_draw_line = self.syms[DRAW_LINE.format(version=state['native_version'])]
        profile = self._draw_line_state(state['drawable'])
        pen_settings = self.get(self.get(state['drawable'] + 72, 'Q')[0], 'Q')[0]
        profile['settings_count_limits'] = list(self.get(pen_settings + 32, '2I'))
        event, rect = self.make_motion(case)
        begin_calls, begin_records = len(self.callback_records), len(self.buffer_records)
        self.call(MOTION_REDRAW.format(version=state['native_version']),
                  [state['drawable'], event, rect], instruction_limit=5_000_000)
        new_calls = self.callback_records[begin_calls:]
        records = self.buffer_records[begin_records:]
        if not new_calls or any(r['count_f32'] != 11 * i for i, r in enumerate(records, 1)):
            raise AssertionError('MotionEvent redraw did not emit 11-float per-instance records')
        if any(not r.get('outcome') or not isinstance(r.get('state_after'), dict)
               for r in self.draw_line_gate_records):
            raise AssertionError('MotionEvent drawLine gate trace is missing an outcome or state snapshot')
        for record in self.draw_line_gate_records:
            timing = record.get('accepted_timing')
            if record['outcome'] == 'accepted':
                if not timing or timing['delta_microseconds_i64'] <= 0 or 'smoothed_speed_bits_le' not in timing:
                    raise AssertionError('accepted drawLine lacks complete positive native timing')
            elif timing is not None:
                raise AssertionError('rejected drawLine unexpectedly reached the timing update')
        rect_values = list(self.get(rect, '4f'))
        return {'label': case['name'], 'path': f'native MotionEvent constructor/getters -> factory-selected GLV{state["native_version"]} RedrawPen -> native drawPattern -> RTV1::AddPoint',
                'synthetic_input': {k: case[k] for k in ('points', 'pressures', 'times', 'tilts', 'orientations', 'tool_type')},
                'admission': self.admission(state), 'bounds_f32': rect_values,
                'draw_line_gate_profile_before_redraw': profile,
                'draw_line_gate_records': self.draw_line_gate_records,
                'callback_arguments': new_calls, 'buffer_records': records,
                'native_host_boundaries': self.boundaries()}

    def run_object_core(self, case):
        state = self.setup_native_pen()
        self.current_motion = case
        self.current_draw_line = self.syms[DRAW_LINE.format(version=state['native_version'])]
        count = len(case['points'])
        points, pressures, times = self.alloc(count * 8), self.alloc(count * 4), self.alloc(count * 4)
        tilts, orientations = self.alloc(count * 4), self.alloc(count * 4)
        self.put(points, 'f' * (count * 2), *(v for xy in case['points'] for v in xy))
        self.put(pressures, 'f' * count, *case['pressures'])
        self.put(times, 'i' * count, *case['times'])
        self.put(tilts, 'f' * count, *case['tilts'])
        self.put(orientations, 'f' * count, *case['orientations'])
        self.current_object = {'points': case['points'], 'tool_type': case['tool_type'], 'points_ptr': points,
                               'version': state['native_version'],
                               'pressures_ptr': pressures, 'times_ptr': times,
                               'tilts_ptr': tilts, 'orientations_ptr': orientations}
        stroke, rect = self.alloc(256), self.alloc(32)
        begin_calls, begin_records = len(self.callback_records), len(self.buffer_records)
        begin_lines = len(self.object_line_records)
        self.stop_before_end_pen = True
        self.call(OBJECT_REDRAW.format(version=state['native_version']),
                  [state['drawable'], stroke, rect], instruction_limit=5_000_000)
        self.stop_before_end_pen = False
        new_calls = self.callback_records[begin_calls:]
        records = self.buffer_records[begin_records:]
        line_calls = self.object_line_records[begin_lines:]
        if not line_calls:
            raise AssertionError('ObjectStroke redraw did not reach native drawLine')
        return {'label': case['name'], 'path': f'native GLV{state["native_version"]} RedrawPen(ObjectStroke) -> synthetic ObjectStroke getters -> native redraw/drawLine/RTV1::AddPoint; endPen intercepted before event finalization',
                'synthetic_input': {k: case[k] for k in ('points', 'pressures', 'times', 'tilts', 'orientations', 'tool_type')},
                'admission': self.admission(state), 'bounds_f32': list(self.get(rect, '4f')),
                'drawLine_arguments': line_calls,
                'callback_arguments': new_calls, 'buffer_records': records,
                'native_host_boundaries': self.boundaries()}

    def admission(self, state):
        return {'constructor': CTOR, 'actual_getVersion': state['native_version'],
                'actual_GetSize_f32': state['native_size'], 'actual_GetStrokeDrawableGL_vptr': hex(state['vptr']),
                'selected_GLVx_vtable_symbol_plus_16': hex(self.syms[f"vtable for SPen::BrushPenStrokeDrawableGLV{state['native_version']}"] + 16),
                'actual_RT_vptr': hex(state['rt_vptr']),
                'constructed_RTV1_vtable_symbol_plus_16': hex(self.syms['vtable for SPen::BrushPenStrokeDrawableRTV1'] + 16),
                'constructor_setting_parser_inputs': list(self.setting_inputs)}

    def boundaries(self):
        return {'host_stub_calls': dict(sorted(self.host_boundary_calls.items())),
                'reached_import_calls': dict(sorted(self.import_calls.items())),
                'explicitly_bounded': ['PenGLDataManager::GetMsgQueue returns a local queue object',
                                       'PenReturnCallback buffer/rect attachment and destruction are host callbacks',
                                       'SPen::String destructor is a no-op host boundary; parser numeric calls are modeled',
                                       '__emutls_get_address uses a bounded per-control TLS allocation',
                                       'pthread mutex lifecycle is modeled as local success',
                                       '__cxa_guard_acquire/release use single-thread first-initialization semantics',
                                       'matrix method returns the explicit identity matrix',
                                       'no GLES shader/texture upload or pixels were executed']}


def run_case(case, path='motion'):
    native = NativeBrush()
    native.import_calls = {}
    native.active_import = '<none>'
    native.stop_before_end_pen = False
    native.vector = 0
    if path == 'object-core':
        return native.run_object_core(case)
    return native.run_motion(case)


MOTION_CASES = [
    {'name': 'synthetic-motion-neutral-tool2', 'tool_type': 2,
     'points': [[0, 0], [4, 2], [9, 5]], 'pressures': [0.25, 0.5, 0.75],
     'times': [10, 20, 30], 'tilts': [0, 0, 0], 'orientations': [0, 0, 0]},
    {'name': 'synthetic-motion-tool1-pressure-tilt-orientation', 'tool_type': 1,
     'points': [[2, 1], [5, 4], [8, 7], [13, 8]], 'pressures': [0.2, 0.45, 0.7, 0.9],
     'times': [100, 112, 129, 150], 'tilts': [0.1, 0.25, 0.4, 0.6],
     'orientations': [0.05, 0.15, 0.3, 0.5]},
    {'name': 'synthetic-motion-tool3-pressure-orientation', 'tool_type': 3,
     'points': [[-3, 2], [-1, 6], [2, 9]], 'pressures': [0.35, 0.65, 0.95],
     'times': [200, 220, 255], 'tilts': [0.2, 0.35, 0.5],
     'orientations': [-0.2, -0.1, 0.1]},
    {'name': 'synthetic-motion-tool2-pressure-low', 'tool_type': 2,
     'points': [[0, 0], [1, 0], [3, 1], [6, 2], [10, 4]],
     'pressures': [0.01, 0.01, 0.01, 0.01, 0.01],
     'times': [0, 10, 20, 30, 40], 'tilts': [0, 0.1, 0.25, 0.4, 0.6],
     'orientations': [0, 0.1, 0.2, 0.35, 0.55]},
    {'name': 'synthetic-motion-tool2-pressure-above-one', 'tool_type': 2,
     'points': [[0, 0], [1, 0], [3, 1], [6, 2], [10, 4]],
     'pressures': [0.4, 0.8, 1.2, 1.5, 2.0],
     'times': [0, 2, 5, 9, 15], 'tilts': [0, 0.1, 0.25, 0.4, 0.6],
     'orientations': [0, 0.1, 0.2, 0.35, 0.55]},
    {'name': 'synthetic-motion-tool1-pressure-insensitive-multisegment', 'tool_type': 1,
     'points': [[0, 0], [1, 0], [3, 1], [6, 2], [10, 4]],
     'pressures': [0.05, 0.2, 0.4, 0.7, 1.0],
     'times': [10, 15, 30, 60, 110], 'tilts': [0, 0.1, 0.25, 0.4, 0.6],
     'orientations': [0, 0.1, 0.2, 0.35, 0.55]},
    {'name': 'synthetic-motion-tool3-pressure-insensitive-multisegment', 'tool_type': 3,
     'points': [[0, 0], [1, 0], [3, 1], [6, 2], [10, 4]],
     'pressures': [1.0, 0.7, 0.4, 0.2, 0.05],
     'times': [10, 13, 19, 28, 40], 'tilts': [0, 0.1, 0.25, 0.4, 0.6],
     'orientations': [0, 0.1, 0.2, 0.35, 0.55]},
    {'name': 'synthetic-motion-tool2-uneven-fast-slow-movement-gate', 'tool_type': 2,
     'points': [[0, 0], [0.1, 0], [2, 0], [2.1, 0], [12, 0]],
     'pressures': [0.2, 0.3, 0.4, 0.5, 0.6],
     'times': [100, 101, 120, 121, 160], 'tilts': [0, 0.1, 0.25, 0.4, 0.6],
     'orientations': [0, 0.1, 0.2, 0.35, 0.55]},
    {'name': 'synthetic-motion-tool1-alternating-distance-gate', 'tool_type': 1,
     'points': [[0, 0], [4, 0], [8, 0], [12, 0], [16, 0]],
     'pressures': [0.5, 0.5, 0.5, 0.5, 0.5],
     'times': [0, 10, 20, 30, 40], 'tilts': [0, 0, 0, 0, 0],
     'orientations': [0, 0, 0, 0, 0]},
]

OBJECT_CASE = {
    'name': 'synthetic-object-stroke-tool2', 'tool_type': 2,
    'points': [[1, -2], [3, 1], [6, 4]], 'pressures': [0.3, 0.6, 0.85],
    'times': [300, 315, 340], 'tilts': [0.12, 0.32, 0.55],
    'orientations': [0.2, 0.35, 0.55],
}


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true', help='replace the local native-output fixture')
    mode.add_argument('--check-fixture', action='store_true', help='compare native output with the checked fixture')
    args = parser.parse_args()
    motion_results = [run_case(case) for case in MOTION_CASES]
    object_result = run_case(OBJECT_CASE, 'object-core')
    result = {'artifact': 'Samsung Notes 4.4.45.37 native BrushPen',
              'evidence_scope': 'Synthetic saved-style inputs execute the fresh-constructor-selected native GL version and RTV1 geometry writer. No captured BrushPen device witness, GPU texture upload, shader execution, or pixels.',
              'library_sha256': HASHES,
              'native_numeric_constants': numeric_constants(),
              'shader_resources': {
                  'RTV1_vertex_elf_address': hex(0x3e374),
                  'RTV1_vertex_source': elf_string(LIB / 'libSPenBrushPen.so', 0x3e374),
                  'RTV1_fragment_elf_address': hex(0x3e69a),
                  'RTV1_fragment_source': elf_string(LIB / 'libSPenBrushPen.so', 0x3e69a),
              },
              'vertex_layout': {'static_vertex_buffer': {'attributes': [{'location': 0, 'type_enum': 3, 'type': 'float32', 'components': 4, 'stride_bytes': 16, 'divisor': 0}],
                                                        'quad_vertices': 4, 'shader_input': 'vec4 aRectPoint'},
                              'instance_buffer': {'attributes': [{'location': 1, 'components': 3, 'shader_input': 'vec3 aPointInfo'},
                                                                   {'location': 2, 'components': 1, 'shader_input': 'float inputTextureID'},
                                                                   {'location': 3, 'components': 2, 'shader_input': 'vec2 inputTilt'},
                                                                   {'location': 4, 'components': 2, 'shader_input': 'vec2 inputDirection'},
                                                                   {'location': 5, 'components': 3, 'shader_input': 'unused by the RTV1 shader'}],
                                                'stride_bytes': 44, 'stride_f32': 11, 'divisor': 1,
                                                'texture_id_consumer': 'shader floors word3 for tile offset; fract(word3)*1000/255 is opacity'},
              },
                              'native_methods': {'RTV1_AddPoint': 'SPen::BrushPenStrokeDrawableRTV1::AddPoint',
                                                 'RTV1_Update_instance_count': 'float32_count / 11',
                                                 'GraphicsObject_DrawInstanced': 'two VBOs; per-attribute glVertexAttribDivisor'},
              'texture_factory_contract': {
                  'scope': 'static call-site evidence only; buildTextures/setTextures and SPGraphicsFactory bitmap creation were not executed',
                  'factory': 'SPen::SPGraphicsFactory::CreateBitmap',
                  'resource_keys': ['BrushPenStrokeDrawableRTV1::buildTextures mPatternsBitmap',
                                    'BrushPenStrokeDrawableRTV1::buildTextures mGrainBitmap'],
                  'source_data_pointer': 'null at both captured call sites; resource bytes are supplied by the host factory',
                  'first_dimensions': ['3 * input width', '3 * input height'],
                  'second_dimensions': ['integer conversion of native stored width/height fields'],
                  'upload_identity_or_bitmap_hash': 'unavailable without executing the host SPGraphicsFactory/GPU boundary',
              },
                              'synthetic_cases': motion_results + [object_result]}
    out = ROOT / 'conformance/brushpen-saved-native.json'
    encoded = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        out.write_text(encoded)
        print(f'wrote {out}')
        return
    if not out.exists():
        raise SystemExit(f'missing fixture {out}; review this native output before using --write-fixture')
    expected = json.loads(out.read_text())
    # Host addresses are allocation-order-dependent; compare stable geometry,
    # ABI bits, library hashes, and native version selection only.
    def stable(document):
        return json.dumps(document, sort_keys=True)
    if stable(result) != stable(expected):
        raise AssertionError('native BrushPen output differs from conformance fixture; inspect with --write-fixture only after review')
    version = motion_results[0]['admission']['actual_getVersion']
    print(f'BrushPen V{version} native oracle passed: {len(motion_results)} synthetic MotionEvent cases + ObjectStroke core case')


if __name__ == '__main__':
    main()
