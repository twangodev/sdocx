"""Capture one synthetic InkPen2 V1 stroke at explicit screen profiles."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[1]
harness = root / 'conformance/inkpen2_v1_controls_native.py'
case = {'points': [[0, 0], [8, 0], [16, 3], [24, 3]], 'size': 4,
        'curve_enabled': True, 'tool_type': 2, 'stroke_type': 0,
        'pressure': [.2, .4, .6, .8], 'timestamps': [0, 10, 20, 30],
        'tilt': [0, 0, 0, 0], 'orientation': [0, 0, 0, 0]}
env = os.environ.copy()
env['PYTHONPATH'] = os.pathsep.join(str(root / p) for p in (
    'scratch/straightinkpen2-alias', 'scratch/apk-analysis-runtime/python',
    'scratch/apk-analysis-runtime/binutils-aarch64/usr/bin'))
profiles = {}
with tempfile.TemporaryDirectory() as directory:
    source = Path(directory) / 'input.json'
    source.write_text(json.dumps(case))
    for width, height in ((1920, 1080), (1080, 1920), (1440, 2560), (2160, 3840),
                          (800, 600), (2560, 1440), (3840, 2160)):
        result = json.loads(subprocess.check_output([
            sys.executable, str(harness), '--print-capture', '--pen-name', 'InkPen2',
            '--advanced-setting', 'null', '--input-json', str(source),
            '--width', str(width), '--height', str(height)], cwd=root, env=env))
        assert result['native_calls']['route']['drawing_selected_drawable_version'] == '1'
        assert result['native_calls']['pen_screen_resolution'] == [{'width': width, 'height': height}]
        events = result['callback_buffer_events']
        draw = [e for e in events if e['phase'] == 'draw_object_stroke'][-1]['after_bytes']
        saved = [e for e in events if e['phase'] == 'saved_redraw'][-1]['after_bytes']
        assert draw == saved
        profiles[f'{width}x{height}'] = {
            'stream_hex': saved, 'stream_sha256': hashlib.sha256(bytes.fromhex(saved)).hexdigest(),
            'record_count': len(saved) // 24,
            'redraw_rect_f32_bits': result['native_calls']['separate_saved_RedrawPen_rect_f32_bits']}
receipt = {'schema': 'sdocx-inkpen2-v1-screen-profiles-native-v1',
           'scope': 'One synthetic null-setting saved stroke, native Drawing and RedrawPen with explicit screen dimensions and identity canvas. Callback geometry only; no paint, device defaults or deserialization.',
           'capture_script': 'conformance/inkpen2_v1_screen_profiles_capture.py',
           'input': case, 'profiles': profiles}
(root / 'conformance/inkpen2-v1-screen-profiles-native.json').write_text(json.dumps(receipt, indent=2) + '\n')
