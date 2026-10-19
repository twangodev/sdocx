"""Replay one embedded real V14 saved stroke through native redraw, RTV4 AddPoint, and its GLSL field.

Requires the repository-pinned ARM64 APK libraries, Unicorn/binutils, and Playwright/Chromium.
The source stroke is an embedded subset extracted from handwritten.sdocx; replay has no corpus-file dependency.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'conformance/fountain-v14-real-native.json'
SAVED_RUNNER = ROOT / 'conformance/fountain_v14_native.py'
FIELD_RUNNER = ROOT / 'conformance/fountain_v14_field_native.py'
NATIVE_HELPER = ROOT / 'conformance/fountain_native.py'
FIELD_FIXTURE = ROOT / 'conformance/fountain-v14-field-native.json'
LIBRARY_DIR = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
OBJDUMP = ROOT / 'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin/aarch64-linux-gnu-objdump'

EXPECTED = {
    'saved_runner': '146ab3863ed9239cf8299d60759827dd346c2e9658e640efc2a56e848b33cbc1',
    'field_runner': '6891169bb01da9ad6442ee6e103b38b8670b9de9e22767aa006a6033b2a4f7c3',
    'native_helper': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
    'field_fixture': '4c1eb14ebae2561c4aceb1d618601a724c2e4b86f99ed4c8533b2cb0682fa3ff',
    'source_document': '77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3',
    'prepared_source': '902c7ed24ae68672051e3ccbbcf66ded545f51ac344eadbae4ba01d4442a78c6',
    'embedded_stroke': '34f782437360eaef824da92c4199702ed0372822d11f37597d2365762091bfba',
}
DIAGNOSTIC_ALPHA_SHA256 = {
    1: '2d1073ab063d7f1f5e0ad30d537efb8309e3e162da857bb85a8ce987ea329e0c',
    2: 'ddc60b07d2c0b41012e29f1e62ddc189be2508982f93184323b102c6de2a3f7c',
    4: '2e446fd326d46e84e4f50dfb96e6b75ec80882800d6254a11798beedab2506a1',
}
ANCHOR = [921.0, 1435.0]
SCALES = (1, 2, 4)


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical_hash(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def words_f32(records: list[list[float]]) -> list[list[str]]:
    return [[struct.pack('<f', value).hex() for value in record] for record in records]


def replay(fixture: dict) -> dict:
    for label, path_key in (('saved_runner', SAVED_RUNNER), ('field_runner', FIELD_RUNNER), ('native_helper', NATIVE_HELPER), ('field_fixture', FIELD_FIXTURE)):
        actual = sha(path_key)
        if actual != EXPECTED[label]:
            raise RuntimeError(f'{label} source changed: {actual}')
    if fixture.get('schema') != 'sdocx-fountain-v14-real-native-v1':
        raise ValueError('unexpected real-stroke fixture schema')
    source_provenance = fixture.get('source_provenance', {})
    if (source_provenance.get('application') != 'Samsung Notes 4.4.45.37 ARM64'
            or source_provenance.get('document') != 'tmp/stroke-conformance/handwritten.sdocx'
            or source_provenance.get('document_sha256') != EXPECTED['source_document']
            or source_provenance.get('prepared_capture') != 'tmp/vector-parity-plan/fountain-real-parity-next/handwritten-prepared.json'
            or source_provenance.get('prepared_capture_sha256') != EXPECTED['prepared_source']
            or source_provenance.get('prepared_entry_index') != 1445):
        raise ValueError('source document/prepared-capture lineage differs from pinned corpus evidence')
    source_stroke = fixture['source_stroke']
    rendering = source_stroke.get('rendering', {})
    if rendering.get('pen_name') != 'com.samsung.android.sdk.pen.pen.preload.FountainPen' or rendering.get('advanced_settings') != '14;' or rendering.get('tool_type_raw') != 2:
        raise ValueError('embedded input is not the pinned V14 FountainPen/tool-2 saved stroke')
    source_hash = canonical_hash(source_stroke)
    if source_hash != EXPECTED['embedded_stroke'] or source_hash != fixture['source_stroke_canonical_sha256']:
        raise ValueError('embedded saved ObjectStroke channels changed')

    sys.path.insert(0, str(ROOT / 'conformance'))
    sys.path.insert(0, str(ROOT / 'scratch/apk-analysis-runtime/python'))
    import fountain_v14_native
    import fountain_v14_field_native as field
    helper = fountain_v14_native.NativeFountain14(LIBRARY_DIR, OBJDUMP)
    saved = helper.render(source_stroke)
    dots = saved['dots']
    if len(source_stroke['points']) != 161 or len(dots) != 24 or len(saved['sample_ends']) != 161:
        raise AssertionError(f"unexpected saved V14 traversal: {len(source_stroke['points'])} inputs, {len(dots)} dots, {len(saved['sample_ends'])} sample boundaries")
    packed_dots = b''.join(struct.pack('<5f', *dot) for dot in dots)
    packed_dot_sha256 = hashlib.sha256(packed_dots).hexdigest()
    if fixture.get('native_saved_traversal', {}).get('packed_f32_sha256') not in (None, packed_dot_sha256):
        raise AssertionError('native saved V14 dot stream changed')

    field_fixture = json.loads(FIELD_FIXTURE.read_text())
    shaders = field_fixture['shader_sources']
    shader_source_evidence = field.verify_shader_sources(field_fixture)
    if field_fixture.get('shader_sources_pinned_dso_evidence') != shader_source_evidence:
        raise RuntimeError('pinned V4 shader C-string/constructor evidence changed')
    shader_native, fountain_module, _helper_hash = field.load_native()
    addpoint_runs = []
    result_runs = []
    shader_case = {
        'name': fixture['case_id'],
        'anchor': ANCHOR,
        'stamps': [{'point': dot[:2], 'radius': dot[2], 'direction': dot[3:5]} for dot in dots],
    }
    for scale in SCALES:
        records = [field.native_addpoint(shader_native,
                    {'point': dot[:2], 'radius': dot[2], 'direction': dot[3:5]}, scale)
                   for dot in dots]
        packed = b''.join(struct.pack('<9f', *record) for record in records)
        addpoint_runs.append({'case': fixture['case_id'], 'scale': scale, 'records': records})
        result_runs.append({
            'scale': scale,
            'record_count': len(records),
            'records_f32_le_hex_words': words_f32(records),
            'packed_records_sha256': hashlib.sha256(packed).hexdigest(),
        })

    # Reuse the existing shader/WebGL2 framework but center its viewport at the
    # diagnostic's documented representative document point rather than stamp 0.
    old_anchor = 'const scale = run.scale, anchor = c.stamps[0].point, quads = [];'
    if field.NODE_HARNESS.count(old_anchor) != 1:
        raise RuntimeError('existing shader harness anchor changed')
    field.NODE_HARNESS = field.NODE_HARNESS.replace(
        old_anchor, 'const scale = run.scale, anchor = c.anchor || c.stamps[0].point, quads = [];', 1)
    rendered = field.run_shader([shader_case], addpoint_runs, shaders)
    if len(rendered['captures']) != len(SCALES):
        raise AssertionError('shader capture count disagrees with requested scales')
    captures = []
    for run, pixels in zip(result_runs, rendered['captures']):
        scale = run['scale']
        if pixels['case'] != fixture['case_id'] or pixels['scale'] != scale:
            raise AssertionError('shader capture order disagrees with native AddPoint scales')
        if pixels['gl_error'] != 0:
            raise AssertionError(f"host shader GL error at scale {scale}: {pixels['gl_error']}")
        if pixels['max_shader_vs_scalar_byte_error'] > 1:
            raise AssertionError(f"native shader/scalar mismatch at scale {scale}: {pixels['max_shader_vs_scalar_byte_error']}")
        rgba, scalar = bytes(pixels['rgba8']), bytes(pixels['scalar_rgba8'])
        if len(rgba) != 64 * 64 * 4 or len(scalar) != 64 * 64 * 4:
            raise AssertionError('expected complete 64x64 RGBA8 shader readbacks')
        alpha = rgba[3::4]
        if hashlib.sha256(alpha).hexdigest() != DIAGNOSTIC_ALPHA_SHA256[scale]:
            raise AssertionError(f'shader field differs from the prior global diagnostic at scale {scale}')
        captures.append({
            **run,
            'shader_rgba8': field.compressed_payload(rgba),
            'scalar_rgba8': field.compressed_payload(scalar),
            'max_shader_vs_scalar_byte_error': pixels['max_shader_vs_scalar_byte_error'],
            'mean_shader_vs_scalar_byte_error': pixels['mean_shader_vs_scalar_byte_error'],
            'active_shader_vs_scalar_mean_abs_error': pixels['active_shader_vs_scalar_mean_abs_error'],
            'gl_error': pixels['gl_error'],
        })
    hashes = {name: fountain_module.HASHES[name] for name in ('Drawing', 'Graphics', 'Engine', 'FountainPen', 'PenCommon', 'Base', 'Renderer')}
    return {
        'source_stroke_canonical_sha256': canonical_hash(source_stroke),
        'source_channels': {name: len(source_stroke[name]) for name in ('points', 'pressures', 'tilts', 'orientations', 'timestamps')},
        'source_identity': {'pen_name': rendering['pen_name'], 'advanced_settings': rendering['advanced_settings'], 'tool_type_raw': rendering['tool_type_raw'], 'pen_width': source_stroke['pen_width'], 'initial_tolerance': rendering['style']['initial_tolerance'], 'fixed_width': rendering['properties']['fixed_width']},
        'native_saved_traversal': {
            'method': 'SPen::FountainPenStrokeDrawableGLV14::redraw(ObjectStroke const*, RectF*, bool)',
            'input_point_count': len(source_stroke['points']),
            'sample_boundaries': saved['sample_ends'],
            'dot_count': len(dots),
            'dots_f32_le_hex_words': words_f32(dots),
            'packed_f32_sha256': hashlib.sha256(packed_dots).hexdigest(),
        },
        'native_addpoint': result_runs,
        'shader_product_field': {
            'frame_anchor_document_units': ANCHOR,
            'shader_source_dso_evidence': shader_source_evidence,
            'diagnostic_alpha_sha256_by_scale': {str(k): v for k, v in DIAGNOSTIC_ALPHA_SHA256.items()},
            'viewport_px': [64, 64],
            'scales_px_per_document_unit': list(SCALES),
            'host_composition': 'Explicit GL_MAX / ONE, ONE; white inputColor; executed shader source pinned to V4 DSO symbols. Native ObjectDrawing and final paint/composition are not executed.',
            'captures': captures,
            'host_gl': rendered['host'],
            'host_toolchain': field.host_toolchain(rendered),
        },
        'native_library_sha256': hashes,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true', help='explicitly replay and replace captures')
    mode.add_argument('--check-fixture', action='store_true', help='replay and verify the fixture')
    args = parser.parse_args()
    fixture = json.loads(FIXTURE.read_text())
    actual = replay(fixture)
    if args.write_fixture:
        fixture.update(actual)
        fixture['runner_sha256'] = sha(Path(__file__))
        FIXTURE.write_text(json.dumps(fixture, indent=2, sort_keys=True) + '\n')
        print('wrote ' + str(FIXTURE.relative_to(ROOT)))
    else:
        expected = {key: fixture[key] for key in ('source_stroke_canonical_sha256', 'source_channels', 'source_identity', 'native_saved_traversal', 'native_addpoint', 'shader_product_field', 'native_library_sha256')}
        if actual != expected:
            raise SystemExit('real V14 saved traversal/shader field fixture mismatch')
        if fixture.get('runner_sha256') != sha(Path(__file__)):
            raise SystemExit('runner hash differs from fixture')
        print('verified one real V14 saved stroke: 161 source points, 24 native dots, 3 shader scales')


if __name__ == '__main__':
    main()
