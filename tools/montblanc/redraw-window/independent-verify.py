#!/usr/bin/env python3
"""Verify frozen independent native stream observations, without executing them."""
import hashlib
import json
import re
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parents[2]
receipt = json.loads((here/'independent-native.json').read_text())
sha = lambda obj: hashlib.sha256(json.dumps(obj, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
hex64 = re.compile(r'[0-9a-f]{64}\Z')
hex8 = re.compile(r'[0-9a-f]{8}\Z')
assert receipt['capture_script_sha256'] == hashlib.sha256((root/'tools/montblanc/narrow-probe/capture.py').read_bytes()).hexdigest()
expected = {
 'zigzag10': (10, 2.4, 32, 14, 9, 'd43778afc8ffa96c68e232d0b6385bb4d8e749fc33bd33735ea062fc731532a9', 'c34579f90b83dabf0039d24647a168d4a0fd5f9e6e44d1a541a4aab53b38c50e'),
 'vertical12': (12, 0.6, 44, 14, 21, '25c513fc53428a1f61e82a7af6889df3de36e3d651b25cbba700fa261d1d07e7', '3ccb999b08a42af180979c47e2a1f56f23562442955d0dfee22d653041914286'),
 'turn20': (20, 3.0, 49, 28, 14, '9987fbfcc6dd333e48722446a784f3c9e240eb2da24be8c9479195d334cbcff4', '5d5e8d72b663f1c6c904015366cd2114fb8d592345f14d6a44f99651a62b814b'),
 'loop32': (32, 1.2, 118, 15, 95, '91b5074f05e64e36e5b3ddd51be5acbff748658f7e2409b1946c8cbc18faf919', '2569a0300862a36089ca4ef1e950d2b3505e3cf0b88f8a099843b7a308eaa03b'),
}
pins = {
 'base': 'e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb',
 'drawing': '788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd',
 'graphics': 'aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621',
 'libcxx': '4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4',
 'marker3_adapter': '9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8',
 'montblanc': '4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350',
 'pencommon': 'afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d',
 'renderer': 'f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed',
 'runner': 'a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8',
}
gl = {'libEGL.so.1': 'f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6', 'libGLESv2.so.2': 'a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812'}
frames = {'zigzag10': 'e2f5b8860e9cda2d47ae1c8329df45fbd98dd609b69891d727868b5bfb5f59da', 'vertical12': '045c8c626ec7a2b55a79d057f30eb7331ac47a208126f6e3873bb2d5be600586', 'turn20': 'bccdecdf8610a8377f52707c8e76a88ba69de34a8c8085b062278a0a89957146', 'loop32': '88dd9438c22afdf428296f9a2974129843a1ea3d74026f70cd4c68b14d5aca60'}
assert set(receipt['cases']) == set(expected)
prefix = 'SPen::MontblancFountainPenStrokeDrawableRTV1::'
suffix = '(SPen::Vector3<float>, SPen::Vector2<float>, float)'
for name, (n, size, first, replay, offset, input_hash, stream_hash) in expected.items():
    case = receipt['cases'][name]
    assert sha(case['input']) == input_hash
    assert len(case['input']['points']) == len(case['input']['pressure']) == n
    assert case['input']['pen_size'] == size
    assert case['input_pins'] == pins and case['host_gl_sha256'] == gl
    assert hex64.fullmatch(case['frame_sha256']) and case['frame_sha256'] == frames[name]
    stream = case['stream']
    assert sha(stream) == stream_hash
    assert len(stream) == first + 1 + replay
    assert [row[0] for row in stream] == [prefix+'AddStartPoint'+suffix] + [prefix+'AddPoint'+suffix]*(first-1) + [prefix+'AddEndPoint'+suffix] + [prefix+'AddPoint'+suffix]*replay
    assert all(len(row) == 2 and len(row[1]) == 6 and all(hex8.fullmatch(word) for word in row[1]) for row in stream)
    matches = [i for i in range(first-replay+1) if stream[i:i+replay] == stream[first+1:]]
    assert matches == [offset], (name, matches)
print('Verified fixed inputs, pins, callback words and unique replay slices; frame bytes and native libraries were not rehashed.')
