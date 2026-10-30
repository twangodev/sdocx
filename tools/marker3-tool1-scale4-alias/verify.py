#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import sys
import struct
import re
import ast
import zlib

root = Path(__file__).resolve().parents[2]
captures = Path(sys.argv[1])
prior = json.loads((root / 'tools/marker3-tool1-scale4-full/receipt.json').read_text())
cases = {}
short = [(25.25, 28.5), (32.25, 39.5), (42.25, 30.5), (49.25, 41.5), (55.25, 31.5)]
expected = {
    'short4': (short, [0.1, 0.9, 0.3, 0.8, 0.2], 4.0, '0x8033cc55'),
    'short6': (short, [0.1, 0.9, 0.3, 0.8, 0.2], 6.0, '0xffd14a87'),
    'fractional': ([(25.25, 28.5), (67.75, 46.125)], [0.2, 0.9], 8.0, '0x8033cc55'),
    'crossing': ([(35.25, 35.5), (85.75, 85.25), (35.25, 85.25), (85.75, 35.5)], [0.2, 0.9, 0.4, 1.0], 8.0, '0xffd14a87'),
}
export_source = (root / 'tools/marker3-tool1-scale4-alias/export.rs').read_text()
assert hashlib.sha256(export_source.encode()).hexdigest() == '484180c2737b9e0b25f8472fa11fe4525b40d23132acaf3e4be7372677a283de'
assert 'preload.StraightMarker' in export_source and '"tool_type_raw":1' in export_source and '"advanced_settings":"5;"' in export_source

def bits(value):
    return struct.pack('<f', value).hex()
for case in ('short4', 'short6', 'fractional', 'crossing'):
    path = captures / f'{case}-tool1'
    report = json.loads((path / 'drawing-paint-positive.json').read_text())
    frame = (path / 'drawing-paint-positive.rgba').read_bytes()
    old = prior['cases'][case]['native']
    source = report['source_caller_case']['input']
    factory = report['source_caller_case']['native_calls']['Drawing_factory_admission']
    assert source['pen_name'].endswith('.StraightMarker')
    assert source['tool_type'] == 1 and source['advanced_setting'] == '5;'
    points, pressures, size, color = expected[case]
    assert source['points_f32_bits'] == [[bits(x), bits(y)] for x, y in points]
    assert source['pressure_f32_bits'] == [bits(x) for x in pressures]
    assert source['pen_size_f32_bits'] == bits(size) and source['color_argb'] == color
    assert source['timestamps_i32'] == [8 * i for i in range(len(points))]
    assert source['tilt_f32_bits'] == source['orientation_f32_bits'] == [bits(0)] * len(points)
    assert source['curve_enabled'] and source['stroke_type'] == 0
    branch = 'short4' if case.startswith('short') else case
    line = next(line for line in export_source.splitlines() if line.lstrip().startswith('"' + branch + '"'))
    vectors = re.findall(r'vec!(\[.*?\])(?=,vec!|,0x|,if)', line)
    export_points, export_pressure = map(ast.literal_eval, vectors[:2])
    shift = -0.0625 if case == 'short4' else 0.0625 if case == 'short6' else 0.0
    phase = 0.0 if case == 'short4' else 0.5 if case == 'short6' else 0.25
    assert [[bits(x + shift), bits(y + shift)] for x, y in export_points] == [[bits(x + phase / 4), bits(y + phase / 4)] for x, y in points]
    assert [bits(x) for x in export_pressure] == [bits(x) for x in pressures]
    assert color[2:] in line.lower() and (case not in ('short4', 'short6') or size == (4.0 if case == 'short4' else 6.0))
    assert factory['selected_v5_or_v1_actual'] == report['selected_drawable_version'] == '5'
    assert len(frame) == 512 * 512 * 4
    assert frame == zlib.decompress((root / 'tools/marker3-tool1-scale4-full' / f'{case}.rgba.zlib').read_bytes())
    assert report['readback']['sha256'] == hashlib.sha256(frame).hexdigest()
    assert all(check['error'] == 0 for check in report['gl_error_checks'])
    for key in ('native_vector_sha256', 'native_vector_bytes', 'callback_count', 'readback', 'draws', 'native_rt_color_rgba_f32', 'matching_native_gpu_uploads', 'library_hashes', 'helper_hashes', 'host_gles'):
        assert report[key] == old[key], (case, key)
    cases[case] = {'input': source, 'factory': factory, 'native_vector_sha256': report['native_vector_sha256'], 'readback': report['readback'], 'library_hashes': report['library_hashes'], 'helper_hashes': report['helper_hashes'], 'host_gles': report['host_gles']}
Path(sys.argv[2]).write_text(json.dumps({'scope': 'Synthetic StraightMarker tool 1, native Drawing and same-vector RTV1 bridge on host Mesa, 4x; no serialized-note or device claim.', 'cases': cases}, indent=2, sort_keys=True) + '\n')
