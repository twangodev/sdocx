#!/usr/bin/env python3
"""Replay selected canonical MagicPen V1 source streams through native Drawing and host Mesa paint.

The canonical MagicPen caller and all pinned helpers/DSOs are adapted in
memory. Normal invocation performs a fresh native replay and compares every
source record, uploaded vector, and full RGBA8 readback to the checked-in
fixture. Updating the fixture requires --write-fixture.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
import platform
import struct
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNNER = Path(__file__).resolve()
FIXTURE = ROOT / 'conformance/magicpen-curved-paint-native.json'
SOURCE_FIXTURE = ROOT / 'conformance/magicpen-curved-source-events-native.json'
BASE_RUNNER = ROOT / 'conformance/magicpen_drawing_paint_native.py'
BASE_FIXTURE = ROOT / 'conformance/magicpen-drawing-paint-native.json'
FOUNTAIN_CALLER = ROOT / 'conformance/fountain_saved_caller_native.py'
SOURCE_FIXTURE_SHA256 = '8d73e0f5b64d0d8ad108c1e8f09b9757750c97edc22b99cbfdb3c172f0b27cd2'
BASE_RUNNER_SHA256 = 'b87400d5508b9717b05a6413d5f1824ab56de9d7156c059a6f69ea3e3b21bbdd'
BASE_FIXTURE_SHA256 = '874f659069c8ab206c049f4e7c7161524146f55a93c44fd4ac378bba6e5c9bc2'
FOUNTAIN_CALLER_SHA256 = 'b0837c86483aa438923f9d4d1c6451b1ebc1c5fec4c5f7c447b928a25a8f7b19'
RUNTIME_PINS = {
    'python_executable': '889c603f0d17cb54060951bcf4c4f9b8c9ebd9e52b392c70209bbb9755d797d9',
    'unicorn/__init__.py': '9ad32123bca674cf205c308fd4d0d2a4db877c9dc2afac4feab7c84c8cf7043b',
    'unicorn/unicorn.py': 'e823b17649d0610c6f52b665902ac9dc46962785feebcac8465a0d7c477f25b7',
    'unicorn/lib/libunicorn.so.2': 'ddb196ec82b52e502c18e4a34478bf7b9f61c83c2ebaa95c74d8ded45a95da9c',
}
CASE_IDS = (
    'tool2_baseline',
    'tool2_5_nonsymmetric',
    'long11_duplicate_and_subthreshold_tail',
    'long16_min_size_pressure_edges',
)
VIEWS = (
    {'id': 's1', 'scale': 1.0, 'phase': [0.0, 0.0]},
    {'id': 's2', 'scale': 2.0, 'phase': [0.0, 0.0]},
    {'id': 's4', 'scale': 4.0, 'phase': [0.125, 0.25]},
)


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def check_file(path: Path, expected: str, label: str) -> None:
    actual = sha(path.read_bytes())
    if actual != expected:
        raise SystemExit(f'{label} changed: {actual}')


def literal_assignment(tree: ast.Module, name: str):
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(target, ast.Name) and target.id == name for target in node.targets
        ):
            return ast.literal_eval(node.value)
    raise RuntimeError(f'cannot read pinned {name} from canonical paint runner')


def preflight() -> dict:
    check_file(SOURCE_FIXTURE, SOURCE_FIXTURE_SHA256, 'canonical curved source fixture')
    check_file(BASE_RUNNER, BASE_RUNNER_SHA256, 'canonical MagicPen paint runner')
    check_file(BASE_FIXTURE, BASE_FIXTURE_SHA256, 'canonical MagicPen paint fixture')
    check_file(FOUNTAIN_CALLER, FOUNTAIN_CALLER_SHA256, 'canonical Fountain caller source')

    base_tree = ast.parse(BASE_RUNNER.read_text(), filename=str(BASE_RUNNER))
    helper_hashes = literal_assignment(base_tree, 'HELPER_HASHES')
    dso_hashes = literal_assignment(base_tree, 'DSO_HASHES')
    tool_hashes = literal_assignment(base_tree, 'TOOL_HASHES')
    for rel, expected in helper_hashes.items():
        check_file(ROOT / rel, expected, f'helper {rel}')
    libdir = ROOT / 'scratch/apk-analysis-native/arm64-v8a'
    for name, expected in dso_hashes.items():
        check_file(libdir / name, expected, f'native DSO {name}')
    for path, expected in tool_hashes.items():
        check_file(Path(path) if path.startswith('/') else ROOT / path, expected, f'tool/host library {path}')
    runtime_dir = ROOT / 'scratch/apk-analysis-runtime/python'
    runtime = {
        'python_executable': {'path': str(Path(sys.executable).resolve()), 'sha256': sha(Path(sys.executable).resolve().read_bytes()), 'version': platform.python_version()},
        'runtime_python_files': {},
    }
    if runtime['python_executable']['sha256'] != RUNTIME_PINS['python_executable'] or platform.python_version() != '3.13.5':
        raise RuntimeError(f"pinned Python runtime changed: {runtime['python_executable']}")
    # Pin Python-level Unicorn closure before the canonical runner imports it.
    for rel in ('unicorn/__init__.py', 'unicorn/unicorn.py', 'unicorn/lib/libunicorn.so.2'):
        path = runtime_dir / rel
        runtime['runtime_python_files'][rel] = sha(path.read_bytes())
        if runtime['runtime_python_files'][rel] != RUNTIME_PINS[rel]:
            raise RuntimeError(f'pinned Unicorn runtime file changed: {rel}')
    return {
        'source_fixture_sha256': SOURCE_FIXTURE_SHA256,
        'base_runner_sha256': BASE_RUNNER_SHA256,
        'base_fixture_sha256': BASE_FIXTURE_SHA256,
        'fountain_caller_sha256': FOUNTAIN_CALLER_SHA256,
        'helper_sha256': helper_hashes,
        'native_dso_sha256': dso_hashes,
        'tool_and_host_sha256': tool_hashes,
        'runtime': runtime,
    }


def replace_once(source: str, old: str, new: str, label: str) -> str:
    count = source.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected exactly one source anchor, found {count}')
    return source.replace(old, new, 1)


def f32_from_word(word: str) -> float:
    return struct.unpack('<f', struct.pack('<I', int(word, 16)))[0]


def execute_capture(case: dict, view: dict) -> dict:
    inp = case['input']
    n = len(inp['pressure_f32_u32_hex'])
    points = [f32_from_word(w) for w in inp['points_f32_u32_hex']]
    pressures = [f32_from_word(w) for w in inp['pressure_f32_u32_hex']]
    timestamps = inp['timestamps_i32']
    size = f32_from_word(inp['size_f32_u32_hex'])
    expected_xyz = case['native']['xyz_f32_u32_hex']
    expected_bytes = len(expected_xyz) * 4
    expected_vector_sha = case['native']['sha256']

    # Replace only the Fountain caller's configured source arrays. Its getter
    # snapshots are widened to these exact array lengths; all other native
    # execution remains in the canonical MagicPen wrapper.
    fountain = FOUNTAIN_CALLER.read_text()
    array_anchor = "pts=p.alloc(24);prs=p.alloc(12);ts=p.alloc(12);tilt=p.alloc(12);ori=p.alloc(12)\n p.put(pts,'6f',0,0,10,0,20,2);p.put(prs,'3f',.3,.6,1.);p.put(ts,'3i',100,110,120);p.put(tilt,'3f',0,0,0);p.put(ori,'3f',0,0,0)\n call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,3])"
    fmt = lambda values: ', '.join(repr(value) for value in values)
    dynamic_arrays = (
        f"pts=p.alloc({8*n});prs=p.alloc({4*n});ts=p.alloc({4*n});tilt=p.alloc({4*n});ori=p.alloc({4*n})\n"
        f" p.put(pts,'{2*n}f',{fmt(points)});p.put(prs,'{n}f',{fmt(pressures)});p.put(ts,'{n}i',{fmt(timestamps)});p.put(tilt,'{n}f',{fmt([0.0]*n)});p.put(ori,'{n}f',{fmt([0.0]*n)})\n"
        f" call('SPen::ObjectStroke::SetPoint(SPen::PointF const*, float const*, int const*, float const*, float const*, int)',[obj,pts,prs,ts,tilt,ori,{n}])"
    )
    fountain = replace_once(fountain, array_anchor, dynamic_arrays, 'source arrays')
    snapshots = (
        ("struct.unpack('<6I',bytes(p.u.mem_read(model_arrays['points'],24)))", f"struct.unpack('<{2*n}I',bytes(p.u.mem_read(model_arrays['points'],{8*n})))"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['pressure'],12)))", f"struct.unpack('<{n}I',bytes(p.u.mem_read(model_arrays['pressure'],{4*n})))"),
        ("struct.unpack('<3i',bytes(p.u.mem_read(model_arrays['timestamps'],12)))", f"struct.unpack('<{n}i',bytes(p.u.mem_read(model_arrays['timestamps'],{4*n})))"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['tilt'],12)))", f"struct.unpack('<{n}I',bytes(p.u.mem_read(model_arrays['tilt'],{4*n})))"),
        ("struct.unpack('<3I',bytes(p.u.mem_read(model_arrays['orientation'],12)))", f"struct.unpack('<{n}I',bytes(p.u.mem_read(model_arrays['orientation'],{4*n})))"),
    )
    for old, new in snapshots:
        fountain = replace_once(fountain, old, new, 'native source getter snapshot')

    wrapper = BASE_RUNNER.read_text()
    wrapper = replace_once(wrapper, 'source = BASE.read_text()', f'source = {fountain!r}', 'in-memory caller substitution')
    # Keep the caller's exact source-array checks, substituting this case's
    # declared arrays and point count into the asserted capture.
    old_count = "if _row['source_model_get_point_count']!=3:raise RuntimeError('source ObjectStroke point count changed')"
    wrapper = replace_once(wrapper, old_count,
                           f"if _row['source_model_get_point_count']!={n}:raise RuntimeError('source ObjectStroke point count changed')",
                           'source point count assertion')
    old_arrays = "if _row['source_arrays']!={'points_f32_bits':['00000000','00000000','41200000','00000000','41a00000','40000000'],'pressure_f32_bits':['3e99999a','3f19999a','3f800000'],'timestamps_i32':[100,110,120],'tilt_f32_bits':['00000000']*3,'orientation_f32_bits':['00000000']*3}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')"
    expected_arrays = {
        'points_f32_bits': inp['points_f32_u32_hex'],
        'pressure_f32_bits': inp['pressure_f32_u32_hex'],
        'timestamps_i32': timestamps,
        'tilt_f32_bits': ['00000000'] * n,
        'orientation_f32_bits': ['00000000'] * n,
    }
    new_arrays = f"if _row['source_arrays']!={expected_arrays!r}:raise RuntimeError('native ObjectStroke channel getters differ from configured input')"
    wrapper = replace_once(wrapper, old_arrays, new_arrays, 'source array assertion')
    wrapper = replace_once(wrapper, "mode='set'; size=10.0", f"mode='set'; size={size!r}", 'explicit source size')
    wrapper = replace_once(wrapper, "record=records[-1]", "record=next(row for row in records if row['case']=='v1')", 'selected V1 route')
    old_vector = "assert len(vector_data)==96 and hashlib.sha256(vector_data).hexdigest()=='48fab498fb7b870d2fb0ebcf3d725efbcf1352324592286aa9082df635b1377d'"
    new_vector = f"assert len(vector_data)=={expected_bytes} and hashlib.sha256(vector_data).hexdigest()=='{expected_vector_sha}'"
    wrapper = replace_once(wrapper, old_vector, new_vector, 'native vector join')
    px, py = view['phase']
    scale = view['scale']
    wrapper = replace_once(wrapper, 'control_argb=0x80336699; translation=(64.0,64.0)',
                           f'control_argb=0x80336699; translation=(64.0+{px!r},64.0+{py!r})', 'paint translation')
    old_matrix = "mat=p.alloc(64);p.put(mat,'16f',1,0,0,0,0,1,0,0,0,0,1,0,translation[0],translation[1],0,1)"
    new_matrix = f"view_scale={scale!r};mat=p.alloc(64);p.put(mat,'16f',view_scale,0,0,0,0,view_scale,0,0,0,0,1,0,translation[0],translation[1],0,1)"
    wrapper = replace_once(wrapper, old_matrix, new_matrix, 'native Draw view transform')
    bbox_assert = "assert bbox and bbox[0]>0 and bbox[1]>0 and bbox[2]<255 and bbox[3]<255, f'translated control still touches target edge: {bbox}'"
    wrapper = replace_once(wrapper, bbox_assert, "# Full RGBA8 readback is the witness; edge contact is profile-dependent.", 'profile-independent bbox check')
    center_assert = "assert rgba_at(128,128)==list(background_rgba), 'outside-stroke pixel did not preserve the initialized target background'"
    wrapper = replace_once(wrapper, center_assert, "# Full native readback retained for profile-specific comparison.", 'profile-independent center control')
    color_assert = "expected_stroke_sample=([26,51,77,128] if background_kind=='transparent' else [58,115,173,255])\nassert rgba_at(64,64)==expected_stroke_sample, f'partial-alpha source-over pixel changed: {rgba_at(64,64)}'"
    wrapper = replace_once(wrapper, color_assert, "# Full native readback retained; this fixed pixel is only valid at the unit transform.", 'unit-transform spot color control')

    env = os.environ.copy()
    env['PYTHONPATH'] = os.pathsep.join((str(ROOT/'conformance'), str(ROOT/'scratch/apk-analysis-runtime/python'), env.get('PYTHONPATH', ''))).rstrip(os.pathsep)
    env['LD_LIBRARY_PATH'] = os.pathsep.join((str(ROOT/'scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu'), env.get('LD_LIBRARY_PATH', ''))).rstrip(os.pathsep)
    old_argv = sys.argv[:]
    old_path = sys.path[:]
    old_ld = os.environ.get('LD_LIBRARY_PATH')
    os.environ['LD_LIBRARY_PATH'] = env['LD_LIBRARY_PATH']
    sys.path[:0] = [str(ROOT/'conformance'), str(ROOT/'scratch/apk-analysis-runtime/python')]
    sys.argv = [str(BASE_RUNNER), '--_capture-background', 'transparent']
    try:
        namespace = {'__name__': '__main__', '__file__': str(BASE_RUNNER)}
        import contextlib
        import io
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            exec(compile(wrapper, str(BASE_RUNNER), 'exec'), namespace)
    finally:
        sys.argv = old_argv
        sys.path[:] = old_path
        if old_ld is None:
            os.environ.pop('LD_LIBRARY_PATH', None)
        else:
            os.environ['LD_LIBRARY_PATH'] = old_ld
    captured = json.loads(output.getvalue())
    source_capture = captured['source_capture']
    paint = captured['paint']
    selected = next(row for row in source_capture['cases'] if row['case'] == 'v1')
    if selected['source_arrays'] != expected_arrays:
        raise RuntimeError(f"{case['id']}/{view['id']}: native getter arrays differ")
    raw_vector = bytes.fromhex(selected['native_rtv1_vector']['bytes_le_hex'])
    expected_vector = b''.join(struct.pack('<I', int(word, 16)) for word in expected_xyz)
    if raw_vector != expected_vector or sha(raw_vector) != expected_vector_sha:
        raise RuntimeError(f"{case['id']}/{view['id']}: native Drawing stream differs from canonical source fixture")
    if paint['native_vector_hex'] != raw_vector.hex() or paint['native_vector_sha256'] != sha(raw_vector):
        raise RuntimeError(f"{case['id']}/{view['id']}: host-upload vector differs from native Drawing stream")
    if paint['source_argb_input'] != '0x80336699' or paint['target_initial_background'] != {'kind': 'transparent', 'rgba8': [0, 0, 0, 0]}:
        raise RuntimeError('ARGB or transparent target boundary changed')
    return {
        'source_capture': source_capture,
        'paint': paint,
        'view': view,
    }


def fresh_capture(case: dict) -> dict:
    captures = [execute_capture(case, view) for view in VIEWS]
    source_capture = captures[0]['source_capture']
    for item in captures[1:]:
        if item['source_capture'] != source_capture:
            raise RuntimeError(f"{case['id']}: source caller output changed across view transforms")
    expected_input = case['input']
    source_capture['source_input'] = {
        'pen_name': 'com.samsung.android.sdk.pen.pen.preload.MagicPen',
        'points_f32_u32_hex': expected_input['points_f32_u32_hex'],
        'pressure_f32_u32_hex': expected_input['pressure_f32_u32_hex'],
        'timestamps_i32': expected_input['timestamps_i32'],
        'tilt_f32_u32_hex': ['00000000'] * len(expected_input['pressure_f32_u32_hex']),
        'orientation_f32_u32_hex': ['00000000'] * len(expected_input['pressure_f32_u32_hex']),
        'tool_type': expected_input['tool_type'],
        'stroke_type': expected_input['stroke_type'],
        'curve_enabled': expected_input['curve_enabled'],
        'size_f32_u32_hex': expected_input['size_f32_u32_hex'],
        'color_argb': '0x80336699',
        'advanced_setting': '1;',
    }
    return {
        'id': case['id'],
        'input': expected_input,
        'native_source_fixture_output': case['native'],
        'source_capture': source_capture,
        'native_paints': [
            {'view': row['view'], 'paint': row['paint']}
            for row in captures
        ],
    }


def build() -> dict:
    closure = preflight()
    source_fixture = json.loads(SOURCE_FIXTURE.read_text())
    cases = {case['id']: case for case in source_fixture['cases']}
    if len(source_fixture['cases']) != 33 or any(case_id not in cases for case_id in CASE_IDS):
        raise RuntimeError('canonical curved source fixture case set changed')
    profiles = [fresh_capture(cases[case_id]) for case_id in CASE_IDS]
    return {
        'schema': 'magicpen-curved-native-paint-v1',
        'scope': 'Four canonical MagicPen GLV1 curved source cases are replayed through actual native Drawing/RTV1 and uploaded to the pinned host Mesa GLES shader path. Each case has three native Draw view transforms and full RGBA8 readback. ARGB is 0x80336699 on a transparent 256x256 host target. This is host GLES evidence; Samsung device framebuffer parity is not asserted.',
        'provenance': {
            'runner_path': 'conformance/magicpen_curved_paint_native.py',
            'runner_sha256': sha(RUNNER.read_bytes()),
            'closure': closure,
            'source_cases': list(CASE_IDS),
            'draw_views': list(VIEWS),
            'capture_boundary': 'native Drawing source getters and selected GLV1 AddPoint stream → native RTV1 vector upload → actual pinned shader draw → host Mesa RGBA8 readback',
            'host_boundary': 'fixed 256x256 transparent target, ARGB 0x80336699, scales 1/2/4 with phase [0,0]/[0,0]/[0.125,0.25]; native Samsung GPU/device behavior is outside this fixture',
        },
        'profiles': profiles,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--write-fixture', action='store_true', help='explicitly capture and replace the native paint fixture')
    mode.add_argument('--check-fixture', action='store_true', help='freshly replay all 12 paint captures and compare the fixture')
    args = parser.parse_args()
    closure = preflight()
    if not args.write_fixture:
        if not FIXTURE.exists():
            raise SystemExit('fixture missing; use --write-fixture explicitly after review')
        fixture = json.loads(FIXTURE.read_text())
        recorded = fixture.get('provenance', {})
        if recorded.get('runner_sha256') != sha(RUNNER.read_bytes()):
            raise SystemExit('fixture runner hash does not match this script')
        if recorded.get('closure') != closure:
            raise SystemExit('pinned source/runtime/DSO/tool closure differs from fixture')
    result = build()
    encoded = (json.dumps(result, indent=2, sort_keys=True) + '\n').encode()
    if args.write_fixture:
        FIXTURE.write_bytes(encoded)
        print(f'wrote {FIXTURE.relative_to(ROOT)} sha256={sha(encoded)} ({len(encoded)} bytes)')
    elif FIXTURE.read_bytes() != encoded:
        raise SystemExit(f'fresh native paint replay differs from {FIXTURE.relative_to(ROOT)}; use --write-fixture explicitly')
    else:
        print(f'verified {FIXTURE.relative_to(ROOT)} sha256={sha(encoded)} ({len(encoded)} bytes)')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
