#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-marker3-two-color.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/src"
git -C "$root" archive 9eaa8fc22d01d1564cc2a87c1b445e6147488460 crates/sdocx | tar -x -C "$work"
mv "$work/crates/sdocx" "$work/sdocx"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/Cargo.lock"
cp "$root/tools/marker3-two-color-scene/export.rs" "$work/src/main.rs"
cat > "$work/Cargo.toml" <<'EOF'
[package]
name = "sdocx-marker3-size9-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = "sdocx", features = ["pdf", "serde"] }
serde_json = "1"
EOF
python3 - "$work/sdocx/src/ink/marker3.rs" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(); assert s.count('(4.0..=8.0)') == 1
p.write_text(s.replace('(4.0..=8.0)', '(4.0..=9.0)'))
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2
cargo build --locked --offline --manifest-path "$work/Cargo.toml"
python_bin=${PYTHON:-python3}
for case in forward reverse; do
  if [ "$case" = reverse ]; then export REVERSE=1; else unset REVERSE || true; fi
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" phase2 "$work/$case"
  "$python_bin" -c 'import sys,zlib; open(sys.argv[2],"wb").write(zlib.decompress(open(sys.argv[1],"rb").read()))' "$root/tools/marker3-two-color-scene/$case.rgba.zlib" "$work/$case.rgba"
  "$python_bin" "$root/tools/marker3-two-color-scene/compare.py" "$work/$case.rgba" "$work/$case" phase2 "$root/tools/marker3-two-color-scene/$case.json" "$case"
done
