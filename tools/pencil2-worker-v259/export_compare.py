import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

import pymupdf
import pypdfium2
from PIL import Image

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='pencil259-export-') as tmp:
    output = Path(tmp)
    prior = output / 'prior'
    prior.write_bytes(bytes(192 * 192 * 4))
    env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_PENCIL2_PRIOR=str(prior), SDOCX_PENCIL2_OUTPUT=str(output / 'frame'), SDOCX_PENCIL2_EXPORT_DIR=str(output))
    subprocess.run(['cargo', 'run', '--offline', '--locked', '--quiet', '--manifest-path', str(root / 'tools/pencil2-worker-v259/Cargo.toml'), '--features', 'export-pdf'], cwd=root, env=env, check=True)
    compressed = (root / 'tools/pencil2-worker-v256/rgba8-frame.zlib').read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == '4f33e63b6c260b7fecc281ccf5891b19b3a96403b552da3a757e6faf2ca8e983'
    native = zlib.decompress(compressed)
    native = b''.join(native[y * 768:(y + 1) * 768] for y in reversed(range(192)))[3::4]
    subprocess.run(['resvg', str(output / 'canvas.svg'), str(output / 'svg.png')], check=True)
    svg = Image.open(output / 'svg.png').convert('RGBA').tobytes()[3::4]
    with pymupdf.open(output / 'canvas.pdf') as document:
        mu = document[0].get_pixmap(alpha=True).samples[3::4]
    with pypdfium2.PdfDocument(output / 'canvas.pdf') as document:
        pi = document[0].render(scale=1, rev_byteorder=True, fill_color=(0, 0, 0, 0)).to_pil().convert('RGBA').tobytes()[3::4]
    for name, actual, expected in [('resvg', svg, (0, 0, 0)), ('MuPDF', mu, (80, 1, 80)), ('PDFium', pi, (0, 0, 0))]:
        assert len(actual) == len(native)
        errors = [abs(a - b) for a, b in zip(actual, native)]
        result = (sum(errors), max(errors), sum(v != 0 for v in errors))
        print(name, 'sum, max, differing pixels:', result)
        assert result == expected
