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
    for target in ("upstream_texture",):
        subprocess.run(["cc", "-O2", "-I", "/tmp", str(here / (target + ".c")), "-lm", "-o", str(root / target)], check=True)
    subprocess.run(["rustc", "-O", "--edition=2024", str(here / "asset_cli.rs"), "-o", str(root / "rust")], check=True)
    for case, svg in cases.items():
        expected_source = receipt["source_sha256"] if case == "original" else receipt["changed_cases"][case]["source_sha256"]
        assert digest(svg) == expected_source
        path = root / (case + ".svg")
        path.write_bytes(svg)
        for size in (64, 128):
            args = [str(path), str(size)]
            rust = subprocess.check_output([str(root / "rust"), *args])
            upstream = subprocess.check_output([str(root / "upstream_texture"), *args])
            assert rust == upstream
            expected = receipt["original_rgba_sha256"][str(size)] if case == "original" else receipt["changed_cases"][case]["rgba_sha256"][str(size)]
            assert digest(rust) == expected
            print(case, size, digest(rust))

    # This width falls below the scaled cutoff at 64 and above it at 128.
    thin = root / "thin.svg"
    thin.write_bytes(source.replace(b'stroke-width="1.7"', b'stroke-width="0.015"'))
    for size in (64, 128):
        args = [str(thin), str(size)]
        rust = subprocess.check_output([str(root / "rust"), *args])
        upstream = subprocess.check_output([str(root / "upstream_texture"), *args])
        assert rust == upstream
        print("thin", size, digest(rust))

    # 0.01874999888241291f32 * (64f32 / 120f32) equals 0.01f32.
    boundary = root / "boundary.svg"
    boundary.write_bytes(source.replace(b'stroke-width="1.7"', b'stroke-width="0.01874999888241291"'))
    args = [str(boundary), "64"]
    rust = subprocess.check_output([str(root / "rust"), *args])
    upstream = subprocess.check_output([str(root / "upstream_texture"), *args])
    assert rust == upstream
    assert rust == subprocess.check_output([str(root / "rust"), str(thin), "64"])
    print("boundary", 64, digest(rust))
    for invalid in (b'#XXXXXX', b'#3F548', b'#3F5480'):
        broken = source.replace(b'#3F5480', invalid)
        if invalid == b'#3F5480':
            broken = broken.replace(b'M7.495 4.5V10.5', b'MNaN 4.5V10.5', 1)
        broken = broken.replace(b'stroke-width="1.7"', b'stroke-width="0.015"')
        path = root / "invalid.svg"
        path.write_bytes(broken)
        assert subprocess.run([str(root / "rust"), str(path), "64"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0

    invalid_cases = {
        "truncated": source[:-20],
        "extra": source + b"extra\n",
        "huge_width": source.replace(b'stroke-width="1.7"', b'stroke-width="1e30"'),
        "nan_width": source.replace(b'stroke-width="1.7"', b'stroke-width="NaN"'),
        "degenerate": source.replace(b'M7.495 4.5V10.5', b'M7.495 4.5V4.5', 1),
    }
    for name, svg in invalid_cases.items():
        path = root / (name + ".svg")
        path.write_bytes(svg)
        result = subprocess.run([str(root / "rust"), str(path), "64"], capture_output=True)
        assert result.returncode != 0 and not result.stdout
        assert b"panicked" not in result.stderr
        print(name, "rejected without panic")
