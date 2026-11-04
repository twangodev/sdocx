#!/usr/bin/env python3
"""Verify retained native geometry, frames and scale crops without exporting."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[2]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def receipt(name):
    return json.loads((ROOT / 'conformance' / name).read_text())


def check_triangles(result):
    draws, provenance = result['draw_calls'], result['draw_provenance']
    assert len(draws) == len(provenance) == 3
    assert [draw['state']['program'] for draw in draws] == [
        proof['actual_program'] for proof in provenance]
    for draw, proof in zip(draws, provenance):
        assert proof['actual_program'] in (3, 4, 5)
        attrs = {attribute['index']: attribute for attribute in proof['attributes']}
        assert {0, 1}.issubset(attrs)
        assert attrs[0]['buffer'] == attrs[1]['buffer']
        assert attrs[0]['stride'] == attrs[1]['stride'] == 20
        assert attrs[0]['type'] == attrs[1]['type'] == 5126
        upload = next(row for row in result['vbo_uploads']
                      if row.get('boundBuffer') == attrs[0]['buffer'] and row.get('dataHex'))
        raw = bytes.fromhex(upload['dataHex'])
        assert len(raw) == upload['size'] and sha(raw) == upload['dataSha256']
        assert len(raw) % 20 == 0
        vertices = list(struct.iter_unpack('<5f', raw))
        assert 0 < len(vertices) <= 4096 and len(vertices) % 3 == 0
        assert len(vertices) == draw['values'][2]


fixture = ROOT / 'crates/sdocx/tests/assets/legacy-marker/synthetic-serialized.sdocx'
assert sha(fixture.read_bytes()) == 'ec4476cba55a9e92e4d5c54eb5789e0842a223e120825009fffd004a952d7de5'
fallback = receipt('montblanc-public-fallback-host-comparison.json')
triangle = receipt('montblanc-triangle-export-comparison.json')
extended = receipt('montblanc-extended-paint-native.json')
for name in ('baseline', 'pressure', 'size', 'far_endpoint', 'four', 'five'):
    is_extended = name in ('four', 'five')
    directory = 'extended-paint' if is_extended else 'captures'
    raw = gzip.decompress((ROOT / 'tools/montblanc' / directory / f'{name}.json.gz').read_bytes())
    assert sha(raw) == (extended if is_extended else fallback)['cases'][name]['capture_sha256']
    capture = json.loads(raw)
    frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
    assert len(frame) == 256 * 256 * 4
    assert sha(frame) == capture['post_readback']['sha256']
    if not is_extended:
        assert len(raw) <= 16 * 1024 * 1024
        assert sha(raw) == triangle['cases'][name]['capture_sha256']
        check_triangles(capture)
    print(f'{name}: native capture/frame verified' + ('' if is_extended else ', triangle ABI verified'))

scale = receipt('montblanc-scene-scale-2x-comparison.json')
crop = (ROOT / 'conformance/montblanc-scale-2x.rgba').read_bytes()
assert len(crop) == 47 * 14 * 4 and sha(crop) == scale['native_crop_sha256']
print('baseline 2x: native crop verified')
for name, info in receipt('montblanc-native-scale-2x-variants.json')['cases'].items():
    crop = (ROOT / f'conformance/montblanc-scale-2x-{name}.rgba').read_bytes()
    assert sha(crop) == info['crop_sha256']
    left, top, right, bottom = info['bounds_half_open']
    assert 0 <= left < right <= 256 and 0 <= top < bottom <= 256
    row_bytes = (right - left) * 4
    assert len(crop) == row_bytes * (bottom - top)
    frame = bytearray(256 * 256 * 4)
    for row in range(bottom - top):
        start = ((top + row) * 256 + left) * 4
        frame[start:start + row_bytes] = crop[row * row_bytes:(row + 1) * row_bytes]
    assert sha(frame) == info['frame_sha256']
    print(f'{name} 2x: native crop and reconstructed frame verified')
