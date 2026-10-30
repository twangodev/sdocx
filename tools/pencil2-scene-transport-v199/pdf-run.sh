#!/usr/bin/env bash
set -euo pipefail
unset SDOCX_PENCIL_TEST_SCALE SDOCX_PENCIL_TEST_SVG_SCALE
root=$(git rev-parse --show-toplevel)
cd "$root"
work=$(mktemp -d /tmp/sdocx-pencil199-pdf.XXXXXX)
trap 'rm -rf "$work"' EXIT
sha256sum --check <<'EOF'
506a7db4204106f0920bc11ba24b75180744237e58c76e2eb09bffb21885e080  tools/pencil2-direct-v195/probe.patch
8259f07774e23fb15a93eb5af693c5e9e0675da6d03d914c31a71e41a51af21c  tools/pencil2-direct-v195/generic.rs
d70b825a5ef88d4bd9f81972a5371a87ca4f5cd728afd128e219787188bd0ed5  tools/pencil2-direct-v195/Cargo.lock
3140d13e7b0d4affd95142d8a2ca6e875f1a743e566e4b4d561db8a8aa143858  tools/pencil2-pdf-scene-v198/marker.patch
6082f08819058ff42b88b9b120411b01babfd80fee4182767f213a3901f0517f  tools/pencil2-scene-transport-v199/colored-marker.patch
EOF
git archive 61fdc795adb2e2b075b6417ec9533f41cf884861 crates/sdocx README.md | tar -x -C "$work"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-direct-v195/probe.patch"
cp tools/pencil2-direct-v195/Cargo.lock "$work/crates/sdocx/Cargo.lock"
cp tools/pencil2-direct-v195/generic.rs "$work/crates/sdocx/examples/pencil_generic_probe.rs"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-pdf-scene-v198/marker.patch"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-scene-transport-v199/colored-marker.patch"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil_generic_probe
mkdir "$work/native" "$work/generic"
PENCIL198_DIR="$work/native" SDOCX_PENCIL_TEST_SCALE=1 /tmp/sdocx-agent-target/debug/examples/pencil_generic_probe
PENCIL198_DIR="$work/generic" /tmp/sdocx-agent-target/debug/examples/pencil_generic_probe
PENCIL199_PDF_DIR="$work" "${PYTHON:-.venv/bin/python}" tools/pencil2-scene-transport-v199/pdf-compare.py
