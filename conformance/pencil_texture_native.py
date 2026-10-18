#!/usr/bin/env python3
"""Execute Pencil3 native mask atlas/paper transport with bounded bitmap interfaces.

From the repository root:
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pencil_texture_native.py --write-fixture
  PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pencil_texture_native.py

Native buildTextures/setTextures run from hash-pinned code. The bitmap factory,
texture/data virtual interfaces, and final upload are explicit host boundaries.
"""
import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'conformance'))
from pencil_native import NativePencil3
from unicorn import UC_HOOK_CODE
from unicorn import arm64_const as R

LIBDIR = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
RENDERER = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenRenderer.so'
RENDERER_SHA256 = 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed'
FIXTURE = Path(__file__).resolve().parent / 'pencil-textures-v1.json'


def read_va(path, address, size):
    b = path.read_bytes()
    phoff = struct.unpack_from('<Q', b, 32)[0]
    entsize, count = struct.unpack_from('<HH', b, 54)
    for i in range(count):
        kind, _, off, va, _, filesz, _, _ = struct.unpack_from('<II6Q', b, phoff + i * entsize)
        if kind == 1 and va <= address and address + size <= va + filesz:
            start = off + address - va
            return b[start:start + size]
    raise ValueError(f'address {address:#x} is not file-backed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--library-dir', type=Path, default=LIBDIR)
    parser.add_argument('--objdump', type=Path, default=OBJDUMP)
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    library_dir = args.library_dir
    native = NativePencil3(library_dir, args.objdump)
    u = native.u
    module_base = 0x3000000
    pen = library_dir / 'libSPenPencil3.so'

    # The ordinary loader applies these relative relocations. The bounded
    # Unicorn mapper does not, so resolve only the two data GOT slots consumed
    # by setTextures, using their independently verified exported objects.
    u.mem_write(module_base + 0x11c2b8, struct.pack('<Q', module_base + 0x12c8c))
    u.mem_write(module_base + 0x11c2c0, struct.pack('<Q', module_base + 0xf3c8c))

    records = {'factories': [], 'parameter_calls': [], 'uploads': []}
    objects = {}
    bitmap_vtable = native.alloc(0x200)
    texture_vtable = native.alloc(0x200)
    data_vtable = native.alloc(0x200)
    queue = native.alloc(0x80)
    native.put(texture_vtable + 32, 'Q', 0x20000100)  # texture parameter
    native.put(texture_vtable + 48, 'Q', 0x20000140)  # bitmap parameter
    native.put(data_vtable + 48, 'Q', 0x20000110)     # pixel upload

    def fake_bitmap():
        factory = {
            'width': native.x(1), 'height': native.x(2),
            'name_ptr': native.x(3), 'data_is_null': native.x(4) == 0,
            'bool5': native.x(5), 'int6': native.x(6), 'bool7': native.x(7),
            'layouts': list(native.get(native.u.reg_read(R.UC_ARM64_REG_SP), 'QQ')),
        }
        string = bytearray()
        for offset in range(128):
            byte = u.mem_read(factory['name_ptr'] + offset, 1)[0]
            if not byte:
                break
            string.append(byte)
        factory['name'] = string.decode('utf-8', errors='replace')
        del factory['name_ptr']
        records['factories'].append(factory)
        bitmap, texture, data_object = native.alloc(0x100), native.alloc(0x100), native.alloc(0x100)
        native.put(bitmap, 'Q', bitmap_vtable)
        native.put(texture, 'Q', texture_vtable)
        native.put(data_object, 'Q', data_vtable)
        native.put(bitmap_vtable + 144, 'Q', 0x20000120)  # texture-object getter
        native.put(bitmap_vtable + 168, 'Q', 0x20000130)  # pixel-data-object getter
        objects[bitmap] = {'texture': texture, 'data': data_object,
                           'width': factory['width'], 'height': factory['height']}
        objects[texture] = {'parent': bitmap}
        objects[data_object] = {'parent': bitmap, 'width': factory['width'], 'height': factory['height']}
        native.x(0, bitmap)

    native.callbacks['SPen::SPGraphicsFactory::CreateBitmap(SPen::IGLMsgQueue*, int, int, char const*, void const*, bool, int, bool, SPen::TextureMemoryLayout, SPen::TextureTargetLayout)'] = fake_bitmap

    def target_call(address, callback):
        def handle(u0, a, size, _):
            if a == address:
                callback()
                u0.reg_write(R.UC_ARM64_REG_PC, u0.reg_read(R.UC_ARM64_REG_LR))
        u.hook_add(UC_HOOK_CODE, handle)

    def set_parameter():
        records['parameter_calls'].append([native.x(1), native.x(2)])

    def getter():
        bitmap = native.x(0)
        native.x(0, objects[bitmap]['texture'])

    def data_getter():
        bitmap = native.x(0)
        native.x(0, objects[bitmap]['data'])

    def upload():
        ptr = native.x(1)
        dimensions = objects[native.x(0)]
        width, height = dimensions['width'], dimensions['height']
        raw = bytes(u.mem_read(ptr, width * height * 4))
        records['uploads'].append({
            'width': width, 'height': height, 'size': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest(), '_bytes': raw,
        })

    for address, callback in ((0x20000100, set_parameter),
                              (0x20000110, upload),
                              (0x20000120, getter),
                              (0x20000130, data_getter),
                              (0x20000140, set_parameter)):
        target_call(address, callback)


    # NativeFountain supplies GetMsgQueue but return its preallocated queue.
    native.callbacks['SPen::PenGLDataManager::GetMsgQueue()'] = lambda: native.x(0, queue)
    obj = native.alloc(0x300)
    manager = native.alloc(0x100)
    native.put(obj + 136, 'Q', manager)
    native.put(obj + 232, 'Q', 0)
    native.put(obj + 240, 'Q', 0)
    native.call('SPen::Pencil3StrokeDrawableRTV1::buildTextures(int, int)', [obj, 120, 120])
    native.call('SPen::Pencil3StrokeDrawableRTV1::setTextures(unsigned int, unsigned int)', [obj, 120, 120])

    source_brush = read_va(pen, 0x12c8c, 16 * 120 * 120 * 4)
    source_paper = read_va(pen, 0xf3c8c, 150 * 150)
    expected_brush = b''.join(source_brush[i:i + 120 * 120 * 4] for i in range(0, len(source_brush), 120 * 120 * 4))
    expected_paper = b''.join(bytes((v, 0, 0, 0)) for v in source_paper)
    expected = [expected_brush, expected_paper]
    for i, raw in enumerate(expected):
        actual = records['uploads'][i].pop('_bytes')
        records['uploads'][i]['expected_sha256'] = hashlib.sha256(raw).hexdigest()
        records['uploads'][i]['byte_exact'] = actual == raw
        if actual != raw:
            first = next((j for j, (a, b) in enumerate(zip(actual, raw)) if a != b), None)
            raise AssertionError(f'upload {i} differs at byte {first}: got {len(actual)} expected {len(raw)}')
    renderer_bytes = RENDERER.read_bytes()
    renderer_sha = hashlib.sha256(renderer_bytes).hexdigest()
    if renderer_sha != RENDERER_SHA256:
        raise ValueError('Renderer DSO does not match pinned sampler-mapping source')
    wrap_values = list(struct.unpack('<4I', read_va(RENDERER, 0x30170, 16)))
    filter_values = list(struct.unpack('<4I', read_va(RENDERER, 0x301c0, 16)))
    parameter_pnames = list(struct.unpack('<3I', read_va(RENDERER, 0x31768, 12)))
    sampler_mapping = {
        'renderer_sha256': renderer_sha,
        'functions': {
            'TextureObject::createTextureInternalsSampler': '0x5c75c',
            'TextureObject::SetParam': '0x5f840',
            'TextureObject::getGlPname': '0x5f9bc',
        },
        'table_vas': {'wrap_enum_to_gl': '0x30170', 'filter_enum_to_gl': '0x301c0',
                      'parameter_id_to_gl_pname': '0x31768'},
        'decoded_values_hex': {
            'wrap_enum_to_gl': [f'{v:#x}' for v in wrap_values],
            'filter_enum_to_gl': [f'{v:#x}' for v in filter_values],
            'parameter_id_0_1_2_to_gl_pname': [f'{v:#x}' for v in parameter_pnames],
            'parameter_id_4_5_to_gl_pname': ['0x2800', '0x2801'],
        },
        'pencil_effective_requests': {
            'brush': {'min_filter': 'NEAREST', 'mag_filter': 'NEAREST',
                      'wrap_s': 'CLAMP_TO_EDGE', 'wrap_t': 'CLAMP_TO_EDGE'},
            'paper': {'min_filter': 'NEAREST', 'mag_filter': 'NEAREST',
                      'wrap_s': 'REPEAT', 'wrap_t': 'REPEAT',
                      'wrap_r_default': 'CLAMP_TO_EDGE'},
        },
        'evidence_boundary': 'Parameter calls are captured from native Pencil3 buildTextures. GL pname/value mappings and empty-map sampler defaults come from this hash-pinned shared Renderer implementation; the full Pencil3 Graphics-to-GLES factory chain is not executed.',
    }
    if wrap_values != [0x8370, 0x812f, 0x812d, 0x2901] or filter_values != [0x2600, 0x2601, 0, 0] or parameter_pnames != [0x2802, 0x2803, 0x8072]:
        raise AssertionError('Renderer sampler mapping tables differ from the decoded contract')
    result = {
        'version': 1,
        'provenance': {
            'apk_sha256': 'daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667',
            'library_sha256': hashlib.sha256(pen.read_bytes()).hexdigest(),
            'native_functions': {'buildTextures': '0x10bfb4', 'setTextures': '0x10c158'},
            'capture_boundary': 'Native buildTextures/setTextures execute under Unicorn. CreateBitmap, bitmap/texture/data virtual objects, queue, and data upload are host interfaces. Native source arrays and both packing loops execute. The mapper resolves the two data GOT references from verified static symbols.',
        },
        'factories': records['factories'],
        'parameter_calls': records['parameter_calls'],
        'sampler_mapping': sampler_mapping,
        'uploads': records['uploads'],
        'assertions': {'brush_upload_matches_expanded_native_g_penPtn': True,
                       'paper_upload_matches_expanded_native_g_backgroundPtn': True},
    }
    encoded = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        args.fixture.write_text(encoded)
    elif not args.fixture.exists() or json.loads(args.fixture.read_text()) != result:
        raise SystemExit('Pencil3 texture fixture mismatch; inspect native library identity and upload contract')
    print('Pencil3 V1 texture transport: 2 native uploads byte-exact')
    for upload in records['uploads']:
        print(f"  {upload['width']}x{upload['height']} {upload['size']} bytes sha256={upload['sha256']}")


if __name__ == '__main__':
    main()
