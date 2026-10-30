#!/usr/bin/env python3
"""Verify source-AA SVG diagnostics against the retained native frame."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import zlib
from PIL import Image

here = Path(__file__).resolve().parent
receipt = json.loads((here / 'identity-screen-512-receipt.json').read_text())
comparison = receipt['source_vector_comparison_crop']
assert subprocess.check_output(['resvg', '--version'], text=True).strip() == comparison['resvg']
assert len(sys.argv) == 3
raw = zlib.decompress((here / 'identity-screen-512-frame.rgba.zlib').read_bytes())
assert hashlib.sha256(raw).hexdigest() == receipt['frame_sha256']
native = b''.join(raw[y * 2048:y * 2048 + 1024] for y in range(256))
with tempfile.TemporaryDirectory(prefix='tape-aa-compare-') as tmp:
    for display, source in zip(('256', '512'), sys.argv[1:], strict=True):
        svg = Path(source)
        assert hashlib.sha256(svg.read_bytes()).hexdigest() == comparison['svg_sha256'][display]
        png = Path(tmp) / (display + '.png')
        subprocess.run(['resvg', str(svg), str(png)], check=True)
        data = Image.open(png).convert('RGBA').tobytes()
        pixels = bytes(round(v * data[i - i % 4 + 3] / 255) if i % 4 < 3 else v for i, v in enumerate(data))
        diff = [abs(a - b) for a, b in zip(native, pixels, strict=True)]
        result = {'absolute_rgba_error': sum(diff), 'maximum_channel_error': max(diff), 'different_channels': sum(v != 0 for v in diff)}
        assert result == comparison[display + '_display_aa'], (display, result)
        print(display, result)
