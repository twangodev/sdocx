#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
[ "$(resvg --version)" = 0.45.1 ]
work=$(mktemp -d /tmp/sdocx-pencil200.XXXXXX)
trap 'rm -rf "$work"' EXIT
git archive 2110c042a0631b120ab06cac371a06ba78fa1a6b crates/sdocx README.md | tar -x -C "$work"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
python3 tools/pencil2-extra-scene-v200/setup.py "$work"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil200
for spec in white:1 white:2 transparent:1 blue:2; do
 backdrop=${spec%:*}; scale=${spec#*:}; mkdir "$work/$spec"
 PENCIL198_DIR="$work/$spec" BACKDROP="$backdrop" SCALE="$scale" /tmp/sdocx-agent-target/debug/examples/pencil200
 for i in 0 1; do resvg "$work/$spec/vec$i.svg" "$work/$spec/vec$i.png"; done
done
PENCIL200_DIR="$work" .venv/bin/python tools/pencil2-extra-scene-v200/compare.py
