#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-pencil-v197.XXXXXX)
trap 'rm -rf "$work"' EXIT
git -C "$root" archive 61fdc795adb2e2b075b6417ec9533f41cf884861 crates/sdocx README.md | tar -x -C "$work"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-direct-v195/probe.patch"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-direct-v197/injection.patch"
cp "$root/tools/pencil2-direct-v195/Cargo.lock" "$work/crates/sdocx/Cargo.lock"
cp "$root/tools/pencil2-direct-v195/generic.rs" "$work/crates/sdocx/examples/pencil_generic_probe.rs"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil_generic_probe
probe=/tmp/sdocx-agent-target/debug/examples/pencil_generic_probe
"$probe" 0.228 "$work/generic.svg" "$work/generic.pdf" ramp
for scale in 1 2; do
 SDOCX_PENCIL_TEST_SVG_SCALE=$scale "$probe" 0.228 "$work/svg$scale.svg" "$work/svg$scale.pdf" ramp
 SDOCX_PENCIL_TEST_SCALE=$scale "$probe" 0.228 "$work/pdf$scale.svg" "$work/pdf$scale.pdf" ramp
 resvg "$work/svg$scale.svg" "$work/svg$scale.png"
done
cd "$root"
PENCIL2_PROBE_DIR="$work" "${PYTHON:-.venv/bin/python}" tools/pencil2-direct-v195/generic_compare.py
PENCIL2_PROBE_DIR="$work" "${PYTHON:-.venv/bin/python}" tools/pencil2-direct-v197/compare.py
