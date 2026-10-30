import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='pencil258-') as tmp:
    prior, output = Path(tmp) / 'prior', Path(tmp) / 'rust'
    prior.write_bytes(bytes(192 * 192 * 4))
    env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_PENCIL2_PRIOR=str(prior), SDOCX_PENCIL2_OUTPUT=str(output))
    subprocess.run(['cargo', 'run', '--offline', '--locked', '--quiet', '--manifest-path', str(root / 'tools/pencil2-worker-v258/Cargo.toml')], cwd=root, env=env, check=True)
    rust = output.read_bytes()
    compressed = (root / 'tools/pencil2-worker-v256/rgba8-frame.zlib').read_bytes()
    assert hashlib.sha256(compressed).hexdigest() == '4f33e63b6c260b7fecc281ccf5891b19b3a96403b552da3a757e6faf2ca8e983'
    native = zlib.decompress(compressed)
    assert len(rust) == len(native) == 192 * 192 * 4
    # Rust pixels are top-row-first; GL readback is bottom-row-first.
    native = b''.join(native[y * 192 * 4:(y + 1) * 192 * 4] for y in reversed(range(192)))
    a, b = native[3::4], rust[3::4]
    result = {'native_alpha_nonzero': sum(v != 0 for v in a), 'rust_alpha_nonzero': sum(v != 0 for v in b), 'different_pixels': sum(x != y for x, y in zip(a, b)), 'maximum_absolute_difference': max(abs(x-y) for x, y in zip(a, b)), 'sum_absolute_difference': sum(abs(x-y) for x, y in zip(a, b)), 'rust_sha256': hashlib.sha256(rust).hexdigest()}
    expected = {'native_alpha_nonzero': 87, 'rust_alpha_nonzero': 88, 'different_pixels': 79, 'maximum_absolute_difference': 85, 'sum_absolute_difference': 1616, 'rust_sha256': 'bc9459001d52c773752d80c583c971d0d4292740eb12122bd295ddd838b94f3e'}
    assert result == expected, result
    print(json.dumps(result, indent=2))
