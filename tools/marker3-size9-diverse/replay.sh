#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-marker3-size9-diverse.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/work/src"
git -C "$root" archive 9eaa8fc22d01d1564cc2a87c1b445e6147488460 crates/sdocx | tar -x -C "$work/work"
mv "$work/work/crates/sdocx" "$work/work/sdocx"
ln -s "$root/conformance" "$work/conformance"
ln -s "$root/tools" "$work/tools"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/work/Cargo.lock"
cp "$root/tools/marker3-size9-diverse/export.rs" "$work/work/src/main.rs"
cat > "$work/work/Cargo.toml" <<'MANIFEST'
[package]
name = "sdocx-marker3-size9-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = "sdocx", features = ["pdf", "serde"] }
serde_json = "1"
MANIFEST
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2
touch "$work/work/sdocx/src/ink/marker3.rs"
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in reverse cross short reverse1 cross1 short1 reverse4 cross4 short4; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case-generic"
done
python3 - "$work/work/sdocx/src/ink/marker3.rs" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(); assert s.count('(4.0..=8.0)') == 1
p.write_text(s.replace('(4.0..=8.0)','(4.0..=9.0)',1))
PY
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in reverse cross short reverse1 cross1 short1 reverse4 cross4 short4; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case"
  cp "$work/$case-generic/baseline.svg" "$work/$case/generic.svg"
  cp "$work/$case-generic/baseline.pdf" "$work/$case/generic.pdf"
done
"${PYTHON:-python3}" "$root/tools/marker3-size9-diverse/verify.py" "$work"
