#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-marker3-size9-diverse.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/work/src"
git -C "$root" archive 9b5346be crates/sdocx | tar -x -C "$work/work"
mv "$work/work/crates/sdocx" "$work/work/sdocx"
ln -s "$root/conformance" "$work/conformance"
ln -s "$root/tools" "$work/tools"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/work/Cargo.lock"
cp "$root/tools/marker3-size9-note-derived/export.rs" "$work/work/src/main.rs"
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
for case in note1 note2 note4; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case"
done
python3 - "$work/work/sdocx/src/ink/marker3.rs" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); s=p.read_text(); assert s.count('(4.0..=9.0)') == 1
p.write_text(s.replace('(4.0..=9.0)','(4.0..=8.0)',1))
PY
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in note1 note2 note4; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case-generic"
  cp "$work/$case-generic/baseline.svg" "$work/$case/generic.svg"
  cp "$work/$case-generic/baseline.pdf" "$work/$case/generic.pdf"
done
# Rebuild the candidate sampler against the same archived crate; retain all
# three render modes for the same source and export options.
cp "$root/crates/sdocx/src/ink/marker3.rs" "$work/work/sdocx/src/ink/marker3.rs"
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in note1 note2 note4; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case-candidate"
  cp "$work/$case/generic.svg" "$work/$case-candidate/generic.svg"
  cp "$work/$case/generic.pdf" "$work/$case-candidate/generic.pdf"
done
for case in note1 note2 note4; do
  "${PYTHON:-python3}" - "$root/tools/marker3-size9-note-derived/$case.rgba.zlib" "$work/$case.rgba" <<'PY2'
import sys,zlib
from pathlib import Path
Path(sys.argv[2]).write_bytes(zlib.decompress(Path(sys.argv[1]).read_bytes()))
PY2
  "${PYTHON:-python3}" "$root/tools/marker3-size9-note-derived/verify.py" "$work/$case.rgba" "$work/$case" "$case"
  "${PYTHON:-python3}" "$root/tools/marker3-size9-note-derived/verify.py" "$work/$case.rgba" "$work/$case-candidate" "$case" candidate-comparison.json
done
