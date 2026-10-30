#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d /tmp/sdocx-pencil2-second-geometry.XXXXXXXX)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/crates"
cp -a "$root/crates/sdocx" "$tmp/crates/sdocx"
cp "$root/Cargo.lock" "$tmp/Cargo.lock"
ln -s "$root/conformance" "$tmp/conformance"
ln -s "$root/tools" "$tmp/tools"
ln -s "$root/README.md" "$tmp/README.md"
printf '[workspace]\nmembers = ["crates/sdocx"]\nresolver = "2"\n' > "$tmp/Cargo.toml"
cp "$root/tools/pencil2-second-source/geometry_test.rs" "$tmp/crates/sdocx/src/ink/pencil2/serialized_geometry_dump.rs"
printf "\n#[cfg(all(test, feature = \"serde\", feature = \"render\"))]\nmod serialized_geometry_dump;\n" >> "$tmp/crates/sdocx/src/ink/pencil2.rs"
cd "$tmp"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde serialized_geometry_dump -- --nocapture
