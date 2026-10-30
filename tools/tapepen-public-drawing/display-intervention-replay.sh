#!/usr/bin/env bash
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
dir=$(mktemp -d /tmp/sdocx-tape-display.XXXXXX)
trap 'rm -rf "$dir"' EXIT
cp "$repo/Cargo.toml" "$repo/Cargo.lock" "$dir/"
cp -a "$repo/crates" "$repo/conformance" "$dir/"
ln -s "$repo/tools" "$dir/tools"
patch -s -p0 -d "$dir" < "$repo/tools/tapepen-public-drawing/v173-source-mask.patch"
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
python3 "$repo/tools/tapepen-public-drawing/display-intervention-edit.py" "$dir/crates/sdocx/src/ink/tapepen.rs" aa
(cd "$dir" && SDOCX_TAPE_V173_MASK_OUT="$dir/old.mask" cargo test -p sdocx --features render v173_source_mask --lib --offline)
python3 "$repo/tools/tapepen-public-drawing/display-intervention-edit.py" "$dir/crates/sdocx/src/ink/tapepen.rs" quad
(cd "$dir" && SDOCX_TAPE_V173_MASK_OUT="$dir/quad.mask" cargo test -p sdocx --features render v173_source_mask --lib --offline)
"${SDOCX_TAPE_PYTHON:-python3}" "$repo/tools/tapepen-public-drawing/display-intervention-compare.py" "$dir"
