"""Replay low-radius V14 Fountain AddPoint fields from pinned native code.

This is a synthetic native-input oracle for display-scale fringe behavior, not
an Android-captured stroke or a device-GPU pixel claim.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-v14-fringe-native.json'
FIELD_RUNNER = ROOT / 'conformance/fountain_v14_field_native.py'
FIELD_FIXTURE = ROOT / 'conformance/fountain-v14-field-native.json'
EXPECTED_FIELD_RUNNER_SHA256 = '6891169bb01da9ad6442ee6e103b38b8670b9de9e22767aa006a6033b2a4f7c3'
EXPECTED_FIELD_FIXTURE_SHA256 = '4c1eb14ebae2561c4aceb1d618601a724c2e4b86f99ed4c8533b2cb0682fa3ff'
EXPECTED_FOUNTAIN_HELPER_SHA256 = '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30'
EXPECTED_DSO_SHA256 = {
    'FountainPen': '1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec',
    'PenCommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
    'Base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
}
EXPECTED_ADDPOINT_RVA = 0x9de50
ANCHOR_TOKEN = 'const scale = run.scale, anchor = c.stamps[0].point, quads = [];'
ANCHOR_REPLACEMENT = 'const scale = run.scale, anchor = [32,32], quads = [];'
RADII = (0.1, 0.499, 0.5, 0.501, 0.75)
DIRECTIONS = ((1.0, 0.0), (0.0, 1.0), (0.7071067811865476, 0.7071067811865476))
POSITIONS = ((32.0, 32.0), (32.125, 32.25), (32.5, 32.5), (32.875, 32.75))
SCALES = (1, 2, 4)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def close(a: float, b: float) -> bool:
    return abs(a - b) < 5e-6


def dependencies():
    if sha256(FIELD_RUNNER.read_bytes()) != EXPECTED_FIELD_RUNNER_SHA256:
        raise RuntimeError('pinned Fountain field runner source changed')
    if sha256(FIELD_FIXTURE.read_bytes()) != EXPECTED_FIELD_FIXTURE_SHA256:
        raise RuntimeError('pinned Fountain field fixture changed')
    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_v14_field_native as field_oracle
    field_fixture = json.loads(FIELD_FIXTURE.read_text())
    if field_fixture.get('runner_sha256') != EXPECTED_FIELD_RUNNER_SHA256:
        raise RuntimeError('field fixture runner pin does not match')
    if field_fixture.get('helper_sha256', {}).get('conformance/fountain_native.py') != EXPECTED_FOUNTAIN_HELPER_SHA256:
        raise RuntimeError('field fixture native helper pin does not match')
    for name, expected in EXPECTED_DSO_SHA256.items():
        path = ROOT / 'scratch/apk-analysis-native/arm64-v8a' / f'libSPen{name}.so'
        if sha256(path.read_bytes()) != expected:
            raise RuntimeError(f'pinned {name} DSO changed')
    native, helper_module, helper_hash = field_oracle.load_native()
    if helper_hash != EXPECTED_FOUNTAIN_HELPER_SHA256:
        raise RuntimeError('NativeFountain helper hash changed')
    if native.syms.get(field_oracle.ADDPOINT) != EXPECTED_ADDPOINT_RVA:
        raise RuntimeError('RTV4::AddPoint symbol RVA changed')
    shader_evidence = field_oracle.verify_shader_sources(field_fixture)
    if field_fixture.get('shader_sources_pinned_dso_evidence') != shader_evidence:
        raise RuntimeError('pinned V4 shader C strings or constructor route changed')
    old = field_oracle.NODE_HARNESS
    if old.count(ANCHOR_TOKEN) != 1:
        raise RuntimeError('expected exactly one source anchor token in pinned field runner')
    field_oracle.NODE_HARNESS = old.replace(ANCHOR_TOKEN, ANCHOR_REPLACEMENT)
    return field_oracle, field_fixture, native, shader_evidence


def capture() -> dict:
    oracle, field_fixture, native, shader_evidence = dependencies()
    cases, casespec, runs = [], [], []
    for ri, radius in enumerate(RADII):
        for di, direction in enumerate(DIRECTIONS):
            for pi, point in enumerate(POSITIONS):
                name = f'r{ri}_d{di}_p{pi}'
                stamp = {'point': list(point), 'radius': radius, 'direction': list(direction)}
                cases.append({'name': name, 'stamps': [stamp]})
                casespec.append({'name': name, **stamp, 'fixed_frame_anchor': [32, 32]})
                for scale in SCALES:
                    record = oracle.native_addpoint(native, stamp, scale)
                    packed = struct.pack('<9f', *record)
                    radius_px = radius * scale
                    expected_extent_px = max(radius_px + 0.5, 1.0)
                    expected_alpha_by_width = min(2 * radius_px, 1.0)
                    expected_inner_px = max(radius_px - 0.5, 0.0)
                    extent_px = record[2] * scale
                    inner_px = 2 * record[7] * record[2] * scale
                    if not close(extent_px, expected_extent_px):
                        raise AssertionError(f'{name} scale {scale}: native outer extent changed')
                    if not close(record[8], expected_alpha_by_width):
                        raise AssertionError(f'{name} scale {scale}: native alphaByWidth changed')
                    if not close(inner_px, expected_inner_px):
                        raise AssertionError(f'{name} scale {scale}: native inner radius changed')
                    if not close(extent_px - inner_px, 1.0):
                        raise AssertionError(f'{name} scale {scale}: native radial transition is not 1px')
                    runs.append({
                        'case': name,
                        'scale': scale,
                        'records': [record],
                        'packed_records_sha256': sha256(packed),
                        'radial_extent_diagnostics': [{
                            'outer_extent_device_px': extent_px,
                            'inner_radius_device_px': inner_px,
                            'radial_transition_width_device_px': extent_px - inner_px,
                        }],
                    })

    rendered = oracle.run_shader(cases, runs, field_fixture['shader_sources'])
    if len(rendered['captures']) != len(runs):
        raise AssertionError('SwiftShader omitted a synthetic radius/scale run')
    rows = []
    phase_hashes: dict[tuple[str, str, int], set[str]] = {}
    for run, pixels in zip(runs, rendered['captures']):
        if (run['case'], run['scale']) != (pixels['case'], pixels['scale']):
            raise AssertionError('SwiftShader capture order differs from input order')
        if pixels['max_shader_vs_scalar_byte_error'] > 1:
            raise AssertionError(f"{run['case']} scale {run['scale']} shader/scalar mismatch")
        rgba = bytes(pixels['rgba8'])
        scalar_rgba = bytes(pixels['scalar_rgba8'])
        if len(rgba) != 64 * 64 * 4 or len(scalar_rgba) != 64 * 64 * 4:
            raise AssertionError('full 64x64 RGBA8 readback missing')
        alpha = rgba[3::4]
        active = [i for i, value in enumerate(alpha) if value]
        case = next(item for item in casespec if item['name'] == run['case'])
        point = case['point']
        center = [32 + (point[0] - 32) * run['scale'], 32 - (point[1] - 32) * run['scale']]
        record = run['records'][0]
        rows.append({
            'case': run['case'],
            'scale': run['scale'],
            'source_point_document_units': point,
            'fixed_frame_anchor_document_units': [32, 32],
            'rendered_center_device_px': center,
            'record_f32_le_hex_words': [struct.pack('<f', value).hex() for value in record],
            'record_f32_values': record,
            'packed_record_sha256': run['packed_records_sha256'],
            'radial_extent_diagnostics': run['radial_extent_diagnostics'][0],
            'shader_vs_scalar_max_byte_error': pixels['max_shader_vs_scalar_byte_error'],
            'active_pixel_count': len(active),
            'active_pixel_bbox': [min(i % 64 for i in active), min(i // 64 for i in active), max(i % 64 for i in active), max(i // 64 for i in active)] if active else None,
            'alpha8_byte_count': len(alpha),
            'alpha8_sha256': sha256(alpha),
            'alpha8_zlib_base64': base64.b64encode(zlib.compress(alpha, 9)).decode('ascii'),
        })
        radius_case, direction_case, _ = run['case'].split('_')
        phase_hashes.setdefault((radius_case, direction_case, run['scale']), set()).add(sha256(alpha))
    phase_uniqueness = [
        {'radius_case': key[0], 'direction_case': key[1], 'scale': key[2], 'distinct_phase_outputs': len(values)}
        for key, values in sorted(phase_hashes.items())
    ]
    if len(phase_uniqueness) != len(RADII) * len(DIRECTIONS) * len(SCALES):
        raise AssertionError('subpixel phase group count is incomplete')
    if any(item['distinct_phase_outputs'] != len(POSITIONS) for item in phase_uniqueness):
        raise AssertionError('fixed-frame subpixel positions did not produce distinct raster outputs')
    if any(row['shader_vs_scalar_max_byte_error'] > 1 for row in rows):
        raise AssertionError('shader/scalar tolerance exceeded')
    return {
        'schema': 'sdocx-fountain-v14-fringe-native-v1',
        'input_profile': {
            'kind': 'explicit synthetic native AddPoint inputs',
            'radii_document_units': list(RADII),
            'directions': [list(value) for value in DIRECTIONS],
            'subpixel_positions_document_units': [list(value) for value in POSITIONS],
            'fixed_frame_anchor_document_units': [32, 32],
            'scales_px_per_document_unit': list(SCALES),
            'case_count': len(cases),
            'run_count': len(rows),
        },
        'provenance': {
            'pinned_field_runner_sha256': EXPECTED_FIELD_RUNNER_SHA256,
            'pinned_field_fixture_sha256': EXPECTED_FIELD_FIXTURE_SHA256,
            'pinned_fountain_native_helper_sha256': EXPECTED_FOUNTAIN_HELPER_SHA256,
            'native_addpoint_rva': hex(EXPECTED_ADDPOINT_RVA),
            'native_library_sha256': EXPECTED_DSO_SHA256,
            'shader_cstrings_and_constructor_route': shader_evidence,
            'field_runner_anchor_patch': {
                'source_token': ANCHOR_TOKEN,
                'replacement': ANCHOR_REPLACEMENT,
                'assertion': 'exactly one source token replaced; original field runner source SHA pinned above',
            },
        },
        'host_profile': {
            'webgl': rendered['host'],
            'toolchain': oracle.host_toolchain(rendered),
        },
        'phase_uniqueness': phase_uniqueness,
        'cases': casespec,
        'runs': rows,
        'host_boundaries': [
            'Native RTV4::AddPoint executes from the hash-pinned ARM64 DSOs through the tracked fountain_native Unicorn loader; input radius/point/direction triples are explicit synthetic values, not captured Samsung strokes.',
            'Recovered V4 GLSL runs in Chromium SwiftShader WebGL2 with GL_MAX and ONE/ONE, white inputColor, 64x64 target, and fixed [32,32] document frame anchor. This is not Samsung device GLES or a device pixel capture.',
            'The existing pinned field runner is imported by SHA and receives one asserted fixed-anchor token replacement so the four source subpixel phases do not recenter onto their own stamp.',
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--write-fixture', action='store_true', help='explicitly write fresh native captures')
    modes.add_argument('--check-fixture', action='store_true', help='replay and compare with fixture (default)')
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    args = parser.parse_args()
    actual = capture()
    runner_hash = sha256(Path(__file__).read_bytes())
    actual['runner_sha256'] = runner_hash
    if args.write_fixture:
        args.fixture.write_text(json.dumps(actual, indent=2) + '\n')
        mode = 'write'
    else:
        expected = json.loads(args.fixture.read_text())
        if expected != actual:
            raise SystemExit('native AddPoint records or SwiftShader readbacks differ from fixture')
        mode = 'check'
    print(json.dumps({
        'mode': mode,
        'cases': actual['input_profile']['case_count'],
        'runs': actual['input_profile']['run_count'],
        'max_shader_vs_scalar_error': max(row['shader_vs_scalar_max_byte_error'] for row in actual['runs']),
        'phase_groups': len(actual['phase_uniqueness']),
        'all_phases_distinct': all(row['distinct_phase_outputs'] == len(POSITIONS) for row in actual['phase_uniqueness']),
        'runner_sha256': runner_hash,
    }, indent=2))


if __name__ == '__main__':
    main()
