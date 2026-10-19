#!/usr/bin/env python3
"""Replay the hash-pinned V14 RTV4 AddPoint numeric controls and f32 model."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-v14-addpoint-native.json'
FIELD_FIXTURE = ROOT / 'conformance/fountain-v14-field-native.json'
EXPECTED_FIELD_FIXTURE_SHA256 = '4c1eb14ebae2561c4aceb1d618601a724c2e4b86f99ed4c8533b2cb0682fa3ff'
HELPER = ROOT / 'conformance/fountain_v14_field_native.py'
NATIVE_HELPER = ROOT / 'conformance/fountain_native.py'
DSO = ROOT / 'scratch/apk-analysis-native/arm64-v8a/libSPenFountainPen.so'
EXPECTED_DSO_SHA256 = '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec'
EXPECTED_FIELD_HELPER_SHA256 = '6891169bb01da9ad6442ee6e103b38b8670b9de9e22767aa006a6033b2a4f7c3'
EXPECTED_NATIVE_HELPER_SHA256 = '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'
ADDPOINT = 'SPen::FountainPenStrokeDrawableRTV4::AddPoint(SPen::Vector3<float>, SPen::Vector2<float>, float)'
ADDPOINT_RVA = 0x9de50
LAYOUT = ['center_x', 'center_y', 'extent_x', 'extent_y', 'direction_x', 'direction_y', 'transparency', 'inner_radius', 'alpha_by_width']
RADII = (0.0, 0.01, 0.1, 0.499, 0.5, 0.501, 1.0, 2.0)


def f32(value: float) -> float:
    return struct.unpack('<f', struct.pack('<f', value))[0]


def bits(value: float) -> str:
    # AArch64 FPCR default-NaN mode emits canonical positive qNaN for these
    # invalid edge operations; Python may preserve a negative NaN sign.
    if math.isnan(value):
        return '0000c07f'
    return struct.pack('<f', value).hex()


def fdiv(a: float, b: float) -> float:
    # IEEE scalar divide behavior, including zero-radius qNaN/inf controls.
    if b == 0.0:
        if a == 0.0:
            return float('nan')
        return math.copysign(float('inf'), a * math.copysign(1.0, b))
    return f32(a / b)


def fmul(a: float, b: float) -> float:
    try:
        return f32(a * b)
    except OverflowError:
        return math.copysign(float('inf'), a * b)


def fadd(a: float, b: float) -> float:
    return f32(a + b)


def native_model(case: dict) -> list[float] | None:
    """Independent lane-level model following the native scalar f32 sequence."""
    radius = f32(case['radius'])
    ix, iy = map(f32, case['inverse_xy'])
    x, y = map(f32, case['point'])
    dx, dy = map(f32, case['direction'])
    transparency = f32(case['transparency'])
    if ix == 0.0 or iy == 0.0:
        return None

    rx = fdiv(radius, ix)
    ry = fdiv(radius, iy)
    # Match the compare/select branch for these finite input controls.
    minimum = rx if rx < ry else ry
    if minimum < 0.5:
        k = fdiv(0.5, minimum)
        alpha_by_width = fdiv(1.0, k)
    else:
        k = 1.0
        alpha_by_width = 1.0
    # The native computes alpha_by_width from the pre-fmaxnm reciprocal above;
    # the multiplier used for extents is then independently clamped to >= 1.
    extent_scale = max(k, 1.0) if not math.isnan(k) else 1.0
    qx = fadd(fmul(rx, extent_scale), 0.5)
    qy = fadd(fmul(ry, extent_scale), 0.5)
    extent_x = fmul(ix, qx)
    extent_y = fmul(iy, qy)
    extent_basis = max(qx, qy) if not (math.isnan(qx) or math.isnan(qy)) else float('nan')
    inner_radius = fmul(fdiv(fadd(extent_basis, -1.0), extent_basis), 0.5)
    return [x, y, extent_x, extent_y, dx, dy, transparency, inner_radius, alpha_by_width]


def run_native(cases: list[dict]) -> list[dict]:
    if hashlib.sha256(DSO.read_bytes()).hexdigest() != EXPECTED_DSO_SHA256:
        raise SystemExit('libSPenFountainPen.so hash differs from the pinned DSO')
    if hashlib.sha256(HELPER.read_bytes()).hexdigest() != EXPECTED_FIELD_HELPER_SHA256:
        raise SystemExit('fountain_v14_field_native.py helper hash changed')
    if hashlib.sha256(NATIVE_HELPER.read_bytes()).hexdigest() != EXPECTED_NATIVE_HELPER_SHA256:
        raise SystemExit('fountain_native.py helper hash changed')
    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_native
    from fountain_v14_field_native import native_addpoint

    if fountain_native.HASHES.get('FountainPen') != EXPECTED_DSO_SHA256:
        raise SystemExit('NativeFountain DSO identity differs from the expected FountainPen pin')
    loader = fountain_native.NativeFountain(DSO.parent, ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump')
    if loader.syms.get(ADDPOINT) != ADDPOINT_RVA:
        raise SystemExit(f'RTV4 AddPoint RVA changed: {loader.syms.get(ADDPOINT)!r}')

    result = []
    for case in cases:
        ix, iy = map(f32, case['inverse_xy'])
        if case['radius'] > 0 and ix == iy and case['transparency'] == 1.0:
            scale = round(1.0 / ix)
            record = native_addpoint(loader, {'point':case['point'], 'radius':case['radius'],
                                              'direction':case['direction']}, scale)
            raw = struct.pack('<9f', *record)
        else:
            loader.heap = 0x10000000
            obj, vector, buffer = loader.alloc(512), loader.alloc(24), loader.alloc(128)
            loader.put(obj + 80, 'Q', vector)
            loader.put(vector, 'QQQ', buffer, buffer, buffer + 128)
            loader.put(obj + 264, 'ff', ix, iy)
            loader.call(ADDPOINT, [obj], [*case['point'], case['radius'], *case['direction'], case['transparency']])
            start, end, _capacity = loader.get(vector, 'QQQ')
            raw = bytes(loader.u.mem_read(start, end - start))
        expected = native_model(case)
        if expected is None:
            if raw:
                raise AssertionError(f"{case['name']}: zero-inverse guard unexpectedly emitted {len(raw)} bytes")
            words = []
        else:
            if len(raw) != 36:
                raise AssertionError(f"{case['name']}: native emitted {len(raw)} bytes, expected 36")
            words = [raw[i:i + 4].hex() for i in range(0, len(raw), 4)]
            modeled = [bits(value) for value in expected]
            if words != modeled:
                raise AssertionError(f"{case['name']}: model {modeled} != native {words}")
        result.append({'name': case['name'], 'kind': case['kind'],
                       'input_f32_le_hex_words': {key: [bits(value) for value in case[key]] if isinstance(case[key], list) else bits(case[key])
                                                   for key in ('point', 'radius', 'direction', 'transparency', 'inverse_xy')},
                       'native_record_f32_le_hex_words': words,
                       'emitted_bytes': len(raw),
                       'model_full_record_match': True})
    return result


def case_matrix() -> list[dict]:
    cases=[]
    for scale in (1, 2, 4):
        inverse=1.0/scale
        for radius in RADII:
            cases.append({'name':f'radius_{radius:g}_scale_{scale}','kind':'radius_scale_control' if radius > 0 else 'invalid_zero_radius_native_edge','point':[3.,5.],'radius':radius,'direction':[.6,.8],'transparency':1.,'inverse_xy':[inverse,inverse]})
    for i,direction in enumerate(((1.,0.),(0.,1.),(-.6,.8),(.6,-.8),(.6,.8))):
        cases.append({'name':f'direction_{i}','kind':'direction_control','point':[3.,5.],'radius':.1,'direction':list(direction),'transparency':1.,'inverse_xy':[1.,1.]})
    for i,(ix,iy) in enumerate(((1.,.5),(.5,1.),(2.,.25),(.25,2.))):
        cases.append({'name':f'anisotropic_{i}','kind':'anisotropic_inverse_scale_control','point':[3.,5.],'radius':.1,'direction':[.6,.8],'transparency':1.,'inverse_xy':[ix,iy]})
    for name,ix,iy in (('zero_inverse_x',0.,1.),('zero_inverse_y',1.,0.)):
        cases.append({'name':name,'kind':'invalid_zero_inverse_native_early_return','point':[3.,5.],'radius':.1,'direction':[.6,.8],'transparency':1.,'inverse_xy':[ix,iy]})
    return cases


def verify_existing_field_fixture() -> int:
    """Check the model's labels/order against every record in the older field fixture."""
    if hashlib.sha256(FIELD_FIXTURE.read_bytes()).hexdigest() != EXPECTED_FIELD_FIXTURE_SHA256:
        raise SystemExit('committed fountain-v14-field-native.json hash changed')
    fixture=json.loads(FIELD_FIXTURE.read_text())
    if fixture.get('schema') != 'sdocx-fountain-v14-field-native-v1':
        raise SystemExit('unexpected existing V14 field fixture schema')
    source_cases={case['name']:case for case in fixture['source_cases']}
    checked=0
    for capture in fixture['captures']:
        source=source_cases[capture['case']]
        scale=capture['scale']
        words=capture['native_AddPoint_record_f32_le_hex_words']
        if len(words) != len(source['stamps']):
            raise AssertionError(f"{capture['case']} scale {scale}: source/record count mismatch")
        inverse=1.0/scale
        for index,(stamp,raw_words) in enumerate(zip(source['stamps'],words)):
            model=native_model({'point':stamp['point'],'radius':stamp['radius'],
                                'direction':stamp['direction'],'transparency':1.0,
                                'inverse_xy':[inverse,inverse]})
            expected=[bits(value) for value in model]
            if raw_words != expected:
                raise AssertionError(f"existing field fixture {capture['case']} scale {scale} record {index}: model {expected} != fixture {raw_words}")
            checked+=1
    return checked


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture',action='store_true',help='explicitly write the current native replay')
    mode.add_argument('--check-fixture',action='store_true',help='replay and compare all fields against the fixture')
    args=parser.parse_args()
    cases=case_matrix()
    captures=run_native(cases)
    cross_fixture_records=verify_existing_field_fixture()
    value={'schema':'sdocx-fountain-v14-addpoint-native-v1',
           'provenance':{'library':'libSPenFountainPen.so','library_sha256':EXPECTED_DSO_SHA256,
                         'native_loader':'conformance/fountain_native.py','native_loader_sha256':EXPECTED_NATIVE_HELPER_SHA256,
                         'field_runner':'conformance/fountain_v14_field_native.py','field_runner_sha256':EXPECTED_FIELD_HELPER_SHA256,
                         'existing_field_fixture':'conformance/fountain-v14-field-native.json',
                         'existing_field_fixture_sha256':EXPECTED_FIELD_FIXTURE_SHA256,
                         'existing_field_fixture_model_records_verified':cross_fixture_records,
                         'symbol':ADDPOINT,'rva':hex(ADDPOINT_RVA),
                         'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
           'record_layout':LAYOUT,
           'numeric_model':'Independent scalar f32 model; each fdiv/fmul/fadd rounds to f32 in native instruction order. Invalid zero-radius controls preserve native raw edge behavior and do not claim production admission.',
           'host_boundaries':['RTV4::AddPoint runs from the hash-pinned ARM64 DSO through the tracked NativeFountain Unicorn loader. Inputs are direct native method arguments with explicit drawable vector storage and inverse-axis scale fields. This is not a full Samsung Notes Drawing caller capture.','Finite positive radius/scale controls represent direct AddPoint numeric behavior. Zero radius is an invalid rendering input captured only to document native edge behavior; zero inverse scales are early-return ABI controls.'],
           'cases':captures}
    if args.write_fixture:
        FIXTURE.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')
        print(f'wrote {len(captures)} RTV4 AddPoint cases to {FIXTURE.relative_to(ROOT)}')
    else:
        if not FIXTURE.exists(): raise SystemExit('fixture is missing; pass --write-fixture explicitly to create it')
        expected=json.loads(FIXTURE.read_text())
        if expected != value: raise SystemExit('native RTV4 AddPoint records/model differ from fixture')
        print(f'verified {len(captures)} native RTV4 AddPoint cases and all full-record model bits')

if __name__=='__main__': main()
