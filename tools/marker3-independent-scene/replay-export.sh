#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-marker3-independent-export.XXXXXX)
trap 'rm -rf "$work"' EXIT
here="$root/tools/marker3-independent-scene"
python_bin=${PYTHON:-.venv/bin/python}
"$python_bin" - "$here/export.rs" "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$here/receipt.json" <<'PY'
import hashlib,sys
from pathlib import Path
for name,expected in zip(sys.argv[1:],('308461600284e30d43ea0ad6065d3d533411645b74f4767779c3165135ef00da','a99534dcebabcc48a45890bd7e383646b4c010869afc47f77e8b190d533bf741','d12446f74b8a50a1a2ddaf8917eac84e8c76854a1d84c22923b3c4046e564733')):
 assert hashlib.sha256(Path(name).read_bytes()).hexdigest()==expected, name
PY
mkdir -p "$work/src"
git -C "$root" archive 9eaa8fc22d01d1564cc2a87c1b445e6147488460 crates/sdocx | tar -x -C "$work"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/Cargo.lock"
cp "$here/export.rs" "$work/src/main.rs"
cat > "$work/Cargo.toml" <<'TOML'
[package]
name = "sdocx-marker3-size9-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = "crates/sdocx", features = ["pdf", "serde"] }
serde_json = "1"
TOML
"$python_bin" - "$work/crates/sdocx/src/ink/marker3.rs" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(); assert s.count('(4.0..=8.0)')==1
p.write_text(s.replace('(4.0..=8.0)','(4.0..=9.0)'))
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2
cargo build --offline --locked --manifest-path "$work/Cargo.toml"
for order in forward reverse; do
 if [ "$order" = reverse ]; then export REVERSE=1; else unset REVERSE || true; fi
 "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" phase2 "$work/$order"
 "$python_bin" - "$here/$order.rgba.zlib" "$work/$order.rgba" <<'PY'
from pathlib import Path
import sys,zlib
Path(sys.argv[2]).write_bytes(zlib.decompress(Path(sys.argv[1]).read_bytes()))
PY
 "$python_bin" "$here/compare.py" "$work/$order.rgba" "$work/$order" phase2 "$here/receipt.json" "$order"
done
