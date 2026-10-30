#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test --offline --locked --manifest-path "$root/tools/pencil2-pdf-transaction-v202/Cargo.toml"
