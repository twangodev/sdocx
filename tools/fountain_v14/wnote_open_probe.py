"""Run the native WNote probe without exposing private paths on failure."""
import hashlib
from pathlib import Path
import runpy
import sys
try:
    implementation = Path(__file__).with_name('wnote_open_probe_impl.py')
    if hashlib.sha256(implementation.read_bytes()).hexdigest() != '42a8ad513aeeb9549c089eec7950a66adf66d48faabdda775fa0dbce60223960':
        raise RuntimeError('implementation drift')
    runpy.run_path(str(implementation), run_name='__main__')
except Exception:
    sys.stderr.write('native probe failed\n')
    raise SystemExit(1) from None
