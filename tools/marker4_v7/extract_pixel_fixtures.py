"""Verify native capture bytes and extract cropped alpha PNG fixtures for Rust tests."""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('capture', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
meta = json.loads(args.capture.read_text())
raw = args.capture.with_suffix('.rgba').read_bytes()
if hashlib.sha256(raw).hexdigest() != meta['rgba_sha256']:
    raise SystemExit('native capture hash mismatch')
image = Image.frombytes('RGBA', tuple(meta['target']), raw).transpose(Image.Transpose.FLIP_TOP_BOTTOM).getchannel('A')
bounds = image.getbbox()
if bounds is None or args.output.exists():
    raise SystemExit('empty capture or output already exists')
image.crop(bounds).save(args.output, optimize=True)
print(json.dumps({'case': meta['case'], 'bounds': bounds, 'native_rgba_sha256': meta['rgba_sha256'],
                  'png_sha256': hashlib.sha256(args.output.read_bytes()).hexdigest()}))
