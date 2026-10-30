#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel); cd "$root"
work=$(mktemp -d /tmp/sdocx-pencil200-pdf.XXXXXX)
trap 'rm -rf "$work"' EXIT
git archive 61fdc795adb2e2b075b6417ec9533f41cf884861 crates/sdocx README.md | tar -x -C "$work"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-direct-v195/probe.patch"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
cp tools/pencil2-direct-v195/generic.rs "$work/crates/sdocx/examples/pencil_generic_probe.rs"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-pdf-scene-v198/marker.patch"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-scene-transport-v199/colored-marker.patch"
python3 tools/pencil2-extra-scene-v200/pdf-setup.py "$work"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil_generic_probe
for spec in white:1 white:2 transparent:1 blue:2; do
 backdrop=${spec%:*}; scale=${spec#*:}; mkdir "$work/$spec"
 PENCIL198_DIR="$work/$spec" BACKDROP="$backdrop" SCALE="$scale" SDOCX_PENCIL_TEST_SCALE="$scale" /tmp/sdocx-agent-target/debug/examples/pencil_generic_probe
done
PENCIL200_DIR="$work" "${PYTHON:-.venv/bin/python}" tools/pencil2-extra-scene-v200/pdf-compare.py
