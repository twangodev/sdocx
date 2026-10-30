import hashlib
import json
import re
import struct
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[2]
receipt = json.loads((root / 'tools/marker3-independent-scene/receipt.json').read_text())
log = Path(sys.argv[1]).read_text()
report = json.loads(Path(sys.argv[2]).read_text())
order = sys.argv[3]
for path, expected in receipt['source_sha256'].items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, path
assert report['library_hashes'] == receipt['host_libraries_sha256']
assert report['helper_hashes'] == receipt['helper_sha256']
for i in (1, 2):
    vector = bytes.fromhex(re.search(rf'^VECTOR{i} (\w+)$', log, re.M).group(1))
    assert vector.hex() == receipt['vectors'][i-1]['bytes_hex']
    assert hashlib.sha256(vector).hexdigest() == receipt['vectors'][i-1]['sha256']
actual = json.loads(re.search(r'^SECOND_INPUT (.*)$', log, re.M).group(1))
inputs = receipt['inputs']
assert actual == {'points': inputs['second_points'], 'pressure': inputs['pressures'], 'timestamps': inputs['timestamps'], 'size': inputs['size'], 'tool_type': inputs['tool_type'], 'color_argb': '0x' + inputs['second_color_argb'], 'advanced_setting': '5'}
first = report['source_caller_case']['input']
def floats(bits): return [struct.unpack('<f', bytes.fromhex(x))[0] for x in bits]
assert [floats(x) for x in first['points_f32_bits']] == inputs['first_points']
assert all(abs(a-b) < 1e-7 for a,b in zip(floats(first['pressure_f32_bits']), inputs['pressures']))
assert first['timestamps_i32'] == inputs['timestamps']
assert floats([first['pen_size_f32_bits']]) == [inputs['size']]
assert first['color_argb'] == '0x'+inputs['first_color_argb'] and first['tool_type'] == inputs['tool_type'] and first['advanced_setting'] == inputs['advanced_setting']
draws = report['draws']
assert len(draws) == 4 and [x['call'] for x in draws] == ['glDrawArraysInstanced','glDrawArrays','glDrawArraysInstanced','glDrawArrays']
assert draws[0]['state']['framebuffer'] != draws[2]['state']['framebuffer']
assert draws[1]['state']['framebuffer'] == draws[3]['state']['framebuffer']
colors = [x for x in report['uniform_calls'] if x['call'] == 'glUniform4fv']
expected_colors = receipt['forward_color_uniforms']
if order == 'forward':
    assert draws == receipt['forward_draws']
    assert colors == expected_colors
else:
    assert colors == expected_colors[2:] + expected_colors[:2]
assert not any(x['error'] for x in report['gl_error_checks'])
print(order, 'input, vectors, libraries, draws, colors and GL errors verified')
