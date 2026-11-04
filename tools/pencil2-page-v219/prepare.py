"""Restore pinned baseline and local runtime links without copying binaries."""
from pathlib import Path
import gzip
import hashlib
import sys

if sys.flags.optimize:
    raise SystemExit("Run without Python optimization; probes require assertions")

here = Path(__file__).resolve().parent
root = here.parents[1]
native = here / "native"
baseline = native / "tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v10/captures/channel_baseline/capture.json"
for directory in baseline.parents:
    if directory.is_symlink():
        raise SystemExit(f"Refusing symlink ancestor: {directory}")
archive = root / "tools/pencil2-drawing-capture-closure/sources/v10_baseline_capture.json.gz"
raw = gzip.decompress(archive.read_bytes())
if hashlib.sha256(raw).hexdigest() != "469b54b231de1b6a8672c36130fda2e2617da53cfe90b0e188442610e2cd5eea":
    raise SystemExit("Baseline digest mismatch")
if baseline.is_symlink():
    raise SystemExit("Refusing baseline symlink")
if baseline.exists():
    if baseline.read_bytes() != raw:
        raise SystemExit("Refusing to replace different baseline")
else:
    baseline.parent.mkdir(parents=True, exist_ok=True)
    with baseline.open("xb") as output:
        output.write(raw)
for link, target in (
    (native / "scratch", "../../../scratch"),
    (native / "tmp/vector-parity-plan/chinesebrush-paint-scale-work/scratch", "../../../scratch"),
):
    if any(parent.is_symlink() for parent in link.parents):
        raise SystemExit(f"Refusing runtime link with symlink ancestor: {link}")
    expected = root / "scratch"
    if not expected.is_dir() or expected.is_symlink():
        raise SystemExit("Provide the local scratch runtime directory first")
    if link.is_symlink():
        if link.resolve() != expected.resolve():
            raise SystemExit(f"Unexpected runtime link: {link}")
    elif link.exists():
        raise SystemExit(f"Refusing existing runtime path: {link}")
    else:
        link.symlink_to(target, target_is_directory=True)
print("Restored pinned baseline and local runtime links")
