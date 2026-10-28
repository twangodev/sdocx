#!/usr/bin/env python3
"""Compare retained native frames with the triangle prototype (requires resvg and Pillow)."""
import base64
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zlib

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
RECEIPT = ROOT / 'conformance/montblanc-triangle-export-comparison.json'
TOOL = ROOT / 'tools/montblanc/triangle_svg_prototype.py'


def main():
    receipt = json.loads(RECEIPT.read_text())
    with tempfile.TemporaryDirectory(prefix='montblanc-compare-') as temp:
        temp = Path(temp)
        for case, expected in receipt['cases'].items():
            raw = gzip.decompress((TOOL.parent / 'captures' / f'{case}.json.gz').read_bytes())
            assert hashlib.sha256(raw).hexdigest() == expected['capture_sha256']
            capture = json.loads(raw)
            frame = zlib.decompress(base64.b64decode(capture['post_readback']['rgba8_zlib_base64']))
            assert len(frame) == 256 * 256 * 4
            assert hashlib.sha256(frame).hexdigest() == capture['post_readback']['sha256']
            source, svg, png = (temp / f'{case}{ext}' for ext in ('.json', '.svg', '.png'))
            source.write_bytes(raw)
            subprocess.run(['python3', str(TOOL), str(source), str(svg)], check=True, stdout=subprocess.DEVNULL)
            subprocess.run(['resvg', str(svg), str(png)], check=True)
            image = Image.open(png).convert('RGBA')
            assert image.size == (256, 256)
            pixels = image.tobytes()
            native = [frame[((255-y)*256+x)*4+3] for y in range(256) for x in range(256)]
            actual = pixels[3::4]
            values = [sum(a > 0 for a in native), sum(b > 0 for b in actual),
                      sum(a > 0 and b > 0 for a, b in zip(native, actual)),
                      sum(a > 0 or b > 0 for a, b in zip(native, actual)),
                      sum(abs(a-b) for a, b in zip(native, actual))]
            print(case, values)
            if values != expected['values']:
                raise SystemExit(f'{case}: mismatch with receipt')


if __name__ == '__main__':
    main()
