#!/usr/bin/env bash
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
here="$repo/tools/tapepen-public-drawing"
dir=$(mktemp -d /tmp/sdocx-tape-public.XXXXXX)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/src/bin" "$dir/out"
cp "$here/v173-public.rs" "$dir/src/bin/public.rs"
cp "$here/pdf-control-Cargo.lock" "$dir/Cargo.lock"
python3 - "$dir/Cargo.toml" "$repo/crates/sdocx" <<'PY'
import json, sys
from pathlib import Path
Path(sys.argv[1]).write_text('''[package]
name = "tapepen-control-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = %s, features = ["render", "pdf", "serde"] }
serde_json = "1"
sha2 = "0.10.9"
lopdf = { version = "=0.42.0", default-features = false }
''' % json.dumps(sys.argv[2]))
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
(cd "$repo" && cargo run --offline --locked --manifest-path "$dir/Cargo.toml" --bin public -- "$dir/out")
"${SDOCX_TAPE_PYTHON:-python3}" "$here/display-public-compare.py" "$dir/out"
