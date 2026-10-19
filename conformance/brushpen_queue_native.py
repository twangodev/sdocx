"""Execute a bounded synthetic BrushPen native queue, clip, and paint path."""
import argparse
import base64
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import zlib
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'conformance/brushpen-queue-native.json'
GEOMETRY = ROOT / 'conformance/brushpen-saved-native.json'
GEOMETRY_RUNNER = ROOT / 'conformance/brushpen_saved_native.py'
POSITIVE_RUNNER = ROOT / 'conformance/brushpen_paint_positive_native.py'
TEXTURE_RUNNER = ROOT / 'conformance/brushpen_texture_native.py'
TEXTURE_FIXTURE = ROOT / 'conformance/brushpen-texture-native.json'
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_paint_positive_native as positive
import brushpen_saved_native as geometry
from unicorn import UC_HOOK_MEM_WRITE, UC_HOOK_MEM_READ_UNMAPPED, arm64_const as R

OBJECT_CASE = geometry.OBJECT_CASE


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def qwords(raw):
    return [f'0x{x:x}' for x in struct.unpack('<' + 'Q' * (len(raw) // 8), raw)]


class HandoffProbe(positive.PositiveProbe):
    def __init__(self):
        super().__init__()
        self.queue_submissions = []
        self.allocation_trace = []
        self.string_append_trace = []
        self.pending_string_appends = {}
        self.shader_sources = []
        self.shader_logs = []
        self.vsnprintf_calls = []
        self.fopen_calls = []
        self.errno_storage = self.alloc(4)
        self.put(self.errno_storage,'I',0)
        self.runtime_phase = 'constructor/setup'
        self.string_append_symbols = {a: n for n, a in self.syms.items()
            if 'basic_string<char' in n and 'append(char const*)' in n}
        for name in ('operator new(unsigned long)', 'operator new[](unsigned long)',
                     'operator new(unsigned long, std::nothrow_t const&)',
                     'operator new[](unsigned long, std::nothrow_t const&)'):
            old_allocator = self.callbacks.get(name)
            if old_allocator:
                def tracked_allocator(old=old_allocator, kind=name):
                    size = self.x(0)
                    if size > 0x1000000: raise RuntimeError(f'{kind} request exceeds probe bound: {size:#x}')
                    old()
                    self.allocation_trace.append({'phase': self.runtime_phase, 'size': size,
                        'result': hex(self.x(0)), 'kind': kind})
                self.callbacks[name] = tracked_allocator
        self.rt_writes = []
        self.native_call_entries = []
        self.rect_traces = []
        self.callback_this = 0
        self.pending_bitmap_returns = set()
        self.native_bitmap_returns = []
        self.watch_rt = 0
        self.paint_state = None
        self.draw_args = None
        self.draw_entry_records = []
        self.draw_pc_trace = []
        self.queue_obj = 0
        self.queue_get_pc, self.queue_submit_pc, self.canvas_bounds_pc = 0x20001040, 0x20001060, 0x20001080
        self.bitmap_queue_submit_pc, self.bitmap_queue_return_pc = 0x20001100, 0x20001120
        self.pending_bitmap_queue_returns = []
        self.bitmap_queue_tasks = []
        for pc in (self.queue_get_pc, self.queue_submit_pc, self.canvas_bounds_pc,
                   self.bitmap_queue_submit_pc, self.bitmap_queue_return_pc):
            self.u.mem_write(pc, b'\0' * 4)
        self.map_cxx()
        self.string_append_symbols = {a: n for n, a in self.syms.items()
            if 'basic_string<char' in n and 'append(char const*)' in n}
        self.callbacks.update({'glGenFramebuffers': self.gl_gen_framebuffers,
            'glBindFramebuffer': self.gl_bind_framebuffer,
            'glViewport': lambda: self._call_gl('glViewport',self.mesa.f['glViewport'],*[self.x(i) for i in range(4)]),
            'glInvalidateFramebuffer': self.gl_invalidate_framebuffer,
            'glFramebufferTexture2D': self.gl_framebuffer_texture_2d,
            'glCheckFramebufferStatus': self.gl_check_framebuffer_status,
            'crc32': self.crc32,
            'glCreateShader': self.gl_create_shader,
            'glShaderSource': self.gl_shader_source,
            'glCompileShader': self.gl_compile_shader,
            'glGetShaderiv': self.gl_get_shader_iv,
            'glGetShaderInfoLog': self.gl_get_shader_log,
            'glCreateProgram': self.gl_create_program,
            'glAttachShader': self.gl_attach_shader,
            'glDetachShader': self.gl_detach_shader,
            'glBindAttribLocation': self.gl_bind_attrib_location,
            'glLinkProgram': self.gl_link_program,
            'glGetProgramiv': self.gl_get_program_iv,
            'glGetProgramInfoLog': self.gl_get_program_log,
            'glGetProgramBinary': self.gl_get_program_binary,
            'glGetActiveUniform': self.gl_get_active_uniform,
            'glDeleteShader': self.gl_delete_shader,
            'glDeleteProgram': self.gl_delete_program,
            'glUseProgram': self.gl_use_program,
            'glGetUniformLocation': self.gl_get_uniform_location,
            'SPen::SPCanvasClipping::EnableClipRT(int)': self.clip_enable,
            'SPen::SPCanvasClipping::DisableClipRT()': self.clip_disable,
            'SPen::SPCanvasClipping::clear(int, int)': self.clip_clear})
        self.clip_calls = []
        self.created_framebuffers = []
        setup = self.setup_native_pen

        def setup_once():
            if self.paint_state is None:
                self.paint_state = setup()
                self.watch_rt = self.paint_state['rt']
            return self.paint_state
        self.setup_native_pen = setup_once
        self.u.hook_add(UC_HOOK_MEM_WRITE, self.memwrite)
        self.invalid_reads = []
        def invalid_read(u, access, address, size, value, data):
            self.invalid_reads.append({'address':hex(address),'size':size,
                'pc':hex(u.reg_read(R.UC_ARM64_REG_PC)),
                'x0_x7':[hex(self.x(i)) for i in range(8)],
                'x8_x15':[hex(u.reg_read(getattr(R,f'UC_ARM64_REG_X{i}'))) for i in range(8,16)]})
            return False
        self.u.hook_add(UC_HOOK_MEM_READ_UNMAPPED, invalid_read)

    def map_cxx(self):
        path = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenCxx.so'
        blob = path.read_bytes()
        actual = hashlib.sha256(blob).hexdigest()
        expected = '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'
        if actual != expected:
            raise RuntimeError(f'unverified Cxx DSO: {actual}')
        base = 7 * 0x1000000
        self.u.mem_map(base, 0x1000000)
        phoff = struct.unpack_from('<Q', blob, 32)[0]
        phentsize, phnum = struct.unpack_from('<HH', blob, 54)
        for i in range(phnum):
            kind, _flags, fileoff, vaddr, _paddr, filesz, _memsz, _align = struct.unpack_from(
                '<II6Q', blob, phoff + i * phentsize)
            if kind == 1:
                self.u.mem_write(base + vaddr, blob[fileoff:fileoff + filesz])
        dynamic = subprocess.check_output(['nm', '-D', str(path)], text=True)
        dynamic_demangled = subprocess.check_output(['nm', '-D', '-C', str(path)], text=True)
        raw_symbols = {}
        for line in dynamic.splitlines():
            parts = line.strip().split(None, 2)
            if len(parts) == 3 and parts[1] in ('T', 'W', 'D', 'B', 'V', 'R'):
                raw_symbols[parts[2]] = base + int(parts[0], 16)
        for line in dynamic_demangled.splitlines():
            parts = line.strip().split(None, 2)
            if len(parts) == 3 and parts[1] in ('T', 'W'):
                self.syms[parts[2]] = base + int(parts[0], 16)
        env = dict(os.environ)
        env['LD_LIBRARY_PATH'] = str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')
        dis = subprocess.check_output([str(geometry.OBJDUMP), '-d', '-C', '-j', '.plt', str(path)], env=env, text=True)
        for address, name in re.findall(r'^([0-9a-f]+) <(.+)@plt>:', dis, re.M):
            self.plt[base + int(address, 16)] = name
        entries, symbols = geometry.elf_relocations(blob)
        # Rendered PLT labels can be wrong; decode each stub's actual GOT slot
        # and join it to .rela.plt before allowing Cxx calls to execute.
        shoff = struct.unpack_from('<Q', blob, 40)[0]
        shentsize, shnum, shstrndx = struct.unpack_from('<HHH', blob, 58)
        shdrs = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * shentsize)
                 for i in range(shnum)]
        shstr_hdr = shdrs[shstrndx]
        shstr = blob[shstr_hdr[4]:shstr_hdr[4] + shstr_hdr[5]]
        section_names = {}
        for sh in shdrs:
            end = shstr.find(b'\0', sh[0])
            section_names[shstr[sh[0]:end].decode()] = sh
        rela_plt, plt_section = section_names['.rela.plt'], section_names['.plt']
        plt_relocations = {}
        for off in range(rela_plt[4], rela_plt[4] + rela_plt[5], rela_plt[9]):
            where, info, _addend = struct.unpack_from('<QQq', blob, off)
            plt_relocations[where] = symbols[info >> 32][0]
        demangled_by_address = {}
        for line in dynamic_demangled.splitlines():
            parts = line.strip().split(None, 2)
            if len(parts) == 3:
                try:
                    demangled_by_address[int(parts[0], 16)] = parts[2]
                except ValueError:
                    pass
        # Some Cxx PLT entries have two leading NOPs and their ADRP begins at
        # +8, so scan instructions rather than assuming a 16-byte entry base.
        for off in range(plt_section[4] + 0x20, plt_section[4] + plt_section[5] - 8, 4):
            va = plt_section[3] + off - plt_section[4]
            adrp, ldr = struct.unpack_from('<II', blob, off)
            if (adrp & 0x9f00001f) != 0x90000010 or (ldr & 0xffc003ff) != 0xf9400211:
                continue
            imm21 = (((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0x3)
            if imm21 & (1 << 20):
                imm21 -= 1 << 21
            slot = (va & ~0xfff) + (imm21 << 12) + (((ldr >> 10) & 0xfff) << 3)
            raw_name = plt_relocations.get(slot)
            if raw_name:
                symbol_address = raw_symbols.get(raw_name, 0) - base
                self.plt[base + va] = demangled_by_address.get(symbol_address, raw_name)
        old_raw = {}
        for name, dso_base in (('FountainPen', 0), ('PenCommon', 0x1000000), ('Base', 0x2000000),
                               ('BrushPen', 0x3000000), ('Graphics', 0x4000000), ('Renderer', 0x5000000),
                               ('Model', 0x6000000)):
            dso = ROOT / f'scratch/apk-analysis-native/arm64-v8a/libSPen{name}.so'
            for line in subprocess.check_output(['nm', '-D', str(dso)], text=True).splitlines():
                parts = line.strip().split(None, 2)
                if len(parts) == 3 and parts[1] in ('T', 'W', 'D', 'B', 'V', 'R'):
                    old_raw[parts[2]] = dso_base + int(parts[0], 16)
        for where, typ, index, addend in entries:
            value = None
            if typ == 0x403:
                value = base + addend
            elif typ in (0x101, 0x401):
                name, defined, symval = symbols[index]
                if defined and symval:
                    value = base + symval + addend
                elif name in raw_symbols:
                    value = raw_symbols[name] + addend
                elif name in old_raw:
                    value = old_raw[name] + addend
            if value is not None:
                self.u.mem_write(base + where, struct.pack('<Q', value & 0xffffffffffffffff))
        # Mirror the InkPen2 oracle's pinned libc++ initialization.  Inspect
        # relocated entries rather than the all-zero file image.
        init_info = struct.unpack_from('<Q', blob, 40)[0]
        init_ent, init_num, init_str = struct.unpack_from('<HHH', blob, 58)
        init_secs = [struct.unpack_from('<IIQQQQIIQQ', blob, init_info + i * init_ent)
                     for i in range(init_num)]
        init_names = blob[init_secs[init_str][4]:init_secs[init_str][4] + init_secs[init_str][5]]
        init = next((sec for sec in init_secs
                     if init_names[sec[0]:init_names.find(b'\0', sec[0])] == b'.init_array'), None)
        self.cxx_init_entries = []
        self.cxx_platform_queries = []
        def getauxval_adapter():
            key = self.x(0)
            self.cxx_platform_queries.append({'name': 'getauxval', 'type': key,
                                               'returned': 0, 'boundary': 'synthetic-no-HWCAP'})
            self.x(0, 0)
        self.callbacks['getauxval'] = getauxval_adapter
        def property_get_adapter():
            key = self.cstr(self.x(0))
            self.cxx_platform_queries.append({'name': '__system_property_get', 'key': key,
                                               'returned_length': 0,
                                               'boundary': 'synthetic-property-absent'})
            self.x(0, 0)
        self.callbacks['__system_property_get'] = property_get_adapter
        locale_ptr = self.alloc(64)
        locale_name = self.alloc(8)
        self.u.mem_write(locale_name, b'C\0')
        current_locale = [locale_ptr]
        def newlocale_adapter():
            mask, name = self.x(0), self.cstr(self.x(1))
            if name not in ('C', 'POSIX'):
                raise RuntimeError(f'unsupported Cxx locale name {name!r}')
            self.cxx_platform_queries.append({'name': 'newlocale', 'mask': mask,
                                               'locale': name, 'returned': hex(locale_ptr)})
            self.x(0, locale_ptr)
        def uselocale_adapter():
            old = current_locale[0]
            if self.x(0): current_locale[0] = self.x(0)
            self.x(0, old)
        self.callbacks['newlocale'] = newlocale_adapter
        self.callbacks['uselocale'] = uselocale_adapter
        self.callbacks['freelocale'] = lambda: self.x(0, 0)
        def mbtowc_adapter():
            destination, source, count = self.x(0), self.x(1), self.x(2)
            if not source:
                self.x(0, 0); return
            if not count:
                self.x(0, 0xffffffffffffffff); return
            byte = self.get(source, 'B')[0]
            if byte >= 0x80:
                raise RuntimeError(f'mbtowc adapter only supports C-locale ASCII; byte={byte:#x}')
            if destination: self.put(destination, 'I', byte)
            self.x(0, 0 if byte == 0 else 1)
        self.callbacks['mbtowc'] = mbtowc_adapter
        self.callbacks['__ctype_get_mb_cur_max'] = lambda: self.x(0, 1)
        def vsnprintf_probe():
            dst, size, fmt, va = (self.x(i) for i in range(4))
            format_bytes = self.native_cstr_bytes(fmt,4096)
            if format_bytes != b'/sh%0lu.dat' or size != 128:
                raise RuntimeError(f'unsupported native vsnprintf format/bound: {format_bytes!r}/{size}')
            stack, gr_top, vr_top, gr_offs, vr_offs = struct.unpack('<QQQii', self.u.mem_read(va,32))
            if gr_offs > -8 or gr_offs < -64:
                raise RuntimeError(f'unsupported va_list GPR offset: {gr_offs}')
            value_ptr = gr_top + gr_offs
            value = self.get(value_ptr,'Q')[0]
            result = f'/sh{value}.dat'.encode('ascii')
            if len(result) >= size: raise RuntimeError('native vsnprintf output exceeds supplied size')
            self.u.mem_write(dst,result+b'\0')
            self.vsnprintf_calls.append({'format':format_bytes.decode(),'size':size,
                'va_list':hex(va),'va_list_fields':[hex(stack),hex(gr_top),hex(vr_top),gr_offs,vr_offs],
                'value_address':hex(value_ptr),'value_u64':value,'output':result.decode()})
            self.x(0,len(result))
        self.callbacks['vsnprintf'] = vsnprintf_probe
        def fopen_adapter():
            path, mode = self.cstr(self.x(0)), self.cstr(self.x(1))
            if not re.fullmatch(r'/sh[0-9]+\.dat', path) or mode not in ('rb', 'r', 'wb'):
                raise RuntimeError(f'unsupported native fopen request: {path!r}/{mode!r}')
            self.fopen_calls.append({'path':path,'mode':mode,'returned':None,
                                     'boundary':'synthetic absent shader cache file'})
            self.put(self.errno_storage,'I',2)
            self.x(0,0)
        self.callbacks['fopen'] = fopen_adapter
        self.callbacks['__errno'] = lambda: self.x(0,self.errno_storage)
        def strlen_chk_adapter():
            pointer, limit = self.x(0), self.x(1)
            if not 0 < limit <= 1 << 20: raise RuntimeError(f'__strlen_chk bound invalid: {limit}')
            raw = self.native_cstr_bytes(pointer, limit)
            self.x(0, len(raw))
        self.callbacks['__strlen_chk'] = strlen_chk_adapter
        def strncpy_chk2_adapter():
            dst, src, count, dst_size, src_size = (self.x(i) for i in range(5))
            if count > 4096 or count > dst_size or count > src_size:
                raise RuntimeError(f'__strncpy_chk2 bound mismatch: n={count} dst={dst_size} src={src_size}')
            raw = bytes(self.u.mem_read(src,count))
            zero = raw.find(b'\0')
            if zero >= 0: raw = raw[:zero] + bytes(count-zero)
            self.u.mem_write(dst,raw); self.x(0,dst)
        self.callbacks['__strncpy_chk2'] = strncpy_chk2_adapter
        for name in ('pthread_mutex_init','pthread_mutex_destroy','pthread_mutex_lock','pthread_mutex_unlock',
                     'pthread_mutexattr_init','pthread_mutexattr_destroy','pthread_mutexattr_settype',
                     'pthread_rwlock_rdlock','pthread_rwlock_wrlock','pthread_rwlock_unlock',
                     'pthread_cond_broadcast','pthread_cond_signal','pthread_cond_init','pthread_cond_destroy'):
            self.callbacks[name] = lambda: self.x(0, 0)
        once = 0x20000080
        self.u.mem_write(once, bytes.fromhex('f90000f9e80000b5fd7bbfa9fd030091280080d2080000f920003fd6fd7bc1a8000080d2c0035fd6'))
        self.syms['pthread_once'] = once
        self.callbacks.pop('pthread_once', None)
        if init:
            for offset in range(0, init[5], 8):
                target = self.get(base + init[3] + offset, 'Q')[0]
                if not target:
                    continue
                self.cxx_init_entries.append(target)
                for i in range(4): self.x(i, 0)
                self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
                self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
                try:
                    self.u.emu_start(target, 0x200f1000, count=3_000_000)
                except Exception as exc:
                    raise RuntimeError(f'Cxx init exception target={target:#x} pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={self.x(30):#x} x0={self.x(0):#x}: {exc}') from exc
                if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
                    raise RuntimeError(f'Cxx init failed at {target:#x}, pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x}')
        # Match the pinned InkPen2 loader's dependency init order.  The
        # Renderer globals consumed by SPCanvasClipping live in these arrays.
        self.native_init_arrays = [{'library':'libSPenCxx.so','entries':[hex(x) for x in self.cxx_init_entries]}]
        init_libraries = [
            ('FountainPen','libSPenFountainPen.so',0), ('PenCommon','libSPenPenCommon.so',0x1000000),
            ('Base','libSPenBase.so',0x2000000), ('BrushPen','libSPenBrushPen.so',0x3000000),
            ('Graphics','libSPenGraphics.so',0x4000000), ('Renderer','libSPenRenderer.so',0x5000000),
            ('Model','libSPenModel.so',0x6000000)]
        for dso, filename, dso_base in init_libraries:
            dso_path = ROOT / 'scratch/apk-analysis-native/arm64-v8a' / filename
            data = dso_path.read_bytes()
            shoff = struct.unpack_from('<Q',data,40)[0]
            ent,num,stridx = struct.unpack_from('<HHH',data,58)
            sections = [struct.unpack_from('<IIQQQQIIQQ',data,shoff+i*ent) for i in range(num)]
            names = data[sections[stridx][4]:sections[stridx][4]+sections[stridx][5]]
            section = next(sec for sec in sections if names[sec[0]:names.find(b'\0',sec[0])] == b'.init_array')
            ran=[]
            for offset in range(0,section[5],8):
                target = self.get(dso_base+section[3]+offset,'Q')[0]
                if not target: continue
                for i in range(4): self.x(i,0)
                self.u.reg_write(R.UC_ARM64_REG_SP,0x200f0000)
                self.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
                try: self.u.emu_start(target,0x200f1000,count=3_000_000)
                except Exception as exc:
                    raise RuntimeError(f'{filename} init exception target={target:#x} pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} lr={self.x(30):#x}: {exc}') from exc
                if self.u.reg_read(R.UC_ARM64_REG_PC)!=0x200f1000:
                    raise RuntimeError(f'{filename} init failed target={target:#x} pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x}')
                ran.append(target)
            self.native_init_arrays.append({'library':filename,'entries':[hex(x) for x in ran]})

    def gl_gen_framebuffers(self):
        count, out = self.x(0), self.x(1)
        if not 0 < count <= 8:
            raise RuntimeError(f'FBO count outside adapter bound: {count}')
        ids = (positive.C.c_uint * count)()
        self._call_gl('glGenFramebuffers', self.mesa.f['glGenFramebuffers'], count, ids)
        for i, value in enumerate(ids):
            self.put(out + i * 4, 'I', value)
            self.created_framebuffers.append(int(value))

    def gl_invalidate_framebuffer(self):
        target, count, pointer = self.x(0), self.x(1), self.x(2)
        if not 0 <= count <= 16: raise RuntimeError(f'framebuffer attachment count outside bound: {count}')
        attachments = self.get(pointer,'I'*count) if count else []
        array = (positive.C.c_uint*count)(*attachments) if count else None
        self._call_gl('glInvalidateFramebuffer',self._glfn('glInvalidateFramebuffer',None,
            [positive.C.c_uint,positive.C.c_int,positive.C.POINTER(positive.C.c_uint)]),
            target,count,array)

    def gl_bind_framebuffer(self):
        args = [self.x(i) for i in range(2)]
        self._call_gl('glBindFramebuffer', self.mesa.f['glBindFramebuffer'], *args)

    def gl_framebuffer_texture_2d(self):
        args = [self.x(i) for i in range(5)]
        self._call_gl('glFramebufferTexture2D', self.mesa.f['glFramebufferTexture2D'], *args)

    def gl_check_framebuffer_status(self):
        status = self._call_gl('glCheckFramebufferStatus', self.mesa.f['glCheckFramebufferStatus'], self.x(0))
        self.x(0, status)

    def crc32(self):
        seed, ptr, size = self.x(0), self.x(1), self.x(2)
        if not 0 <= size <= 16 * 1024 * 1024:
            raise RuntimeError(f'crc32 size outside adapter bound: {size}')
        raw = bytes(self.u.mem_read(ptr, size)) if size else b''
        self.x(0, zlib.crc32(raw, seed) & 0xffffffff)

    def _glfn(self, name, restype, argtypes):
        key = '_native_' + name
        if not hasattr(self, key):
            setattr(self, key, positive.ip.cfunc(self.mesa.gl, name, restype, argtypes))
        return getattr(self, key)

    def gl_create_shader(self):
        result = self._call_gl('glCreateShader', self._glfn('glCreateShader', positive.C.c_uint, [positive.C.c_uint]), self.x(0))
        self.x(0, result)

    def gl_shader_source(self):
        c, n, strings, lengths = positive.C, self.x(1), self.x(2), self.x(3)
        if not 0 < n <= 16:
            raise RuntimeError(f'glShaderSource count outside bound: {n}')
        ptrs = self.get(strings, 'Q' * n)
        lens = self.get(lengths, 'i' * n) if lengths else [None] * n
        keepalive, cstrings = [], (c.c_char_p * n)()
        clens = (c.c_int * n)()
        for i, (ptr, length) in enumerate(zip(ptrs, lens)):
            if not ptr:
                raise RuntimeError('glShaderSource received null source')
            if length is None or length < 0:
                raw = self.native_cstr_bytes(ptr, 1 << 20)
            else:
                raw = bytes(self.u.mem_read(ptr, min(length, 1 << 20)))
            buf = c.create_string_buffer(raw)
            self.shader_sources.append({'phase': self.runtime_phase, 'shader_id': self.x(0),
                'source_index': i, 'source_pointer': hex(ptr), 'declared_length': length,
                'length': len(raw), 'source_hex': raw.hex(), 'sha256': sha256(raw)})
            keepalive.append(buf); cstrings[i] = c.cast(buf, c.c_char_p); clens[i] = len(raw)
        args = [self.x(0), n, cstrings, clens]
        self._call_gl('glShaderSource', self._glfn('glShaderSource', None,
            [c.c_uint,c.c_int,c.POINTER(c.c_char_p),c.POINTER(c.c_int)]), *args)

    def native_cstr_bytes(self, pointer, limit):
        result = bytearray()
        while len(result) < limit:
            part = bytes(self.u.mem_read(pointer + len(result), min(4096, limit - len(result))))
            end = part.find(b'\0')
            if end >= 0:
                return bytes(result) + part[:end]
            result.extend(part)
        raise RuntimeError(f'native C string at {pointer:#x} is not terminated within {limit} bytes')

    def gl_compile_shader(self):
        self._call_gl('glCompileShader', self._glfn('glCompileShader', None, [positive.C.c_uint]), self.x(0))

    def gl_get_shader_iv(self):
        c, out = positive.C, self.x(2); value = c.c_int()
        self._call_gl('glGetShaderiv', self._glfn('glGetShaderiv', None,[c.c_uint,c.c_uint,c.POINTER(c.c_int)]), self.x(0),self.x(1),c.byref(value))
        self.put(out, 'i', value.value)

    def gl_get_shader_log(self):
        c, cap, outlen, out = positive.C, self.x(1), self.x(2), self.x(3)
        if not 0 < cap <= 1 << 20: raise RuntimeError('shader log bound exceeded')
        buf=c.create_string_buffer(cap); written=c.c_int()
        self._call_gl('glGetShaderInfoLog',self._glfn('glGetShaderInfoLog',None,[c.c_uint,c.c_int,c.POINTER(c.c_int),c.c_char_p]),self.x(0),cap,c.byref(written),buf)
        self.shader_logs.append({'phase': self.runtime_phase, 'shader_id': self.x(0),
                                 'bytes_written': written.value, 'text': buf.raw[:written.value].decode(errors='replace')})
        if outlen:self.put(outlen,'i',written.value)
        if out:self.u.mem_write(out,buf.raw[:cap])

    def gl_create_program(self):
        self.x(0,self._call_gl('glCreateProgram',self._glfn('glCreateProgram',positive.C.c_uint,[])))

    def gl_attach_shader(self):
        self._call_gl('glAttachShader',self._glfn('glAttachShader',None,[positive.C.c_uint,positive.C.c_uint]),self.x(0),self.x(1))

    def gl_detach_shader(self):
        self._call_gl('glDetachShader',self._glfn('glDetachShader',None,[positive.C.c_uint,positive.C.c_uint]),self.x(0),self.x(1))

    def gl_bind_attrib_location(self):
        self._call_gl('glBindAttribLocation',self._glfn('glBindAttribLocation',None,[positive.C.c_uint,positive.C.c_uint,positive.C.c_char_p]),self.x(0),self.x(1),self.cstr(self.x(2)).encode())

    def gl_link_program(self):
        self._call_gl('glLinkProgram',self._glfn('glLinkProgram',None,[positive.C.c_uint]),self.x(0))

    def gl_get_program_iv(self):
        c, out = positive.C, self.x(2); value=c.c_int()
        self._call_gl('glGetProgramiv',self._glfn('glGetProgramiv',None,[c.c_uint,c.c_uint,c.POINTER(c.c_int)]),self.x(0),self.x(1),c.byref(value)); self.put(out,'i',value.value)

    def gl_get_program_log(self):
        c, cap, outlen, out=positive.C,self.x(1),self.x(2),self.x(3)
        if not 0<cap<=1<<20:raise RuntimeError('program log bound exceeded')
        buf=c.create_string_buffer(cap);written=c.c_int()
        self._call_gl('glGetProgramInfoLog',self._glfn('glGetProgramInfoLog',None,[c.c_uint,c.c_int,c.POINTER(c.c_int),c.c_char_p]),self.x(0),cap,c.byref(written),buf)
        if outlen:self.put(outlen,'i',written.value)
        if out:self.u.mem_write(out,buf.raw[:cap])

    def gl_get_program_binary(self):
        c = positive.C
        program, cap, length_ptr, format_ptr, output = [self.x(i) for i in range(5)]
        if not 0 <= cap <= 1 << 20: raise RuntimeError(f'program binary size outside bound: {cap}')
        size = c.c_int(); fmt = c.c_uint(); buf = c.create_string_buffer(max(cap, 1))
        self._call_gl('glGetProgramBinary', self._glfn('glGetProgramBinary', None,
            [c.c_uint,c.c_int,c.POINTER(c.c_int),c.POINTER(c.c_uint),c.c_void_p]),
            program, cap, c.byref(size), c.byref(fmt), c.cast(buf,c.c_void_p))
        if length_ptr: self.put(length_ptr,'i',size.value)
        if format_ptr: self.put(format_ptr,'I',fmt.value)
        if output and size.value: self.u.mem_write(output,buf.raw[:size.value])

    def gl_get_active_uniform(self):
        c = positive.C
        program, index, cap = self.x(0), self.x(1), self.x(2)
        if not 0 < cap <= 4096: raise RuntimeError(f'active-uniform name bound invalid: {cap}')
        length,size,kind=c.c_int(),c.c_int(),c.c_uint(); name=c.create_string_buffer(cap)
        self._call_gl('glGetActiveUniform',self._glfn('glGetActiveUniform',None,
            [c.c_uint,c.c_uint,c.c_int,c.POINTER(c.c_int),c.POINTER(c.c_int),c.POINTER(c.c_uint),c.c_char_p]),
            program,index,cap,c.byref(length),c.byref(size),c.byref(kind),name)
        if self.x(3): self.put(self.x(3),'i',length.value)
        if self.x(4): self.put(self.x(4),'i',size.value)
        if self.x(5): self.put(self.x(5),'I',kind.value)
        if self.x(6): self.u.mem_write(self.x(6),name.raw[:cap])

    def gl_delete_shader(self): self._call_gl('glDeleteShader',self._glfn('glDeleteShader',None,[positive.C.c_uint]),self.x(0))
    def gl_delete_program(self): self._call_gl('glDeleteProgram',self._glfn('glDeleteProgram',None,[positive.C.c_uint]),self.x(0))
    def gl_use_program(self): self._call_gl('glUseProgram',self._glfn('glUseProgram',None,[positive.C.c_uint]),self.x(0))

    def gl_get_uniform_location(self):
        name=self.cstr(self.x(1)).encode()
        result=self._call_gl('glGetUniformLocation',self._glfn('glGetUniformLocation',positive.C.c_int,[positive.C.c_uint,positive.C.c_char_p]),self.x(0),name)
        self.x(0,result)

    def clip_enable(self):
        self.clip_calls.append({'method': 'EnableClipRT', 'x0_x2': [hex(self.x(i)) for i in range(3)]})

    def clip_disable(self):
        self.clip_calls.append({'method': 'DisableClipRT', 'x0': hex(self.x(0))})

    def clip_clear(self):
        self.clip_calls.append({'method': 'clear(int,int)', 'x0_x2': [hex(self.x(i)) for i in range(3)]})

    def memwrite(self, u, access, address, size, value, data):
        if self.watch_rt and self.watch_rt <= address < self.watch_rt + 128:
            self.rt_writes.append({'offset': address - self.watch_rt, 'size': size,
                                   'value_hex': f'0x{value:x}'})

    def hook(self, u, address, size, data):
        if 0x307774c <= address <= 0x3077a04 and len(self.draw_pc_trace) < 1024:
            self.draw_pc_trace.append(hex(address))
        create_bitmap = self.syms.get('SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)')
        if create_bitmap and address == create_bitmap:
            self.pending_bitmap_returns.add(u.reg_read(R.UC_ARM64_REG_LR))
        if address in self.pending_bitmap_returns:
            bitmap = self.x(0)
            self.native_bitmap_returns.append({'return_pc': hex(address), 'bitmap': hex(bitmap),
                'vptr': hex(self.get(bitmap, 'Q')[0]) if bitmap else '0'})
            self.pending_bitmap_returns.remove(address)
        symname = self.string_append_symbols.get(address)
        if symname:
            receiver, source = self.x(0), self.x(1)
            self.string_append_trace.append({'phase': self.runtime_phase, 'symbol': symname,
                'entry': hex(address), 'receiver': hex(receiver), 'source': hex(source),
                'receiver_before_24': bytes(u.mem_read(receiver, 24)).hex(),
                'source_ascii': self.cstr(source)[:512], 'source_hex': self.cstr(source).encode()[:512].hex(),
                'return_pc': hex(u.reg_read(R.UC_ARM64_REG_LR))})
            self.pending_string_appends[u.reg_read(R.UC_ARM64_REG_LR)] = self.string_append_trace[-1]
        if address in self.pending_string_appends:
            row = self.pending_string_appends.pop(address)
            receiver = int(row['receiver'],16)
            row['receiver_after_24'] = bytes(u.mem_read(receiver,24)).hex()
        rect_names = ('SPen::RectF::IsIntersect(SPen::Rect const&) const',
                      'SPen::RectF::IsIntersect(SPen::RectF const&) const',
                      'SPen::Rect::Width() const','SPen::Rect::Height() const',
                      'SPen::RectF::MakeRect(float, float)')
        for rect_name in rect_names:
            if self.syms.get(rect_name) == address:
                row={'method':rect_name,'phase':self.runtime_phase,
                     'x0_x2':[hex(self.x(i)) for i in range(3)],
                     's0_s3':[self.f(i) for i in range(4)],
                     'x0_first16':bytes(u.mem_read(self.x(0),16)).hex() if self.x(0) else None,
                     'x1_first16':bytes(u.mem_read(self.x(1),16)).hex() if self.x(1) else None,
                     'return_pc':hex(u.reg_read(R.UC_ARM64_REG_LR))}
                self.rect_traces.append(row)
        if address == self.queue_get_pc:
            self.x(0, self.queue_obj)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.bitmap_queue_submit_pc:
            task = self.x(1)
            vtable = self.get(task, 'Q')[0]
            method = self.get(vtable + 16, 'Q')[0]
            self.bitmap_queue_tasks.append({'task': hex(task), 'vtable': hex(vtable),
                'entry': hex(method), 'receiver': hex(task)})
            self.pending_bitmap_queue_returns.append(self.x(30))
            self.x(0, task)
            self.x(30, self.bitmap_queue_return_pc)
            u.reg_write(R.UC_ARM64_REG_PC, method)
            return
        if address == self.bitmap_queue_return_pc:
            if not self.pending_bitmap_queue_returns:
                raise RuntimeError('bitmap queue task returned without saved caller')
            u.reg_write(R.UC_ARM64_REG_PC, self.pending_bitmap_queue_returns.pop())
            return
        if address == self.queue_submit_pc:
            task = self.x(1)
            raw = bytes(u.mem_read(task, 96))
            self.queue_submissions.append({'task': task, 'queue': self.x(0),
                'vptr': self.get(task, 'Q')[0], 'raw96': raw,
                'words': struct.unpack('<12Q', raw)})
            self.x(0, 1)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.canvas_bounds_pc:
            u.mem_write(u.reg_read(R.UC_ARM64_REG_X8), b'\0' * 32)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        draw = self.syms.get('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)')
        if draw and address == draw:
            self.draw_args = [self.x(i) for i in range(3)]
            rt = self.x(0)
            vector_object = self.get(rt + 88, 'Q')[0]
            vec = self.get(vector_object, '3Q') if vector_object else [0, 0, 0]
            self.draw_entry_records.append({
                'receiver': hex(rt), 'bitmap': hex(self.x(1)), 'matrix_or_rect': hex(self.x(2)),
                'rt_flag_193': self.get(rt + 193, 'B')[0],
                'rt_flag_260': self.get(rt + 260, 'B')[0],
                'rt_plus_88_vector_object': hex(vector_object),
                'vector_begin_end_capacity': [hex(v) for v in vec],
                'vector_byte_count': vec[1] - vec[0] if vec[1] >= vec[0] else None,
                'bitmap_vptr': hex(self.get(self.x(1), 'Q')[0]),
            })
        if address == self.end_pen_plt and self.stop_before_end_pen:
            self.stop_before_end_pen = False
        watched = {
            self.syms.get('SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)'): 'PenReturnCallback::constructor',
            self.syms.get('SPen::PenReturnCallback::GetBuffer()'): 'PenReturnCallback::GetBuffer',
            self.syms.get('SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)'): 'PenReturnCallback::SetPenRect',
            self.syms.get('SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'): 'PenDrawableRT::SetBuffer',
            self.syms.get('SPen::PenReturnCallback::~PenReturnCallback()'): 'PenReturnCallback::destructor',
            self.syms.get('SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'): 'PenDrawableRT::SendDataToGPU',
        }
        if address in watched and address:
            receiver = self.x(0)
            rt = self.watch_rt
            if watched[address] == 'PenReturnCallback::constructor':
                self.callback_this = receiver
                matches = self.x(1) == rt
                role = 'callback_this; RT is argument x1'
            elif watched[address] in ('PenReturnCallback::GetBuffer', 'PenReturnCallback::SetPenRect', 'PenReturnCallback::destructor'):
                matches = receiver == self.callback_this
                role = 'PenReturnCallback this'
            else:
                matches = receiver == rt
                role = 'RT this'
            call_record = {'method': watched[address], 'receiver': hex(receiver),
                'receiver_role': role, 'receiver_matches_expected_object': matches,
                'rt_fields_at_entry_72_to_127_hex': bytes(u.mem_read(rt + 72, 56)).hex() if rt else None,
                'x0_x3': [hex(self.x(i)) for i in range(4)]}
            if watched[address] == 'PenReturnCallback::SetPenRect':
                rect_ptr = self.x(1)
                call_record['rect_pointer'] = hex(rect_ptr)
                call_record['rect_f32'] = list(self.get(rect_ptr, '4f'))
            self.native_call_entries.append(call_record)
        super().hook(u, address, size, data)

    def run_handoff(self):
        initial = super().run()
        texture_fixture = json.loads(TEXTURE_FIXTURE.read_text())
        factory_layout_sources = [row for row in texture_fixture['factory_calls']
            if row.get('resource') in (
                'BrushPenStrokeDrawableRTV1::buildTextures mPatternsBitmap',
                'BrushPenStrokeDrawableRTV1::buildTextures mGrainBitmap')]
        if (len(factory_layout_sources) != 2 or
                any(row.get('stack_args') != [6, 2] for row in factory_layout_sources)):
            raise RuntimeError(f'BrushPen native bitmap layout source calls changed: {factory_layout_sources}')
        initial_draw_args = [hex(value) for value in self.draw_args] if self.draw_args else None
        initial_draw_entries = list(self.draw_entry_records)
        rt = self.paint_state['rt']
        before = bytes(self.u.mem_read(rt + 72, 56))
        self.queue_obj = self.alloc(32)
        queue_vtable = self.alloc(32)
        self.put(self.queue_obj, 'Q', queue_vtable)
        self.put(queue_vtable + 16, 'Q', self.queue_submit_pc)
        # The two native BrushPen Init bitmaps are shader inputs (pattern atlas
        # and grain). Supply a third, separate native SPTextureBitmap as the
        # explicit synthetic SPPenCanvas surface to avoid framebuffer feedback.
        bitmap_queue = self.alloc(32)
        bitmap_queue_vtable = self.alloc(64)
        self.put(bitmap_queue, 'Q', bitmap_queue_vtable)
        self.put(bitmap_queue_vtable + 16, 'Q', self.bitmap_queue_submit_pc)
        blank_canvas = bytes(321 * 228 * 4)
        blank_canvas_ptr = self.alloc(len(blank_canvas))
        self.u.mem_write(blank_canvas_ptr, blank_canvas)
        bitmap_tag = self.alloc(24)
        self.u.mem_write(bitmap_tag, b'synthetic-canvas\0')
        factory_symbol = 'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
        factory_stack = 0x200f0000
        self.put(factory_stack, '2Q', 6, 2)
        texture_ids_before_canvas = list(self.texture_ids)
        self.call(factory_symbol,
            [bitmap_queue, 321, 228, bitmap_tag, blank_canvas_ptr, 0, 0, 0],
            instruction_limit=5_000_000)
        bitmap = self.x(0)
        bitmap_create_record = {'factory': factory_symbol,
            'bitmap': hex(bitmap), 'width': 321, 'height': 228,
            'resource_tag': 'synthetic-canvas', 'source_bytes': len(blank_canvas),
            'source_sha256': sha256(blank_canvas), 'source_kind': 'explicit transparent synthetic canvas surface',
            'texture_memory_layout_raw_u32': 6, 'texture_target_layout_raw_u32': 2,
            'layout_note': 'Raw values repeat those captured from actual BrushPen pattern/grain CreateBitmap calls; no symbolic enum names are asserted.',
            'layout_source_calls': factory_layout_sources,
            'queue_adapter': 'host queue synchronously invokes each native task vtable slot +16',
            'queue_tasks': self.bitmap_queue_tasks,
            'texture_ids_before': texture_ids_before_canvas}
        if not bitmap or self.get(bitmap, 'Q')[0] == 0:
            raise RuntimeError('native CreateBitmap did not return an actual bitmap')
        bitmap_create_record['vptr'] = hex(self.get(bitmap, 'Q')[0])
        bitmap_create_record['texture_ids_after'] = list(self.texture_ids)
        self.canvas_bitmap_factory_record = bitmap_create_record
        canvas = self.get(self.paint_state['drawable'] + 32, 'Q')[0]
        canvas_vtable = self.get(canvas, 'Q')[0]
        self.put(canvas_vtable + 184, 'Q', self.queue_get_pc)
        actual_canvas = self.alloc(256)
        self.call('SPen::SPPenCanvas::SPPenCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)',
                  [actual_canvas, self.queue_obj, bitmap])
        self.call('SPen::SPBitmap::GetWidth() const', [bitmap])
        canvas_width = self.x(0)
        self.call('SPen::SPBitmap::GetHeight() const', [bitmap])
        canvas_height = self.x(0)
        clip_task_count_before = len(self.queue_submissions)
        self.call('SPen::SPPenCanvas::SetClipRect(int, int, int, int, SPen::ISPCanvasBase::ClipOp)',
                  [actual_canvas, 0, 0, canvas_width, canvas_height, 0])
        canvas_clip_setup = {'bitmap_dimensions_from_native_methods':[canvas_width,canvas_height],
            'native_set_clip_rect_args':[0,0,canvas_width,canvas_height,0],
            'submitted_task_count':len(self.queue_submissions)-clip_task_count_before,
            'submitted_tasks_after_call':[hex(row['task']) for row in self.queue_submissions[clip_task_count_before:]]}
        native_canvas_vptr = self.get(actual_canvas, 'Q')[0]
        impl = self.get(actual_canvas + 32, 'Q')[0]
        clip_adapter = self.get(impl + 16, 'Q')[0] if impl else 0
        actual_bitmap = self.get(impl + 8, 'Q')[0]
        native_get_bitmap = self.get(native_canvas_vptr + 192, 'Q')[0]
        expected_get_bitmap = self.syms['SPen::SPPenCanvas::GetBitmap()']
        if native_get_bitmap != expected_get_bitmap or actual_bitmap != bitmap:
            raise RuntimeError('native SPPenCanvas::GetBitmap slot/constructed bitmap mismatch')
        self.native_canvas = {'object': actual_canvas, 'vptr': native_canvas_vptr,
            'impl': impl, 'bitmap': bitmap, 'bitmap_from_impl': actual_bitmap,
            'synthetic_clip_adapter': hex(clip_adapter), 'bitmap_ctor_adapter': None,
            'slot192': native_get_bitmap, 'expected_slot192': expected_get_bitmap,
            'factory_bitmap': self.canvas_bitmap_factory_record}
        self.native_canvas['clip_setup'] = canvas_clip_setup
        self.put(self.paint_state['drawable'] + 32, 'Q', actual_canvas)
        canvas, canvas_vtable = actual_canvas, native_canvas_vptr
        # These paths now execute their native bodies. All other unresolved
        # reached imports continue through the fail-closed inherited loader.
        for name in (
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)',
            'SPen::PenReturnCallback::GetBuffer()',
            'SPen::PenReturnCallback::SetPenRect(SPen::RectF const&)',
            'SPen::PenReturnCallback::~PenReturnCallback()',
        ):
            self.callbacks.pop(name, None)
        self.stop_before_end_pen = False
        source = self.run_object_core(OBJECT_CASE)
        after_callback = bytes(self.u.mem_read(rt + 72, 56))
        if len(self.end_pen_records) != 1:
            raise RuntimeError(f'expected one completed native endPen, got {len(self.end_pen_records)}')
        if len(self.queue_submissions) < 3:
            raise RuntimeError(f'expected at least 3 native callback tasks, got {len(self.queue_submissions)}')
        callback_names = [name for name in self.import_calls
                          if 'PenReturnCallback' in name or 'PenDrawableRT::SetBuffer' in name]
        callback_counts = {name: self.import_calls[name] for name in callback_names}
        if any(callback_counts.get(name, 0) != 1 for name in (
            'SPen::PenDrawableRT::SetBuffer(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)',
            'SPen::PenReturnCallback::PenReturnCallback(SPen::PenDrawableRT*, SPen::ISPPenCanvas*)',
            'SPen::PenReturnCallback::GetBuffer()',
            'SPen::PenReturnCallback::~PenReturnCallback()',
        )):
            raise RuntimeError(f'native callback import counts unexpected: {callback_counts}')
        callback_rt = sorted(set(row['rt_this'] for row in source['callback_arguments']))
        if callback_rt != [hex(rt)]:
            raise RuntimeError(f'AddPoint callbacks used unexpected RTV1 receiver(s): {callback_rt}')
        if not self.native_call_entries or any(not row['receiver_matches_expected_object']
                                               for row in self.native_call_entries):
            raise RuntimeError(f'native callback/task receiver identity mismatch: {self.native_call_entries}')
        if sum(row['method'] == 'PenReturnCallback::SetPenRect' for row in self.native_call_entries) != 1:
            raise RuntimeError('native PenReturnCallback::SetPenRect did not execute exactly once')

        def vtable_name(row):
            return next((name for name, address in self.syms.items()
                         if address + 16 == row['vptr'] and name.startswith('vtable for ')), None)
        setup_task = next((row for row in self.queue_submissions
                     if vtable_name(row) == 'vtable for SPen::WorkerTask::Member0<SPen::SPPenCanvasImpl, void (SPen::SPPenCanvasImpl::*)()>'), None)
        if setup_task:
            task0_entry = self.get(setup_task['vptr'] + 16, 'Q')[0]
            self.runtime_phase = 'native createClippingRT task0'
            task0_gl_begin = len(self.gl_calls_real)
            task0_alloc_begin = len(self.allocation_trace)
            task0_string_begin = len(self.string_append_trace)
            self.x(0, setup_task['task']); self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
            self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
            try:
                self.u.emu_start(task0_entry, 0x200f1000, count=2_000_000)
                task0_result = {'returned': self.u.reg_read(R.UC_ARM64_REG_PC) == 0x200f1000}
            except Exception as exc:
                task0_result = {'returned': False, 'error': str(exc),
                    'pc': hex(self.u.reg_read(R.UC_ARM64_REG_PC)),
                    'lr': hex(self.u.reg_read(R.UC_ARM64_REG_LR)),
                    'x0_x7': [hex(self.x(i)) for i in range(8)],
                    'sp': hex(self.u.reg_read(R.UC_ARM64_REG_SP))}
            task0_result.update({'gl_calls': self.gl_calls_real[task0_gl_begin:],
                'allocations': self.allocation_trace[task0_alloc_begin:],
                'string_appends': self.string_append_trace[task0_string_begin:],
                'shader_sources': self.shader_sources,
                'shader_logs': self.shader_logs,
                'vsnprintf_calls': self.vsnprintf_calls,
                'fopen_calls': self.fopen_calls,
                'invalid_reads': self.invalid_reads,
                'cxx_platform_queries': self.cxx_platform_queries,
                'native_init_arrays': self.native_init_arrays,
                'rect_traces': self.rect_traces,
                'exception_object_64_hex': bytes(self.u.mem_read(self.x(0), 64)).hex()
                    if 'native C++ exception object=' in task0_result.get('error', '') else None})
            self.runtime_phase = 'post task0'
        else:
            task0_entry = 0
            task0_result = {'status': 'not created; constructor-only canvas used explicit clip adapter'}
        if not task0_result.get('returned'):
            raise RuntimeError(f'native canvas clipping target setup task failed: {task0_result}')
        clip_member = self.syms.get('SPen::SPPenCanvasImpl::AddClipRectRT(SPen::ClipInfo)')
        clip_task = next((row for row in self.queue_submissions
            if row['words'][3] == clip_member and row['words'][2] == impl), None)
        if clip_task:
            clip_entry = self.get(clip_task['vptr']+16,'Q')[0]
            self.x(0,clip_task['task']); self.u.reg_write(R.UC_ARM64_REG_SP,0x200f0000)
            self.u.reg_write(R.UC_ARM64_REG_LR,0x200f1000)
            self.u.emu_start(clip_entry,0x200f1000,count=2_000_000)
            clip_task_result={'returned':self.u.reg_read(R.UC_ARM64_REG_PC)==0x200f1000,
                'vtable_symbol':vtable_name(clip_task),'entry':hex(clip_entry),
                'member_target':hex(clip_member),'receiver':hex(impl),
                'native_canvas_clip_call':self.native_canvas['clip_setup']}
        else:
            clip_task_result={'status':'native SetClipRect did not enqueue expected AddClipRectRT task',
                              'native_canvas_clip_call':self.native_canvas['clip_setup']}
        if not clip_task_result.get('returned'):
            raise RuntimeError(f'native SetClipRect task failed: {clip_task_result}')
        task1 = next(row for row in self.queue_submissions if row['words'][3] == self.syms['SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)'])
        task1_vtable_symbol = vtable_name(task1)
        expected_task1_vtable = 'vtable for SPen::WorkerTask::Member2<SPen::PenDrawableRT, void (SPen::PenDrawableRT::*)(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*), std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*>'
        if task1_vtable_symbol != expected_task1_vtable:
            raise RuntimeError(f'unexpected native task1 vtable: {task1_vtable_symbol}')
        task1_target = task1['words'][3]
        expected_target = self.syms['SPen::PenDrawableRT::SendDataToGPU(std::__ndk1::vector<float, std::__ndk1::allocator<float> >*, std::__ndk1::vector<float, std::__ndk1::allocator<float> >*)']
        if task1_target != expected_target or task1['words'][2] != rt:
            raise RuntimeError('task 1 receiver/member target does not match native SendDataToGPU contract')
        entry1 = self.get(task1['vptr'] + 16, 'Q')[0]
        self.x(0, task1['task'])
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(entry1, 0x200f1000, count=2_000_000)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError('native task 1 trampoline did not return')
        after_task1 = bytes(self.u.mem_read(rt + 72, 56))
        task1_fields = struct.unpack('<7Q', after_task1)
        if (task1['words'][5] != task1_fields[2] or task1['words'][6] != task1_fields[5]
                or task1_fields[1] != 0 or task1_fields[4] != 0):
            raise RuntimeError('task1 vector arguments do not match the post-task RT fields')
        draw_vector = self.get(rt + 88, 'Q')[0]
        begin, end, capacity = self.get(draw_vector, '3Q')
        raw = bytes(self.u.mem_read(begin, end - begin))
        if len(raw) != 26 * 44 or end > capacity:
            raise RuntimeError(f'invalid native transferred vector bounds/size: {len(raw)}')
        saved = json.loads(GEOMETRY.read_text())
        expected_case = next(c for c in saved['synthetic_cases'] if c['label'] == OBJECT_CASE['name'])
        expected_bytes = b''.join(bytes.fromhex(word) for row in expected_case['buffer_records']
                                  for word in row['instance_bits_le'])
        if raw != expected_bytes:
            raise RuntimeError('native callback/task1 vector differs from pinned saved geometry fixture')

        task2 = next(row for row in self.queue_submissions if vtable_name(row) == 'vtable for SPen::PenGLRenderMsg')
        task2_vtable_symbol = vtable_name(task2)
        if task2_vtable_symbol != 'vtable for SPen::PenGLRenderMsg':
            raise RuntimeError(f'unexpected native task2 vtable: {task2_vtable_symbol}')
        entry2 = self.get(task2['vptr'] + 16, 'Q')[0]
        canvas_vtable = self.get(canvas, 'Q')[0]
        missing_slot_address = canvas_vtable + 192
        missing_slot_target = self.get(missing_slot_address, 'Q')[0]
        if missing_slot_target != expected_get_bitmap or task2['words'][5] != canvas:
            raise RuntimeError('task2 did not retain the native SPPenCanvas GetBitmap contract')
        # The initial native Draw from PositiveProbe is only a control. Clear
        # its independent RGBA FBO here so the readback below can be attributed
        # exclusively to this queued task execution.
        output = initial['output']['framebuffer_id']
        self.mesa.f['glBindFramebuffer'](0x8D40, output)
        self.mesa.f['glViewport'](0, 0, 321, 228)
        self.mesa.f['glClearColor'](0.0, 0.0, 0.0, 0.0)
        self.mesa.f['glClear'](0x4000)
        if self.mesa.f['glGetError']():
            raise RuntimeError('GL error while clearing task2-only output target')
        task2_target_before = positive.C.create_string_buffer(321 * 228 * 4)
        self.mesa.f['glReadPixels'](0, 0, 321, 228, 0x1908, 0x1401,
                                    positive.C.cast(task2_target_before, positive.C.c_void_p))
        if any(task2_target_before.raw):
            raise RuntimeError('task2-only independent target was not zero after clear')
        task2_gl_calls_start = len(self.gl_calls_real)
        task2_uploads_start = len(self.graphics_buffer_uploads)
        task2_uniforms_before = dict(self.uniform_values)
        get_integer = positive.ip.cfunc(self.mesa.gl, 'glGetIntegerv', None,
            [positive.C.c_uint, positive.C.POINTER(positive.C.c_int)])
        get_attachment = positive.ip.cfunc(self.mesa.gl, 'glGetFramebufferAttachmentParameteriv', None,
            [positive.C.c_uint, positive.C.c_uint, positive.C.c_uint, positive.C.POINTER(positive.C.c_int)])
        get_tex_level = positive.ip.cfunc(self.mesa.gl, 'glGetTexLevelParameteriv', None,
            [positive.C.c_uint, positive.C.c_int, positive.C.c_uint, positive.C.POINTER(positive.C.c_int)])
        def target_snapshot(fbo):
            prior_fbo, prior_active, prior_texture = positive.C.c_int(), positive.C.c_int(), positive.C.c_int()
            get_integer(0x8CA6, positive.C.byref(prior_fbo))
            get_integer(0x84E0, positive.C.byref(prior_active))
            get_integer(0x8069, positive.C.byref(prior_texture))
            self.mesa.f['glBindFramebuffer'](0x8D40, fbo)
            values = {}
            for name, pname in [('attachment_type', 0x8CD0), ('attachment_object', 0x8CD1)]:
                value = positive.C.c_int(-1)
                get_attachment(0x8D40, 0x8CE0, pname, positive.C.byref(value))
                values[name] = value.value
            object_id = values['attachment_object']
            if values['attachment_type'] == 0x1702 and object_id:
                self.mesa.f['glBindTexture'](0x0DE1, object_id)
                for name, pname in [('width', 0x1000), ('height', 0x1001), ('internal_format', 0x1003)]:
                    value = positive.C.c_int(-1)
                    get_tex_level(0x0DE1, 0, pname, positive.C.byref(value))
                    values[name] = value.value
                self.mesa.f['glBindTexture'](0x0DE1, prior_texture.value)
            rawbuf = positive.C.create_string_buffer(321 * 228 * 4)
            self.mesa.f['glReadPixels'](0, 0, 321, 228, 0x1908, 0x1401,
                positive.C.cast(rawbuf, positive.C.c_void_p))
            raw = rawbuf.raw
            values['sha256'] = sha256(raw)
            values['channel_nonzero'] = [sum(value != 0 for value in raw[i::4]) for i in range(4)]
            values['rgb_nonzero_alpha_zero_pixels'] = sum(1 for i in range(0, len(raw), 4)
                if raw[i + 3] == 0 and any(raw[i:i + 3]))
            self.mesa.f['glBindFramebuffer'](0x8D40, prior_fbo.value)
            self.mesa.f['glActiveTexture'](prior_active.value)
            return values
        fbo_before_task2 = positive.C.c_int()
        get_integer(0x8CA6, positive.C.byref(fbo_before_task2))
        host_target_before_task2 = target_snapshot(output)
        if len(self.created_framebuffers) != 1:
            raise RuntimeError(f'expected one native canvas framebuffer, got {self.created_framebuffers}')
        native_canvas_fbo = self.created_framebuffers[0]
        native_canvas_target_before_clear = target_snapshot(native_canvas_fbo)
        self.mesa.f['glBindFramebuffer'](0x8D40, native_canvas_fbo)
        self.mesa.f['glClearColor'](0.0, 0.0, 0.0, 0.0)
        self.mesa.f['glClear'](0x4000)
        canvas_clear_error = self.mesa.f['glGetError']()
        native_canvas_target_before_task2 = target_snapshot(native_canvas_fbo)
        if canvas_clear_error or any(native_canvas_target_before_task2['channel_nonzero']):
            raise RuntimeError(f'native canvas test-target clear failed: error={canvas_clear_error}, state={native_canvas_target_before_task2}')
        self.mesa.f['glBindFramebuffer'](0x8D40, output)
        self.x(0, task2['task'])
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        draws_before_task2 = len(self.graphics_draw_calls)
        import_counts_before_task2 = dict(self.import_calls)
        clip_calls_before_task2 = len(self.clip_calls)
        try:
            self.u.emu_start(entry2, 0x200f1000, count=2_000_000)
            task2_result = {'returned': self.u.reg_read(R.UC_ARM64_REG_PC) == 0x200f1000}
        except Exception as exc:
            task2_result = {'returned': False, 'error': str(exc),
                'pc': hex(self.u.reg_read(R.UC_ARM64_REG_PC)),
                'lr': hex(self.u.reg_read(R.UC_ARM64_REG_LR)),
                'x0_x3': [hex(self.x(i)) for i in range(4)],
                'x19_x25': [hex(self.u.reg_read(getattr(R, f'UC_ARM64_REG_X{i}'))) for i in range(19, 26)]}
        task2_result.update({'entry': hex(entry2), 'canvas': hex(canvas),
            'canvas_vtable': hex(canvas_vtable), 'canvas_slot192': hex(missing_slot_target),
            'canvas_slot192_symbol': 'SPen::SPPenCanvas::GetBitmap()',
            'graphics_draw_calls_before': draws_before_task2,
            'graphics_draw_calls_after': len(self.graphics_draw_calls),
            'clip_calls_during_task2': self.clip_calls[clip_calls_before_task2:],
            'import_call_deltas': {k: v - import_counts_before_task2.get(k, 0)
                for k, v in self.import_calls.items()
                if v != import_counts_before_task2.get(k, 0)},
            'gl_calls_during_task2': self.gl_calls_real[task2_gl_calls_start:],
            'graphics_buffer_uploads_during_task2': self.graphics_buffer_uploads[task2_uploads_start:],
            'uniform_values_before': task2_uniforms_before,
            'uniform_values_after': dict(self.uniform_values)})
        task2_pixels = positive.C.create_string_buffer(321 * 228 * 4)
        read_fbo, draw_fbo = positive.C.c_int(), positive.C.c_int()
        get_integer(0x8CAA, positive.C.byref(read_fbo))
        get_integer(0x8CA6, positive.C.byref(draw_fbo))
        read_attachment_type, read_attachment_object = positive.C.c_int(-1), positive.C.c_int(-1)
        get_attachment(0x8D40, 0x8CE0, 0x8CD0, positive.C.byref(read_attachment_type))
        get_attachment(0x8D40, 0x8CE0, 0x8CD1, positive.C.byref(read_attachment_object))
        self.mesa.f['glReadPixels'](0, 0, 321, 228, 0x1908, 0x1401,
                                    positive.C.cast(task2_pixels, positive.C.c_void_p))
        task2_target_error = self.mesa.f['glGetError']()
        pixel_values = [tuple(task2_pixels.raw[index:index + 4])
                        for index in range(0, len(task2_pixels.raw), 4)]
        nonzero_pixels = [{'index': index, 'rgba': list(pixel)} for index, pixel in enumerate(pixel_values)
                          if any(pixel)]
        blend_enabled_fn = positive.ip.cfunc(self.mesa.gl, 'glIsEnabled', positive.C.c_ubyte,
            [positive.C.c_uint])
        blend_state = {'blend_enabled': bool(blend_enabled_fn(0x0BE2))}
        for name, pname in [('equation_rgb', 0x8009), ('equation_alpha', 0x883D),
                            ('src_rgb', 0x80C9), ('dst_rgb', 0x80C8),
                            ('src_alpha', 0x80CB), ('dst_alpha', 0x80CA)]:
            value = positive.C.c_int(-1)
            get_integer(pname, positive.C.byref(value))
            blend_state[name] = value.value
        color_mask = (positive.C.c_ubyte * 4)()
        get_boolean = positive.ip.cfunc(self.mesa.gl, 'glGetBooleanv', None,
            [positive.C.c_uint, positive.C.POINTER(positive.C.c_ubyte)])
        get_boolean(0x0C23, color_mask)
        blend_state['color_write_mask'] = list(color_mask)
        task2_result['native_canvas_rgba_target'] = {
            'framebuffer_id': native_canvas_fbo, 'attachment_texture_id': self.canvas_bitmap_factory_record['texture_ids_after'][-1],
            'read_framebuffer_at_read': read_fbo.value, 'draw_framebuffer_at_read': draw_fbo.value,
            'attachment_type_at_read': read_attachment_type.value,
            'attachment_texture_id_at_read': read_attachment_object.value,
            'dimensions': [321, 228], 'format': 'RGBA8', 'before_native_task2_verified_zero': True,
            'after_task2_sha256': sha256(task2_pixels.raw), 'after_task2_base64': base64.b64encode(task2_pixels.raw).decode(),
            'after_task2_nonzero_bytes': sum(value != 0 for value in task2_pixels.raw),
            'after_task2_channel_nonzero': [sum(value != 0 for value in task2_pixels.raw[i::4]) for i in range(4)],
            'after_task2_nonzero_alpha_pixels': sum(value != 0 for value in task2_pixels.raw[3::4]),
            'after_task2_rgb_nonzero_alpha_zero_pixels': sum(1 for pixel in pixel_values if pixel[3] == 0 and any(pixel[:3])),
            'pixel_rgba_histogram': {','.join(map(str, rgba)): count for rgba, count in sorted(Counter(pixel_values).items()) if any(rgba)},
            'nonzero_pixels_linear_index_rgba': nonzero_pixels,
            'actual_gl_blend_state_after_draw': blend_state,
            'projection_matrix_after_draw_f32': task2_result['uniform_values_after'].get('ProjectionMatrix'),
            'initial_control_projection_matrix_f32': task2_uniforms_before.get('ProjectionMatrix'),
            'projection_comparison_note': 'Measured Draw scale terms are 8x smaller than the explicit initial control matrix in both X and Y; this records the transform difference and does not assert it caused a particular pixel-count delta.',
            'uniform_values_after_draw': task2_result['uniform_values_after'],
            'input_color_state_provenance': 'The captured inputColor is the explicit host RT+44 state used by the paint helper, not a color decoded from ObjectStroke.',
            'gl_error_after_readback': task2_target_error,
            'feedback_alias_with_pattern_or_grain': self.canvas_bitmap_factory_record['texture_ids_after'][-1] in self.texture_ids[:2]}
        if (task2_target_error or read_fbo.value != native_canvas_fbo or draw_fbo.value != native_canvas_fbo
                or read_attachment_object.value != task2_result['native_canvas_rgba_target']['attachment_texture_id']
                or task2_result['native_canvas_rgba_target']['feedback_alias_with_pattern_or_grain']):
            raise RuntimeError(f'invalid native canvas paint target: error={task2_target_error}, target={task2_result["native_canvas_rgba_target"]}')
        if not blend_state['blend_enabled']:
            raise RuntimeError(f'native task2 did not leave expected enabled blend state: {blend_state}')
        if len(self.graphics_draw_calls) != draws_before_task2 + 1:
            raise RuntimeError('native queued task2 did not add exactly one GraphicsObject instanced draw')
        bound_after_task2 = positive.C.c_int()
        get_integer(0x8CA6, positive.C.byref(bound_after_task2))
        native_target_after_read = target_snapshot(bound_after_task2.value)
        if (native_target_after_read.get('width') != 321 or native_target_after_read.get('height') != 228
                or native_target_after_read.get('internal_format') != 0x8058
                or native_target_after_read.get('attachment_object') != read_attachment_object.value):
            raise RuntimeError(f'native queued task target is not the expected RGBA8 bitmap: {native_target_after_read}')
        task2_result['target_debug'] = {'bound_fbo_before_native_task2': fbo_before_task2.value,
            'host_output_before_task2': host_target_before_task2,
            'native_canvas_fbo': native_canvas_fbo,
            'native_canvas_target_before_test_clear': native_canvas_target_before_clear,
            'native_canvas_target_before_task2': native_canvas_target_before_task2,
            'test_clear': {'clear_color': [0, 0, 0, 0], 'gl_error': canvas_clear_error,
                'verified_all_zero': True},
            'native_bound_framebuffer_after_task2': bound_after_task2.value,
            'native_canvas_target_after_task2': native_target_after_read,
            'fresh_host_fbo_after_task2': target_snapshot(output)}
        if bound_after_task2.value != native_canvas_fbo:
            raise RuntimeError(f'native task2 drew to unexpected FBO {bound_after_task2.value}; expected {native_canvas_fbo}')
        self.mesa.f['glBindFramebuffer'](0x8D40, output)
        after_task2 = bytes(self.u.mem_read(rt + 72, 56))

        task3_result = {'status': 'deferred because native task2 failed'}
        if task2_result.get('returned'):
            task3 = next(row for row in self.queue_submissions if vtable_name(row) == 'vtable for SPen::WorkerTask::Delete<std::__ndk1::vector<float, std::__ndk1::allocator<float> > >')
            task3_entry = self.get(task3['vptr'] + 16, 'Q')[0]
            self.x(0, task3['task'])
            self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
            self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
            try:
                self.u.emu_start(task3_entry, 0x200f1000, count=2_000_000)
                task3_result = {'status': 'executed after native task2 returned',
                    'returned': self.u.reg_read(R.UC_ARM64_REG_PC) == 0x200f1000,
                    'entry': hex(task3_entry)}
            except Exception as exc:
                task3_result = {'status': 'attempted after native task2 returned', 'returned': False,
                    'error': str(exc), 'entry': hex(task3_entry),
                    'pc': hex(self.u.reg_read(R.UC_ARM64_REG_PC))}

        # On success task2 is the native draw route, so do not touch the vector
        # after task3 frees it. On failure, use the committed explicit Draw only
        # as a separate control of the preserved task1 vector.
        if not task2_result.get('returned'):
            self._call_gl('glBindFramebuffer', self.mesa.f['glBindFramebuffer'], 0x8D40,
                          initial['output']['framebuffer_id'])
            self._call_gl('glClearColor', self.mesa.f['glClearColor'], 0.0, 0.0, 0.0, 0.0)
            self._call_gl('glClear', self.mesa.f['glClear'], 0x4000)
            self.call('SPen::BrushPenStrokeDrawableRTV1::Update(bool)', [rt, 0])
            if not self.draw_args or self.draw_args[0] != rt:
                raise RuntimeError('missing native Draw receiver for initialized RTV1')
            self.call('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)', self.draw_args)
        pixels = positive.C.create_string_buffer(321 * 228 * 4)
        self._call_gl('glPixelStorei', self.mesa.f['glPixelStorei'], 0x0D05, 1)
        self._call_gl('glReadPixels', self.mesa.f['glReadPixels'], 0, 0, 321, 228,
                      0x1908, 0x1401, positive.C.cast(pixels, positive.C.c_void_p))
        if self.gl_errors:
            raise RuntimeError(f'host GLES errors: {self.gl_errors[:8]}')
        after_draw = bytes(self.u.mem_read(rt + 72, 56))

        tasks = []
        for index, row in enumerate(self.queue_submissions):
            words = row['words']
            tasks.append({'index': index + 1, 'object_address': hex(row['task']),
                'queue_address': hex(row['queue']), 'vptr': hex(row['vptr']),
                'raw_96_bytes_hex': row['raw96'].hex(), 'qwords_le_hex': qwords(row['raw96']),
                'worker_trampoline': hex(self.get(row['vptr'] + 16, 'Q')[0]),
                'receiver_word2': hex(words[2]), 'member_target_word3': hex(words[3]),
                'pointer_argument_word5': hex(words[5])})
        task3 = next(row for row in self.queue_submissions if vtable_name(row) == 'vtable for SPen::WorkerTask::Delete<std::__ndk1::vector<float, std::__ndk1::allocator<float> > >')
        task3_vtable_symbol = vtable_name(task3)
        task3_entry = self.get(task3['vptr'] + 16, 'Q')[0]
        if task3_vtable_symbol != 'vtable for SPen::WorkerTask::Delete<std::__ndk1::vector<float, std::__ndk1::allocator<float> > >':
            raise RuntimeError(f'unexpected deferred task3 vtable symbol: {task3_vtable_symbol}')
        if not task3_result.get('returned'):
            raise RuntimeError(f'native vector delete task did not return: {task3_result}')
        fields = lambda raw: {'rt_plus_72_through_127_raw_hex': raw.hex(),
                              'qwords_at_72_80_88_96_104_112_120': qwords(raw[:56]),
                              'count_u32_at_72': struct.unpack_from('<I', raw, 0)[0]}
        execution_order = [
            {'order': 1, 'role': 'SPPenCanvasImpl::createClippingRT', 'task': hex(setup_task['task']),
             'returned': task0_result.get('returned')},
            {'order': 2, 'role': 'SPPenCanvasImpl::AddClipRectRT', 'task': hex(clip_task['task']),
             'returned': clip_task_result.get('returned')},
            {'order': 3, 'role': 'PenDrawableRT::SendDataToGPU', 'task': hex(task1['task']),
             'returned': True},
            {'order': 4, 'role': 'SPen::PenGLRenderMsg', 'task': hex(task2['task']),
             'returned': task2_result.get('returned')},
            {'order': 5, 'role': 'WorkerTask::Delete<vector<float>>', 'task': hex(task3['task']),
             'returned': task3_result.get('returned')},
        ]
        return {
            'schema': 'brushpen-native-queued-canvas-paint-v1',
            'scope': 'An explicit synthetic ObjectStroke runs through the pinned native BrushPen GLV10 callback/endPen path and queued task1 upload. Task2 runs native SPenGLRenderMsg with a native SPPenCanvas/native GetBitmap receiver and a separate actual native SPTextureBitmap created from a zeroed synthetic RGBA surface. The host queue synchronously dispatches the canvas bitmap task and captures callback submissions; Mesa GLES executes intercepted native GL calls. Task3 vector deletion runs only after task2 returns. This is not natural application scheduling, Android/device execution, or captured-device input.',
            'input': {'kind': 'explicit synthetic ObjectStroke', 'case': OBJECT_CASE['name'],
                      'motion': expected_case['synthetic_input'], 'constructor_selected_version': source['admission']['actual_getVersion']},
            'native_callback': {'native_import_counts': callback_counts,
                'addpoint_callback_count': len(source['callback_arguments']),
                'addpoint_rt_this_values': callback_rt,
                'arguments': source['callback_arguments'],
                'endPen_completed_count': len(self.end_pen_records),
                'terminal_addpoint_count': self.end_pen_records[0]['emitted_addpoint_count']},
            'rt_field_snapshots': {'before_ObjectStroke': fields(before),
                'after_callback_destructor_before_task1': fields(after_callback),
                'after_task1_before_task2': fields(after_task1),
                'after_task2_before_task3': fields(after_task2),
                'after_task3': fields(after_draw),
                'observed_native_writes': self.rt_writes,
                'native_method_entry_snapshots': self.native_call_entries},
            'tasks': {'task_count': len(tasks), 'entries': tasks,
                'execution_order': execution_order,
                'rtv1_draw_entries': self.draw_entry_records,
                'rtv1_draw_pc_trace': self.draw_pc_trace,
                'initial_control_draw_args': initial_draw_args,
                'initial_control_draw_entries': initial_draw_entries,
                'canvas_impl_createClippingRT_task': task0_result,
                'native_clip_setup_task': clip_task_result,
                'task1_execution': {'returned': True, 'target_symbol': 'SPen::PenDrawableRT::SendDataToGPU',
                    'target_elf_offset_PenCommon': '0x4a5c4', 'vtable_symbol': task1_vtable_symbol,
                    'receiver_matches_rt': True,
                    'vector_argument': hex(task1['words'][5]), 'second_vector_argument': hex(task1['words'][6])},
                'task2_native_canvas_execution': {**task2_result,
                    'vtable_symbol': task2_vtable_symbol,
                    'receiver_canvas': hex(canvas), 'canvas_vtable': hex(canvas_vtable),
                    'missing_slot_offset': 192, 'missing_slot_address': hex(missing_slot_address),
                    'missing_slot_target': hex(missing_slot_target),
                    'task2_native_returned': task2_result.get('returned')},
                'task3_execution': {**task3_result, 'vtable_symbol': task3_vtable_symbol,
                    'vtable_addresspoint': hex(task3['vptr']),
                    'vtable_symbol_address': hex(task3['vptr'] - 16),
                    'vtable_addresspoint_offset_from_symbol': 16,
                    'entry_address': hex(task3_entry),
                    'entry_elf_offset_PenCommon': hex(task3_entry - 0x01000000),
                    'entry_code_first_16_bytes': bytes(self.u.mem_read(task3_entry, 16)).hex(),
                    'pointer_argument': hex(task3['words'][2]),
                    'meaning': 'The native vtable symbol is WorkerTask::Delete<std::__ndk1::vector<float>>; its slot-0 entry is recorded above. The entry body reads the vector object at task+16, destroys its storage, then frees the object.'},
            },
            'buffer_handoff': {'rt_plus_80_after_task1': hex(task1_fields[1]),
                'rt_plus_88_after_task1': hex(task1_fields[2]),
                'rt_plus_104_after_task1': hex(task1_fields[4]),
                'rt_plus_112_after_task1': hex(task1_fields[5]),
                'task1_vector_argument_matches_rt_plus_88': task1['words'][5] == task1_fields[2],
                'task1_second_vector_argument_matches_rt_plus_112': task1['words'][6] == task1_fields[5],
                'producer_fields_cleared_after_task1': task1_fields[1] == 0 and task1_fields[4] == 0,
                'vector_bytes': len(raw), 'vector_sha256': sha256(raw),
                'same_bytes_as_saved_geometry_fixture': True,
                'native_update_draw_instanced_args': self.graphics_draw_calls[-1]},
            'native_pattern_material': {
                'pattern_and_grain_texture_ids': initial['texture_ids'][:2],
                'sampler_ids': initial['sampler_ids'],
                'texture_uploads': [{key: row.get(key) for key in ('kind', 'dimensions', 'format', 'type', 'bytes', 'sha256')}
                                    for row in initial['texture_uploads']],
                'initialized_pattern_tile_hashes': initial['pattern_valid_initialized_tile_hashes'],
                'diagnostic_pattern_sentinel': initial['pattern_poison_cells'],
                'shader_source_sha256': initial['shader_hashes'],
                'texture_fixture_sha256': initial['provenance']['texture_fixture_sha256']},
            'host_mesa_paint': {'control_fbo_final_readback_sha256': sha256(pixels.raw),
                'control_fbo_role': 'separate host-created FBO; native task2 does not bind it',
                'gl_errors': self.gl_errors, 'draw_count_including_initial_paint': len(self.graphics_draw_calls),
                'mesa': initial['mesa'],
                'initial_control_framebuffer': {key: initial['output'].get(key)
                    for key in ('dimensions', 'format', 'texture_id', 'framebuffer_id', 'complete',
                                'framebuffer_status_gl_enum', 'feedback_alias', 'rgba_sha256',
                                'bytes', 'pixel_count', 'nonzero_alpha_pixels', 'alpha_range')}},
            'native_canvas': self.native_canvas,
            'provenance': {'geometry_fixture_sha256': sha256(GEOMETRY.read_bytes()),
                'geometry_runner_sha256': sha256(GEOMETRY_RUNNER.read_bytes()),
                'positive_runner_sha256': sha256(POSITIVE_RUNNER.read_bytes()),
                'texture_runner_sha256': sha256(TEXTURE_RUNNER.read_bytes()),
                'texture_fixture_sha256': sha256(TEXTURE_FIXTURE.read_bytes()),
            'native_dsos': {**initial['provenance']['native_elf_sha256'],
                'libSPenCxx.so': hashlib.sha256((ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenCxx.so').read_bytes()).hexdigest()},
                'native_dso_paths': {name: f'scratch/apk-analysis-native/arm64-v8a/libSPen{name}.so'
                    for name in (*initial['provenance']['native_elf_sha256'].keys(), 'Cxx')},
                'host_gl_dsos': initial['provenance']['host_gles_dsos'],
                'input_motion_sha256': sha256(json.dumps(expected_case['synthetic_input'], sort_keys=True).encode())},
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--write-fixture', action='store_true')
    modes.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    report = HandoffProbe().run_handoff()
    encoded = json.dumps(report, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
        return
    if not OUT.exists():
        raise SystemExit(f'missing reviewed fixture {OUT}; inspect then run --write-fixture')
    expected = json.loads(OUT.read_text())
    if json.dumps(expected, sort_keys=True) != json.dumps(report, sort_keys=True):
        raise AssertionError('native BrushPen handoff output differs from reviewed fixture')
    print('BrushPen native callback handoff oracle passed')


if __name__ == '__main__':
    main()
