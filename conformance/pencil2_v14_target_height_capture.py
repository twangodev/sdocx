"""Compare host-Mesa Pencil2 RTV4 replay at two framebuffer heights."""
import hashlib
import json
from pathlib import Path

source = Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py').resolve()
code = source.read_text()
old = 'width=192; height=192'
assert code.count(old) == 1
code = code.replace(old, 'width=192; height=267')
namespace = {'__name__': 'target_height_capture', '__file__': str(source)}
exec(compile(code, str(source), 'exec'), namespace)
result = namespace['capture'](Path('/tmp'))
baseline_path = Path('tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/captures/root-draw-v38-a/capture.json')
baseline = bytes.fromhex(json.loads(baseline_path.read_text())['mesa']['readback_rgba_hex'])
taller = bytes.fromhex(result['mesa']['readback_rgba_hex'])
comparisons = []
for dy in (0, 75):
    rows = []
    for y in range(192):
        for x in range(192):
            a = baseline[(y * 192 + x) * 4 + 3]
            b = taller[((y + dy) * 192 + x) * 4 + 3]
            if a or b:
                rows.append((x, y, a, b))
    differences = [[x, y, a, b] for x, y, a, b in rows if a != b]
    comparisons.append({'taller_y_offset': dy, 'nonzero_union': len(rows),
                        'alpha_mismatches': len(differences),
                        'alpha_absolute_difference_sum': sum(abs(a-b) for _, _, a, b in rows),
                        'difference_examples': differences[:10]})
receipt = {'schema': 'sdocx-pencil2-v14-target-height-native-v1',
           'scope': 'Controlled native RTV4 host Mesa replay with the same 84 records, draw matrix and inverse scale, on 192x192 and 192x267 RGBA8 targets. Readback coordinates have OpenGL bottom-left origin. Not full Drawing, Android, PDF or device evidence.',
           'command': 'PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/pencil2_v14_target_height_capture.py',
           'capture_source': str(source.relative_to(Path.cwd())),
           'capture_source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
           'baseline_rgba_sha256': hashlib.sha256(baseline).hexdigest(),
           'taller_rgba_sha256': hashlib.sha256(taller).hexdigest(),
           'comparisons': comparisons,
           'draw_uniform_events': [e for e in result['mesa']['draw_gl_events'] if 'Uniform' in e.get('name', '')]}
print(json.dumps(receipt, indent=2))
