#!/usr/bin/env bash
set -euo pipefail
unset SDOCX_PENCIL_TEST_SCALE SDOCX_PENCIL_TEST_SVG_SCALE
root=$(git rev-parse --show-toplevel)
work=$(mktemp -d /tmp/sdocx-pencil-v198.XXXXXX)
trap 'rm -rf "$work"' EXIT
cd "$root"
[[ $(resvg --version) == "0.45.1" ]] || { echo "Expected resvg 0.45.1" >&2; exit 1; }
sha256sum --check <<'EOF'
506a7db4204106f0920bc11ba24b75180744237e58c76e2eb09bffb21885e080  tools/pencil2-direct-v195/probe.patch
8259f07774e23fb15a93eb5af693c5e9e0675da6d03d914c31a71e41a51af21c  tools/pencil2-direct-v195/generic.rs
d70b825a5ef88d4bd9f81972a5371a87ca4f5cd728afd128e219787188bd0ed5  tools/pencil2-direct-v195/Cargo.lock
fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b  conformance/pencil2-v14-device-profile/baseline.alpha
e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4  conformance/pencil2-v14-device-profile/scale2.alpha
EOF
git archive 61fdc795adb2e2b075b6417ec9533f41cf884861 crates/sdocx README.md | tar -x -C "$work"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-direct-v195/probe.patch"
cp "$root/tools/pencil2-direct-v195/Cargo.lock" "$work/crates/sdocx/Cargo.lock"
cp "$root/tools/pencil2-direct-v195/generic.rs" "$work/crates/sdocx/examples/pencil_generic_probe.rs"
patch -s -p0 -d "$work" -i "$root/tools/pencil2-pdf-scene-v198/marker.patch"
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --offline --locked --manifest-path "$work/crates/sdocx/Cargo.toml" --features pdf,serde --example pencil_generic_probe
probe=/tmp/sdocx-agent-target/debug/examples/pencil_generic_probe
for scale in 1 2; do
 SDOCX_PENCIL_TEST_SVG_SCALE=$scale "$probe" 0.228 "$work/svg$scale.svg" "$work/svg$scale.pdf" ramp
 resvg "$work/svg$scale.svg" "$work/svg$scale.png"
 SDOCX_PENCIL_TEST_SCALE=$scale "$probe" 0.228 "$work/pdf$scale.svg" "$work/pdf$scale.pdf" ramp
 SDOCX_PENCIL_TEST_SCALE=$scale "$probe" 0.228 "$work/scene$scale.svg" "$work/scene$scale.pdf" ramp scene
 SDOCX_PENCIL_TEST_SCALE=$scale "$probe" 0.228 "$work/reverse$scale.svg" "$work/reverse$scale.pdf" ramp scene reverse
done
PENCIL2_PROBE_DIR="$work" "${PYTHON:-.venv/bin/python}" tools/pencil2-pdf-scene-v198/compare.py
