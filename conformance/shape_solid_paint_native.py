#!/usr/bin/env python3
"""Execute a native ObjectShape paint path on pinned Samsung Notes plus Mesa.

The two declared cases exercise the no-outline solid-rectangle fast path and
the outlined software-Skia path. The native object, drawing, Skia raster,
Mesa texture upload/composite, and final RGBA readback execute. Queue dispatch,
libc math/allocation, EGL context queries, flush/invalidate, and GL forwarding
are explicit host boundaries; this is not Android device output parity.
"""
from pathlib import Path
import hashlib
import json
import os
import re
import argparse
import struct
import subprocess
import zlib
import sys
import os

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'conformance'))
import airbrush_texture_native as texture
from airbrush_texture_native import TextureOracle
from calligraphy_draw_native import NativeDrawCalligraphy
import inkpen_paint_native as mesa
import ctypes as C
from unicorn import arm64_const as R

LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
APK_SHA256 = 'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667'
SOURCE_HASHES = {
    'fountainpen': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
    'pencommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
    'airbrushpen': '92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423',
    'graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
    'renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
    'model': '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a',
    'drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
    'view': 'c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8',
    'skia': '42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d',
    'libcxx_shared': '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
}
SOURCE_FILES = {
    'fountainpen': 'libSPenFountainPen.so', 'pencommon': 'libSPenPenCommon.so',
    'base': 'libSPenBase.so', 'airbrushpen': 'libSPenAirBrushPen.so',
    'graphics': 'libSPenGraphics.so', 'renderer': 'libSPenRenderer.so',
    'model': 'libSPenModel.so', 'drawing': 'libSPenDrawing.so',
    'view': 'libSPenView.so', 'skia': 'libSPenSkia.so',
    'libcxx_shared': 'libc++_shared.so',
}
for library, expected_sha256 in SOURCE_HASHES.items():
    library_path = LIB / SOURCE_FILES[library]
    actual_sha256 = hashlib.sha256(library_path.read_bytes()).hexdigest()
    if actual_sha256 != expected_sha256:
        raise RuntimeError(f'unpinned {library_path.name}: {actual_sha256}')
APK_PATH = ROOT / 'scratch/apk-input/samsung-notes-4.4.45.37.apk'
if hashlib.sha256(APK_PATH.read_bytes()).hexdigest() != APK_SHA256:
    raise RuntimeError('APK hash does not match pinned Samsung Notes 4.4.45.37')
HOST_LIBM = Path('/usr/lib/x86_64-linux-gnu/libm.so.6')
HOST_LIBM_SHA256 = '6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3'
if hashlib.sha256(HOST_LIBM.read_bytes()).hexdigest() != HOST_LIBM_SHA256:
    raise RuntimeError(f'unpinned host libm: {HOST_LIBM}')
EXTRA = {
    'Model': (6, '4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a'),
    'Drawing': (7, '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd'),
    'View': (8, 'c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8'),
    'Skia': (9, '42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d'),
    'Cpp': (10, '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4'),
}

class ShapeOracle(TextureOracle):
    def alloc(self, n):
        address = super().alloc(n)
        records = getattr(self, 'alloc_records', None)
        if records is not None:
            records.append({'address': address, 'size': n,
                            'caller': getattr(self, 'pc_ring', [])[-1] if getattr(self, 'pc_ring', []) else None})
        return address

    def map_extra(self, name, index):
        path = LIB / ('libc++_shared.so' if name == 'Cpp' else f'libSPen{name}.so')
        blob = path.read_bytes()
        base = index * 0x1000000
        self.extra_bases[name] = base
        self.extra_paths = getattr(self, 'extra_paths', {})
        self.extra_paths[name] = path
        self.raw_plt_by_address = getattr(self, 'raw_plt_by_address', {})
        phoff = struct.unpack_from('<Q', blob, 32)[0]
        entsize, count = struct.unpack_from('<HH', blob, 54)
        segments = [struct.unpack_from('<II6Q', blob, phoff + i * entsize)
                    for i in range(count)]
        extent = max(p[3] + p[5] for p in segments if p[0] == 1)
        self.u.mem_map(base, (extent + 0xffff) & ~0xffff)
        for ph in segments:
            if ph[0] == 1 and ph[5]:
                self.u.mem_write(base + ph[3], blob[ph[2]:ph[2] + ph[5]])
        env = {**os.environ, 'LD_LIBRARY_PATH': str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
        nm = subprocess.run([str(texture.NM), '-D', '-C', str(path)], capture_output=True,
                            text=True, check=True, env=env)
        for line in nm.stdout.splitlines():
            parts = line.strip().split(' ', 2)
            if len(parts) == 3 and parts[1] in ('T', 'W', 'V', 'D', 'B', 'R'):
                self.syms[parts[2]] = base + int(parts[0], 16)
        dis = subprocess.run([str(texture.OBJ), '-d', '-C', '-j', '.plt', str(path)],
                             capture_output=True, text=True, check=True, env=env)
        for address, symbol in re.findall(r'^([0-9a-f]+) <(.+)@plt>:', dis.stdout, re.M):
            self.plt[base + int(address, 16)] = symbol
        # This Android PLT labels some veneers after their leading ADRP/LDR
        # instructions. Recover each true veneer start from its loaded GOT
        # relocation instead of treating objdump's label address as callable.
        shoff = struct.unpack_from('<Q', blob, 40)[0]
        es, count, stridx = struct.unpack_from('<HHH', blob, 58)
        sections = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * es)
                    for i in range(count)]
        shstr = sections[stridx]
        section_names = blob[shstr[4]:shstr[4] + shstr[5]]
        by_name = {section_names[s[0]:section_names.find(b'\0', s[0])].decode(): s for s in sections}
        dynsym, dynstr, relsec = by_name['.dynsym'], by_name['.dynstr'], by_name['.rela.plt']
        strings = blob[dynstr[4]:dynstr[4] + dynstr[5]]
        names = []
        for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
            no = struct.unpack_from('<I', blob, off)[0]
            end = strings.find(b'\0', no)
            names.append(strings[no:end].decode() if no else '')
        got_names = {}
        for off in range(relsec[4], relsec[4] + relsec[5], relsec[9]):
            where, info, _addend = struct.unpack_from('<QQq', blob, off)
            if info & 0xffffffff == 0x402:
                got_names[where] = names[info >> 32]
        for address in [a for a in self.plt if base <= a < base + 0x200000]:
            self.plt.pop(address)
        pending = page = addend = None
        for line in dis.stdout.splitlines():
            match = re.match(r'^\s*([0-9a-f]+):\s+[0-9a-f]+\s+adrp\s+x16,\s*([0-9a-f]+)', line)
            if match:
                pending, page, addend = int(match.group(1), 16), int(match.group(2), 16), None
                continue
            if pending is None:
                continue
            match = re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+add\s+x16, x16, #0x([0-9a-f]+)', line)
            if match:
                addend = int(match.group(1), 16)
                continue
            if re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+br\s+x17', line):
                raw_name = got_names.get(page + addend) if addend is not None else None
                if raw_name:
                    pretty = subprocess.run([str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt'), '-n', raw_name],
                                            capture_output=True, text=True, check=True, env=env).stdout.strip()
                    self.plt[base + pending] = pretty
                    self.raw_plt_by_address[base + pending] = raw_name
                pending = page = addend = None
            elif re.match(r'^\s*[0-9a-f]+:', line) and not re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+(nop|ldr)\b', line):
                pending = page = addend = None

    def load_extra_relocs(self):
        for name, base in self.extra_bases.items():
            blob = self.extra_paths[name].read_bytes()
            shoff = struct.unpack_from('<Q', blob, 40)[0]
            es, count, stridx = struct.unpack_from('<HHH', blob, 58)
            sections = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * es)
                        for i in range(count)]
            shstr = sections[stridx]
            names = blob[shstr[4]:shstr[4] + shstr[5]]
            by_name = {names[s[0]:names.find(b'\0', s[0])].decode(): s for s in sections}
            dynsym, dynstr = by_name['.dynsym'], by_name['.dynstr']
            strings = blob[dynstr[4]:dynstr[4] + dynstr[5]]
            syms = []
            for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
                no, _info, _other, shndx, value, _size = struct.unpack_from('<IBBHQQ', blob, off)
                end = strings.find(b'\0', no)
                syms.append((strings[no:end].decode() if no else '', shndx, value))
            relocs = by_name.get('.rela.dyn')
            if not relocs:
                continue
            for off in range(relocs[4], relocs[4] + relocs[5], relocs[9]):
                where, info, addend = struct.unpack_from('<QQq', blob, off)
                kind, index = info & 0xffffffff, info >> 32
                if kind == 0x403:
                    value = base + addend
                elif kind == 0x401 and syms[index][1]:
                    value = base + syms[index][2]
                elif kind == 0x101 and syms[index][1]:
                    # Defined symbol relocations retain symbol identity here.
                    # Demangled names are not unique: e.g. all SPSubBitmap
                    # non-virtual destructor thunks demangle to the same
                    # string but apply different `this` adjustments.
                    value = base + syms[index][2] + addend
                elif kind == 0x101:
                    value = self.resolve_raw_symbol(syms[index][0]) + addend
                elif kind == 0x401:
                    value = self.resolve_raw_symbol(syms[index][0])
                else:
                    continue
                self.put(base + where, 'Q', value)
            # The ordinary dynamic linker resolves JUMP_SLOTs before running
            # constructors. Point each slot at its own loaded PLT stub so the
            # normal import hook can supply its explicit host ABI behavior.
            jump_relocs = by_name.get('.rela.plt')
            if jump_relocs:
                raw_stubs = {raw: address for address, raw in self.raw_plt_by_address.items()
                             if base <= address < base + 0x1000000}
                for off in range(jump_relocs[4], jump_relocs[4] + jump_relocs[5], jump_relocs[9]):
                    where, info, _addend = struct.unpack_from('<QQq', blob, off)
                    kind, index = info & 0xffffffff, info >> 32
                    if kind != 0x402:
                        continue
                    raw_name = syms[index][0]
                    target = raw_stubs.get(raw_name)
                    if target is None:
                        # A symbol implemented by a loaded provider can bind
                        # directly. Preserve the raw ELF identity: different
                        # adjustment thunks can share one demangled spelling.
                        target = self.raw_provider.get(raw_name)
                    if target is None:
                        pretty = self.demangle(raw_name)
                        raise RuntimeError(f'{name} unresolved JUMP_SLOT {raw_name!r} ({pretty})')
                    self.put(base + where, 'Q', target)

    @staticmethod
    def demangle(raw):
        if not raw.startswith('_Z'):
            return raw
        env = {**os.environ, 'LD_LIBRARY_PATH': str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
        return subprocess.run([str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-c++filt'), '-n', raw],
                              capture_output=True, text=True, check=True, env=env).stdout.strip()

    def build_raw_provider_map(self):
        # These are the exact pinned load bases used by NativeFountain and
        # map_extra. Provider order follows that load order. Keep mangled ELF
        # identity because demangling is many-to-one for ABI thunks.
        modules = [
            ('FountainPen', 0x0000000), ('PenCommon', 0x1000000),
            ('Base', 0x2000000), ('AirBrushPen', 0x3000000),
            ('Graphics', 0x4000000), ('Renderer', 0x5000000),
            *[(name, base) for name, base in self.extra_bases.items()
              if name not in ('Graphics', 'Renderer')],
        ]
        providers = {}
        self.raw_plt_by_address = {}
        for name, base in modules:
            path = self.extra_paths.get(name)
            if path is None:
                path = LIB / ('libc++_shared.so' if name == 'Cpp' else f'libSPen{name}.so')
            blob = path.read_bytes()
            shoff = struct.unpack_from('<Q', blob, 40)[0]
            es, count, stridx = struct.unpack_from('<HHH', blob, 58)
            sections = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * es)
                        for i in range(count)]
            shstr = sections[stridx]
            shnames = blob[shstr[4]:shstr[4] + shstr[5]]
            by_name = {shnames[s[0]:shnames.find(b'\0', s[0])].decode(): s for s in sections}
            dynsym, dynstr = by_name['.dynsym'], by_name['.dynstr']
            strings = blob[dynstr[4]:dynstr[4] + dynstr[5]]
            raw_names = []
            for off in range(dynsym[4], dynsym[4] + dynsym[5], dynsym[9]):
                no, info, other, shndx, value, size = struct.unpack_from('<IBBHQQ', blob, off)
                raw = strings[no:strings.find(b'\0', no)].decode() if no else ''
                raw_names.append(raw)
                binding = info >> 4
                if raw and shndx and binding in (1, 2, 10):
                    providers.setdefault(raw, base + value)
            if '.rela.plt' not in by_name:
                continue
            relsec = by_name['.rela.plt']
            got_raw = {}
            for off in range(relsec[4], relsec[4] + relsec[5], relsec[9]):
                where, info, _addend = struct.unpack_from('<QQq', blob, off)
                if (info & 0xffffffff) == 0x402:
                    got_raw[where] = raw_names[info >> 32]
            # Recover each veneer start from its ADRP/LDR pair exactly as in
            # map_extra, but associate it with the raw relocation spelling.
            dis = subprocess.run([str(texture.OBJ), '-d', '-C', '-j', '.plt', str(path)],
                                 capture_output=True, text=True, check=True,
                                 env={**os.environ, 'LD_LIBRARY_PATH': str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}).stdout
            pending = page = addend = None
            for line in dis.splitlines():
                match = re.match(r'^\s*([0-9a-f]+):\s+[0-9a-f]+\s+adrp\s+x16,\s*([0-9a-f]+)', line)
                if match:
                    pending, page, addend = int(match.group(1), 16), int(match.group(2), 16), None
                    continue
                if pending is None:
                    continue
                match = re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+add\s+x16, x16, #0x([0-9a-f]+)', line)
                if match:
                    addend = int(match.group(1), 16)
                    continue
                if re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+br\s+x17', line):
                    raw = got_raw.get(page + addend) if addend is not None else None
                    if raw:
                        self.raw_plt_by_address[base + pending] = raw
                    pending = page = addend = None
                elif re.match(r'^\s*[0-9a-f]+:', line) and not re.match(r'^\s*[0-9a-f]+:\s+[0-9a-f]+\s+(nop|ldr)\b', line):
                    pending = page = addend = None
        self.raw_provider = providers

    def resolve_raw_symbol(self, raw):
        target = getattr(self, 'raw_provider', {}).get(raw)
        if target is None:
            pretty = self.demangle(raw)
            # Platform runtime imports intentionally remain named traps unless
            # their exact raw ELF definition is among the loaded providers.
            return self.relocation_trap(pretty)
        return target

    def __init__(self):
        self.alloc_records = []
        self.host_libm = C.CDLL('libm.so.6')
        self.host_libm.pow.argtypes = (C.c_double, C.c_double)
        self.host_libm.pow.restype = C.c_double
        self.host_libm.acosf.argtypes = (C.c_float,)
        self.host_libm.acosf.restype = C.c_float
        super().__init__()
        self.gles = mesa.GLES()
        self.gl_driver_events = []
        self.pending_gl_errors = []
        self.pending_gl_error_history = []
        self.last_tracked_gl_call = None
        self.gl_api_call_history = []
        self.host_locales = {}
        self.host_locale_tokens = {}
        self.owned_locale_tokens = set()
        self.host_files = {}
        self.callbacks.pop('SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)', None)
        self.callbacks.pop('SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)', None)
        for name, (index, expected) in EXTRA.items():
            path = LIB / ('libc++_shared.so' if name == 'Cpp' else f'libSPen{name}.so')
            actual = hashlib.sha256(path.read_bytes()).hexdigest()
            if not actual.startswith(expected):
                raise RuntimeError(f'unexpected {name} hash {actual}')
            self.map_extra(name, index)
        self.build_raw_provider_map()
        self.load_extra_relocs()
        self.callbacks.update({
            'pthread_mutex_init': lambda: self.x(0, 0),
            'pthread_mutex_destroy': lambda: self.x(0, 0),
            'pthread_mutex_lock': lambda: self.x(0, 0),
            'pthread_mutex_unlock': lambda: self.x(0, 0),
            'pthread_rwlock_init': lambda: self.x(0, 0),
            'pthread_rwlock_destroy': lambda: self.x(0, 0),
            'pthread_rwlock_rdlock': lambda: self.x(0, 0),
            'pthread_rwlock_wrlock': lambda: self.x(0, 0),
            'pthread_rwlock_unlock': lambda: self.x(0, 0),
            'pthread_cond_init': lambda: self.x(0, 0),
            'pthread_cond_destroy': lambda: self.x(0, 0),
            'pthread_cond_broadcast': lambda: self.x(0, 0),
            'pthread_cond_signal': lambda: self.x(0, 0),
            'pthread_cond_wait': self.reject_cond_wait,
            'getpid': lambda: self.x(0, 4242),
            'getauxval': lambda: self.x(0, 0),
            '__system_property_get': self.system_property_get,
            'syscall': self.syscall,
            'dl_iterate_phdr': lambda: self.x(0, 0),
            'pow': self.host_pow,
            'acosf': self.host_acosf,
            'realloc': self.host_realloc,
            'vsnprintf': self.vsnprintf,
            '__cxa_allocate_exception': self.allocate_exception,
            '__cxa_throw': self.capture_exception_throw,
            'wcscoll_l': self.wcscoll_l,
            'newlocale': self.newlocale_host,
            'freelocale': self.freelocale_host,
            'uselocale': self.uselocale_host,
            'mbtowc': self.mbtowc_host,
            '__ctype_get_mb_cur_max': self.ctype_mb_cur_max,
            'glTexSubImage2D': self.capture_texture_upload,
            '__android_log_print': self.android_log_print,
            'std::__ndk1::__next_prime(unsigned long)': self.next_prime,
            'std::__ndk1::recursive_mutex::lock()': lambda: self.x(0, 0),
            'std::__ndk1::recursive_mutex::unlock()': lambda: self.x(0, 0),
            'std::__ndk1::recursive_mutex::recursive_mutex()': lambda: self.x(0, 0),
            'std::__ndk1::recursive_mutex::~recursive_mutex()': lambda: self.x(0, 0),
            'uuid_create': self.uuid_create,
            'uuid_make': self.uuid_make,
            'uuid_export': self.uuid_export,
            'uuid_destroy': lambda: self.x(0, 0),
            'memcmp': self.memcmp,
            'strncmp': self.strncmp,
            'strcmp': self.strcmp,
            '__strlen_chk': self.strlen_checked,
            'glFenceSync': self.gl_fence_sync,
            'glClientWaitSync': lambda: self.x(0, 0x911a),
            'glWaitSync': lambda: self.x(0, 0),
            'glDeleteSync': lambda: self.x(0, 0),
            'glFlush': lambda: self.x(0, 0),
            'glFinish': lambda: self.x(0, 0),
            'glDeleteTextures': self.gl_delete_textures,
            'glDeleteSamplers': self.gl_delete_textures,
            'glGenFramebuffers': lambda: self.gen_names('framebuffer'),
            'glDeleteFramebuffers': self.gl_delete_textures,
            'glBindFramebuffer': lambda: self.record('glBindFramebuffer', target=self.x(0), object=self.x(1)),
            'glFramebufferTexture2D': lambda: self.record('glFramebufferTexture2D', target=self.x(0), attachment=self.x(1), textarget=self.x(2), texture=self.x(3), level=self.x(4)),
            'glCheckFramebufferStatus': lambda: self.x(0, 0x8cd5),
            'eglGetCurrentContext': lambda: self.x(0, 0xE61),
            'eglGetCurrentDisplay': lambda: self.x(0, 0xE62),
            'crc32': self.crc32,
            'fopen': self.fopen_cache_miss,
            'unlink': self.unlink_cache_miss,
            'fwrite': self.fwrite_host,
            'fread': self.fread_host,
            'fclose': self.fclose_host,
            'fseek': self.fseek_host,
            'ftell': self.ftell_host,
            'fflush': self.fflush_host,
            'setbuf': lambda: self.x(0, 0),
            'fileno': self.fileno_host,
            'fsync': self.fsync_host,
            '__errno': lambda: self.x(0, self.errno_ptr),
            '__strncpy_chk2': self.strncpy_checked2,
            'strstr': self.strstr_large,
            'strchr': self.strchr_host,
        })
        self.errno_ptr = self.alloc(4)
        self.queue = self.alloc(32)
        self.queue_vtable = self.alloc(64)
        self.queue_post = self.alloc(16)
        self.queue_task_return = self.alloc(16)
        self.queue_managed = self.alloc(16)
        self.managed_queue = self.alloc(32)
        self.managed_queue_vtable = self.alloc(64)
        self.managed_register = self.alloc(16)
        self.managed_unregister = self.alloc(16)
        self.managed_queue_events = []
        self.managed_tokens = {}
        self.queue_returns = []
        self.queue_tasks = []
        self.uuid_sequence = 0
        self.put(self.queue, 'Q', self.queue_vtable)
        queue_typeinfo = self.syms.get('typeinfo for SPen::IGLManagedMsgQueue')
        if queue_typeinfo is None:
            raise RuntimeError('pinned Renderer DSO has no IGLManagedMsgQueue typeinfo')
        self.put(self.queue_vtable - 16, 'qQ', 0, queue_typeinfo)
        self.put(self.queue_vtable + 16, 'Q', self.queue_post)
        self.put(self.queue_vtable + 56, 'Q', self.queue_managed)
        self.put(self.managed_queue, 'Q', self.managed_queue_vtable)
        self.put(self.managed_queue_vtable + 16, 'Q', self.managed_register)
        self.put(self.managed_queue_vtable + 24, 'Q', self.managed_unregister)
        self.uploads = []
        self.gl_calls = []
        self.gl_next_id = 1000
        self.gl_shader_sources = {}
        self.gl_program_shaders = {}
        self.gl_locations = {}
        self.gl_strings = {}
        self.u.mem_write(self.queue_post, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.u.mem_write(self.queue_task_return, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.u.mem_write(self.queue_managed, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.u.mem_write(self.managed_register, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.u.mem_write(self.managed_unregister, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.canvas_get_matrix = self.alloc(16)
        self.canvas_matrix = self.alloc(36)
        self.u.mem_write(self.canvas_get_matrix, struct.pack('<IIQ', 0x58000040, 0xd65f03c0, self.canvas_matrix))
        self.put(self.canvas_matrix, '9f', 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
        # Keep texture construction native while accepting dimensions from the
        # actual shape effect, and capture every uploaded payload byte.
        self.callbacks['glTexSubImage2D'] = self.capture_texture_upload
        self.native_shape_entries = []
        self.shape_events = []
        self.pc_ring = []
        self.pending_returns = getattr(self, 'pending_returns', [])
        # Execute each loaded provider's relocated init_array once in ELF
        # dependency order. The earlier probe manually ran selected Model
        # entries and omitted Base/View/Skia/Drawing/C++ provider initializers.
        for name in ('Cpp', 'Base', 'View', 'Skia', 'Drawing', 'Model', 'Renderer'):
            self.run_dso_initializers(name)

    def syscall(self):
        number = self.x(8)
        if number == 178: self.x(0, 4242)
        else: raise RuntimeError(f'unmodeled syscall {number}')

    def android_log_print(self):
        priority, tag, fmt = self.x(0), self.x(1), self.x(2)
        sp = self.u.reg_read(R.UC_ARM64_REG_SP)
        self.shape_events.append({'android_log': {'priority': priority,
                                'tag': self.read_c_string(tag),
                                'format': self.read_c_string(fmt),
                                'register_args': [self.x(i) for i in range(3, 8)],
                                'stack_args': [self.get(sp + i * 8, 'Q')[0] for i in range(4)]}})
        self.x(0, 0)

    def system_property_get(self):
        name = self.read_c_string(self.x(0))
        self.shape_events.append({'host_android_property': name, 'value': ''})
        self.u.mem_write(self.x(1), b'\0')
        self.x(0, 0)

    def fopen_cache_miss(self):
        path, mode = self.read_c_string(self.x(0)), self.read_c_string(self.x(1))
        cache_dir = ROOT / 'scratch/native-pattern-probe/cache'
        cache_dir.mkdir(parents=True, exist_ok=True)
        filename = hashlib.sha256(path.encode()).hexdigest() + '.bin'
        host = open(cache_dir / filename, mode)
        token = self.alloc(8)
        self.host_files[token] = host
        self.shape_events.append({'host_file_open': {'path': path, 'mode': mode,
                                                     'mapped_file': filename, 'result': hex(token)}})
        self.x(0, token)

    def unlink_cache_miss(self):
        path = self.read_c_string(self.x(0))
        filename = hashlib.sha256(path.encode()).hexdigest() + '.bin'
        target = ROOT / 'scratch/native-pattern-probe/cache' / filename
        try:
            target.unlink()
            result = 0
        except FileNotFoundError:
            self.put(self.errno_ptr, 'i', 2)
            result = -1
        self.shape_events.append({'host_file_unlink': {'path': path, 'result': result}})
        self.x(0, result)

    def fwrite_host(self):
        pointer, size, count, token = [self.x(i) for i in range(4)]
        if size > 1 << 20 or count > 1 << 20 or size * count > 16 << 20:
            raise RuntimeError(f'oversized native fwrite: {size=} {count=}')
        host = self.host_files.get(token)
        if host is None: raise RuntimeError(f'fwrite unknown guest FILE token {token:#x}')
        raw = bytes(self.u.mem_read(pointer, size * count)) if pointer and size * count else b''
        host.write(raw); host.flush()
        self.x(0, len(raw) // size if size else 0)

    def fread_host(self):
        pointer, size, count, token = [self.x(i) for i in range(4)]
        if size > 1 << 20 or count > 1 << 20 or size * count > 16 << 20:
            raise RuntimeError(f'oversized native fread: {size=} {count=}')
        host = self.host_files.get(token)
        if host is None: raise RuntimeError(f'fread unknown guest FILE token {token:#x}')
        raw = host.read(size * count)
        if pointer and raw: self.u.mem_write(pointer, raw)
        self.x(0, len(raw) // size if size else 0)

    def fclose_host(self):
        token = self.x(0); host = self.host_files.pop(token, None)
        if host is None: raise RuntimeError(f'fclose unknown guest FILE token {token:#x}')
        host.close(); self.x(0, 0)

    def fseek_host(self):
        token, offset, whence = self.x(0), self.x(1), self.x(2)
        host = self.host_files.get(token)
        if host is None: raise RuntimeError(f'fseek unknown guest FILE token {token:#x}')
        host.seek(struct.unpack('<q', struct.pack('<Q', offset))[0], whence); self.x(0, 0)

    def ftell_host(self):
        token = self.x(0); host = self.host_files.get(token)
        if host is None: raise RuntimeError(f'ftell unknown guest FILE token {token:#x}')
        self.x(0, host.tell())

    def fflush_host(self):
        token = self.x(0); host = self.host_files.get(token)
        if host is not None: host.flush()
        self.x(0, 0)

    def fileno_host(self):
        token = self.x(0); host = self.host_files.get(token)
        if host is None: raise RuntimeError(f'fileno unknown guest FILE token {token:#x}')
        self.x(0, host.fileno())

    def fsync_host(self):
        fd = self.x(0)
        os.fsync(fd)
        self.x(0, 0)

    def reject_cond_wait(self):
        raise RuntimeError('native init requested pthread_cond_wait; synchronous queue host is not a worker runtime')

    def vsnprintf(self):
        destination, capacity = self.x(0), self.x(1)
        if capacity:
            self.u.mem_write(destination, b'\0')
        self.x(0, 0)

    def allocate_exception(self):
        size = self.x(0)
        if not 0 < size <= 4096:
            raise RuntimeError(f'unbounded native exception allocation: {size}')
        address = self.alloc(size)
        self.shape_events.append({'native_exception_allocation': {'size': size, 'address': hex(address)}})
        self.x(0, address)

    def capture_exception_throw(self):
        exception, typeinfo, destructor = [self.x(i) for i in range(3)]
        type_name = next((name.removeprefix('typeinfo for ') for name, address in self.syms.items()
                          if name.startswith('typeinfo for ') and address == typeinfo), None)
        evidence = {'native_exception_throw': {'object': hex(exception), 'typeinfo': hex(typeinfo),
                                                'type': type_name, 'destructor': hex(destructor),
                                                'pc': hex(self.u.reg_read(R.UC_ARM64_REG_LR) - 4),
                                                'recent': [hex(p) for p in self.pc_ring[-24:]]}}
        self.shape_events.append(evidence)
        raise RuntimeError(json.dumps(evidence, sort_keys=True))

    def wcscoll_l(self):
        # The emulated libc++ init path asks the host locale facet to compare
        # guest wchar strings. Use the real host libc in explicit C locale,
        # copying only bounded guest data across the ABI boundary.
        left, right, guest_locale = [self.x(i) for i in range(3)]
        def wide(pointer):
            values = []
            for i in range(4096):
                value = self.get(pointer + i * 4, 'I')[0]
                if value == 0:
                    return values
                values.append(value)
            raise RuntimeError('wcscoll_l guest string exceeds bound')
        a, b = wide(left), wide(right)
        if any(value > 0x10ffff or 0xd800 <= value <= 0xdfff for value in a + b):
            raise RuntimeError(f'wcscoll_l non-Unicode inputs x0={left:#x} x1={right:#x} '
                               f'x2={self.x(2):#x} lr={self.u.reg_read(R.UC_ARM64_REG_LR):#x} '
                               f'left={a[:12]!r} right={b[:12]!r} '
                               f'raw0={bytes(self.u.mem_read(left, 48)).hex()} '
                               f'raw1={bytes(self.u.mem_read(right, 48)).hex()}')
        libc = C.CDLL(None)
        locale = self.host_locales.get(guest_locale)
        if not locale:
            raise RuntimeError(f'wcscoll_l received unregistered guest locale {guest_locale:#x}')
        fn = libc.wcscoll_l
        fn.restype, fn.argtypes = C.c_int, [C.c_wchar_p, C.c_wchar_p, C.c_void_p]
        aa = C.create_unicode_buffer(''.join(chr(v) for v in a))
        bb = C.create_unicode_buffer(''.join(chr(v) for v in b))
        self.x(0, fn(aa, bb, locale))

    def newlocale_host(self):
        mask, name_ptr, base_locale = [self.x(i) for i in range(3)]
        name = self.read_c_bytes(name_ptr) if name_ptr else b'C'
        libc = C.CDLL(None)
        fn = libc.newlocale
        fn.restype, fn.argtypes = C.c_void_p, [C.c_int, C.c_char_p, C.c_void_p]
        # Android and glibc assign different LC_ALL_MASK constants. This
        # isolated oracle requests the same named locale under host LC_ALL.
        host_locale = fn(0x1fbf, name or b'C', None)
        if not host_locale:
            raise RuntimeError(f'host newlocale failed for {name!r}')
        token = self.alloc(8)
        self.host_locales[token] = host_locale
        self.host_locale_tokens[host_locale] = token
        self.owned_locale_tokens.add(token)
        self.shape_events.append({'host_locale': {'mask': mask, 'name': name.decode(errors='replace'),
                                                   'base': hex(base_locale), 'token': hex(token)}})
        self.x(0, token)

    def freelocale_host(self):
        token = self.x(0)
        host_locale = self.host_locales.pop(token, None)
        if host_locale:
            if token in self.owned_locale_tokens:
                fn = C.CDLL(None).freelocale
                fn.restype, fn.argtypes = None, [C.c_void_p]
                fn(host_locale)
                self.owned_locale_tokens.discard(token)
            self.host_locale_tokens.pop(host_locale, None)
        self.x(0, 0)

    def uselocale_host(self):
        token = self.x(0)
        locale = self.host_locales.get(token)
        if token and locale is None:
            raise RuntimeError(f'uselocale received unregistered guest locale {token:#x}')
        fn = C.CDLL(None).uselocale
        fn.restype, fn.argtypes = C.c_void_p, [C.c_void_p]
        previous = fn(locale)
        previous_token = self.host_locale_tokens.get(previous)
        if previous_token is None:
            previous_token = self.alloc(8)
            self.host_locales[previous_token] = previous
            self.host_locale_tokens[previous] = previous_token
        self.x(0, previous_token)

    def mbtowc_host(self):
        output, source, length = [self.x(i) for i in range(3)]
        raw = bytes(self.u.mem_read(source, min(length, 4096))) if source and length else b''
        if length > 4096:
            raise RuntimeError(f'mbtowc input exceeds bound: {length}')
        source_buf = C.create_string_buffer(raw + b'\0') if source else None
        result = C.c_int()
        value = C.c_wchar()
        fn = C.CDLL(None).mbtowc
        fn.restype, fn.argtypes = C.c_int, [C.POINTER(C.c_wchar), C.c_char_p, C.c_size_t]
        result.value = fn(C.byref(value) if output else None,
                          C.cast(source_buf, C.c_char_p) if source_buf else None, length)
        if output and result.value >= 0:
            self.put(output, 'I', ord(value.value))
        self.x(0, result.value)

    def ctype_mb_cur_max(self):
        fn = getattr(C.CDLL(None), '__ctype_get_mb_cur_max')
        fn.restype, fn.argtypes = C.c_size_t, []
        self.x(0, fn())

    def call_address(self, address, instruction_limit=2_000_000):
        self.u.reg_write(R.UC_ARM64_REG_SP, 0x200f0000)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        self.u.emu_start(address, 0x200f1000, count=instruction_limit)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError(f'instruction limit at {address:#x}')

    def run_initializers(self, name, section_address, section_size):
        base = self.extra_bases[name]
        entries = [self.get(base + section_address + i, 'Q')[0]
                   for i in range(0, section_size, 8)]
        for entry in entries:
            if entry in (0, (1 << 64) - 1):
                continue
            self.call_address(entry)

    def run_dso_initializers(self, name):
        path = self.extra_paths.get(name, LIB / f'libSPen{name}.so')
        blob = path.read_bytes()
        shoff = struct.unpack_from('<Q', blob, 40)[0]
        entsize, count, stridx = struct.unpack_from('<HHH', blob, 58)
        sections = [struct.unpack_from('<IIQQQQIIQQ', blob, shoff + i * entsize)
                    for i in range(count)]
        strings_section = sections[stridx]
        strings = blob[strings_section[4]:strings_section[4] + strings_section[5]]
        init = next((s for s in sections
                     if strings[s[0]:strings.find(b'\0', s[0])] == b'.init_array'), None)
        if init is None:
            self.shape_events.append({'init_array': name, 'entries': 0})
            return
        base = self.extra_bases[name] if name in self.extra_bases else {'Base': 0x2000000}[name]
        entries = [self.get(base + init[3] + off, 'Q')[0]
                   for off in range(0, init[5], 8)]
        called = []
        for entry in entries:
            if entry in (0, (1 << 64) - 1):
                continue
            try:
                self.call_address(entry)
            except Exception as error:
                raise RuntimeError(f'{name} init_array entry {entry:#x} failed at '
                                   f'pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} '
                                   f'recent={[hex(p) for p in self.pc_ring[-12:]]!r}') from error
            called.append(hex(entry))
        self.shape_events.append({'init_array': name, 'entries': called})

    def call_stack_args(self, name, x, stack_args, instruction_limit=5_000_000):
        for i, value in enumerate(x):
            self.x(i, value)
        sp = 0x200f0000
        self.u.reg_write(R.UC_ARM64_REG_SP, sp)
        self.u.reg_write(R.UC_ARM64_REG_LR, 0x200f1000)
        for i, value in enumerate(stack_args):
            self.put(sp + i * 8, 'Q', value)
        self.u.emu_start(self.syms[name], 0x200f1000, count=instruction_limit)
        if self.u.reg_read(R.UC_ARM64_REG_PC) != 0x200f1000:
            raise RuntimeError(f'instruction limit in {name}')

    def next_prime(self):
        n = self.x(0)
        candidate = max(2, n)
        while True:
            prime = True
            d = 2
            while d * d <= candidate:
                if candidate % d == 0:
                    prime = False
                    break
                d += 1
            if prime:
                self.x(0, candidate)
                return
            candidate += 1

    def uuid_create(self):
        ptr = self.x(0)
        self.put(ptr, 'Q', 0x6000000 + 0x7d000)
        self.x(0, 0)

    def uuid_make(self):
        # Deterministic host-side identity for object initialization; this is
        # not the native platform UUID implementation.
        self.x(0, 0)

    def uuid_export(self):
        self.uuid_sequence += 1
        destination = self.get(self.x(2), 'Q')[0]
        value = f'00000000-0000-4000-8000-{self.uuid_sequence:012x}\0'.encode()
        self.u.mem_write(destination, value)
        self.x(0, 0)

    def memcmp(self):
        left, right, size = self.x(0), self.x(1), self.x(2)
        a, b = bytes(self.u.mem_read(left, size)), bytes(self.u.mem_read(right, size))
        result = next((x - y for x, y in zip(a, b) if x != y), 0)
        self.x(0, result)

    def strncmp(self):
        a, b, size = self.x(0), self.x(1), self.x(2)
        if size > 1 << 20:
            raise RuntimeError(f'strncmp input too large: {size}')
        left = bytes(self.u.mem_read(a, size))
        right = bytes(self.u.mem_read(b, size))
        result = 0
        for x, y in zip(left, right):
            if x != y:
                result = x - y
                break
            if x == 0:
                break
        self.x(0, result)

    def strcmp(self):
        a, b = self.read_c_bytes(self.x(0)), self.read_c_bytes(self.x(1))
        self.x(0, next((x - y for x, y in zip(a + b'\0', b + b'\0') if x != y), 0))

    def strlen_checked(self):
        ptr, limit = self.x(0), self.x(1)
        data = bytearray()
        for i in range(min(limit, 4096)):
            b = self.u.mem_read(ptr + i, 1)[0]
            if not b:
                break
            data.append(b)
        else:
            raise RuntimeError(f'__strlen_chk exceeded bound {limit}')
        self.x(0, len(data))

    def read_c_bytes(self, pointer, limit=1 << 20):
        out = bytearray()
        for i in range(limit):
            b = self.u.mem_read(pointer + i, 1)[0]
            if b == 0:
                return bytes(out)
            out.append(b)
        raise RuntimeError(f'unterminated C string exceeds bound at {pointer:#x}')

    def read_c_string(self, pointer, limit=1 << 20):
        return self.read_c_bytes(pointer, limit).decode('utf-8', 'replace')

    def program_active_uniforms(self, program):
        return NativeDrawCalligraphy.program_active_uniforms(self, program)

    def crc32(self):
        seed, pointer, size = self.x(0), self.x(1), self.x(2)
        if size > 1 << 20:
            raise RuntimeError(f'crc32 input too large: {size}')
        self.x(0, zlib.crc32(bytes(self.u.mem_read(pointer, size)), seed))

    def capture_texture_upload(self):
        sp = self.u.reg_read(R.UC_ARM64_REG_SP)
        pixels = self.get(sp, 'Q')[0]
        width, height = self.x(4), self.x(5)
        size = width * height * 4
        if not (0 < width <= 2048 and 0 < height <= 2048 and size <= 16 * 1024 * 1024):
            raise RuntimeError(f'out-of-scope texture upload {width}x{height}')
        raw = bytes(self.u.mem_read(pixels, size))
        self.uploads.append({'width': width, 'height': height, 'target': self.x(0),
                             'format': self.x(6), 'type': self.x(7),
                             'bytes': size, 'sha256': hashlib.sha256(raw).hexdigest(),
                             'payload_hex': raw.hex()})

    def host_pow(self):
        left = struct.unpack('<d', struct.pack('<Q', self.u.reg_read(R.UC_ARM64_REG_D0)))[0]
        right = struct.unpack('<d', struct.pack('<Q', self.u.reg_read(R.UC_ARM64_REG_D1)))[0]
        result = self.host_libm.pow(left, right)
        self.u.reg_write(R.UC_ARM64_REG_D0,
                         struct.unpack('<Q', struct.pack('<d', result))[0])
        self.shape_events.append({'host_math_boundary': {'symbol': 'pow',
                                                          'args': [left, right],
                                                          'result': result}})

    def host_acosf(self):
        value = struct.unpack('<f', struct.pack('<I', self.u.reg_read(R.UC_ARM64_REG_S0)))[0]
        result = self.host_libm.acosf(value)
        self.u.reg_write(R.UC_ARM64_REG_S0,
                         struct.unpack('<I', struct.pack('<f', result))[0])
        self.shape_events.append({'host_math_boundary': {'symbol': 'acosf',
                                                          'args': [value],
                                                          'result': result}})

    def host_realloc(self):
        old, size = self.x(0), self.x(1)
        if not 0 <= size <= 16 * 1024 * 1024:
            raise RuntimeError(f'realloc size outside probe bound: {size}')
        if old == 0:
            result = self.alloc(size)
            copied = 0
        else:
            record = next((entry for entry in reversed(self.alloc_records)
                           if entry['address'] == old), None)
            if record is None:
                raise RuntimeError(f'realloc of untracked pointer {old:#x}')
            result = self.alloc(size)
            copied = min(record['size'], size)
            if copied:
                self.u.mem_write(result, self.u.mem_read(old, copied))
        self.x(0, result)
        self.shape_events.append({'host_allocation_boundary': {
            'symbol': 'realloc', 'old': hex(old), 'size': size,
            'result': hex(result), 'copied_bytes': copied,
        }})

    def gl_fence_sync(self):
        sync = self.alloc(8)
        self.record('glFenceSync', condition=self.x(0), flags=self.x(1), synthetic_sync=hex(sync))
        self.x(0, sync)

    def gl_delete_textures(self):
        count, pointer = self.x(0), self.x(1)
        if not 0 <= count <= 64:
            raise RuntimeError(f'glDeleteTextures count {count}')
        ids = list(self.get(pointer, 'I' * count)) if count else []
        self.record('glDeleteTextures', ids=ids)
        self.x(0, 0)

    def hook(self, u, address, size, data):
        self.pc_ring.append(address)
        if len(self.pc_ring) > 256:
            del self.pc_ring[0]
        if address == 0x7000000 + 0x8ae14:
            self.shape_events.append({'draw_solid_rectangle_return': self.x(0),
                                      'x19': hex(self.x(19)), 'x20': hex(self.x(20)),
                                      'x21': hex(self.x(21)), 'x22': hex(self.x(22))})
        if address == 0x7000000 + 0x8b05c:
            self.shape_events.append({'canvas_draw_rect_call': {
                'canvas': hex(self.x(0)), 'rect': hex(self.x(1)),
                'paint': hex(self.x(2)),
                'rect_words': self.get(self.x(1), '4f') if self.x(1) else None,
            }})
        if address in (0x7000000 + 0x8af80, 0x7000000 + 0x8afbc,
                       0x7000000 + 0x8afd0, 0x7000000 + 0x8affc):
            self.shape_events.append({'draw_solid_rectangle_probe_return': {
                'pc': hex(address), 'x0': hex(self.x(0)),
                'x21': hex(self.x(21)), 'x22': hex(self.x(22)),
            }})
        if address in (0x7000000 + 0x8add0, 0x7000000 + 0x8ade4,
                       0x7000000 + 0x8adf8):
            sp = u.reg_read(R.UC_ARM64_REG_SP)
            self.shape_events.append({'drawpath_outer_canvas_step': {
                'pc': hex(address), 'x0': hex(self.x(0)),
                'x1': hex(self.x(1)),
                'fp_args': [struct.unpack('<f', struct.pack('<I', self.u.reg_read(
                    getattr(R, f'UC_ARM64_REG_S{i}')) & 0xffffffff))[0] for i in (0, 1)],
                'stack_xy': self.get(sp + 8, '2f'),
            }})
        if address == 0x7000000 + 0x8b43c:
            self.shape_events.append({'draw_moving_path_entry': {
                'buffer': hex(self.x(0)), 'rect': hex(self.x(1)),
                'path': hex(self.x(2)), 'args': [hex(self.x(i)) for i in range(6)],
                'buffer_prefix_before': bytes(self.u.mem_read(self.x(0), 64)).hex()
                    if self.x(0) else None,
            }})
        if address == 0x7000000 + 0x8b2d0:
            ptr = self.x(23)
            raw = bytes(self.u.mem_read(ptr, 80 * 80 * 4)) if ptr else b''
            self.shape_events.append({'draw_moving_path_return': {
                'buffer': hex(ptr), 'nonzero_bytes': sum(value != 0 for value in raw),
                'first_nonzero': next((i for i, value in enumerate(raw) if value), None),
                'sha256': hashlib.sha256(raw).hexdigest(),
                'sample_hex': raw[:64].hex(),
            }})
        if address == 0x7000000 + 0x8a8fc:
            obj = self.x(0)
            try:
                vtable = self.get(obj, 'Q')[0]
                target = self.get(vtable + 24, 'Q')[0]
            except Exception:
                vtable, target = 0, 0
            reverse = {value: name for name, value in self.syms.items()}
            self.shape_events.append({'shape_path_draw_dispatch': {
                'receiver': hex(obj), 'vtable': hex(vtable),
                'target': hex(target), 'target_symbol': reverse.get(target),
                'path': hex(self.x(1)), 'shape': hex(self.x(2)),
                'style': hex(self.x(3)),
            }})
        if address in tuple(0x7000000 + off for off in
                            (0x8a844, 0x8a85c, 0x8a86c, 0x8a878, 0x8a8b0)):
            self.shape_events.append({'native_shape_path_stage': {
                'pc': hex(address), 'x0': hex(self.x(0)), 'x1': hex(self.x(1)),
                'x2': hex(self.x(2)), 'x3': hex(self.x(3)),
                's0': struct.unpack('<f', struct.pack('<I', self.u.reg_read(R.UC_ARM64_REG_S0)))[0],
            }})
        if address == 0x9000000 + 0x19d5b8:
            self.shape_events.append({'native_skcanvas_draw_path': {
                'canvas': hex(self.x(0)), 'path': hex(self.x(1)),
                'paint': hex(self.x(2)),
            }})
        if address in (self.syms.get('SPen::RefCount::Unref()'),
                       self.syms.get('SPen::SPSubBitmap::~SPSubBitmap()'),
                       0x4000000 + 0x8b0f4, 0x4000000 + 0x8b0fc,
                       0x4000000 + 0x8b104, 0x4000000 + 0x8b10c):
            ptr = self.x(0)
            try:
                words = self.get(ptr, '10Q')
                vt_slots = self.get(words[0], '4Q') if words and words[0] else None
            except Exception:
                words, vt_slots = None, None
            self.shape_events.append({'refcount_or_subbitmap_dtor': hex(address),
                                      'x0': hex(ptr), 'x1': hex(self.x(1)),
                                      'words': words, 'vtable_slots': vt_slots,
                                      'recent': [hex(p) for p in self.pc_ring[-8:]]})
        for exception_symbol in ('__cxa_allocate_exception', '__cxa_throw', '__cxa_begin_catch',
                                 '__cxa_rethrow', '__cxa_end_catch'):
            if address == self.syms.get(exception_symbol):
                self.shape_events.append({'native_exception_entry': exception_symbol,
                                          'pc': hex(address),
                                          'args': [hex(self.x(i)) for i in range(4)],
                                          'lr': hex(u.reg_read(R.UC_ARM64_REG_LR))})
        if address == self.queue_post:
            task = self.x(1)
            vtable = self.get(task, 'Q')[0]
            method = self.get(vtable + 16, 'Q')[0]
            reverse = {value: name for name, value in self.syms.items()}
            self.queue_returns.append(u.reg_read(R.UC_ARM64_REG_LR))
            self.queue_tasks.append({'task': hex(task), 'run': hex(method),
                                     'run_symbol': reverse.get(method),
                                     'caller': hex(self.pc_ring[-2]) if len(self.pc_ring) > 1 else None})
            self.x(0, task)
            u.reg_write(R.UC_ARM64_REG_LR, self.queue_task_return)
            u.reg_write(R.UC_ARM64_REG_PC, method)
            return
        if address == self.queue_task_return:
            if not self.queue_returns: raise RuntimeError('unexpected queue task return')
            u.reg_write(R.UC_ARM64_REG_PC, self.queue_returns.pop())
            return
        if address == self.queue_managed:
            self.shape_events.append({'host_queue_method': 'IGLManagedMsgQueue.slot56',
                                      'receiver': hex(self.x(0)),
                                      'result': hex(self.managed_queue)})
            u.reg_write(R.UC_ARM64_REG_X0, self.managed_queue)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.managed_register:
            manager, managed_object = self.x(0), self.x(1)
            token = self.alloc(8)
            self.put(token, 'Q', managed_object)
            self.managed_tokens[token] = managed_object
            self.managed_queue_events.append({'method': 'Register', 'manager': hex(manager),
                                              'object': hex(managed_object), 'token': hex(token)})
            self.x(0, token)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.managed_unregister:
            manager, token = self.x(0), self.x(1)
            managed_object = self.managed_tokens.pop(token, None)
            self.managed_queue_events.append({'method': 'Unregister', 'manager': hex(manager),
                                              'token': hex(token),
                                              'object': None if managed_object is None else hex(managed_object)})
            self.x(0, 0)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address == self.canvas_get_matrix:
            u.reg_write(R.UC_ARM64_REG_X0, self.canvas_matrix)
            u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
            return
        if address in self.plt and (self.plt[address].startswith('gl') or self.plt[address].startswith('egl')):
            symbol = self.plt[address]
            if self.gl_driver_import(symbol):
                u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
                return
            if symbol == 'glFenceSync':
                self.gl_fence_sync()
                u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
                return
            if symbol == 'glClientWaitSync':
                self.x(0, 0x911a)
                u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
                return
            if symbol in ('glWaitSync', 'glDeleteSync'):
                u.reg_write(R.UC_ARM64_REG_PC, u.reg_read(R.UC_ARM64_REG_LR))
                return
            self.gl_driver_events.append({'call': 'synthetic_gl_fallback', 'symbol': symbol,
                                          'pc': hex(address), 'args': [hex(self.x(i)) for i in range(8)]})
            NativeDrawCalligraphy.gl_host_import(self, symbol)
            return
        for symbol in (
            'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::SPSubBitmapRT::SPSubBitmapRT(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::SPSubBitmap::SPSubBitmap(SPen::IGLMsgQueue*, int, SPen::Rect const&, SPen::ITextureObject*, char const*)',
            'SPen::SPSubBitmap::SPSubBitmap(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::GLES::TextureObject::TextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int)',
            'SPen::GLES::TextureObject::CreateTexture(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)',
            'SPen::GLES::TextureObject::createTextureInternalsSampler(SPen::TextureDescriptor const&, char const*, void const*, std::__ndk1::map<int, int, std::__ndk1::less<int>, std::__ndk1::allocator<std::__ndk1::pair<int const, int> > > const&, SPen::TextureBufferInfo const*)',
            'SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)',
            'SPen::GLES::RendererFactoryImpl::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)',
            'SPen::GLES::OffscreenRenderTarget::OffscreenRenderTarget(SPen::ITextureObject*)',
            'SPen::RendererFactory::CreateOffscreenRenderTarget(SPen::ITextureObject*)',
            'SPen::GLES::RendererFactoryImpl::CreateOffscreenRenderTarget(SPen::ITextureObject*)',
            'SPen::ObjectShapeDrawing::Construct(SPen::IContext const*)',
            'SPen::ObjectShapeDrawing::SetObject(SPen::ObjectShapeBase const*)',
            'SPen::ObjectShapeDrawing::SetEffect()',
            'SPen::ObjectShapeDrawing::Draw(SPen::ISPCanvas&)',
            'SPen::ObjectShapeDrawing::DrawPath(SPen::ISPCanvas&, SPen::Rect const*)',
            'SPen::ObjectDrawing::Construct(SPen::IContext const*)',
            'SPen::ObjectDrawing::drawObjectShape(SPen::ISPCanvas&, SPen::ObjectShape*, SPen::Rect const*)',
            'SPen::ShapeDrawingFillEffect::Construct(SPen::IContext const*)',
            'SPen::ShapeDrawingFillEffect::SetEffect(SPen::ObjectShapeBase const*)',
            'SPen::ShapeDrawingFillEffect::SetFillColorEffectSolid(SPen::ObjectShapeBase const*)',
            'SPen::ShapeDrawingFillEffect::DrawEffectColor(SPen::CanvasBitmap*, Spen_SkPath, SPen::ObjectShapeBase const*)',
            'SPen::ShapeDrawingFillEffect::DrawEffect(SPen::CanvasBitmap*, Spen_SkPath, SPen::ObjectShapeBase const*)',
            'SPen::ObjectShapeDrawing::drawPath(SPen::CanvasBitmap*, float, SPen::RectF)',
            'SPen::ObjectShapeDrawing::drawSolidRectangle(SPen::ObjectShapeBase*, SPen::ISPCanvas&, SPen::CanvasBitmap*)',
        ):
            if address == self.syms.get(symbol):
                self.native_shape_entries.append(symbol)
                if symbol.startswith(('SPen::SPGraphicsFactory', 'SPen::SPTextureBitmap', 'SPen::SPSubBitmap', 'SPen::GLES::TextureObject', 'SPen::RendererFactory', 'SPen::GLES::RendererFactory', 'SPen::GLES::Offscreen')):
                    self.shape_events.append({'native_entry': symbol,
                                              'args': [hex(self.x(i)) for i in range(8)],
                                              'stack_args': self.get(u.reg_read(R.UC_ARM64_REG_SP), '2Q')})
        if address in self.raw_plt_by_address:
            imported_name = self.plt.get(address)
            if imported_name not in self.callbacks and not (imported_name or '').startswith(('gl', 'egl')):
                u.reg_write(R.UC_ARM64_REG_PC,
                            self.resolve_raw_symbol(self.raw_plt_by_address[address]))
                return
        canvas_ctor = 'SPen::SPCanvas::SPCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)'
        for ctor_return in (0x4000000 + 0x8f29c, 0x4000000 + 0x8f33c):
            if address == ctor_return:
                obj = self.x(22)
                self.shape_events.append({'subbitmap_ctor_return': hex(obj),
                                          'words': self.get(obj, '12Q')})
        if address == self.syms.get(canvas_ctor):
            bitmap_obj = self.x(2)
            bitmap_vtable = self.get(bitmap_obj, 'Q')[0]
            vtable = self.get(bitmap_vtable, '24Q')
            reverse = {value: name for name, value in self.syms.items()}
            attachable = bitmap_obj + 200
            attachable_words = self.get(attachable, '8Q')
            attachable_vtable = attachable_words[0]
            attachable_slots = self.get(attachable_vtable, '8Q')
            self.shape_events.append({'native_entry': canvas_ctor,
                                      'bitmap': hex(bitmap_obj),
                                      'bitmap_words': self.get(bitmap_obj, '16Q'),
                                      'bitmap_vtable': hex(bitmap_vtable),
                                      'canvas_attachable': hex(attachable),
                                      'canvas_attachable_words': attachable_words,
                                      'canvas_attachable_vtable_symbols': [reverse.get(pointer)
                                                                           for pointer in attachable_slots],
                                      'bitmap_vtable_slots': [
                                          {'offset': offset, 'target': hex(vtable[offset // 8]),
                                           'symbol': reverse.get(vtable[offset // 8])}
                                          for offset in (128, 160)]})
        super().hook(u, address, size, data)

    def gl_driver_import(self, name):
        """Handle bounded shader/program operations with real Mesa GLES."""
        raw_f, g = self.gles.f, self.gles.gl
        raw_get_error = raw_f['glGetError']
        def tracked(symbol, fn):
            def call(*args):
                before = raw_get_error()
                if before:
                    self.pending_gl_errors.append(before)
                    self.pending_gl_error_history.append({'phase': 'before', 'caller': hex(self.pc_ring[-2]) if len(self.pc_ring) > 1 else None, 'symbol': symbol, 'error': before, 'last_tracked_call': self.last_tracked_gl_call, 'preceding_calls': self.gl_api_call_history[-12:]})
                result = fn(*args)
                after = raw_get_error()
                self.last_tracked_gl_call = {'caller': hex(self.pc_ring[-2]) if len(self.pc_ring) > 1 else None, 'symbol': symbol}
                self.gl_api_call_history.append({'caller': self.last_tracked_gl_call['caller'], 'symbol': symbol, 'before_error': before, 'after_error': after})
                if after:
                    self.pending_gl_errors.append(after)
                    self.pending_gl_error_history.append({'phase': 'after', 'caller': hex(self.pc_ring[-2]) if len(self.pc_ring) > 1 else None, 'symbol': symbol, 'error': after})
                    self.gl_driver_events.append({'call': 'mesa_call_error',
                                                  'native_call': symbol,
                                                  'error': after})
                return result
            return call
        f = {symbol: (fn if symbol == 'glGetError' else tracked(symbol, fn))
             for symbol, fn in raw_f.items()}
        x = self.x
        def function(symbol, restype, argtypes):
            return tracked(symbol, mesa.cfunc(g, symbol, restype, argtypes))
        def integer_output(symbol, count_arg=0, pointer_arg=1):
            count, pointer = x(count_arg), x(pointer_arg)
            if not 0 <= count <= 64: raise RuntimeError(f'bad {symbol} count {count}')
            before = self.get(pointer, f'{count}I') if pointer and count else ()
            values = (C.c_uint * count)()
            function(symbol, None, [C.c_int, C.POINTER(C.c_uint)])(count, values)
            for i, value in enumerate(values): self.put(pointer + 4 * i, 'I', value)
            owner = next((record for record in self.alloc_records
                          if record['address'] <= pointer < record['address'] + record['size']), None)
            self.gl_driver_events.append({'call': symbol, 'count': count,
                                          'names': [int(value) for value in values],
                                          'output_ptr': hex(pointer), 'before': before,
                                          'after': self.get(pointer, f'{count}I') if pointer and count else (),
                                          'caller_pc': hex(self.pc_ring[-2]) if len(self.pc_ring) > 1 else None,
                                          'owner': owner})
        if name == 'glCreateShader':
            shader_type = x(0); handle = f[name](shader_type); x(0, handle)
            self.gl_driver_events.append({'call': name, 'handle': handle, 'type': shader_type})
        elif name == 'glCreateProgram':
            handle = f[name](); x(0, handle)
            self.gl_driver_events.append({'call': name, 'handle': handle})
        elif name == 'glShaderSource':
            shader, count, array, lens = [x(i) for i in range(4)]
            if not 0 < count <= 64: raise RuntimeError(f'bad Mesa glShaderSource count {count}')
            keep, ptrs, sizes, sources = [], [], [], []
            for i in range(count):
                ptr = self.get(array + i * 8, 'Q')[0]
                length = self.get(lens + i * 4, 'i')[0] if lens else -1
                raw = self.read_c_bytes(ptr) if length < 0 else bytes(self.u.mem_read(ptr, length))
                buf = C.create_string_buffer(raw); keep.append(buf); ptrs.append(C.cast(buf, C.c_char_p)); sizes.append(len(raw))
                sources.append(raw.decode('utf-8', 'replace'))
            f[name](shader, count, (C.c_char_p * count)(*ptrs), (C.c_int * count)(*sizes))
            self.gl_driver_events.append({'call': name, 'shader': shader, 'sources': sources})
        elif name in ('glCompileShader', 'glLinkProgram', 'glUseProgram', 'glAttachShader',
                      'glDeleteShader', 'glDetachShader', 'glDeleteProgram'):
            argc = {'glAttachShader': 2, 'glDetachShader': 2}.get(name, 1)
            function(name, None, [C.c_uint] * argc)(*[x(i) for i in range(argc)])
            self.gl_driver_events.append({'call': name,
                                          'args': [x(i) for i in range(argc)]})
        elif name in ('glGetShaderiv', 'glGetProgramiv'):
            out = C.c_int(); f[name](x(0), x(1), C.byref(out)); self.put(x(2), 'i', out.value)
            self.gl_driver_events.append({'call': name, 'object': x(0), 'pname': x(1), 'value': out.value})
        elif name in ('glGetShaderInfoLog', 'glGetProgramInfoLog'):
            buf = C.create_string_buffer(x(1)); length = C.c_int()
            f[name](x(0), x(1), C.byref(length), buf)
            if x(2): self.put(x(2), 'i', length.value)
            if x(3) and x(1): self.u.mem_write(x(3), buf.raw[:x(1)])
        elif name in ('glGetProgramBinary', 'glGetProgramBinaryOES'):
            addr = self.gles.egl.eglGetProcAddress(b'glGetProgramBinary')
            if not addr: raise RuntimeError('Mesa GLES lacks glGetProgramBinary')
            fn = tracked(name, C.CFUNCTYPE(None, C.c_uint, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_uint), C.c_void_p)(addr))
            buf = C.create_string_buffer(x(1)); length, fmt = C.c_int(), C.c_uint()
            fn(x(0), x(1), C.byref(length), C.byref(fmt), buf)
            if x(2): self.put(x(2), 'i', length.value)
            if x(3): self.put(x(3), 'I', fmt.value)
            if x(4) and length.value: self.u.mem_write(x(4), buf.raw[:length.value])
            self.gl_driver_events.append({'call': name, 'program': x(0), 'length': length.value, 'format': fmt.value})
        elif name in ('glProgramBinary', 'glProgramBinaryOES'):
            addr = self.gles.egl.eglGetProcAddress(b'glProgramBinary')
            if not addr: raise RuntimeError('Mesa GLES lacks glProgramBinary entry point')
            fn = tracked(name, C.CFUNCTYPE(None, C.c_uint, C.c_uint, C.c_void_p, C.c_int)(addr))
            program, fmt, binary, length = [x(i) for i in range(4)]
            if not 0 <= length <= 1 << 20: raise RuntimeError(f'bad glProgramBinary length {length}')
            raw = bytes(self.u.mem_read(binary, length)) if binary and length else b''
            buf = C.create_string_buffer(raw) if raw else None
            fn(program, fmt, C.cast(buf, C.c_void_p) if buf else None, length)
            self.gl_driver_events.append({'call': name, 'program': program, 'format': fmt, 'length': length})
        elif name in ('glGetActiveUniform', 'glGetActiveAttrib'):
            fn = function(name, None, [C.c_uint, C.c_uint, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_uint), C.c_void_p])
            buf = C.create_string_buffer(x(2)); length, size, typ = C.c_int(), C.c_int(), C.c_uint()
            fn(x(0), x(1), x(2), C.byref(length), C.byref(size), C.byref(typ), buf)
            self.put(x(3), 'i', length.value); self.put(x(4), 'i', size.value); self.put(x(5), 'I', typ.value)
            self.u.mem_write(x(6), buf.raw[:x(2)])
        elif name == 'glGetUniformBlockIndex':
            fn = function(name, C.c_uint, [C.c_uint, C.c_char_p])
            x(0, fn(x(0), self.read_c_string(x(1)).encode()))
        elif name == 'glGetUniformLocation': x(0, f[name](x(0), self.read_c_string(x(1)).encode()))
        elif name == 'glGetAttribLocation':
            fn = function(name, C.c_int, [C.c_uint, C.c_char_p])
            x(0, fn(x(0), self.read_c_string(x(1)).encode()))
        elif name == 'glBindAttribLocation': f[name](x(0), x(1), self.read_c_string(x(2)).encode())
        elif name == 'glGetString':
            enum = x(0)
            text = self.gles.gl_string(enum) or b''
            host = self.alloc(len(text) + 1); self.u.mem_write(host, text + b'\0'); x(0, host)
            self.gl_driver_events.append({'call': name, 'enum': enum, 'value': text.decode(errors='replace')})
        elif name == 'glGetIntegerv':
            fn = function(name, None, [C.c_uint, C.POINTER(C.c_int)])
            out = C.c_int(); fn(x(0), C.byref(out)); self.put(x(1), 'i', out.value)
            self.gl_driver_events.append({'call': name, 'pname': x(0), 'value': out.value})
        elif name in ('glGenTextures', 'glGenBuffers', 'glGenFramebuffers', 'glGenSamplers',
                      'glGenVertexArrays', 'glGenRenderbuffers'):
            integer_output(name)
        elif name in ('glDeleteTextures', 'glDeleteBuffers', 'glDeleteFramebuffers',
                      'glDeleteSamplers', 'glDeleteVertexArrays', 'glDeleteRenderbuffers'):
            count, pointer = x(0), x(1)
            if not 0 <= count <= 64: raise RuntimeError(f'bad {name} count {count}')
            values = (C.c_uint * count)(*[self.get(pointer + i * 4, 'I')[0] for i in range(count)])
            function(name, None, [C.c_int, C.POINTER(C.c_uint)])(count, values)
        elif name in ('glBindTexture', 'glBindBuffer', 'glBindFramebuffer', 'glBindRenderbuffer',
                      'glBindSampler', 'glBindVertexArray', 'glActiveTexture', 'glEnable', 'glDisable',
                      'glPixelStorei', 'glTexParameteri', 'glSamplerParameteri', 'glEnableVertexAttribArray',
                      'glDisableVertexAttribArray', 'glBlendEquation', 'glBlendEquationSeparate',
                      'glBlendFunc', 'glBlendFuncSeparate', 'glColorMask', 'glCullFace', 'glDepthFunc',
                      'glDepthMask', 'glScissor', 'glViewport', 'glClear', 'glClearColor', 'glClearStencil',
                      'glClearDepthf', 'glFramebufferTexture2D', 'glFramebufferRenderbuffer',
                      'glRenderbufferStorage', 'glCheckFramebufferStatus'):
            signatures = {
                'glBindTexture': (None, [C.c_uint, C.c_uint]), 'glBindBuffer': (None, [C.c_uint, C.c_uint]),
                'glBindFramebuffer': (None, [C.c_uint, C.c_uint]), 'glBindRenderbuffer': (None, [C.c_uint, C.c_uint]),
                'glBindSampler': (None, [C.c_uint, C.c_uint]), 'glBindVertexArray': (None, [C.c_uint]),
                'glActiveTexture': (None, [C.c_uint]), 'glEnable': (None, [C.c_uint]), 'glDisable': (None, [C.c_uint]),
                'glPixelStorei': (None, [C.c_uint, C.c_int]), 'glTexParameteri': (None, [C.c_uint, C.c_uint, C.c_int]),
                'glSamplerParameteri': (None, [C.c_uint, C.c_uint, C.c_int]),
                'glEnableVertexAttribArray': (None, [C.c_uint]), 'glDisableVertexAttribArray': (None, [C.c_uint]),
                'glBlendEquation': (None, [C.c_uint]), 'glBlendEquationSeparate': (None, [C.c_uint, C.c_uint]),
                'glBlendFunc': (None, [C.c_uint, C.c_uint]), 'glBlendFuncSeparate': (None, [C.c_uint]*4),
                'glColorMask': (None, [C.c_ubyte]*4), 'glCullFace': (None, [C.c_uint]),
                'glDepthFunc': (None, [C.c_uint]), 'glDepthMask': (None, [C.c_ubyte]),
                'glScissor': (None, [C.c_int]*4), 'glViewport': (None, [C.c_int]*4),
                'glClear': (None, [C.c_uint]), 'glClearColor': (None, [C.c_float]*4),
                'glClearStencil': (None, [C.c_int]), 'glClearDepthf': (None, [C.c_float]),
                'glFramebufferTexture2D': (None, [C.c_uint]*3+[C.c_uint, C.c_int]),
                'glFramebufferRenderbuffer': (None, [C.c_uint]*4),
                'glRenderbufferStorage': (None, [C.c_uint, C.c_uint, C.c_int, C.c_int]),
                'glCheckFramebufferStatus': (C.c_uint, [C.c_uint]),
            }
            restype, argtypes = signatures[name]
            fn = function(name, restype, argtypes)
            if name == 'glClearColor':
                values = [self.f(i) for i in range(4)]
                fn(*values)
                self.gl_driver_events.append({'call': name, 'values': values})
            elif name == 'glClearDepthf':
                value = self.f(0)
                fn(value)
                self.gl_driver_events.append({'call': name, 'value': value})
            elif name in ('glCheckFramebufferStatus',): x(0, fn(x(0)))
            else: fn(*[x(i) for i in range(len(argtypes))])
            if name in ('glBindFramebuffer', 'glFramebufferTexture2D', 'glCheckFramebufferStatus',
                        'glBindTexture', 'glBindBuffer', 'glBindVertexArray',
                        'glViewport', 'glScissor', 'glColorMask', 'glBlendFunc',
                        'glBlendFuncSeparate', 'glBlendEquation', 'glEnable', 'glDisable',
                        'glEnableVertexAttribArray', 'glDisableVertexAttribArray',
                        'glActiveTexture', 'glBindSampler'):
                extra = {}
                if name == 'glFramebufferTexture2D':
                    object_ptr = x(22)
                    try: object_value = self.get(object_ptr + 44, 'I')[0]
                    except Exception: object_value = None
                    try: object_words = self.get(object_ptr, '16I')
                    except Exception: object_words = None
                    owner = next((record for record in self.alloc_records
                                  if record['address'] <= object_ptr < record['address'] + record['size']), None)
                    extra = {'x22': hex(object_ptr), 'x22_plus44': object_value,
                             'x22_words': object_words, 'x22_owner': owner}
                self.gl_driver_events.append({'call': name, 'args': [x(i) for i in range(len(argtypes))], **extra})
        elif name == 'glTexImage2D' or name == 'glTexSubImage2D':
            if name == 'glTexImage2D':
                target, level, internal, width, height, border, fmt, typ = [x(i) for i in range(8)]
                pointer = self.get(self.u.reg_read(R.UC_ARM64_REG_SP), 'Q')[0]
            else:
                target, level, xoff, yoff, width, height, fmt, typ = [x(i) for i in range(8)]
                pointer = self.get(self.u.reg_read(R.UC_ARM64_REG_SP), 'Q')[0]
            if width < 0 or height < 0 or width * height > 1 << 26: raise RuntimeError(f'bad {name} size {width}x{height}')
            byte_count = width * height * (4 if fmt in (0x1908, 0x80e1, 0x8d99) else 1)
            raw = bytes(self.u.mem_read(pointer, byte_count)) if pointer and byte_count else b''
            buf = C.create_string_buffer(raw) if raw else None
            if name not in f:
                fn = function(name, None, [C.c_uint,C.c_int,C.c_int,C.c_int,C.c_int,C.c_int,C.c_uint,C.c_uint,C.c_void_p])
            else: fn = f[name]
            if name == 'glTexImage2D': fn(target, level, internal, width, height, border, fmt, typ, C.cast(buf,C.c_void_p) if buf else None)
            else: fn(target, level, xoff, yoff, width, height, fmt, typ, C.cast(buf,C.c_void_p) if buf else None)
            if name == 'glTexSubImage2D':
                self.uploads.append({'width': width, 'height': height, 'target': target,
                                     'format': fmt, 'type': typ, 'bytes': len(raw),
                                     'sha256': hashlib.sha256(raw).hexdigest(), 'payload_hex': raw.hex()})
            self.gl_driver_events.append({'call': name, 'width': width, 'height': height,
                                              'target': target, 'level': level,
                                              'internal_format': internal, 'border': border,
                                              'format': fmt, 'type': typ,
                                          'byte_count': byte_count, 'pixels_ptr': hex(pointer),
                                          'payload_len': len(raw),
                                          'sha256': hashlib.sha256(raw).hexdigest(),
                                          'first16': raw[:16].hex()})
        elif name == 'glBufferData':
            target, length, pointer, usage = [x(i) for i in range(4)]
            if length > 1 << 26: raise RuntimeError(f'oversized glBufferData {length}')
            raw = bytes(self.u.mem_read(pointer, length)) if pointer and length else b''
            buf = C.create_string_buffer(raw) if raw else None
            f[name](target, length, C.cast(buf, C.c_void_p) if buf else None, usage)
            self.gl_driver_events.append({'call': name, 'target': target, 'length': length,
                                          'usage': usage, 'pointer': hex(pointer),
                                          'sha256': hashlib.sha256(raw).hexdigest(),
                                          'payload_hex': raw.hex() if length <= 4096 else None})
        elif name == 'glVertexAttribPointer':
            f[name](x(0), x(1), x(2), x(3), x(4), C.c_void_p(x(5)))
            self.gl_driver_events.append({'call': name, 'index': x(0), 'size': x(1),
                                          'type': x(2), 'normalized': x(3),
                                          'stride': x(4), 'pointer': hex(x(5))})
        elif name == 'glUniformMatrix4fv':
            location, count, transpose, pointer = [x(i) for i in range(4)]
            values = (C.c_float * (count * 16))(*self.get(pointer, 'f' * (count * 16)))
            f[name](location, count, transpose, values)
            self.gl_driver_events.append({'call': name, 'location': location,
                                          'count': count, 'transpose': transpose,
                                          'values': list(values)})
        elif name == 'glUniform4f':
            vals = [self.f(i) for i in range(4)]
            f[name](x(0), *vals)
            self.gl_driver_events.append({'call': name, 'location': x(0), 'values': vals})
        elif name in ('glUniform2f', 'glUniform3f'):
            width = int(name[9])
            vals = [self.f(i) for i in range(width)]
            function(name, None, [C.c_int] + [C.c_float] * width)(x(0), *vals)
            self.gl_driver_events.append({'call': name, 'location': x(0), 'values': vals})
        elif name in ('glUniform1fv', 'glUniform2fv', 'glUniform3fv', 'glUniform4fv'):
            location, count, pointer = x(0), x(1), x(2)
            width = int(name[9])
            if not 0 <= count <= 64:
                raise RuntimeError(f'bad {name} count {count}')
            values = self.get(pointer, 'f' * (count * width)) if pointer and count else ()
            array = (C.c_float * len(values))(*values)
            function(name, None, [C.c_int, C.c_int, C.POINTER(C.c_float)])(
                location, count, array if values else None)
            self.gl_driver_events.append({'call': name, 'location': location,
                                          'count': count, 'values': list(values)})
        elif name == 'glUniform1i':
            f[name](x(0), x(1))
            self.gl_driver_events.append({'call': name, 'location': x(0), 'value': x(1)})
        elif name == 'glUniform1f':
            value = self.f(0)
            f[name](x(0), value)
            self.gl_driver_events.append({'call': name, 'location': x(0), 'value': value})
        elif name == 'glDrawArrays':
            f[name](x(0), x(1), x(2))
            self.gl_driver_events.append({'call': name, 'mode': x(0),
                                          'first': x(1), 'count': x(2)})
        elif name == 'glGetError':
            from_queue = bool(self.pending_gl_errors)
            error = self.pending_gl_errors.pop(0) if from_queue else f[name]()
            x(0, error)
            self.gl_driver_events.append({'call': name, 'error': error,
                                          'source': 'queued Mesa error' if from_queue
                                          else 'Mesa glGetError',
                                          'pending_history': self.pending_gl_error_history[-8:] if from_queue else []})
        else:
            return False
        return True

    def read_texture(self, texture_id, width, height):
        f = self.gles.f
        framebuffer = C.c_uint()
        f['glGenFramebuffers'](1, C.byref(framebuffer))
        f['glBindFramebuffer'](0x8D40, framebuffer.value)
        f['glFramebufferTexture2D'](0x8D40, 0x8CE0, 0x0DE1, texture_id, 0)
        status = f['glCheckFramebufferStatus'](0x8D40)
        before_error = f['glGetError']()
        raw = C.create_string_buffer(width * height * 4)
        f['glPixelStorei'](0x0D05, 1)
        f['glReadPixels'](0, 0, width, height, 0x1908, 0x1401, raw)
        after_error = f['glGetError']()
        return {'fbo': framebuffer.value, 'status': status,
                'errors': [before_error, after_error], 'pixels': raw.raw,
                'sha256': hashlib.sha256(raw.raw).hexdigest()}

    def strstr_large(self):
        hay_ptr, needle_ptr = self.x(0), self.x(1)
        hay = self.read_c_string(hay_ptr, 1 << 20).encode()
        needle = self.read_c_string(needle_ptr, 1 << 20).encode()
        index = hay.find(needle)
        self.x(0, 0 if index < 0 else hay_ptr + index)

    def strncpy_checked2(self):
        dst, src, count, dst_size, src_size = [self.x(i) for i in range(5)]
        if count > dst_size or count > src_size or count > (1 << 20):
            raise RuntimeError(f'__strncpy_chk2 bounds count={count} dst={dst_size} src={src_size}')
        raw = bytes(self.u.mem_read(src, count))
        end = raw.find(b'\0')
        if end >= 0:
            raw = raw[:end] + bytes(count - end)
        self.u.mem_write(dst, raw)
        self.x(0, dst)

    def strchr_host(self):
        pointer, byte = self.x(0), self.x(1) & 0xff
        raw = self.read_c_string(pointer, 1 << 20).encode() + b'\0'
        index = raw.find(bytes([byte]))
        self.x(0, 0 if index < 0 else pointer + index)

    def run_shape(self):
        self.call('SPen::AttachToThread(SPen::IGLMsgQueue*)', [self.queue])
        ctx, color, drawing, shape, path = [self.alloc(n) for n in (128, 64, 256, 512, 128)]
        self.call('SPen::Color::Color()', [color])
        self.call('SPen::Context::Context(SPen::IDisplay*, SPen::IGLMsgQueue*, SPen::RendererType, SPen::IConfiguration*, SPen::Color*, SPen::AnimatorUpdateManager*)',
                  [ctx, 0, self.queue, 0, 0, color, 0])
        rect = self.alloc(16)
        self.put(rect, '4f', 10.0, 20.0, 90.0, 100.0)
        segments = self.alloc(5 * 28)
        rows = [
            [1, 10.0, 20.0, 0, 0, 0, 0],
            [2, 90.0, 20.0, 0, 0, 0, 0],
            [2, 90.0, 100.0, 0, 0, 0, 0],
            [2, 10.0, 100.0, 0, 0, 0, 0],
            [6, 0, 0, 0, 0, 0, 0],
        ]
        self.u.mem_write(segments, b''.join(struct.pack('<I6f', *row) for row in rows))
        self.call('SPen::Path::Path()', [path])
        self.call('SPen::Path::Construct()', [path])
        self.call('SPen::Path::SetSegment(SPen::Path::Segment const*, int)', [path, segments, len(rows)])
        init_mode = os.environ.get('SHAPE_INIT_MODE', 'direct')
        if init_mode == 'direct':
            shape_obj = self.alloc(512)
            self.call('SPen::ObjectShape::ObjectShape()', [shape_obj])
            self.call('SPen::ObjectShape::Construct(SPen::Path*, bool)', [shape_obj, path, 1])
            construct_result = self.x(0)
        else:
            factory_flag = int(os.environ.get('SHAPE_FACTORY_FLAG', '1'), 0)
            self.call('SPen::ObjectFactory::CreateObject(SPen::Object::Type, bool)', [7, factory_flag])
            shape_obj = self.x(0)
            self.call('SPen::ObjectShape::SetPath(SPen::Path const*)', [shape_obj, path])
            construct_result = None
        self.call('SPen::ObjectBase::GetType() const', [shape_obj])
        self.shape_events.append({'shape_construction_probe': {
            'init_mode': init_mode, 'construct_result': construct_result,
            'shape_type': self.x(0), 'object': hex(shape_obj), 'path': hex(path),
        }})
        self.call('SPen::ObjectShape::GetPath() const', [shape_obj])
        self.shape_events.append({'object_shape_get_path_probe': {
            'result': hex(self.x(0)), 'source_path': hex(path),
            'segment_count': len(rows),
        }})
        self.call('SPen::ObjectShape::SetRect(SPen::RectF, bool)', [shape_obj, 0, 1], [10.0, 20.0, 90.0, 100.0])
        effect = self.alloc(128)
        self.call('SPen::FillColorEffect::FillColorEffect()', [effect])
        self.call('SPen::FillColorEffect::Construct()', [effect])
        self.call('SPen::FillColorEffect::SetSolidColor(unsigned int)', [effect, 0xff336699])
        self.call('SPen::ObjectShape::SetFillEffect(SPen::FillEffectBase*)', [shape_obj, effect])
        # Explicit no-outline diagnostic input: native's solid-rectangle fast
        # path is only eligible when its line color is absent/zero.
        line_effect = self.alloc(128)
        self.call('SPen::LineColorEffect::LineColorEffect()', [line_effect])
        self.call('SPen::LineColorEffect::Construct()', [line_effect])
        line_color = int(os.environ.get('SHAPE_LINE_COLOR', '0'), 0)
        self.call('SPen::LineColorEffect::SetSolidColor(unsigned int)', [line_effect, line_color])
        self.call('SPen::ObjectShapeBase::SetLineColorEffect(SPen::LineColorEffect*)',
                  [shape_obj, line_effect])
        line_width = float(os.environ.get('SHAPE_LINE_WIDTH', '2' if line_color else '0'))
        line_style = self.alloc(128)
        self.call('SPen::LineStyleEffect::LineStyleEffect()', [line_style])
        self.call('SPen::LineStyleEffect::Construct()', [line_style])
        self.call('SPen::LineStyleEffect::SetWidth(float)', [line_style], [line_width])
        self.call('SPen::ObjectShapeBase::SetLineStyleEffect(SPen::LineStyleEffect*)',
                  [shape_obj, line_style])
        line_style_copy = self.alloc(128)
        self.call('SPen::LineStyleEffect::LineStyleEffect()', [line_style_copy])
        self.call('SPen::LineStyleEffect::Construct()', [line_style_copy])
        self.call('SPen::ObjectShapeBase::GetLineStyleEffect(SPen::LineStyleEffect*) const',
                  [shape_obj, line_style_copy])
        line_style_present = self.x(0)
        measured_line_width = None
        if line_style_present:
            self.call('SPen::LineStyleEffect::GetWidth() const', [line_style_copy])
            measured_line_width = self.f(0)
        self.shape_events.append({'declared_shape_input': {
            'fill_color': '0xff336699', 'line_color': hex(line_color),
            'path_rect': [10.0, 20.0, 90.0, 100.0],
            'line_width': line_width,
            'line_style_effect_present': bool(line_style_present),
            'measured_line_width': measured_line_width,
        }})
        fill_effect_copy = self.alloc(128)
        self.call('SPen::FillColorEffect::FillColorEffect()', [fill_effect_copy])
        self.call('SPen::FillColorEffect::Construct()', [fill_effect_copy])
        self.call('SPen::ObjectShape::GetFillEffect(SPen::FillEffectBase*) const',
                  [shape_obj, fill_effect_copy])
        fill_present = self.x(0)
        if fill_present:
            self.call('SPen::FillColorEffect::GetSolidColor() const', [fill_effect_copy])
            fill_color = self.x(0)
        else:
            fill_color = None
        self.shape_events.append({'fill_effect_probe': {'present': fill_present,
                                                        'solid_color': fill_color,
                                                        'object': hex(shape_obj)}})
        bitmap = self.alloc(128 * 128 * 4)
        self.u.mem_write(bitmap, bytes([0, 0, 0, 0]) * (128 * 128))
        fmt = self.alloc(8)
        self.u.mem_write(fmt, b'RGBA\0')
        bitmap_factory = 'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'
        try:
            self.call_stack_args(bitmap_factory, [self.queue, 128, 128, fmt, bitmap, 1, 512, 1], [6, 2])
        except Exception as error:
            raise RuntimeError(f'bitmap factory crash pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} '
                               f'regs={[self.x(i) for i in range(9)]!r} '
                               f'queue={self.queue:#x} queue_vtable={self.queue_vtable:#x} '
                               f'vtable_words={self.get(self.queue_vtable-16,"2Q")!r} '
                               f'tail={[hex(x) for x in self.pc_ring[-128:]]!r}') from error
        target_bitmap = self.x(0)
        if not target_bitmap:
            raise RuntimeError('native SPGraphicsFactory returned null target bitmap')
        target_bitmap_words = self.get(target_bitmap, '4Q')
        target_bitmap_snapshot = self.get(target_bitmap, '32I')
        target_texture_id = next((event['names'][0] for event in self.gl_driver_events
                                  if event.get('call') == 'glGenTextures'
                                  and event.get('names')), 0)
        pre_draw_readback = (self.read_texture(target_texture_id, 128, 128)
                             if target_texture_id else None)
        canvas = self.alloc(512)
        canvas_ctor = 'SPen::SPCanvas::SPCanvas(SPen::IGLMsgQueue*, SPen::ISPBitmap*)'
        try:
            self.call(canvas_ctor, [canvas, self.queue, target_bitmap])
        except Exception as error:
            raise RuntimeError(f'canvas ctor crash pc={self.u.reg_read(R.UC_ARM64_REG_PC):#x} '
                               f'regs={[self.x(i) for i in range(9)]!r} '
                               f'target_bitmap={target_bitmap:#x} bitmap_words={target_bitmap_words!r} '
                               f'bitmap_snapshot={target_bitmap_snapshot!r} '
                               f'shape_events={self.shape_events[-80:]!r} '
                               f'queue_tasks={self.queue_tasks!r} '
                               f'managed_queue_events={self.managed_queue_events!r} '
                               f'gl_driver={self.gl_driver_events[-30:]!r} '
                               f'tail={[hex(x) for x in self.pc_ring[-24:]]!r}') from error
        object_drawing = self.alloc(512)
        self.call('SPen::ObjectDrawing::ObjectDrawing()', [object_drawing])
        self.call('SPen::ObjectDrawing::Construct(SPen::IContext const*)', [object_drawing, ctx])
        clip = self.alloc(16)
        self.put(clip, '4i', 0, 0, 128, 128)
        try:
            self.call('SPen::ObjectDrawing::drawObjectShape(SPen::ISPCanvas&, SPen::ObjectShape*, SPen::Rect const*)',
                      [object_drawing, canvas, shape_obj, clip], instruction_limit=10_000_000)
        except Exception as error:
            pc = self.u.reg_read(R.UC_ARM64_REG_PC)
            regs = [self.x(i) for i in range(29)]
            try:
                vtable = self.get(regs[0], 'Q')[0]
                vt_words = self.get(vtable, '4Q')
            except Exception:
                vtable, vt_words = None, None
            owner = next((r for r in self.alloc_records
                          if r['address'] <= regs[0] < r['address'] + r['size']), None)
            owner_snapshot = None
            if owner is not None:
                owner_snapshot = {'record': owner,
                                  'words': self.get(owner['address'], '32Q')}
            object_snaps = []
            for index in (19, 20, 21, 22):
                pointer = regs[index]
                if 0x10000000 <= pointer < 0x20000000:
                    try:
                        words = self.get(pointer, '20Q')
                    except Exception:
                        words = None
                    object_owner = next((r for r in self.alloc_records
                                         if r['address'] <= pointer < r['address'] + r['size']), None)
                    object_snaps.append({'reg': index, 'pointer': hex(pointer),
                                         'words': words, 'owner': object_owner})
            subbitmap_owner = next((r for r in self.alloc_records
                                    if r['address'] <= regs[21] < r['address'] + r['size']), None)
            subbitmap_snapshot = None
            if subbitmap_owner is not None:
                subbitmap_snapshot = {'record': subbitmap_owner,
                                      'words': self.get(subbitmap_owner['address'], '32Q')}
            raise RuntimeError(f'DrawPath failed at pc={pc:#x} regs={regs!r} '
                               f'target_bitmap={target_bitmap:#x} bitmap_words={target_bitmap_words!r} '
                               f'obj_vtable={vtable!r} vt_words={vt_words!r} owner={owner!r} '
                               f'owner_snapshot={owner_snapshot!r} '
                               f'subbitmap_snapshot={subbitmap_snapshot!r} '
                               f'object_snaps={object_snaps!r} '
                               f'native_entries={self.native_shape_entries!r} '
                               f'events_tail={self.shape_events[-120:]!r} '
                               f'gl_tail={self.gl_calls[-30:]!r} '
                               f'tail={[hex(x) for x in self.pc_ring]!r}') from error
        target_vector = self.get(target_bitmap + 216, '3Q')
        target_subbitmap = target_vector[0] if target_vector[0] else 0
        target_texture_event = next((event for event in self.gl_driver_events
                                     if event.get('call') == 'glFramebufferTexture2D'
                                     and event.get('args', [0, 0, 0, 0])[3] == 1), None)
        target_texture = int(target_texture_event['x22'], 16) if target_texture_event else 0
        target_texture_words = self.get(target_texture, '32Q') if target_texture else ()
        # Read the bitmap texture captured from the native SPSubBitmap through
        # an independent Mesa FBO after DrawPath has returned. This adds no
        # native writes and does not reuse the native renderer's FBO.
        texture_id = target_texture_event['x22_plus44'] if target_texture_event else 0
        post_draw_readback = (self.read_texture(texture_id, 128, 128)
                              if texture_id else None)
        generated_texture_ids = [event['names'][0] for event in self.gl_driver_events
                                 if event.get('call') == 'glGenTextures' and event.get('names')]
        intermediate_id = generated_texture_ids[1] if len(generated_texture_ids) > 1 else 0
        intermediate_readback = (self.read_texture(intermediate_id, 80, 80)
                                 if intermediate_id else None)
        readback = post_draw_readback['pixels'] if post_draw_readback else None
        return {'queue_tasks': self.queue_tasks, 'native_entries': self.native_shape_entries,
                'native_entry_calls': [event for event in self.shape_events if 'native_entry' in event],
                'shape_events': self.shape_events,
                'uploads': self.uploads, 'shape_object': hex(shape_obj),
                'managed_queue_events': self.managed_queue_events,
                'gl_driver_events': self.gl_driver_events,
                'gl_error_history': self.pending_gl_error_history,
                'pending_gl_errors': self.pending_gl_errors,
                'gl_checked_call_count': len(self.gl_api_call_history),
                'target_bitmap': hex(target_bitmap), 'target_vector': [hex(v) for v in target_vector],
                'target_subbitmap': hex(target_subbitmap), 'target_texture': hex(target_texture),
                'target_texture_words': [hex(v) for v in target_texture_words],
                'target_texture_id': texture_id,
                'pre_draw_readback': ({'fbo': pre_draw_readback['fbo'],
                                       'status': pre_draw_readback['status'],
                                       'errors': pre_draw_readback['errors'],
                                       'sha256': pre_draw_readback['sha256'],
                                       'pixels_hex': pre_draw_readback['pixels'].hex()}
                                      if pre_draw_readback else None),
                'readback_fbo': post_draw_readback['fbo'] if post_draw_readback else None,
                'readback_fbo_status': post_draw_readback['status'] if post_draw_readback else None,
                'readback_errors': post_draw_readback['errors'] if post_draw_readback else None,
                'readback_sha256': hashlib.sha256(readback).hexdigest() if readback else None,
                'readback_hex': readback.hex() if readback else None,
                'intermediate_texture_id': intermediate_id,
                'intermediate_readback': ({'fbo': intermediate_readback['fbo'],
                                           'status': intermediate_readback['status'],
                                           'errors': intermediate_readback['errors'],
                                           'sha256': intermediate_readback['sha256'],
                                           'pixels_hex': intermediate_readback['pixels'].hex()}
                                          if intermediate_readback else None),
                'pending_queue_returns': len(self.queue_returns)}

def summarize_case(name, line_color, line_width, result):
    if result['gl_error_history'] or result['pending_gl_errors']:
        raise RuntimeError(f'{name}: native GL error history {result["gl_error_history"]!r}')
    if result['readback_fbo_status'] != 0x8cd5 or any(result['readback_errors']):
        raise RuntimeError(f'{name}: incomplete or invalid final readback')
    pixels = bytes.fromhex(result['readback_hex'])
    if len(pixels) != 128 * 128 * 4:
        raise RuntimeError(f'{name}: unexpected final readback size {len(pixels)}')
    from collections import Counter
    counts = Counter(tuple(pixels[i:i + 4]) for i in range(0, len(pixels), 4))
    opaque = [i // 4 for i in range(0, len(pixels), 4) if pixels[i + 3]]
    bounds = ([min(i % 128 for i in opaque), min(i // 128 for i in opaque),
               max(i % 128 for i in opaque), max(i // 128 for i in opaque)]
              if opaque else None)
    evidence_keys = {
        'declared_shape_input', 'shape_construction_probe', 'object_shape_get_path_probe',
        'fill_effect_probe', 'draw_solid_rectangle_return', 'canvas_draw_rect_call',
        'draw_solid_rectangle_probe_return', 'drawpath_outer_canvas_step',
        'shape_path_draw_dispatch', 'native_shape_path_stage', 'native_skcanvas_draw_path',
        'draw_moving_path_return',
    }
    calls = [{key: event[key] for key in evidence_keys if key in event}
             for event in result['shape_events'] if evidence_keys.intersection(event)]
    return {
        'name': name,
        'inputs': {
            'path': [['M', 10, 20], ['L', 90, 20], ['L', 90, 100],
                     ['L', 10, 100], ['Close']],
            'shape_rect': [10, 20, 90, 100],
            'fill_rgba': [51, 102, 153, 255],
            'line_color_argb': line_color,
            'line_width': line_width,
            'output': [128, 128],
            'density': 1,
        },
        'route': 'native ObjectShapeDrawing; direct ObjectShape::Construct; '
                 'solid rectangle fastpath' if line_color == 0 else
                 'native ObjectShapeDrawing; direct ObjectShape::Construct; '
                 'ShapePath DrawEffect and pinned Skia software raster',
        'native_calls': calls,
        'queue_task_count': len(result['queue_tasks']),
        'native_observations': calls,
        'gl_call_counts': {name: sum(event.get('call') == name
                                     for event in result['gl_driver_events'])
                           for name in sorted({event.get('call') for event in result['gl_driver_events']})},
        'final': {
            'byte_count': len(pixels),
            'sha256': result['readback_sha256'],
            'rgba_counts': {','.join(map(str, color)): count
                            for color, count in sorted(counts.items())},
            'nontransparent_bbox_inclusive': bounds,
            'fbo_status': result['readback_fbo_status'],
            'gl_errors': result['readback_errors'],
            'checked_native_gl_calls': result['gl_checked_call_count'],
            'native_gl_error_history': result['gl_error_history'],
            'post_draw_gl_errors': [event.get('error') for event in result['gl_driver_events']
                                    if event.get('call') == 'glGetError'],
        },
    }


def build_fixture():
    cases = []
    case_inputs = [('solid_fill_no_outline', 0, 0.0),
                   ('outlined_software_path_width_1', 0xff112233, 1.0),
                   ('outlined_software_path_width_2', 0xff112233, 2.0),
                   ('outlined_software_path_width_4', 0xff112233, 4.0)]
    for name, line_color, line_width in case_inputs:
        env = dict(os.environ)
        env['SHAPE_INIT_MODE'] = 'direct'
        env['SHAPE_LINE_COLOR'] = hex(line_color)
        env['SHAPE_LINE_WIDTH'] = str(line_width)
        run = subprocess.run([sys.executable, str(Path(__file__).resolve()),
                              '--native-case'], env=env, check=True,
                             capture_output=True, text=True)
        cases.append(summarize_case(name, line_color, line_width, json.loads(run.stdout)))
    return {
        'schema': 'sdocx.native-object-shape-paint.v1',
        'source': {
            'application': 'Samsung Notes 4.4.45.37',
            'apk_sha256': APK_SHA256,
            'libraries': SOURCE_HASHES,
            'host_libm': {'path': str(HOST_LIBM), 'sha256': HOST_LIBM_SHA256},
        },
        'scope': {
            'native': 'Pinned arm64 Samsung Notes Model/Drawing/Skia shape path and paint code',
            'host_boundaries': ['Mesa GLES forwarding', 'synthetic EGL current context',
                                'synchronous queue task dispatch', 'pinned host libm',
                                'tracked realloc', 'GL flush/invalidate'],
            'claim': 'Executed native output under these host boundaries; not Android-device parity',
        },
        'cases': cases,
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    mode.add_argument('--native-case', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--fixture', type=Path,
                        default=Path(__file__).with_name('shape-solid-paint-native.json'))
    args = parser.parse_args()
    if args.native_case:
        print(json.dumps(ShapeOracle().run_shape(), sort_keys=True))
    else:
        actual = build_fixture()
        rendered = json.dumps(actual, indent=2, sort_keys=True) + '\n'
        if args.write_fixture:
            args.fixture.write_text(rendered)
        else:
            expected = args.fixture.read_text()
            if expected != rendered:
                raise SystemExit(f'{args.fixture} differs; run --write-fixture to refresh')
        print(f'{args.fixture}: {len(actual["cases"])} native cases verified')
