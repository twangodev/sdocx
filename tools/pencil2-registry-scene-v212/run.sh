#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
work=$(mktemp -d /tmp/sdocx-pencil212.XXXXXX)
trap 'rm -rf "$work"' EXIT
git archive b4bfeacb04f12b61ce3490cff6c5be498a9cfffe | tar -x -C "$work"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
cat tools/pencil2-registry-scene-v212/form.rs >> "$work/crates/sdocx/src/render/legacy_marker_vector/pdf.rs"
printf '\n#[cfg(all(test, feature = "pdf", feature = "serde"))]\nmod pencil212;\n' >> "$work/crates/sdocx/src/render.rs"
cp tools/pencil2-registry-scene-v212/test.rs "$work/crates/sdocx/src/render/pencil212.rs"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --lib pencil212:: -- --nocapture
