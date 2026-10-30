#!/usr/bin/env python3
"""Verify a retained v251 oracle against its original native capture log."""
import base64
import hashlib
import json
from pathlib import Path
import sys
import zlib

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'v251-geometry-receipt.json').read_text())
raw = Path(sys.argv[1]).read_bytes()
assert hashlib.sha256(raw).hexdigest() == receipt['native_log_sha256']
text = raw.decode()
log, _ = json.JSONDecoder().raw_decode(text[text.index('\n{') + 1:])
for key in ('input', 'profile', 'native_hashes'):
    assert log[key] == receipt[key]
uniforms = {
    event['uniform_location_binding']['name']: base64.b64decode(event['payload_base64'], validate=True).hex()
    for draw in log['mesa_draw_observations']
    for event in draw['uniform_events_since_prior_draw']
    if event['call'] == 'glUniform2fv'
}
assert uniforms == receipt['uniform_f32_le_hex']
capture = log['mesa_draw_observations'][0]['after']['pixels']
rgba = zlib.decompress(base64.b64decode(capture['payload_base64'], validate=True))
assert hashlib.sha256(rgba).hexdigest() == capture['sha256']
alpha = rgba[3::4]
assert len(alpha) == 256 * 256
assert hashlib.sha256(alpha).hexdigest() == receipt['native_mask_alpha_sha256']
assert alpha == zlib.decompress((here / 'v251-geometry-mask.alpha.zlib').read_bytes())
print('v251 original native log, uniform payloads and retained mask verified')
