"""Verify retained native size captures, optionally against the three original logs."""
import sys, json, base64, zlib, hashlib
from pathlib import Path

base = Path(__file__).resolve().parent
receipt = json.loads((base / "v249-size-receipt.json").read_text())
if len(sys.argv) not in (1,4): raise SystemExit('pass no arguments or native logs for sizes 2, 8, and 16')
for size, filename in zip((2,8,16),sys.argv[1:]):
 raw=Path(filename).read_bytes(); text=raw.decode(); j,_=json.JSONDecoder().raw_decode(text[text.index('\n{')+1:]); expected=receipt['cases'][str(size)]
 assert hashlib.sha256(raw).hexdigest()==expected['native_log_sha256']
 assert j['input']==expected['input'] and j['profile']==expected['profile'] and j['native_hashes']==expected['native_hashes']
 uniforms={}
 for d in j['mesa_draw_observations']:
  for e in d['uniform_events_since_prior_draw']:
   if e['call']=='glUniform2fv': uniforms[e['uniform_location_binding']['name']]=base64.b64decode(e['payload_base64'],validate=True).hex()
 assert uniforms==expected['uniform_f32_le_hex']
 mask_capture=j['mesa_draw_observations'][0]['after']['pixels']; mask_rgba=zlib.decompress(base64.b64decode(mask_capture['payload_base64'])); assert hashlib.sha256(mask_rgba).hexdigest()==mask_capture['sha256']; assert hashlib.sha256(mask_rgba[3::4]).hexdigest()==expected['native_mask_alpha_sha256']
 f=j['mesa_frame_observations'][-1]['rgba8_readback_bound']; assert hashlib.sha256(zlib.decompress(base64.b64decode(f['data_base64']))).hexdigest()==expected['native_sha256']

for size in (2, 8, 16):
 expected = receipt['cases'][str(size)]
 for suffix, length, key in (
  ('frame.rgba.zlib', 256 * 256 * 4, 'native_sha256'),
  ('mask.alpha.zlib', 256 * 256, 'native_mask_alpha_sha256'),
 ):
  data = zlib.decompress((base / f'v249-size{size}-{suffix}').read_bytes())
  assert len(data) == length, (size, suffix, len(data))
  assert hashlib.sha256(data).hexdigest() == expected[key], (size, suffix)
print('Verified retained native frames and mask alpha for sizes 2, 8, and 16.')
