#!/usr/bin/env python3
"""Check paired native Drawing and host-frame captures for tool-1 aliases."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(sys.argv[1])
results = {}
for case in ('short', 'fractional', 'crossing'):
    captures = []
    for pen in ('Marker3', 'StraightMarker'):
        for setting in ('5', 'empty'):
            path = root / f'{case}-{pen}-{setting}'
            report = json.loads((path / 'drawing-paint-positive.json').read_text())
            frame = (path / 'drawing-paint-positive.rgba').read_bytes()
            source = report['source_caller_case']['input']
            factory = report['source_caller_case']['native_calls']['Drawing_factory_admission']
            assert source['pen_name'].endswith('.' + pen)
            assert source['tool_type'] == 1 and source['advanced_setting'] == ('5;' if setting == '5' else '')
            assert factory['selected_v5_or_v1_actual'] == report['selected_drawable_version'] == '5'
            assert report['readback']['sha256'] == hashlib.sha256(frame).hexdigest()
            assert len(frame) == 256 * 256 * 4 and report['readback']['nonzero_pixels'] > 0
            assert all(item['error'] == 0 for item in report['gl_error_checks'])
            captures.append((report, frame))
    first, frame = captures[0]
    for report, other in captures[1:]:
        for key in ('native_vector_sha256', 'native_vector_bytes', 'callback_count', 'readback', 'shader_source_calls', 'draws', 'native_rt_color_rgba_f32', 'matching_native_gpu_uploads'):
            assert report[key] == first[key], (case, key)
        assert other == frame
    results[case] = {'input': first['source_caller_case']['input'], 'callback_count': first['callback_count'], 'native_vector_sha256': first['native_vector_sha256'], 'rgba_sha256': hashlib.sha256(frame).hexdigest(), 'readback': first['readback'], 'library_hashes': first['library_hashes'], 'helper_hashes': first['helper_hashes'], 'host_gles': first['host_gles']}
Path(sys.argv[2]).write_text(json.dumps({'scope': 'Synthetic Model/Drawing tool 1, RTV1 same-vector bridge, host Mesa 2x with quarter-pixel translation; not serialized-note, Android device, or SetPenData call-site evidence.', 'cases': results}, indent=2, sort_keys=True) + '\n')
