#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
work=$(mktemp -d /tmp/sdocx-scene203.XXXXXX)
trap 'rm -rf "$work"' EXIT
git archive 504b0654 crates/sdocx README.md | tar -x -C "$work"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
patch -s -p1 -d "$work" < tools/pencil2-scene-transaction-v203/planner.patch
sed -i 's/^mod source;/pub(crate) mod source;/' "$work/crates/sdocx/src/ink/pencil2.rs"
printf '\n#[cfg(all(feature = "pdf", feature = "serde"))]\npub mod scene203;\n' >> "$work/crates/sdocx/src/lib.rs"
cp tools/pencil2-scene-transaction-v203/scene.rs "$work/crates/sdocx/src/scene203.rs"
cp tools/pencil2-scene-transaction-v203/probe.rs "$work/crates/sdocx/examples/scene203.rs"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo run --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example scene203
