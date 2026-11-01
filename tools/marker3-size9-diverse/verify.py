import gzip, hashlib, json, os, struct, subprocess, sys, zlib
from pathlib import Path
root = Path(__file__).resolve().parents[2]
tool = root / 'tools/marker3-size9-diverse'
work = Path(sys.argv[1])
receipt = json.loads((tool / 'receipt.json').read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
for path, expected in receipt['input_sha256'].items():
    assert sha((root / path).read_bytes()) == expected, path
fixture = json.loads((root / 'conformance/marker3-size9-diverse-native.json').read_text())
points = {'reverse': [[14.25,20.5],[44.5,20.5],[17.5,22.5],[48.25,24.5]], 'cross': [[18.25,12.5],[45.5,41.25],[18.25,41.25],[45.5,12.5]], 'short': [[12.5,18.25],[15.25,20.5],[18.5,17.75]]}
pressures = {'reverse': [1.0,0.15,1.0,0.15], 'cross': [0.2,0.9,0.4,1.0], 'short': [0.1,0.8,0.4]}
colors = {'reverse': '0x80cc331a', 'cross': '0xff3366cc', 'short': '0x6633aa55'}
for name in points:
    case = json.loads(gzip.decompress((tool / f'{name}-caller.json.gz').read_bytes()))['cases']['marker3_linked_saved1:5:tool1:curve1']
    assert fixture[name] == {'input': case['input'], 'native_calls': {'source_ObjectStroke_arrays': case['native_calls']['source_ObjectStroke_arrays']}, 'phase_final_vectors': case['phase_final_vectors']}
    assert case['native_calls']['source_ObjectStroke_arrays']['points']['data_hex'] == ''.join(struct.pack('<f', x).hex() for pair in points[name] for x in pair)
    canon = lambda v: sha(json.dumps(v, sort_keys=True, separators=(',', ':')).encode())
    fields = {'input_sha256': canon(case['input']), 'source_arrays_sha256': canon(case['native_calls']['source_ObjectStroke_arrays']), 'drawing_vector_sha256': canon(case['phase_final_vectors']['draw_object_stroke']), 'saved_redraw_vector_sha256': canon(case['phase_final_vectors']['saved_redraw'])}
    assert fields == receipt['cross_scale_caller_fields_sha256'][name]
    assert fields == receipt['cross_scale_caller_fields_sha256'][name+'1'] == receipt['cross_scale_caller_fields_sha256'][name+'4']
    inp = case['input']
    assert inp['pen_size_f32_bits'] == struct.pack('<f',9).hex()
    assert inp['pressure_f32_bits'] == [struct.pack('<f', x).hex() for x in pressures[name]]
    assert inp['color_argb'] == colors[name] and inp['advanced_setting'] == '5;'
    assert inp['tool_type'] == 1 and inp['curve_enabled'] and inp['stroke_type'] == 0
    assert inp['timestamps_i32'] == [8*i for i in range(len(points[name]))]
    assert inp['tilt_f32_bits'] == inp['orientation_f32_bits'] == ['00000000']*len(points[name])
for case, expected in receipt['metrics'].items():
    raw = zlib.decompress((tool / f'{case}.rgba.zlib').read_bytes())
    assert sha(raw) == receipt['native_rgba_sha256'][case]
    (work / f'{case}.rgba').write_bytes(raw)
    for name, digest in receipt['output_sha256'][case].items():
        assert sha((work / case / name).read_bytes()) == digest, (case,name)
    result = subprocess.check_output([sys.executable, str(tool / 'compare.py'), str(work / f'{case}.rgba'), str(work / case), case], text=True, env=os.environ)
    assert json.loads(result) == expected, case
    print(case, 'verified', flush=True)
