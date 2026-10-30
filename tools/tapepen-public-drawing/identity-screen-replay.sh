#!/usr/bin/env bash
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
dir=$(mktemp -d /tmp/sdocx-tape-aa.XXXXXX)
trap 'rm -rf "$dir"' EXIT
cp "$repo/Cargo.toml" "$repo/Cargo.lock" "$dir/"
cp -a "$repo/crates" "$repo/conformance" "$dir/"
ln -s "$repo/tools" "$dir/tools"
patch -s -p0 -d "$dir" < "$repo/tools/tapepen-public-drawing/v173-source-mask.patch"
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
for display in 256 512; do
  if [[ $display == 512 ]]; then
    sed -i 's/tape_size_level(12.0, 256, 256)/tape_size_level(12.0, 512, 512)/' "$dir/crates/sdocx/src/ink/tapepen.rs"
  fi
  (cd "$dir" && SDOCX_TAPE_V173_MASK_OUT="$dir/$display.mask" cargo test -p sdocx --features render v173_source_mask --lib --offline)
  python3 "$repo/tools/tapepen-public-drawing/v173-vector.py" "$dir/$display.mask" "$dir/$display.svg" 77 59
done
if [[ -n ${SDOCX_TAPE_PYTHON:-} ]]; then
  "$SDOCX_TAPE_PYTHON" "$repo/tools/tapepen-public-drawing/identity-screen-compare.py" "$dir/256.svg" "$dir/512.svg"
else
  UV_CACHE_DIR=/tmp/sdocx-uv-cache uv run --offline --with pillow python "$repo/tools/tapepen-public-drawing/identity-screen-compare.py" "$dir/256.svg" "$dir/512.svg"
fi
