"""Read-only single-point Drawing probe; requires the documented local runtime."""
import atexit
import tempfile
import os
import contextlib
import gzip
import io
import json
import hashlib
import sys
from pathlib import Path

if not __debug__:
    raise SystemExit('Python optimization is unsupported')
if sys.argv[1:] not in (['--print-capture', '--advanced-setting', 'empty'], ['--positive']):
    raise SystemExit('only the read-only single-point and --positive probes are supported')
positive = sys.argv[1:] == ['--positive']
sys.argv[1:] = ['--print-capture', '--advanced-setting', 'empty']
root = Path(__file__).resolve().parents[3]
bundle = Path(__file__).resolve().parent
work = tempfile.TemporaryDirectory(prefix='inkpen-v4-single-point-')
original_cwd = Path.cwd()
def cleanup():
    os.chdir(original_cwd)
    work.cleanup()
atexit.register(cleanup)
layout = Path(work.name)
(layout / 'scratch').symlink_to(root / 'scratch', target_is_directory=True)
p = layout / 'conformance/inkpen_saved_caller_native.py'
p.parent.mkdir()
for name, digest in {
    'elf_plt.py': '816eceff5b532b35c8ef9ea904a8aa64767422b0799347a18fbd4602b70f373b',
    'fountain_native.py': '40cb917c7f286302d8519e9b953c3c21b7fe6ea0b6b0a80ccc13e2e724cfad30',
    'oblique_canvas_native.py': '1cabf3f8bf907ce6fd48952682b4cad5208827cec4aa0bd0a1ec864aa750a1f2',
}.items():
    data = gzip.decompress((bundle / (name + '.gz')).read_bytes())
    assert hashlib.sha256(data).hexdigest() == digest
    (p.parent / name).write_bytes(data)
source = gzip.decompress(Path(__file__).with_name('caller.py.gz').read_bytes())
assert hashlib.sha256(source).hexdigest() == 'c0b1657cdcf94bcf2c173ce68ed7817e55a4de246883b47265bb7b4d3f50dc04'
p.write_bytes(source)
s = source.decode()
if positive:
    assert hashlib.sha256(p.read_bytes()).digest() == hashlib.sha256(source).digest()
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(compile(s, str(p), 'exec'), {'__name__': '__main__', '__file__': str(p)})
    result = json.loads(output.getvalue())
    receipt = json.loads(Path(__file__).with_name('receipt.json').read_text())
    control = receipt['positive_control']
    assert result['route']['drawing_return'] == control['drawing_return'] == 1
    assert result['route']['selected_drawable_vtable_id'] == control['drawable'] == 'v4'
    assert result['streams']['draw_object_stroke']['callback_count'] == control['callback_count'] == 26
    assert result['streams']['draw_object_stroke']['sha256'] == control['stream_sha256']
    assert result['libraries'] == receipt['native_library_sha256']
    assert result['helper_hashes'] == receipt['baseline_helper_sha256']
    print(json.dumps(control, sort_keys=True))
    raise SystemExit(0)
a = "input_case={'points':[[0.,0.],[10.,0.],[20.,0.],[30.,10.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25,.5,.75,1.0],'timestamps':[10,20,30,40],'tilt':[0.]*4,'orientation':[0.]*4}"
b = "input_case={'points':[[8.,8.]],'size':10.0,'curve_enabled':True,'tool_type':1,'stroke_type':0,'pressure':[.25],'timestamps':[10],'tilt':[0.],'orientation':[0.]}"
assert s.count(a) == 1
s = s.replace(a, b)
a = 'def taskqueue(u,a,z,d):\n'
b = "queue_events=[]\ndef taskqueue(u,a,z,d):\n if phase == 'draw_object_stroke' and a in (0x20001040, 0x20001000): queue_events.append(hex(a))\n"
assert s.count(a) == 1
s = s.replace(a, b)
a = 'if draw_result != 1 or not factory_created_pens:'
b = '''expected={'label':'before_drawObjectStroke','size_bits':'00002041','point_count':1,'curve_enabled':1,'tool_type':1,'stroke_type':0,'color_argb':4278190080,'shape':0,'line_type':0,'fixed_width_enabled':0,'fixed_width_bits':'00000000','rainbow_enabled':0,'rainbow_distance':0,'rainbow_offset_bits':'00000000'}
 assert object_stroke_snapshots == [expected] and advanced_utf8 == '' and advanced_getter
 assert source_object_arrays == {'points': {'count': 1, 'data_hex': '0000004100000041', 'element_size': 8}, 'pressures': {'count': 1, 'data_hex': '0000803e', 'element_size': 4}, 'timestamps': {'count': 1, 'data_hex': '0a000000', 'element_size': 4}, 'tilts': {'count': 1, 'data_hex': '00000000', 'element_size': 4}, 'orientations': {'count': 1, 'data_hex': '00000000', 'element_size': 4}}
 assert draw_result == 0 and not factory_created_pens and not loader_calls and not queue_events
 print(json.dumps({'drawing_return':draw_result,'factory_count':len(factory_created_pens),'loader_count':len(loader_calls),'queue_events':queue_events,'advanced_setting':advanced_utf8,'getters':object_stroke_snapshots,'arrays':source_object_arrays},sort_keys=True),_always=True)
 raise SystemExit(0)
 if draw_result != 1 or not factory_created_pens:'''
assert s.count(a) == 1
s = s.replace(a, b)
exec(compile(s, str(p), 'exec'), {'__name__': '__main__', '__file__': str(p)})
