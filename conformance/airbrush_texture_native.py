#!/usr/bin/env python3
"""Execute AirBrush V2 texture construction through the pinned GLES Renderer.

This records native GL calls under Unicorn. GL names, limits, and callbacks are
synthetic host inputs; this is not a device-driver or rendered-pixel oracle.
"""
import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
from pathlib import Path

import airbrush_saved_native as saved
from unicorn import arm64_const as R

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJ = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
NM = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-nm'
EXPECTED = {
    'FountainPen': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
    'AirBrushPen': '92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423',
    'Graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
    'Renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
}
CREATE_BITMAP = ('SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, '
                 'char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, '
                 'SPen::TextureTargetLayout)')
CREATE_TEXTURE = ('SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, '
                   'char const*, void const*, unsigned int const*, unsigned int, '
                   'SPen::TextureBufferInfo const*)')
CREATE_TEXTURE_IMPL = ('SPen::GLES::RendererFactoryImpl::CreateTextureObject('
                       'SPen::TextureDescriptor const&, char const*, void const*, '
                       'unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)')
SAVED_FIXTURE = ROOT / 'conformance/airbrush-saved-native.json'
OUT = ROOT / 'conformance/airbrush-texture-native.json'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class TextureOracle(saved.Oracle):
    def __init__(self):
        self.extra_bases = {}
        self.native_entries = []
        self.factory_records = []
        self.gl = []
        self.pending_returns = []
        self.next_gl_id = 100
        self.gl_names = {}
        self.emutls = {}
        for name, expected in EXPECTED.items():
            actual = digest(LIB / f'libSPen{name}.so')
            if actual != expected:
                raise ValueError(f'unpinned native DSO {name}: {actual}')
        super().__init__()
        self.return_tramp = self.alloc(8)
        self.u.mem_write(self.return_tramp, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.map_extra('Graphics', 4)
        self.map_extra('Renderer', 5)
        self.load_extra_relocs()
        renderer = 0x5000000
        # RendererFactoryImpl reads these pinned DSO globals for synthetic
        # device limits/capability state.
        self.put(renderer + 0x73388, 'II', 4096, 4096)
        self.u.mem_write(renderer + 0x733c0, b'\x01')
        self.callbacks.pop(CREATE_BITMAP, None)
        self.callbacks.pop(CREATE_TEXTURE, None)
        self.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()'] = self.device_caps
        self.callbacks.update({
            'glGetString': self.get_string,
            'glGetIntegerv': self.get_integer,
            'glGetError': lambda: self.x(0, 0),
            'glGenTextures': lambda: self.gen_names('texture'),
            'glGenSamplers': lambda: self.gen_names('sampler'),
            'glBindTexture': lambda: self.record('glBindTexture', target=self.x(0), object=self.x(1)),
            'glSamplerParameteri': lambda: self.record('glSamplerParameteri', sampler=self.gl_names.get(self.x(0), 'unknown'),
                                                       pname=self.x(1), value=self.x(2)),
            'glTexParameteri': lambda: self.record('glTexParameteri', target=self.x(0),
                                                  pname=self.x(1), value=self.x(2)),
            'glPixelStorei': lambda: self.record('glPixelStorei', pname=self.x(0), value=self.x(1)),
            'glTexImage2D': self.tex_image,
            'glTexSubImage2D': self.tex_sub_image,
            '__strncpy_chk': self.strncpy_checked,
            'strstr': self.strstr,
            'strncpy': self.strncpy,
            '__emutls_get_address': self.emutls_get_address,
            'clock_gettime': self.clock_gettime,
        })
        self.callbacks['SPen::PenGLDataManager::GetMsgQueue()'] = lambda: self.x(0, self.queue)
        self.callbacks['SPen::RendererFactory::CreateGraphicsObject(unsigned int, SPen::PrimitiveType, SPen::VertexDescriptor const*, unsigned int, unsigned int)'] = self.create_graphics
        self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'] = self.create_blend

    def create_graphics(self):
        self.x(0, self.alloc(64))

    def create_blend(self):
        self.x(0, self.alloc(32))

    def map_extra(self, name, index):
        path = LIB / f'libSPen{name}.so'
        blob = path.read_bytes()
        base = index * 0x1000000
        self.extra_bases[name] = base
        self.u.mem_map(base, 0x200000)
        phoff = struct.unpack_from('<Q', blob, 32)[0]
        entsize, count = struct.unpack_from('<HH', blob, 54)
        for i in range(count):
            ph = struct.unpack_from('<II6Q', blob, phoff + i * entsize)
            if ph[0] == 1 and ph[5]:
                self.u.mem_write(base + ph[3], blob[ph[2]:ph[2] + ph[5]])
        env = {**os.environ, 'LD_LIBRARY_PATH': str(ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu')}
        nm = subprocess.run([str(NM), '-D', '-C', str(path)], capture_output=True,
                            text=True, check=True, env=env)
        for line in nm.stdout.splitlines():
            parts = line.strip().split(' ', 2)
            if len(parts) == 3 and parts[1] in ('T', 'W', 'V', 'D', 'B', 'R'):
                self.syms[parts[2]] = base + int(parts[0], 16)
        dis = subprocess.run([str(OBJ), '-d', '-C', '-j', '.plt', str(path)],
                             capture_output=True, text=True, check=True, env=env)
        for address, symbol in re.findall(r'^([0-9a-f]+) <(.+)@plt>:', dis.stdout, re.M):
            self.plt[base + int(address, 16)] = symbol

    def load_extra_relocs(self):
        for name, base in self.extra_bases.items():
            blob = (LIB / f'libSPen{name}.so').read_bytes()
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
                elif kind == 0x101:
                    value = self.resolve_symbol(syms[index][0]) + addend
                elif kind == 0x401:
                    value = self.resolve_symbol(syms[index][0])
                else:
                    continue
                self.put(base + where, 'Q', value)

    def record(self, name, **values):
        self.gl.append({'call': name, **values})

    def device_caps(self):
        ptr, vtable, function = self.alloc(16), self.alloc(32), self.alloc(8)
        self.put(ptr, 'Q', vtable)
        self.u.mem_write(function, struct.pack('<II', 0x52820000, 0xd65f03c0))
        self.put(vtable + 16, 'Q', function)
        self.x(0, ptr)

    def gen_names(self, kind):
        count, pointer = self.x(0), self.x(1)
        if not 0 < count <= 8:
            raise RuntimeError(f'bounded glGen{kind}: {count}')
        ids = list(range(self.next_gl_id, self.next_gl_id + count))
        self.next_gl_id += count
        for i, object_id in enumerate(ids):
            self.gl_names[object_id] = f'airbrush-pattern-{kind}-{i}'
        self.put(pointer, 'I' * count, *ids)
        self.record('glGen' + kind.title() + 's', count=count, ids=ids)

    def get_string(self):
        value = self.alloc(64)
        self.u.mem_write(value, b'OpenGL ES 3.2\0')
        self.x(0, value)

    def get_integer(self):
        enum, pointer = self.x(0), self.x(1)
        value = {0x8869: 16, 0x8b4d: 16, 0x8a2f: 256}.get(enum, 0)
        self.put(pointer, 'I', value)

    def tex_image(self):
        self.record('glTexImage2D', args=[self.x(i) for i in range(8)])

    def tex_sub_image(self):
        sp = self.u.reg_read(R.UC_ARM64_REG_SP)
        pixels = self.get(sp, 'Q')[0]
        target, level, xoff, yoff = [self.x(i) for i in range(4)]
        width, height, fmt, pixel_type = [self.x(i) for i in range(4, 8)]
        if (target, width, height, fmt, pixel_type) != (0x0de1, 96, 96, 0x1908, 0x1401):
            raise RuntimeError(f'unexpected AirBrush upload tuple: {(target,width,height,fmt,pixel_type)}')
        raw = bytes(self.u.mem_read(pixels, width * height * 4))
        self.upload = {'dimensions': [width, height], 'target': target, 'level': level,
                       'offset': [xoff, yoff], 'format': fmt, 'type': pixel_type,
                       'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                       'sample_red_bytes': [raw[(48 * 96 + x) * 4] for x in range(44, 52)],
                       'other_channels_zero': not any(raw[i] for i in range(1, len(raw), 4)) and
                                              not any(raw[i] for i in range(2, len(raw), 4)) and
                                              not any(raw[i] for i in range(3, len(raw), 4))}
        self.record('glTexSubImage2D', **self.upload)

    def strncpy_checked(self):
        length, dest_size = self.x(2), self.x(3)
        if length > 4096 or length > dest_size:
            raise RuntimeError(f'__strncpy_chk bounds: {length}/{dest_size}')
        source = bytes(self.u.mem_read(self.x(1), length)).split(b'\0', 1)[0]
        self.u.mem_write(self.x(0), source + b'\0' * max(0, length - len(source)))
        self.x(0, self.x(0))

    def strstr(self):
        haystack, needle = self.cstr(self.x(0)), self.cstr(self.x(1))
        index = haystack.find(needle)
        self.x(0, self.x(0) + index if index >= 0 else 0)

    def strncpy(self):
        length = self.x(2)
        if length > 4096:
            raise RuntimeError(f'strncpy length exceeds bound: {length}')
        source = bytes(self.u.mem_read(self.x(1), length)).split(b'\0', 1)[0]
        self.u.mem_write(self.x(0), source + b'\0' * max(0, length - len(source)))
        self.x(0, self.x(0))

    def cstr(self, pointer):
        raw = bytearray()
        for offset in range(4096):
            byte = bytes(self.u.mem_read(pointer + offset, 1))
            if byte == b'\0':
                return raw.decode(errors='replace')
            raw.extend(byte)
        raise RuntimeError('native string exceeds 4096-byte bound')

    def emutls_get_address(self):
        descriptor = self.x(0)
        if descriptor not in self.emutls:
            size, alignment = self.get(descriptor, 'QQ')
            if not 0 < size <= (1 << 20) or not 0 < alignment <= 4096 or alignment & (alignment - 1):
                raise RuntimeError(f'invalid emutls descriptor: size={size}, alignment={alignment}')
            allocation = self.alloc(size + alignment)
            address = (allocation + alignment - 1) & -alignment
            template = self.get(descriptor + 16, 'Q')[0]
            if template:
                self.u.mem_write(address, bytes(self.u.mem_read(template, size)))
            self.emutls[descriptor] = address
        self.x(0, self.emutls[descriptor])

    def clock_gettime(self):
        self.put(self.x(1), 'qq', 0, 0)
        self.x(0, 0)

    def hook(self, u, address, size, data):
        if address in self.plt and self.plt[address] == CREATE_TEXTURE:
            desc, label, source, pairs, count, info = [self.x(i) for i in range(6)]
            self.factory_records.append({'descriptor': list(self.get(desc, '8I')),
                                         'name': self.cstr(label), 'source_is_null': source == 0,
                                         'pairs': list(self.get(pairs, 'I' * (count * 2))),
                                         'pair_count': count,
                                         'buffer_info': list(self.get(info, '4I'))})
            self.pending_returns.append(u.reg_read(R.UC_ARM64_REG_LR))
            u.reg_write(R.UC_ARM64_REG_LR, self.return_tramp)
            u.reg_write(R.UC_ARM64_REG_PC, self.syms[CREATE_TEXTURE_IMPL])
            return
        if address in self.plt and self.plt[address] == CREATE_BITMAP:
            self.pending_returns.append(u.reg_read(R.UC_ARM64_REG_LR))
            u.reg_write(R.UC_ARM64_REG_LR, self.return_tramp)
            u.reg_write(R.UC_ARM64_REG_PC, self.syms['SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'])
            return
        if address == self.return_tramp:
            if not self.pending_returns:
                raise RuntimeError('unexpected native factory return')
            u.reg_write(R.UC_ARM64_REG_PC, self.pending_returns.pop())
            return
        for symbol in ('SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)',
                       'SPen::AirBrushPenStrokeDrawableRTV1::Init()',
                       'SPen::AirBrushPenStrokeDrawableRTV1::buildTextures(int, int)',
                       'SPen::AirBrushPenStrokeDrawableRTV1::setTextures(unsigned int, unsigned int)',
                       'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
                       'SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
                       CREATE_TEXTURE_IMPL,
                       'SPen::GLES::TextureObject::CreateTexture(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)',
                       'SPen::GLES::TextureObject::createTextureInternalsSampler(SPen::TextureDescriptor const&, char const*, void const*, std::__ndk1::map<int, int, std::__ndk1::less<int>, std::__ndk1::allocator<std::__ndk1::pair<int const, int> > > const&, SPen::TextureBufferInfo const*)',
                       'SPen::GLES::TextureObject::SetParam(int, int)',
                       'SPen::SPTextureBitmap::SetTextureFilter(SPen::TextureParamFiltering, SPen::TextureParamFilteringValue)',
                       'SPen::SPTextureBitmap::UpdateBitmap(void const*, int)',
                       'non-virtual thunk to SPen::SPSubBitmapRT::UpdateBitmap(void const*, int)'):
            if address == self.syms.get(symbol):
                self.native_entries.append(symbol)
        super().hook(u, address, size, data)

    def run(self):
        # GLV2 constructor is the shipped default. Host resources are explicit;
        # Init's graphics allocation stays synthetic, texture allocation does not.
        trampoline = struct.pack('<II', 0xd503201f, 0xd65f03c0)
        if bytes(self.u.mem_read(self.return_tramp, len(trampoline))) != trampoline:
            raise RuntimeError('return trampoline was changed before native initialization')
        allocation_start = self.heap
        obj, data, settings, manager, canvas = [self.alloc(n) for n in (512, 64, 64, 64, 64)]
        if self.return_tramp + 8 > allocation_start or obj <= self.return_tramp < obj + 512:
            raise RuntimeError('native object allocation overlaps the reserved return trampoline')
        queue, queue_vtable = self.alloc(32), self.alloc(64)
        self.queue_post, self.queue_task_return = self.alloc(16), self.alloc(16)
        self.put(queue, 'Q', queue_vtable)
        self.put(queue_vtable + 16, 'Q', self.queue_post)
        self.queue = queue
        rt_constructor = 'SPen::AirBrushPenStrokeDrawableGLV2::AirBrushPenStrokeDrawableGLV2(SPen::AirBrushPenData*, SPen::PenGLDataManager*)'
        self.call(rt_constructor, [obj, data, manager])
        rt = self.get(obj + 16, 'Q')[0]
        self.put(obj + 24, 'Q', manager)
        self.put(obj + 32, 'Q', canvas)
        self.put(obj + 72, 'Q', data)
        self.put(data, 'Q', settings)
        self.put(settings, 'fI', 14.0, 0xff336699)
        self.put(rt + 136, 'Q', manager)
        self.put(rt + 200, 'Q', 0)
        self.put(rt + 208, 'Q', self.alloc(32))  # skips unrelated shader init branch
        self.put(rt + 216, 'Q', 0)
        self.call('SPen::AirBrushPenStrokeDrawableRTV1::Init()', [rt], instruction_limit=5_000_000)
        if bytes(self.u.mem_read(self.return_tramp, len(trampoline))) != trampoline:
            raise RuntimeError('native initialization overwrote the return trampoline')
        if len(self.factory_records) != 1 or len(self.pending_returns):
            raise RuntimeError('native texture factory count/return stack mismatch')
        entries = self.native_entries
        required = {
            rt_constructor: 1,
            'SPen::AirBrushPenStrokeDrawableRTV1::Init()': 1,
            'SPen::AirBrushPenStrokeDrawableRTV1::buildTextures(int, int)': 1,
            'SPen::AirBrushPenStrokeDrawableRTV1::setTextures(unsigned int, unsigned int)': 1,
            'SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)': 1,
            'SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)': 1,
            CREATE_TEXTURE_IMPL: 1,
            'SPen::GLES::TextureObject::CreateTexture(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)': 1,
            'SPen::GLES::TextureObject::createTextureInternalsSampler(SPen::TextureDescriptor const&, char const*, void const*, std::__ndk1::map<int, int, std::__ndk1::less<int>, std::__ndk1::allocator<std::__ndk1::pair<int const, int> > > const&, SPen::TextureBufferInfo const*)': 1,
            'SPen::GLES::TextureObject::SetParam(int, int)': 2,
            'SPen::SPTextureBitmap::SetTextureFilter(SPen::TextureParamFiltering, SPen::TextureParamFilteringValue)': 2,
            'SPen::SPTextureBitmap::UpdateBitmap(void const*, int)': 1,
            'non-virtual thunk to SPen::SPSubBitmapRT::UpdateBitmap(void const*, int)': 1,
        }
        counts = {name: entries.count(name) for name in required}
        if counts != required:
            raise RuntimeError(f'native texture phase counts differ: {counts}')
        record = self.factory_records[0]
        if record['descriptor'] != [96, 96, 0, 2, 6, 1, 0, 0] or record['pairs'] != [4, 1, 5, 1]:
            raise RuntimeError(f'AirBrush texture descriptor changed: {record}')
        expected_upload = json.loads(SAVED_FIXTURE.read_text())['texture']
        if self.upload['sha256'] != expected_upload['payload_sha256']:
            raise RuntimeError('native GL upload bytes differ from saved native CPU pattern witness')
        return self.result(rt_constructor, counts, record, expected_upload)

    def result(self, constructor, counts, factory, cpu_pattern):
        gl_calls = self.gl
        sampler = [x for x in gl_calls if x['call'] == 'glSamplerParameteri']
        expected_sampler = [
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2802,'value':0x812f},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2803,'value':0x812f},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2800,'value':0x2601},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2801,'value':0x2601},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x813a,'value':0},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x813b,'value':0},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2800,'value':0x2600},
            {'call':'glSamplerParameteri','sampler':'airbrush-pattern-sampler-0','pname':0x2801,'value':0x2600},
        ]
        if sampler != expected_sampler:
            raise RuntimeError(f'native sampler sequence changed: {sampler}')
        return {
            'schema': 'sdocx.airbrush-texture-native.v1',
            'source': {'application': 'Samsung Notes 4.4.45.37', 'native_libraries_sha256': EXPECTED},
            'runtime': {'default_profile_constructor': constructor, 'default_profile_version': 2,
                        'synthetic_input': {'data_size':14.0,'data_color_argb':'0xff336699',
                                            'manager':'synthetic','canvas':'synthetic',
                                            'render_queue':'synthetic synchronous task runner'},
                        'texture_factory_entry_count': 1, 'native_entry_counts': counts},
            'texture': {'factory': factory, 'cpu_pattern_sha256': cpu_pattern['payload_sha256'],
                        'shader_sha256': {k: v['sha256'] for k, v in
                            json.loads(SAVED_FIXTURE.read_text())['texture']['native_shader_resources'].items()},
                        'allocation': next(x['args'] for x in gl_calls if x['call']=='glTexImage2D'),
                        'sampler_calls': sampler,
                        'final_sampler_contract': {'wrap_s':'CLAMP_TO_EDGE','wrap_t':'CLAMP_TO_EDGE',
                                                   'min_filter':'NEAREST','mag_filter':'NEAREST',
                                                   'min_lod':0,'max_lod':0},
                        'upload': self.upload},
            'host_boundary': {'GLES callbacks': 'GL object IDs, device caps, and every GL function are explicit synthetic host callbacks; no driver, context, GPU allocation, shader execution, framebuffer, or rendered pixels.',
                              'input profile data': 'native GLV2 constructor runs with synthetic AirBrush data color/size and queue/manager/canvas state; graphics object creation remains a synthetic renderer boundary.',
                              'bitmap construction': 'SPGraphicsFactory::CreateBitmap, SPTextureBitmap constructor, RendererFactoryImpl::CreateTextureObject, Renderer TextureObject::CreateTexture/sampler constructor, and bitmap Update execute natively from hash-pinned DSOs.'},
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--write-fixture', action='store_true')
    group.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    output = TextureOracle().run()
    encoded = json.dumps(output, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
    elif not OUT.exists() or OUT.read_text() != encoded:
        raise SystemExit('AirBrush texture fixture mismatch')
    else:
        print('AirBrush V2 native texture allocation, sampler, and upload match fixture')


if __name__ == '__main__':
    main()
