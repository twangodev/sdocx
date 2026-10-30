#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
[ "$(resvg --version)" = '0.45.1' ]
sha256sum -c <<'PINS'
d70b825a5ef88d4bd9f81972a5371a87ca4f5cd728afd128e219787188bd0ed5  tools/pencil2-direct-v195/Cargo.lock
PINS
work=$(mktemp -d /tmp/sdocx-pencil198.XXXXXX)
trap 'rm -rf "$work"' EXIT
git archive 2110c042a0631b120ab06cac371a06ba78fa1a6b crates/sdocx README.md | tar -x -C "$work"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
cat tools/pencil2-svg-scene-v198/helper.rs >> "$work/crates/sdocx/src/lib.rs"
cp tools/pencil2-svg-scene-v198/probe.rs "$work/crates/sdocx/examples/pencil198.rs"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil198
PENCIL198_DIR="$work" /tmp/sdocx-agent-target/debug/examples/pencil198
for name in vec0 vec1 generic0 generic1; do resvg "$work/$name.svg" "$work/$name.png"; done
PENCIL198_DIR="$work" .venv/bin/python tools/pencil2-svg-scene-v198/compare.py
