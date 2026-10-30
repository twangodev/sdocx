#!/usr/bin/env python3
"""Verify the exact v173 2× log and its v248 intermediate observations."""
import base64
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib

here = Path(__file__).resolve().parent
log = Path(sys.argv[1])
data = log.read_bytes()
assert hashlib.sha256(data).hexdigest() == 'a9e081c01cf767a31be035d49bd3a47217fb17eec1e9fffa729bd0be91bc6d01'
subprocess.run([sys.executable, str(here / 'v173-verify-native.py'), '2', str(log)], check=True)
s = data.decode()
a = json.JSONDecoder().raw_decode(s, s.index('\n{') + 1)[0]
draw = a['mesa_draw_observations']
assert len(draw) == 3 and draw[0]['before']['viewport'] == [0, 0, 512, 512]
assert draw[0]['arguments'] == [5, 0, 4, 70]
def uniforms(row):
    return {v['uniform_location_binding']['name']: base64.b64decode(v['payload_base64']) for v in row['uniform_events_since_prior_draw'] if v.get('uniform_location_binding')}
u = uniforms(draw[0])
assert struct.unpack('<ff', u['uVertexHalfExtent']) == (1.7754546403884888, 7.101818561553955)
assert struct.unpack('<ff', u['uEllipseRadii']) == (1.5, 6.0)
assert any(v.get('location') == 4 and v.get('value') == 0.1836363673210144 for v in draw[0]['before']['adapter_uniform_calls_tail'])
u = uniforms(draw[1])
assert struct.unpack('<ff', u['patternSize']) == (256.0, 256.0)
assert struct.unpack('<ff', u['grainOffset']) == (154.0, 118.0)
bind = draw[1]['before']['bound_texture_units']['captured_units'][1]
assert bind['texture_2d'] == 6 and bind['sampler_parameters'] == {'mag_filter': 9729, 'min_filter': 9729, 'wrap_s': 10497, 'wrap_t': 10497}
upload = a['mesa_texture_uploads']['events'][6]
assert upload['texture_id'] == 6 and upload['api'] == 'glTexSubImage2D' and (upload['width'], upload['height']) == (256, 256)
pattern = zlib.decompress(base64.b64decode(upload['payload']['data_base64']))
assert hashlib.sha256(pattern).hexdigest() == upload['payload']['sha256'] == '42abf1113cfd13c27f1e0ccb29affae62221c0888f5ffcdc2425e2169b71b13d'
assert pattern == zlib.decompress((here / 'v248-native-pattern.rgba.zlib').read_bytes())
p = draw[0]['after']['pixels']
rgba = zlib.decompress(base64.b64decode(p['payload_base64']))
assert hashlib.sha256(rgba).hexdigest() == p['sha256']
mask = rgba[3::4]
assert hashlib.sha256(mask).hexdigest() == '60cdce6862739174abda6dc8d815ccb738a94016518cde9c95114c21d31d3788'
assert mask == zlib.decompress((here / 'v248-native-mask.alpha.zlib').read_bytes())
print('Verified native 2× uniforms, sampler, upload and intermediate mask.')
