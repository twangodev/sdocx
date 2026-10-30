#!/usr/bin/env python3
import hashlib
import json
import struct
from pathlib import Path

root = Path(__file__).resolve().parents[3]
receipt = json.loads((Path(__file__).parent / 'streams.json').read_text())
assert hashlib.sha256((root / 'tools/montblanc/narrow-probe/capture.py').read_bytes()).hexdigest() == receipt['capture_source_sha256']
cases = receipt['cases']
prefix = 'SPen::MontblancFountainPenStrokeDrawableRTV1::'
suffix = '(SPen::Vector3<float>, SPen::Vector2<float>, float)'
symbols = {kind: prefix + kind + suffix for kind in ('AddStartPoint', 'AddPoint', 'AddEndPoint')}
expected = {'9': (45, 36, []), '10-distinct': (49, 18, [18]),
            '11': (54, 27, [18]), '12': (58, 18, [27]),
            '13': (63, 27, [27]), 'duplicate10': (45, 36, []),
            'duplicate12': (45, 36, [])}
assert cases.keys() == expected.keys()
for name, case in cases.items():
    source = case['input']
    assert len(source['points']) == len(source['pressure'])
    assert all(len(point) == 2 for point in source['points'])
    assert all(0 <= value <= 1 for value in source['pressure']) and source['pen_size'] > 0
    records = case['stream']
    split = case['first_count']
    assert (split, case['replay_count'], case['full_word_matching_offsets']) == expected[name]
    assert len(records) == split + case['replay_count']
    assert records[0][0] == symbols['AddStartPoint']
    assert records[split - 1][0] == symbols['AddEndPoint']
    assert all(symbol == symbols['AddPoint'] for symbol, _ in records[1:split - 1] + records[split:])
    assert all(len(words) == 6 and all(len(bytes.fromhex(word)) == 4 for word in words) for _, words in records)
    offsets = [i for i in range(split) if i + case['replay_count'] <= split and all(
        x[1] == y[1] and x[0] == symbols['AddPoint'] and y[0] == symbols['AddPoint']
        for x, y in zip(records[i:i + case['replay_count']], records[split:])
    )]
    assert offsets == case['full_word_matching_offsets'], name
    assert all(len(bytes.fromhex(digest)) == 32 for digest in [case['frame_sha256'], *case['input_pins'].values(), *case['host_gl_sha256'].values()])
nine = cases['9']
a = nine['stream']
k = nine['first_count']
assert a[0][1][:5] == a[k][1][:5] and a[0][1][5] != a[k][1][5]
assert a[1:nine['replay_count']] == a[k + 1:]
for name, copies in (('duplicate10', 1), ('duplicate12', 3)):
    case = cases[name]
    for channel in ('points', 'pressure'):
        base = nine['input'][channel]
        assert case['input'][channel] == base[:3] + [base[2]] * copies + base[3:]
    assert case['input']['pen_size'] == nine['input']['pen_size']
    assert case['stream'] == nine['stream']
print(f'Verified {len(cases)} native streams, expected replay offsets, digest syntax and duplicate controls.')

trace = json.loads((Path(__file__).parent / 'event-traces.json').read_text())
assert hashlib.sha256((Path(__file__).parent / 'trace.py').read_bytes()).hexdigest() == trace['trace_script_sha256']
assert trace['capture_script_sha256'] == receipt['capture_source_sha256']
traces = trace['cases']
assert set(traces) == set(expected) | {'pressure-duplicate', 'short-move'}
for name, case in traces.items():
    if name in cases:
        assert case['input'] == cases[name]['input'] and case['stream'] == cases[name]['stream']
    assert len(case['draw_calls']) == 5
    actual = [[struct.unpack('<f', bytes.fromhex(word))[0] for word in row['s0_s5_f32_bits'][:2]] for row in case['draw_calls']]
    base = traces['9']['input']['points'] if name in ('duplicate10', 'duplicate12', 'pressure-duplicate', 'short-move') else case['input']['points']
    assert actual == base[-6:-1], name
    assert all(row['lr'] == '0xb01dd90' for row in case['draw_calls'])
    assert case['end_state']['lr'] == '0xb01e7a0'
    assert len(bytes.fromhex(case['end_state']['motion_event_bytes'])) == 160
    assert all(len(bytes.fromhex(row['receiver_bytes_80_144'])) == 64 for row in case['draw_calls'])
for name in ('duplicate10', 'duplicate12', 'pressure-duplicate', 'short-move'):
    assert traces[name]['stream'] == traces['9']['stream']
    assert [row['s0_s5_f32_bits'] for row in traces[name]['draw_calls']] == [row['s0_s5_f32_bits'] for row in traces['9']['draw_calls']]
print(f'Verified {len(traces)} endPen event traces and complete streams.')

base_input = traces['9']['input']
for name, delta in (('pressure-duplicate', 0), ('short-move', 1)):
    source = traces[name]['input']
    assert source['points'] == base_input['points'][:3] + [[base_input['points'][2][0] + delta, base_input['points'][2][1]]] + base_input['points'][3:]
    assert source['pressure'] == base_input['pressure'][:3] + [0.9] + base_input['pressure'][3:]
    assert source['pen_size'] == base_input['pen_size']

irregular = json.loads((Path(__file__).parent / 'irregular-native.json').read_text())
assert irregular['input'] == json.loads((Path(__file__).parent / 'irregular-input.json').read_text())
assert irregular['trace_source_sha256'] == trace['trace_script_sha256']
assert irregular['candidate_patch_sha256'] == hashlib.sha256((Path(__file__).parent / 'irregular-candidate.patch').read_bytes()).hexdigest()
assert len(irregular['stream']) == irregular['candidate_result']['callback_count'] == 83
assert irregular['candidate_result']['methods_and_six_f32_words_equal'] is True
assert len(irregular['draw_calls']) == 5 and all(row['lr'] == '0xb01dd90' for row in irregular['draw_calls'])
assert all(len(row[1]) == 6 and all(len(bytes.fromhex(w)) == 4 for w in row[1]) for row in irregular['stream'])
print('Verified retained irregular trace structure and test-only candidate identity; candidate equality requires rerunning the Rust test.')

variants = json.loads((Path(__file__).parent / 'irregular-variants.json').read_text())
assert variants['candidate_patch_sha256'] == irregular['candidate_patch_sha256']
assert variants['trace_source_sha256'] == trace['trace_script_sha256']
assert variants['candidate_results'] == {'short14': 73, 'long17': 97, 'reverse17': 95}
base = irregular['input']
short, long, reverse = (variants['cases'][name]['input'] for name in ('short14', 'long17', 'reverse17'))
assert short == {'points': base['points'][:14], 'pressure': base['pressure'][:14], 'pen_size': base['pen_size']}
assert long == {'points': base['points'] + [[97, 8]], 'pressure': base['pressure'] + [0.48], 'pen_size': base['pen_size']}
assert reverse == {'points': long['points'][::-1], 'pressure': long['pressure'][::-1], 'pen_size': long['pen_size']}
for name, case in variants['cases'].items():
    assert len(case['stream']) == variants['candidate_results'][name]
    assert len(case['draw_calls']) == 5 and all(row['lr'] == '0xb01dd90' for row in case['draw_calls'])
    assert all(len(row[1]) == 6 and all(len(bytes.fromhex(w)) == 4 for w in row[1]) for row in case['stream'])
print('Verified three retained irregular variants; candidate equality requires rerunning the Rust test for each case.')

exports = json.loads((Path(__file__).parent / 'export-receipt.json').read_text())
assert exports['complete'] and len(exports['cases']) == 8
for source in ('tools/montblanc/redraw-window/irregular-candidate.patch', 'tools/montblanc/many-point-public/src/main.rs', 'tools/montblanc/many-point-public/Cargo.toml', 'tools/montblanc/many-point-public/Cargo.lock'):
    assert exports['source_sha256'][source] == hashlib.sha256((root / source).read_bytes()).hexdigest()
assert set(exports['isolated_source_sha256']) == {'baseline', 'candidate'}
assert all(set(values) == {'sampler_dispatch', 'sampler', 'generated_driver'} and all(len(bytes.fromhex(digest)) == 32 for digest in values.values()) for values in exports['isolated_source_sha256'].values())
assert exports['isolated_source_sha256']['baseline']['generated_driver'] == exports['isolated_source_sha256']['candidate']['generated_driver']
assert exports['isolated_source_sha256']['baseline']['sampler'] != exports['isolated_source_sha256']['candidate']['sampler']
assert exports['source_sha256']['tools/montblanc/redraw-window/export-compare.py'] == hashlib.sha256((Path(__file__).parent / 'export-compare.py').read_bytes()).hexdigest()
for case in exports['cases'].values():
    for reader in ('svg', 'pdf', 'pdfium'):
        baseline = case['metrics']['baseline'][reader]
        candidate = case['metrics']['candidate'][reader]
        assert len(baseline) == len(candidate) == 5
        assert candidate[4] < baseline[4]
        assert candidate[2] == candidate[0] and candidate[1] > candidate[0]
print('Verified eight retained SVG/PDF comparison summaries; raster reproduction requires export-compare.py verify.')
