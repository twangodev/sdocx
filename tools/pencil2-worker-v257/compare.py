import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zlib

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix='pencil257-compare-') as tmp:
    prior, output = Path(tmp) / 'prior', Path(tmp) / 'rust'
    prior.write_bytes(bytes(192 * 192 * 4))
    env = dict(os.environ, CARGO_TARGET_DIR='/tmp/sdocx-agent-target', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_BUILD_JOBS='2', SDOCX_PENCIL2_PRIOR=str(prior), SDOCX_PENCIL2_OUTPUT=str(output))
    subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(root / 'tools/pencil2-worker-v257/Cargo.toml')], cwd=root, env=env, check=True)
    rust = output.read_bytes()
    native = zlib.decompress((root / 'tools/pencil2-worker-v256/rgba8-frame.zlib').read_bytes())
    assert len(rust) == len(native) == 192 * 192 * 4
    a, b = native[3::4], rust[3::4]
    result = {'native_alpha_nonzero': sum(v != 0 for v in a), 'rust_alpha_nonzero': sum(v != 0 for v in b), 'different_pixels': sum(x != y for x, y in zip(a, b)), 'maximum_absolute_difference': max(abs(x-y) for x, y in zip(a, b)), 'sum_absolute_difference': sum(abs(x-y) for x, y in zip(a, b)), 'rust_sha256': hashlib.sha256(rust).hexdigest()}
    print(json.dumps(result, indent=2))
