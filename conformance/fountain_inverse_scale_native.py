#!/usr/bin/env python3
"""Replay hash-pinned Fountain V14/V16 model-matrix inverse-scale getters."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-inverse-scale-native.json'
DSO = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so'
NATIVE_HELPER = ROOT / 'conformance/fountain_native.py'
LIBRARY_DIR = DSO.parent
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
EXPECTED_DSO_SHA256 = '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
EXPECTED_NATIVE_HELPER_SHA256 = '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'
EXPECTED_EXTRA_DSO_HASHES = {
    'libSPenPenCommon.so': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'libSPenBase.so': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
}
SYMBOL_RVAS = {
    'SPen::FountainPenStrokeDrawableGLV14::getInverseScale()': 0x72228,
    'SPen::FountainPenStrokeDrawableGLV16::getInverseScale()': 0x77e58,
    'SPen::FountainPenStrokeDrawableRTV4::SetInverseScale(SPen::Vector2<float> const&)': 0x9e5f8,
    'SPen::FountainPenStrokeDrawableRTV5::SetInverseScale(SPen::Vector2<float> const&)': 0x9fd28,
}
# Raw little-endian AArch64 instruction bytes anchored to the pinned DSO.
INSTRUCTION_ANCHORS = {
    'v14_get_model_matrix': (0x7224c, '001040f9'),       # ldr x0,[x0,#32]
    'v14_get_matrix_slot': (0x7225c, '096d40f9'),        # ldr x9,[x8,#216]
    'v14_inverse_x_store': (0x72308, '605201bd'),        # str s0,[x19,#336]
    'v14_inverse_y_store': (0x7230c, '615601bd'),        # str s1,[x19,#340]
    'v16_get_model_matrix': (0x77e7c, '001040f9'),
    'v16_get_matrix_slot': (0x77e8c, '096d40f9'),
    'v16_inverse_x_store': (0x77f38, '604a01bd'),        # str s0,[x19,#328]
    'v16_inverse_y_store': (0x77f3c, '614e01bd'),        # str s1,[x19,#332]
    'rtv4_setter_load_pair': (0x9e5f8, '280040f9'),     # ldr x8,[x1]
    'rtv4_setter_store_pair': (0x9e5fc, '088400f9'),    # str x8,[x0,#264]
    'rtv5_setter_load_pair': (0x9fd28, '280040f9'),
    'rtv5_setter_store_pair': (0x9fd2c, '088400f9'),
    'v14_redraw_get_inverse': (0x72a64, '2b610194'),    # bl GLV14 getter PLT
    'v14_redraw_x_direct_store': (0x72a68, '200b01bd'), # str s0,[x25,#264]
    'v14_redraw_y_direct_store': (0x72a6c, '210f01bd'), # str s1,[x25,#268]
    'v16_redraw_get_inverse': (0x78618, 'ca4a0194'),    # bl GLV16 getter PLT
    'v16_redraw_x_direct_store': (0x78840, '080901bd'), # str s8,[x8,#264]
    'v16_redraw_y_direct_store': (0x78844, '090d01bd'), # str s9,[x8,#268]
}
MATRICES = [
    {'id': 'anisotropic_scale', 'f32': [2, 0, 0, 0, 4, 0, 0, 0, 1]},
    {'id': 'shear', 'f32': [2, 3, 0, 4, 5, 0, 9, -7, 1]},
    {'id': 'rotated_3_4', 'f32': [0, 3, 0, -4, 0, 0, 0, 0, 1]},
    {'id': 'third_component', 'f32': [2, 3, 6, 4, 5, 12, 9, -7, 1]},
    {'id': 'translation_only', 'f32': [1, 0, 0, 0, 1, 0, 500, -300, 1]},
]
PROBE_ADDR = 0x20010000


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bits_f32(value: float) -> str:
    return struct.pack('<f', value).hex()


def elf_bytes_at_vaddr(path: Path, address: int, size: int) -> bytes:
    data = path.read_bytes()
    if data[:6] != b'\x7fELF\x02\x01':
        raise ValueError('expected little-endian ELF64')
    phoff = struct.unpack_from('<Q', data, 32)[0]
    phentsize, phnum = struct.unpack_from('<HH', data, 54)
    for i in range(phnum):
        off = phoff + i * phentsize
        p_type, _flags, p_offset, p_vaddr, _paddr, p_filesz, _memsz, _align = struct.unpack_from('<IIQQQQQQ', data, off)
        if p_type == 1 and p_vaddr <= address and address + size <= p_vaddr + p_filesz:
            start = p_offset + address - p_vaddr
            return data[start:start + size]
    raise ValueError(f'no file-backed PT_LOAD for {address:#x}+{size:#x}')


def instruction_witness() -> dict:
    witness = {}
    for name, (address, expected) in INSTRUCTION_ANCHORS.items():
        actual = elf_bytes_at_vaddr(DSO, address, len(bytes.fromhex(expected))).hex()
        if actual != expected:
            raise AssertionError(f'{name} at {address:#x}: {actual} != pinned instruction {expected}')
        witness[name] = {'rva': hex(address), 'bytes_le': actual}
    encoded = json.dumps(witness, sort_keys=True, separators=(',', ':')).encode()
    return {'anchors': witness, 'sha256': hashlib.sha256(encoded).hexdigest()}


def run_native() -> list[dict]:
    if sha256(DSO) != EXPECTED_DSO_SHA256:
        raise SystemExit('FountainPen DSO hash differs from pin')
    if sha256(NATIVE_HELPER) != EXPECTED_NATIVE_HELPER_SHA256:
        raise SystemExit('fountain_native.py helper hash differs from pin')
    for name, expected in EXPECTED_EXTRA_DSO_HASHES.items():
        if sha256(LIBRARY_DIR / name) != expected:
            raise SystemExit(f'{name} hash differs from pin')

    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_native

    if fountain_native.HASHES.get('FountainPen') != EXPECTED_DSO_SHA256:
        raise SystemExit('NativeFountain DSO table does not match expected pin')
    loader = fountain_native.NativeFountain(LIBRARY_DIR, OBJDUMP)
    for name, rva in SYMBOL_RVAS.items():
        if loader.syms.get(name) != rva:
            raise SystemExit(f'symbol RVA differs for {name}: {loader.syms.get(name)!r}')

    compare_calls = []
    def bounded_memcmp():
        left, right, length = loader.x(0), loader.x(1), loader.x(2)
        a = bytes(loader.u.mem_read(left, length))
        b = bytes(loader.u.mem_read(right, length))
        result = 0
        for av, bv in zip(a, b):
            if av != bv:
                result = av - bv
                break
        compare_calls.append({'left_address': hex(left), 'right_address': hex(right),
                              'length': length, 'equal': result == 0, 'return': result})
        loader.x(0, result)
    loader.callbacks['memcmp'] = bounded_memcmp
    loader.plt[PROBE_ADDR] = 'probe::getModelMatrix'

    rows = []
    for version in (14, 16):
        matrices = [dict(m) for m in MATRICES]
        gl = loader.alloc(512)  # explicit zeroed receiver; no GL constructor is invoked
        canvas = loader.alloc(256)
        vtable = loader.alloc(256)
        loader.put(canvas, 'Q', vtable)
        loader.put(vtable + 216, 'Q', PROBE_ADDR)
        loader.put(gl + 32, 'Q', canvas)
        cache_offset = 300 if version == 14 else 292
        scale_offset = 336 if version == 14 else 328
        initial_cache = bytes(loader.u.mem_read(gl + cache_offset, 36))
        if initial_cache != bytes(36):
            raise AssertionError('new controlled GL receiver was not zero initialized')

        getter = f'SPen::FountainPenStrokeDrawableGLV{version}::getInverseScale()'
        per_version_calls = []
        for matrix in matrices:
            matrix_bytes = struct.pack('<9f', *matrix['f32'])
            def write_matrix(payload=matrix_bytes):
                out = loader.x(8)  # native canvas virtual ABI: result reference in X8
                loader.u.mem_write(out, payload)
            loader.callbacks['probe::getModelMatrix'] = write_matrix
            for phase in ('changed', 'same'):
                before_count = len(compare_calls)
                loader.call(getter, [gl])
                native_matrix = bytes(loader.u.mem_read(gl + cache_offset, 36))
                native_pair = bytes(loader.u.mem_read(gl + scale_offset, 8))
                returned_pair = struct.pack('<2f', loader.f(0), loader.f(1))
                if native_matrix != matrix_bytes:
                    raise AssertionError(f'V{version} {matrix["id"]}/{phase}: cached matrix differs')
                if returned_pair != native_pair:
                    raise AssertionError(f'V{version} {matrix["id"]}/{phase}: return pair differs from cached fields')
                calls = compare_calls[before_count:]
                if len(calls) != 1 or calls[0]['length'] != 36:
                    raise AssertionError('getter did not perform exactly one bounded 36-byte matrix comparison')
                expected_equal = phase == 'same'
                if calls[0]['equal'] != expected_equal:
                    raise AssertionError(f'V{version} {matrix["id"]}/{phase}: memcmp equality path differs')
                tolerance_bits = bytes(loader.u.mem_read(gl + 96, 4)).hex()
                row = {'version': version, 'matrix_id': matrix['id'], 'phase': phase,
                       'input_matrix_f32_le_hex_words': [bits_f32(v) for v in matrix['f32']],
                       'memcmp_equal': calls[0]['equal'], 'native_cached_matrix_f32_le_hex_words':
                           [native_matrix[i:i+4].hex() for i in range(0,36,4)],
                       'native_inverse_xy_f32_le_hex_words':
                           [native_pair[i:i+4].hex() for i in range(0,8,4)],
                       'getter_return_xy_f32_le_hex_words':
                           [returned_pair[i:i+4].hex() for i in range(0,8,4)],
                       'native_tolerance_field_f32_le_hex': tolerance_bits}
                rows.append(row)
                per_version_calls.append(row)
        if len(per_version_calls) != 2 * len(MATRICES):
            raise AssertionError('expected changed/same phases for every matrix')
    if len(MATRICES) != 5 or len(rows) != 20:
        raise AssertionError('expected five matrices, two phases, and two native versions')
    for index in range(0, len(rows), 2):
        v14_rows = [row for row in rows if row['version'] == 14]
        v16_rows = [row for row in rows if row['version'] == 16]
        if v14_rows[index // 2]['native_inverse_xy_f32_le_hex_words'] != v16_rows[index // 2]['native_inverse_xy_f32_le_hex_words']:
            raise AssertionError('V14/V16 inverse-scale outputs differ for the same input/phase')
        if v14_rows[index // 2]['native_tolerance_field_f32_le_hex'] != v16_rows[index // 2]['native_tolerance_field_f32_le_hex']:
            raise AssertionError('V14/V16 tolerance fields differ for the same input/phase')

    return rows


def generate() -> dict:
    witness = instruction_witness()
    cases = run_native()
    return {
        'schema': 'sdocx-fountain-inverse-scale-native-v1',
        'evidence': {
            'runner_sha256': sha256(Path(__file__)),
            'apk': 'Samsung Notes 4.4.45.37 pinned ARM64 libraries',
            'libraries': {'libSPenFountainPen.so': EXPECTED_DSO_SHA256,
                          **EXPECTED_EXTRA_DSO_HASHES},
            'native_helper': {'path': 'conformance/fountain_native.py',
                              'sha256': EXPECTED_NATIVE_HELPER_SHA256},
            'symbols': {name: hex(rva) for name, rva in SYMBOL_RVAS.items()},
            'instruction_witness': witness,
            'host_boundary': [
                'The actual GLV14/GLV16 getInverseScale code executes in the hash-pinned NativeFountain Unicorn loader.',
                'The canvas receiver is synthetic. Its vtable GetModelMatrix slot is a host callback that writes the exact nine-float input matrix to the native output-reference pointer in X8.',
                'The GL drawable receiver is an explicitly zeroed host allocation; its constructor is not invoked, so this oracle makes no GL-constructor/default-scale claim.',
                'memcmp is a faithful bounded bytewise comparison callback over the native pointers and requested 36-byte range; changed and unchanged cache paths are both exercised.',
                'Redraw caller disassembly witnesses show direct stores into attached RT fields; this oracle does not execute the full GL RedrawPen caller or RTV Draw.',
            ],
            'native_contract': 'The getter reads the canvas Matrix3 through vtable slot +216, byte-compares it to the drawable cache, and on change calculates two reciprocal column norms in scalar AArch64 f32 using fmul/fmadd/fsqrt/fdiv. Getter return words equal the stored cache pair on changed and unchanged calls.',
            'comparison_order': 'For matrix words m0..m8, t0=fmul(m1,m1); a0=fmadd(m0,m0,t0); a1=fmadd(m2,m2,a0); t1=fmul(m4,m4); b0=fmadd(m3,m3,t1); b1=fmadd(m5,m5,b0); s0=fdiv(1,fsqrt(a1)); s1=fdiv(1,fsqrt(b1)).',
            'scale_boundary': 'These inverse factors are derived from the canvas model matrix basis. The separately passed Matrix4 in RTV Draw remains a per-draw transform; no display-density or bitmap-dimension input is part of this getter.',
        },
        'cases': cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    actual = generate()
    encoded = json.dumps(actual, indent=2, sort_keys=True) + '\n'
    if args.write_fixture:
        FIXTURE.write_text(encoded)
        print(f'wrote {len(actual["cases"])} V14/V16 inverse-scale observations to {FIXTURE.relative_to(ROOT)}')
    else:
        if not FIXTURE.exists() or json.loads(FIXTURE.read_text()) != actual:
            raise SystemExit('native Fountain inverse-scale replay differs from fixture')
        print(f'verified {len(actual["cases"])} native Fountain inverse-scale observations')


if __name__ == '__main__':
    main()
