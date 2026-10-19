#!/usr/bin/env python3
"""Replay native FountainPen V16 RTV5 AddPoint radial-field records.

This captures the pinned ARM64 AddPoint numeric contract directly. It does not
claim a Drawing caller, GPU execution, device pixels, or PDF parity.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-v16-addpoint-native.json'
LIBRARY_DIR = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'
DSO_SHA256 = '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
HELPER_SHA256 = '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'
SYMBOL = 'SPen::FountainPenStrokeDrawableRTV5::AddPoint(SPen::Vector3<float>, float)'
RVA = 0x9f700
FIELDS = ['center_x', 'center_y', 'extent_x', 'extent_y', 'transparency', 'inner_radius', 'alpha_by_width']
RADII = (0.0, 0.1, 0.499, 0.5, 0.501, 1.0, 2.0)


def f32(value: float) -> float:
    return struct.unpack('<f', struct.pack('<f', value))[0]


def bits(value: float) -> str:
    return struct.pack('<f', value).hex()


def native_addpoint(native, radius: float, inverse_xy: tuple[float, float]) -> list[str]:
    native.heap = 0x10000000
    obj, vector, buffer = native.alloc(512), native.alloc(24), native.alloc(128)
    native.put(obj + 80, 'Q', vector)
    native.put(vector, 'QQQ', buffer, buffer, buffer + 128)
    native.put(obj + 264, 'ff', *inverse_xy)
    native.call(SYMBOL, [obj], [32.25, 32.25, radius, 1.0])
    start, end, capacity = native.get(vector, 'QQQ')
    raw = bytes(native.u.mem_read(start, end - start))
    if len(raw) != 7 * 4 or capacity < end:
        raise AssertionError(f'RTV5 emitted {len(raw)} bytes, expected one seven-float record')
    return [raw[i:i + 4].hex() for i in range(0, len(raw), 4)]


def cases() -> list[dict]:
    result = []
    for scale in (1, 2, 4):
        inv = 1.0 / scale
        for radius in RADII:
            result.append({'case_id': f'radius_{radius:g}_scale_{scale}', 'radius': radius,
                           'inverse_xy': [inv, inv], 'kind': 'positive_radius' if radius else 'zero_radius_edge'})
    for i, inverse in enumerate(((0.5, 1.0), (1.0, 0.5), (0.25, 1.0), (1.0, 0.25))):
        result.append({'case_id': f'anisotropic_{i}', 'radius': 0.1, 'inverse_xy': list(inverse),
                       'kind': 'anisotropic_inverse_scale'})
    return result


def replay() -> dict:
    dso = LIBRARY_DIR / 'libSPenFountainPen.so'
    if hashlib.sha256(dso.read_bytes()).hexdigest() != DSO_SHA256:
        raise SystemExit('libSPenFountainPen.so hash differs from pinned DSO')
    helper = ROOT / 'conformance/fountain_native.py'
    if hashlib.sha256(helper.read_bytes()).hexdigest() != HELPER_SHA256:
        raise SystemExit('fountain_native.py helper hash differs from pinned helper')
    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_native
    if fountain_native.HASHES.get('FountainPen') != DSO_SHA256:
        raise SystemExit('NativeFountain DSO pin differs')
    native = fountain_native.NativeFountain(LIBRARY_DIR, OBJDUMP)
    if native.syms.get(SYMBOL) != RVA:
        raise SystemExit(f'RTV5 AddPoint RVA changed: {native.syms.get(SYMBOL)!r}')
    rows = []
    for case in cases():
        words = native_addpoint(native, case['radius'], tuple(case['inverse_xy']))
        values = [struct.unpack('<f', bytes.fromhex(word))[0] for word in words]
        printable_values = [v if v == v and abs(v) != float('inf') else
                            ('NaN' if v != v else ('Infinity' if v > 0 else '-Infinity'))
                            for v in values]
        rows.append({**case, 'input_f32_le_hex_words': {
            'center': [bits(32.25), bits(32.25)], 'radius': bits(case['radius']),
            'inverse_xy': [bits(v) for v in case['inverse_xy']], 'transparency': bits(1.0)},
            'record_f32_le_hex_words': words,
            'record_values': printable_values,
            'record_byte_count': 28})
    return {'schema': 'sdocx-fountain-v16-addpoint-native-v1',
            'provenance': {'library': 'libSPenFountainPen.so', 'library_sha256': DSO_SHA256,
                           'helper': 'conformance/fountain_native.py', 'helper_sha256': HELPER_SHA256,
                           'symbol': SYMBOL, 'rva': hex(RVA),
                           'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
            'record_layout': FIELDS,
            'host_boundaries': ['Direct native RTV5 AddPoint call with explicit drawable vector storage and inverse-axis scale. No saved Drawing call, GL shader execution, or device raster is included.',
                                'The zero-radius row documents native edge output only; finite positive-radius rows establish the radial record fields.'],
            'cases': rows}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true')
    mode.add_argument('--check-fixture', action='store_true')
    args = parser.parse_args()
    actual = replay()
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(actual, indent=2, sort_keys=True) + '\n')
        print(f'wrote {len(actual["cases"])} RTV5 records')
    else:
        expected = json.loads(FIXTURE.read_text())
        # The runner hash changes when the JSON is regenerated after a source edit;
        # a normal check still requires the current source hash to match the pinned row.
        if expected != actual:
            raise SystemExit('native RTV5 records or provenance differ from fixture')
        print(f'verified {len(actual["cases"])} native RTV5 records')


if __name__ == '__main__':
    main()
