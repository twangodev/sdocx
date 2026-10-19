"""Hash-pinned BrushPen RTV1 texture upload and Draw-binding ABI oracle.

This executes the native GLV10 constructor, RTV1::Init texture branch,
Renderer::TextureObject::Update, and RTV1::Draw math. Renderer objects and
shader binding dispatch remain explicit host boundaries; intercepted GL calls
establish upload arguments, not rendered pixels or device heap contents.
"""
import argparse
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
sys.path.insert(0, str(ROOT / 'conformance'))
import brushpen_saved_native as brushpen_geometry
import fountain_native
from brushpen_saved_native import NativeBrush
from unicorn import arm64_const as R

OUT = ROOT / 'conformance/brushpen-texture-native.json'
BRUSH_BASE = 3 * 0x1000000
RENDERER_BASE = 5 * 0x1000000
GRAIN_SOURCE_CAPTURE_PC = BRUSH_BASE + 0x767e8
CREATE_BITMAP = ('SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, '
                 'char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, '
                 'SPen::TextureTargetLayout)')


def native_hashes():
    names = ('FountainPen', 'PenCommon', 'Base', 'BrushPen', 'Graphics', 'Renderer', 'Model')
    expected = {**fountain_native.HASHES, **brushpen_geometry.HASHES}
    result = {name: hashlib.sha256((LIB / f'libSPen{name}.so').read_bytes()).hexdigest()
              for name in names}
    for name in names:
        if result[name] != expected[name]:
            raise ValueError(f'unverified native library {name}: {result[name]}')
    return result


class TextureProbe(NativeBrush):
    def __init__(self):
        super().__init__()
        self.import_calls = {}
        self.active_import = '<none>'
        self.array_allocations = {}
        self.bitmap_factory_calls = []
        self.texture_descriptors = []
        self.texture_state_calls = []
        self.native_entries = {}
        self.gl_calls = []
        self.upload_records = []
        self.grain_source = None
        self.draw_stubs = {}
        self.draw_capture = {'bindings': {}, 'events': []}
        self.blend_descriptor = None
        self.state_stub = 0x20001160
        self.state_default_stub = 0x20001140
        self.device_caps = self.alloc(16)
        caps_vtable = self.alloc(32)
        self.put(self.device_caps, 'Q', caps_vtable)
        self.put(caps_vtable + 16, 'Q', 0x20001200)
        self.u.mem_write(0x20001200, struct.pack('<II', 0x52820000, 0xd65f03c0))
        for address in (self.state_stub, self.state_default_stub):
            self.u.mem_write(address, struct.pack('<II', 0xaa0003e0, 0xd65f03c0))

        self.texture_vtable = self.alloc(128)
        for offset in range(0, 128, 8):
            self.put(self.texture_vtable + offset, 'Q', self.state_default_stub)
        self.put(self.texture_vtable + 32, 'Q', self.syms['SPen::GLES::TextureObject::GetWidth() const'])
        self.put(self.texture_vtable + 40, 'Q', self.syms['SPen::GLES::TextureObject::GetHeight() const'])
        self.put(self.texture_vtable + 64, 'Q', self.syms['SPen::GLES::TextureObject::Update(void const*, int)'])
        self.put(self.texture_vtable + 112, 'Q', self.state_stub)

        self.callbacks.pop(CREATE_BITMAP, None)
        self.callbacks['SPen::RendererFactory::GetRendererDeviceCaps()'] = self.get_device_caps
        self.callbacks['SPen::RendererFactory::CreateTextureObject(SPen::TextureDescriptor const&, char const*, void const*, unsigned int const*, unsigned int, SPen::TextureBufferInfo const*)'] = self.create_texture_object
        self.callbacks['strncpy'] = self.strncpy
        self.callbacks['operator new[](unsigned long)'] = self.alloc_array
        self.callbacks['operator delete[]'] = lambda: None
        self.callbacks['clock_gettime'] = self.clock_gettime
        self.callbacks['glBindTexture'] = self.gl_bind_texture
        self.callbacks['glPixelStorei'] = self.gl_pixel_store
        self.callbacks['glTexSubImage2D'] = self.gl_tex_sub_image_2d
        self.callbacks['SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'] = self.create_blend_state

    def create_blend_state(self):
        descriptor = self.x(0)
        self.blend_descriptor = bytes(self.u.mem_read(descriptor, 32)).hex()
        obj = self.alloc(16)
        vtable = self.alloc(64)
        self.put(obj, 'Q', vtable)
        self.put(vtable + 24, 'Q', self.new_draw_stub('blend_activate'))
        self.x(0, obj)

    def new_draw_stub(self, kind, **fields):
        address = self.alloc(8)
        self.u.mem_write(address, struct.pack('<II', 0xd503201f, 0xd65f03c0))
        self.draw_stubs[address] = {'kind': kind, **fields}
        return address

    def draw_bitmap_callback(self, kind):
        if kind == 'bitmap_matrix':
            self.u.mem_write(self.x(8), bytes.fromhex(self.draw_capture['bitmap_matrix_bytes_hex']))
            self.x(0, self.x(8))
            self.draw_capture['events'].append({'call': kind, 'x8_output': True})
        elif kind in ('bitmap_width', 'bitmap_height'):
            self.x(0, self.draw_capture['bitmap_size'][0 if kind.endswith('width') else 1])
            self.draw_capture['events'].append({'call': kind, 'result': self.x(0)})
        elif kind == 'bitmap_start_probe':
            self.x(0, 0)
            self.draw_capture['events'].append({'call': kind, 'arg1': self.x(1)})
        else:
            self.x(0, 0)
            self.draw_capture['events'].append({'call': kind, 'x1_x2': [self.x(1), self.x(2)]})

    def dispatch_draw_stub(self, address):
        stub = self.draw_stubs[address]
        kind = stub['kind']
        if kind == 'binding_set':
            name = stub['name']
            length = stub['bytes']
            raw = bytes(self.u.mem_read(self.x(1), length))
            entry = {
                'binding_offset': stub['offset'], 'bytes': raw.hex(),
                'value_type': stub['value_type'],
            }
            if stub['value_type'] == 'f32':
                entry['f32'] = list(struct.unpack('<' + 'f' * (length // 4), raw))
            else:
                entry['u32'] = list(struct.unpack('<' + 'I' * (length // 4), raw))
            self.draw_capture['bindings'][name] = entry
            self.draw_capture['events'].append({'call': 'binding_set', 'name': name,
                                                 'byte_count': length})
        elif kind == 'texture_activate':
            self.draw_capture['events'].append({'call': kind, 'texture_object': hex(self.x(0)),
                                                 'unit_args': [self.x(1), self.x(2)]})
        elif kind == 'program_activate':
            self.draw_capture['events'].append({'call': kind})
        elif kind == 'blend_activate':
            self.draw_capture['events'].append({'call': kind})
        elif kind == 'graphics_update':
            sp = self.u.reg_read(R.UC_ARM64_REG_SP)
            self.draw_capture['events'].append({'call': kind,
                                                 'x1_x4': [self.x(i) for i in range(1, 5)],
                                                 'stack32_hex': bytes(self.u.mem_read(sp, 32)).hex()})
        elif kind == 'graphics_draw':
            sp = self.u.reg_read(R.UC_ARM64_REG_SP)
            self.draw_capture['events'].append({'call': kind, 'x1_x7': [self.x(i) for i in range(1, 8)],
                                                 'stack32_hex': bytes(self.u.mem_read(sp, 32)).hex()})
        elif kind.startswith('bitmap_'):
            self.draw_bitmap_callback(kind)
        else:
            raise RuntimeError(f'unknown native Draw stub {kind}')
        self.u.reg_write(R.UC_ARM64_REG_PC, self.u.reg_read(R.UC_ARM64_REG_LR))

    def capture_native_draw(self, rt):
        # The lower renderer/GLES interfaces are explicit host boundaries. The
        # native RTV1::Draw method and its parameter-binding calls remain real.
        shader = self.alloc(64)
        binding_sizes = {
            'ProjectionMatrix': (64, 0, 'f32'), 'sampler0': (4, 8, 'u32'),
            'sampler1': (4, 16, 'u32'), 'inputColor': (16, 24, 'f32'),
            'grain': (8, 32, 'f32'), 'grainOffset': (8, 40, 'f32'),
            'numTiles': (4, 48, 'f32'),
        }
        for name, (size, offset, value_type) in binding_sizes.items():
            binding = self.alloc(16)
            table = self.alloc(8)
            self.put(binding, 'Q', table)
            self.put(table, 'Q', self.new_draw_stub('binding_set', name=name, bytes=size,
                                                    offset=offset, value_type=value_type))
            self.put(shader + offset, 'Q', binding)
        program, program_table = self.alloc(16), self.alloc(24)
        self.put(program, 'Q', program_table)
        self.put(program_table + 16, 'Q', self.new_draw_stub('program_activate'))
        self.put(shader + 56, 'Q', program)
        self.put(rt + 216, 'Q', shader)

        graphics, graphics_table = self.alloc(16), self.alloc(64)
        self.put(graphics, 'Q', graphics_table)
        self.put(graphics_table + 16, 'Q', self.new_draw_stub('graphics_update'))
        self.put(graphics_table + 56, 'Q', self.new_draw_stub('graphics_draw'))
        self.put(rt + 200, 'Q', graphics)
        blend = self.get(rt + 208, 'Q')[0]
        if not blend:
            raise RuntimeError('native Init did not create a blend state object')
        blend_table = self.get(blend, 'Q')[0]
        self.put(blend_table + 24, 'Q', self.new_draw_stub('blend_activate'))

        texture_table = self.texture_vtable
        self.put(texture_table + 16, 'Q', self.new_draw_stub('texture_activate'))

        bitmap, bitmap_table = self.alloc(16), self.alloc(160)
        self.put(bitmap, 'Q', bitmap_table)
        self.draw_capture['bitmap_size'] = [321, 228]
        bitmap_matrix = [1.1, 0.2, 0.0, 0.001, -0.1, 0.9, 0.0, -0.002,
                         0.0, 0.0, 1.0, 0.0, 2.0, 3.0, 0.0, 1.0]
        self.draw_capture['bitmap_matrix_bytes_hex'] = struct.pack('<16f', *bitmap_matrix).hex()
        for offset, kind in ((16, 'bitmap_matrix'), (40, 'bitmap_start_probe'),
                             (48, 'bitmap_finish'), (96, 'bitmap_width'),
                             (104, 'bitmap_height')):
            self.put(bitmap_table + offset, 'Q', self.new_draw_stub(kind))

        # One exact native 11-word instance is enough to reach the real Draw
        # method. The native RT list itself is supplied as an explicit fixture
        # input; downstream VBO submission is intercepted.
        geometry_fixture = json.loads((ROOT / 'conformance/brushpen-saved-native.json').read_text())
        case = next(c for c in geometry_fixture['synthetic_cases']
                    if c['label'] == 'synthetic-motion-tool1-pressure-tilt-orientation')
        record = case['buffer_records'][1]['instance_bits_le']
        vector, data = self.alloc(24), self.alloc(44)
        self.u.mem_write(data, b''.join(bytes.fromhex(word) for word in record))
        self.put(vector, 'QQQ', data, data + 44, data + 44)
        self.put(rt + 88, 'Q', vector)
        self.put(rt + 44, '4f', 0.25, 0.5, 0.75, 0.8)
        self.call('SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)', [rt], [17.25, 22.5])

        caller_matrix = [0.8, 0.15, 0.0, 0.0005, -0.12, 1.2, 0.0, -0.0003,
                         0.0, 0.0, 1.0, 0.0, 5.0, -4.0, 0.0, 1.0]
        matrix_address = self.alloc(64)
        self.put(matrix_address, '16f', *caller_matrix)
        self.draw_capture.update({'caller_matrix_f32': caller_matrix,
                                  'bitmap_matrix_f32': bitmap_matrix,
                                  'set_start_point_f32': [17.25, 22.5],
                                  'input_color_f32': [0.25, 0.5, 0.75, 0.8],
                                  'input_instance_bits_le': record,
                                  'host_boundary': 'synthetic ISPSubBitmap matrix/dimensions, shader bindings/program, GraphicsObject, blend-state factory/activation; actual RTV1::Draw and SetStartPoint execute, binding dispatches are intercepted'})
        self.call('SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
                  [rt, bitmap, matrix_address], instruction_limit=2_000_000)
        return self.draw_capture

    @staticmethod
    def matrix_product_col_major(left, right):
        return [sum(left[k * 4 + row] * right[col * 4 + k] for k in range(4))
                for col in range(4) for row in range(4)]

    @staticmethod
    def transform_col_major(matrix, point):
        return [sum(matrix[col * 4 + row] * point[col] for col in range(4))
                for row in range(4)]

    @classmethod
    def expected_draw_uniforms(cls, capture):
        bitmap = capture['bitmap_matrix_f32']
        caller = capture['caller_matrix_f32']
        projection = cls.matrix_product_col_major(bitmap, caller)
        px, py = capture['set_start_point_f32']
        point = cls.transform_col_major(projection, [px, py, 0.0, 1.0])
        offset = [(point[i] / point[3] + 1.0) * capture['bitmap_size'][i] / 2.0
                  for i in range(2)]
        corners = []
        for xy in ((321.0, 228.0), (0.0, 0.0)):
            transformed = cls.transform_col_major(caller, [xy[0], xy[1], 0.0, 1.0])
            corners.append([transformed[i] / transformed[3] for i in range(2)])
        grain = [corners[0][i] - corners[1][i] for i in range(2)]
        return projection, grain, offset

    def alloc_array(self):
        size = self.x(0)
        if not 0 < size <= 0x1000000:
            raise RuntimeError(f'native upload allocation outside bound: {size:#x}')
        pointer = self.alloc(size)
        # Diagnostic poison lets the oracle distinguish native writes from
        # untouched allocation bytes; it does not model device heap defaults.
        self.u.mem_write(pointer, b'\xcd' * size)
        self.array_allocations[pointer] = size
        self.x(0, pointer)

    def get_device_caps(self):
        self.x(0, self.device_caps)

    def clock_gettime(self):
        self.put(self.x(1), 'qq', 0, 0)
        self.x(0, 0)

    def strncpy(self):
        length = self.x(2)
        if length > 4096:
            raise RuntimeError(f'strncpy host shim bound exceeded: {length}')
        source = bytes(self.u.mem_read(self.x(1), length)).split(b'\0', 1)[0]
        self.u.mem_write(self.x(0), source + b'\0' * max(0, length - len(source)))
        self.x(0, self.x(0))

    def gl_bind_texture(self):
        self.gl_calls.append({'call': 'glBindTexture', 'target': self.x(0), 'texture': self.x(1)})

    def gl_pixel_store(self):
        self.gl_calls.append({'call': 'glPixelStorei', 'pname': self.x(0), 'value': self.x(1)})

    def gl_tex_sub_image_2d(self):
        sp = self.u.reg_read(R.UC_ARM64_REG_SP)
        pixels = struct.unpack('<Q', self.u.mem_read(sp, 8))[0]
        width, height, fmt, pixel_type = self.x(4), self.x(5), self.x(6), self.x(7)
        if (fmt, pixel_type) != (0x1908, 0x1401):
            raise RuntimeError(f'unexpected native texture format/type: {fmt:#x}/{pixel_type:#x}')
        read_bytes = width * height * 4
        allocation = self.array_allocations.get(pixels)
        if allocation != read_bytes:
            raise RuntimeError(f'GL upload reads {read_bytes} bytes from allocation of {allocation}')
        payload = bytes(self.u.mem_read(pixels, read_bytes))
        call = {'call': 'glTexSubImage2D', 'target': self.x(0), 'level': self.x(1),
                'xoffset': self.x(2), 'yoffset': self.x(3), 'width': width,
                'height': height, 'format': fmt, 'type': pixel_type}
        self.gl_calls.append(call)
        record = {'call': call, 'read_bytes': read_bytes, 'allocation_bytes': allocation,
                  'upload_sha256': hashlib.sha256(payload).hexdigest()}
        if (width, height) == (144, 144):
            tiles = []
            sentinels = []
            for tile_y in range(3):
                tile_row, sentinel_row = [], []
                for tile_x in range(3):
                    tile = bytearray()
                    sentinel_pixels = 0
                    for y in range(tile_y * 48, (tile_y + 1) * 48):
                        start = (y * width + tile_x * 48) * 4
                        row_bytes = payload[start:start + 48 * 4]
                        tile.extend(row_bytes)
                        sentinel_pixels += sum(row_bytes[i:i + 4] == b'\xcd' * 4
                                               for i in range(0, len(row_bytes), 4))
                    tile_row.append(hashlib.sha256(tile).hexdigest())
                    sentinel_row.append(sentinel_pixels)
                tiles.append(tile_row)
                sentinels.append(sentinel_row)
            if sentinels != [[0, 0, 0], [0, 0, 0], [0, 2304, 2304]]:
                raise RuntimeError(f'pattern atlas native-write map changed: {sentinels}')
            record['tile_sha256_3x3'] = tiles
            record['diagnostic_sentinel_pixels_3x3'] = sentinels
        elif (width, height) == (321, 228):
            if b'\xcd\xcd\xcd\xcd' in payload:
                raise RuntimeError('grain texture contains diagnostic sentinel words')
        else:
            raise RuntimeError(f'unexpected upload dimensions: {width}x{height}')
        self.upload_records.append(record)

    def create_texture_object(self):
        descriptor, label_ptr, initial_source = self.x(0), self.x(1), self.x(2)
        pair_data_ptr, pair_count, info_ptr = self.x(3), self.x(4), self.x(5)
        descriptor_words = list(self.get(descriptor, '8I'))
        info_words = list(self.get(info_ptr, '4I'))
        pair_words = list(self.get(pair_data_ptr, f'{pair_count * 2}I'))
        label = self.cstr(label_ptr)
        texture_target = self.get(RENDERER_BASE + 0x313f4 + descriptor_words[5] * 4, 'I')[0]
        if texture_target != 0x0de1:
            raise RuntimeError(f'unexpected renderer target mapping: {texture_target:#x}')
        self.texture_descriptors.append({
            'resource': label,
            'descriptor_u32': descriptor_words,
            'buffer_info_u32': info_words,
            'u32_pairs': pair_words,
            'pair_count': pair_count,
            'initial_source_is_null': initial_source == 0,
            'texture_target_gl_enum': texture_target,
        })
        texture = self.alloc(320)
        self.put(texture, 'Q', self.texture_vtable)
        # Actual Renderer::TextureObject::Update reads these object fields and
        # dispatches GetWidth/GetHeight through its native interface slots.
        self.put(texture + 40, 'II', texture_target, 1)
        self.put(texture + 64, 'II', descriptor_words[0], descriptor_words[1])
        self.put(texture + 76, 'II', descriptor_words[3], descriptor_words[4])
        self.put(texture + 268, 'I', 0)
        self.x(0, texture)

    def hook(self, u, address, size, data):
        if address in self.draw_stubs:
            self.dispatch_draw_stub(address)
            return
        entries = (
            'SPen::BrushPenStrokeDrawableRTV1::Init()',
            'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)',
            'SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)',
            'SPen::BrushPenStrokeDrawableRTV1::buildTextures(int, int)',
            'SPen::BrushPenStrokeDrawableRTV1::setTextures(int, int)',
            CREATE_BITMAP,
            'SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::SPSubBitmapRT::SPSubBitmapRT(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)',
            'SPen::SPTextureBitmap::UpdateBitmap(void const*, int)',
            'non-virtual thunk to SPen::SPSubBitmapRT::UpdateBitmap(void const*, int)',
            'SPen::SPTextureBitmap::SetTextureFilter(SPen::TextureParamFiltering, SPen::TextureParamFilteringValue)',
            'SPen::SPTextureBitmap::SetTextureWrap(SPen::TextureParamWrapping, SPen::TextureParamWrappingValue)',
        )
        for name in entries:
            if address == self.syms.get(name):
                self.native_entries[name] = self.native_entries.get(name, 0) + 1
                if name == CREATE_BITMAP:
                    sp = self.u.reg_read(R.UC_ARM64_REG_SP)
                    key = self.cstr(self.x(3))
                    self.bitmap_factory_calls.append({
                        'resource': key, 'width': self.x(1), 'height': self.x(2),
                        'source_is_null': self.x(4) == 0,
                        'bool_arg': self.x(5), 'int_arg': self.x(6), 'bool_arg_2': self.x(7),
                        'stack_args': list(struct.unpack('<I4xI', self.u.mem_read(sp, 12))),
                    })
                if name.endswith('buildTextures(int, int)') or name.endswith('setTextures(int, int)'):
                    self.native_entries.setdefault('texture_wrapper_args', []).append([self.x(1), self.x(2)])
                if 'SetTextureFilter' in name or 'SetTextureWrap' in name:
                    operation = 'SetTextureFilter' if 'SetTextureFilter' in name else 'SetTextureWrap'
                    self.texture_state_calls.append({'wrapper': operation,
                                                     'enum': self.x(1), 'value': self.x(2)})
        if address == self.state_stub:
            self.texture_state_calls.append({'wrapper': 'lower ITextureObject state stub',
                                             'enum': self.x(1), 'value': self.x(2)})
        if address == GRAIN_SOURCE_CAPTURE_PC and self.x(9) and self.grain_source is None:
            source, length = self.x(9), self.x(10)
            raw = bytes(u.mem_read(source, length))
            self.grain_source = {'bytes': length, 'sha256': hashlib.sha256(raw).hexdigest()}
        try:
            super().hook(u, address, size, data)
        except Exception as exc:
            raise RuntimeError(f'{exc}; pc={address:#x} lr={self.x(30):#x} '
                               f'active_import={self.active_import}') from exc

    def run(self):
        self.native_hashes = native_hashes()
        self.callbacks.pop(CREATE_BITMAP, None)
        state = self.setup_native_pen()
        rt = state['rt']
        grain_dims = list(self.get(rt + 240, '2f'))
        if state['native_version'] != 10 or grain_dims != [321.0, 228.0]:
            raise RuntimeError(f'constructor profile mismatch: GLV{state["native_version"]} grain={grain_dims}')
        # Init sees nonnull sibling resources and skips the unrelated graphics
        # and shader construction branches. These addresses
        # are deliberately unmapped, so any unexpected dereference faults.
        skipped_resources = {field: 0x13000000 + index * 0x1000
                             for index, field in enumerate((200, 216))}
        for field, pointer in skipped_resources.items():
            self.put(rt + field, 'Q', pointer)
        regions = self.u.mem_regions()
        if any(start <= pointer <= end for pointer in skipped_resources.values()
               for start, end, _permissions in regions):
            raise AssertionError('synthetic skip pointer unexpectedly overlaps mapped memory')
        self.call('SPen::BrushPenStrokeDrawableRTV1::Init()', [rt], instruction_limit=5_000_000)
        blend_factory = 'SPen::RendererFactory::CreateBlendStateObject(SPen::BlendStateDescriptor const&)'
        if self.import_calls.get(blend_factory) != 1:
            raise RuntimeError(f'native Init blend factory count mismatch: {self.import_calls.get(blend_factory)}')
        expected_counts = {
            'SPen::BrushPenStrokeDrawableRTV1::Init()': 1,
            'SPen::BrushPenStrokeDrawableRTV1::buildTextures(int, int)': 1,
            'SPen::BrushPenStrokeDrawableRTV1::setTextures(int, int)': 1,
            'SPen::SPTextureBitmap::SPTextureBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, bool, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)': 2,
            'SPen::SPSubBitmapRT::SPSubBitmapRT(SPen::IGLMsgQueue*, int, SPen::Rect const&, int, int, char const*, void const*, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)': 2,
            'SPen::SPTextureBitmap::UpdateBitmap(void const*, int)': 2,
            'non-virtual thunk to SPen::SPSubBitmapRT::UpdateBitmap(void const*, int)': 2,
        }
        for name, count in expected_counts.items():
            if self.native_entries.get(name) != count:
                raise RuntimeError(f'native entry count mismatch for {name}: {self.native_entries.get(name)}')
        if self.native_entries.get('texture_wrapper_args') != [[48, 48], [48, 48]]:
            raise RuntimeError(
                f'Init texture wrapper args mismatch: {self.native_entries.get("texture_wrapper_args")}')
        forbidden = ('RendererFactory::CreateGraphicsObject',
                     'ShaderManager::GetInstance', 'BrushPenStrokeShaderV1::BrushPenStrokeShaderV1')
        reached_forbidden = {name: count for name, count in self.import_calls.items()
                             if count and any(token in name for token in forbidden)}
        if reached_forbidden:
            raise RuntimeError(f'Init entered a skipped render sibling: {reached_forbidden}')
        if len(self.texture_descriptors) != 2 or len(self.upload_records) != 2:
            raise RuntimeError('expected pattern and grain descriptors and uploads')
        if self.grain_source != {'bytes': 73188,
                                 'sha256': 'd6e0743f9343efac472d3d00ba44f4138572620b376fe6fd74d9b5ebf1aad2aa'}:
            raise RuntimeError(f'embedded grain source changed: {self.grain_source}')
        draw_capture = self.capture_native_draw(rt)
        expected_draw_entries = {
            'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)': 1,
            'SPen::BrushPenStrokeDrawableRTV1::SetStartPoint(float, float)': 1,
        }
        for name, count in expected_draw_entries.items():
            if self.native_entries.get(name) != count:
                raise RuntimeError(f'native Draw entry mismatch for {name}: {self.native_entries.get(name)}')
        if set(draw_capture['bindings']) != {'ProjectionMatrix', 'sampler0', 'sampler1',
                                             'inputColor', 'grain', 'grainOffset', 'numTiles'}:
            raise RuntimeError(f'native Draw binding capture incomplete: {sorted(draw_capture["bindings"])}')
        byte_sizes = {'ProjectionMatrix': 64, 'sampler0': 4, 'sampler1': 4,
                      'inputColor': 16, 'grain': 8, 'grainOffset': 8, 'numTiles': 4}
        offsets = {'ProjectionMatrix': 0, 'sampler0': 8, 'sampler1': 16,
                   'inputColor': 24, 'grain': 32, 'grainOffset': 40, 'numTiles': 48}
        for name, size in byte_sizes.items():
            entry = draw_capture['bindings'][name]
            if len(bytes.fromhex(entry['bytes'])) != size or entry['binding_offset'] != offsets[name]:
                raise RuntimeError(f'native Draw binding ABI mismatch for {name}: {entry}')
        binding_events = [e for e in draw_capture['events'] if e['call'] == 'binding_set']
        if sorted((e['name'], e['byte_count']) for e in binding_events) != sorted(byte_sizes.items()):
            raise RuntimeError(f'native Draw binding dispatch mismatch: {binding_events}')
        if draw_capture['bindings']['sampler0']['u32'] != [0] or \
                draw_capture['bindings']['sampler1']['u32'] != [1]:
            raise RuntimeError('native Draw sampler unit mismatch')
        if draw_capture['bindings']['numTiles']['f32'] != [3.0]:
            raise RuntimeError(f'native numTiles binding mismatch: {draw_capture["bindings"]["numTiles"]}')
        expected_color = list(struct.unpack('<4f', struct.pack('<4f', 0.25, 0.5, 0.75, 0.8)))
        if draw_capture['bindings']['inputColor']['f32'] != expected_color:
            raise RuntimeError(f'native inputColor binding mismatch: {draw_capture["bindings"]["inputColor"]}')
        expected_projection, expected_grain, expected_offset = self.expected_draw_uniforms(draw_capture)
        for binding, expected in (('ProjectionMatrix', expected_projection),
                                  ('grain', expected_grain), ('grainOffset', expected_offset)):
            observed = draw_capture['bindings'][binding]['f32']
            if len(observed) != len(expected) or any(
                    not math.isfinite(a) or not math.isfinite(b) or abs(a - b) > 0.002
                    for a, b in zip(observed, expected)):
                raise RuntimeError(f'native Draw {binding} differs from independent transform: {observed} != {expected}')
        if self.blend_descriptor is None:
            raise RuntimeError('native Init did not create the blend-state descriptor')
        if self.blend_descriptor != '0101000000000000000000000600000007000000010000000700000000000000':
            raise RuntimeError(f'native Init blend descriptor changed: {self.blend_descriptor}')
        return {
            'artifact': 'Samsung Notes 4.4.45.37 native BrushPen',
            'library_sha256': self.native_hashes,
            'constructor_profile': {'selected_native_version': state['native_version'],
                                    'native_rt_vtable_matches_RTV1': True,
                                    'fresh_grain_dimensions_f32': grain_dims,
                                    'Init_pattern_tile_count': self.get(rt + 248, 'I')[0]},
            'init_path': {'native_function_entries': {k: v for k, v in self.native_entries.items()
                                                       if k != 'texture_wrapper_args'},
                          'buildTextures_setTextures_args': self.native_entries['texture_wrapper_args'],
                          'synthetic_preinitialized_sibling_fields': sorted(skipped_resources),
                          'skipped_sibling_pointer_dereference_guard': 'unmapped addresses; execution faults if read',
                          'skipped_branches_reached': False},
            'factory_calls': self.bitmap_factory_calls,
            'texture_descriptors': self.texture_descriptors,
            'texture_state_calls': self.texture_state_calls,
            'embedded_grain_source': self.grain_source,
            'native_draw': {'entry_count': self.native_entries[
                                'SPen::BrushPenStrokeDrawableRTV1::Draw(SPen::ISPSubBitmap*, SPen::Matrix4<float>&)'],
                            'synthetic_inputs_and_native_bindings': draw_capture,
                            'init_blend_descriptor_raw_32_bytes': self.blend_descriptor,
                            'blend_descriptor_note': 'actual native Init input to intercepted CreateBlendStateObject; raw bytes only, not a GLES blend-state mapping'},
            'uploads': self.upload_records,
            'gl_calls': self.gl_calls,
            'host_boundaries': {
                'RendererFactory::GetRendererDeviceCaps': 'synthetic caps object with maximum dimension 4096',
                'RendererFactory::CreateTextureObject': 'synthetic ITextureObject storage/interface; actual Renderer::TextureObject::Update executes',
                'GLES imports': 'arguments intercepted; no driver/context, native GPU allocation, shader execution, or pixel rendering',
                'pattern allocator': 'diagnostic 0xcd poison distinguishes writes; it is not an assertion about production heap contents',
                'clock_gettime and strncpy': 'deterministic bounded host shims',
                'skipped Init siblings': 'explicit unmapped nonnull placeholders for existing graphics and shader fields',
            },
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    result = TextureProbe().run()
    encoded = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        OUT.write_text(encoded)
        print(f'wrote {OUT}')
        return
    if not OUT.exists():
        raise SystemExit(f'missing fixture {OUT}; review native output before --write-fixture')
    expected = json.loads(OUT.read_text())
    if json.dumps(result, sort_keys=True) != json.dumps(expected, sort_keys=True):
        raise AssertionError('native BrushPen texture upload differs from the reviewed fixture')
    print('BrushPen native RTV1 texture and Draw-binding oracle passed')


if __name__ == '__main__':
    main()
