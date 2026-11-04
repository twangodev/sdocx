"""Compare ChineseBrush 4; and 5; native geometry and host-Mesa paint.

Pass a project-shaped tree restored as in tools/chinesebrush-default-size/README.md.
Run with the Unicorn-enabled Python and the project native runtime on PYTHONPATH.
"""
import hashlib
import base64
import zlib
import json
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
tree = Path(sys.argv[1]).resolve()
python = sys.executable
geometry = (root / 'tools/chinesebrush_omitted_color_capture.py').read_text()
assert geometry.count('("four", "4;", 4, "V3")]') == 1
geometry = geometry.replace('("four", "4;", 4, "V3")]', '("four", "4;", 4, "V3"), ("five", "5;", 5, "V3")]')
paint = tree / 'conformance/chinesebrush_paint_native.py'
original = paint.read_text()
assert hashlib.sha256(original.encode()).hexdigest() == '0377f7de9c48e7c79fc596603128d49bff3a7427ce59e6052db14a53d8cf342a'
cases = {}
for version, name in ((4, 'four'), (5, 'five')):
    code = 'import sys; path,name=sys.argv[1:]; sys.argv=[path,"--_capture",name]; exec(compile(' + repr(geometry) + ',path,"exec"),{"__name__":"__main__","__file__":path})'
    result = subprocess.check_output([python, '-c', code, str(root / 'tools/chinesebrush_omitted_color_capture.py'), name], cwd=root)
    drawing = json.loads(result)
    assert drawing['native_selection']['version'] == version
    assert 'ChineseBrushStrokeDrawableGLV3::RedrawPen' in drawing['native_selection']['redraw_target_symbols'][0]
    assert original.count("b'3;\\0'") == 1
    assert original.count("'advanced_setting_ascii':'3;'") == 1
    assert original.count("FIXTURE=HERE.with_name('chinesebrush-paint-native.json')") == 1
    changed = original.replace("b'3;\\0'", f"b'{version};\\0'").replace("'advanced_setting_ascii':'3;'", f"'advanced_setting_ascii':'{version};'")
    changed = changed.replace("FIXTURE=HERE.with_name('chinesebrush-paint-native.json')", "FIXTURE=HERE.with_name('chinesebrush-v5-temporary.json')")
    script = tree / 'conformance/chinesebrush_v5_temporary.py'
    try:
        script.write_text(changed)
        subprocess.run([python, str(script), '--write-fixture'], cwd=root, check=True, stdout=subprocess.DEVNULL)
        frame = json.loads((tree / 'conformance/chinesebrush-v5-temporary.json').read_text())
    finally:
        script.unlink(missing_ok=True)
        (tree / 'conformance/chinesebrush-v5-temporary.json').unlink(missing_ok=True)
    assert frame['source_to_native']['source_result']['configured_object_stroke']['advanced_setting_ascii'] == f'{version};'
    target = frame['paint_readback']['target']
    pixels = zlib.decompress(base64.b64decode(frame['paint_readback']['full_rgba8_bottom_left_first_zlib_base64']))
    assert hashlib.sha256(pixels).hexdigest() == frame['paint_readback']['full_rgba8_sha256']
    assert len(target['nonzero_pixels_linear_index_rgba']) > 0
    assert target['gl_error_before_readback'] == target['gl_error_after_readback'] == 0
    assert all(row['is_framebuffer_complete'] for row in frame['native_draw']['fbo_status'])
    calls = frame['native_draw']['graphics_gl_calls']
    assert all(row['prior_error'] == row['error_after'] == 0 for row in calls)
    trace = sorted({row['symbol'] for row in frame['native_draw']['native_caller_trace'] if 'ChineseBrush' in row['symbol']})
    cases[str(version)] = {'drawing_input': drawing['input'], 'drawing_selection': drawing['native_selection'], 'drawing_producer': drawing['producer'], 'paint_upload_sha256': frame['native_draw']['producer_vector_sha256'], 'paint_rgba_sha256': frame['paint_readback']['full_rgba8_sha256'], 'paint_rgba_bytes': frame['paint_readback']['byte_count'], 'paint_input': frame['source_to_native']['source_result']['configured_object_stroke'], 'paint_motion_case': frame['source_to_native']['source_result']['motion_case'], 'paint_native_symbols': trace, 'paint_framebuffer_status': frame['native_draw']['fbo_status'], 'paint_dimensions': target['dimensions'], 'paint_nonzero_pixels': target['nonzero_pixels_linear_index_rgba'], 'paint_gl_errors': {'before_readback': target['gl_error_before_readback'], 'after_readback': target['gl_error_after_readback'], 'graphics_calls': calls}, 'paint_provenance': frame['provenance'], 'geometry_dsos': drawing['native_libraries_sha256'], 'geometry_helpers': drawing['native_helper_sha256']}
assert cases['4']['drawing_producer']['buffer_hex'] == cases['5']['drawing_producer']['buffer_hex']
assert cases['4']['paint_upload_sha256'] == cases['5']['paint_upload_sha256']
assert cases['4']['paint_rgba_sha256'] == cases['5']['paint_rgba_sha256']
print(json.dumps({'schema':'chinesebrush-setting-5-paired-native-v1', 'scope':'Synthetic ObjectStroke, host adapters, native Drawing and RTV1, Unicorn and host Mesa; no device or real-note parity.', 'provenance': {'capture_script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'geometry_script_sha256': hashlib.sha256((root / 'tools/chinesebrush_omitted_color_capture.py').read_bytes()).hexdigest(), 'paint_script_sha256': hashlib.sha256(original.encode()).hexdigest()}, 'cases':cases}, indent=2, sort_keys=True))
