#!/usr/bin/env python3
"""Compare Rust scan/blend to pinned upstream; native hashes are retained observations."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

if not __debug__:
    raise SystemExit("Run without Python optimization; verification requires assertions.")

here = Path(__file__).resolve().parent
receipt = json.loads((here / "texture-receipt.json").read_text())
source = Path("scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg").read_bytes()
def digest(data):
    return hashlib.sha256(data).hexdigest()
for name, expected in receipt["headers"].items():
    assert digest((Path("/tmp") / name).read_bytes()) == expected
assert digest(source) == receipt["source_sha256"]
assert source.count(b"#3F5480") == source.count(b'stroke-width="1.7"') == 136
cases = {"original": source, "color": source.replace(b"#3F5480", b"#AD2244"),
         "geometry": source.replace(b'stroke-width="1.7"', b'stroke-width="2.3"')}
with tempfile.TemporaryDirectory(prefix="sdocx-tape-texture-") as tmp:
    root = Path(tmp)
    for target in ("upstream_paths", "upstream_texture"):
        subprocess.run(["cc", "-O2", "-I", "/tmp", str(here / (target + ".c")), "-lm", "-o", str(root / target)], check=True)
    subprocess.run(["rustc", "-O", "--edition=2024", str(here / "path_compare.rs"), "-o", str(root / "rust")], check=True)
    for case, svg in cases.items():
        expected_source = receipt["source_sha256"] if case == "original" else receipt["changed_cases"][case]["source_sha256"]
        assert digest(svg) == expected_source
        path = root / (case + ".svg")
        path.write_bytes(svg)
        for size in (64, 128):
            args = [str(path), str(size)]
            edges = subprocess.check_output([str(root / "upstream_paths"), *args])
            rust = subprocess.run([str(root / "rust")], input=edges, check=True, stdout=subprocess.PIPE).stdout
            upstream = subprocess.check_output([str(root / "upstream_texture"), *args])
            assert rust == upstream
            expected = receipt["original_rgba_sha256"][str(size)] if case == "original" else receipt["changed_cases"][case]["rgba_sha256"][str(size)]
            assert digest(rust) == expected
            print(case, size, digest(rust))
