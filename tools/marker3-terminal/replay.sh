#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
sha256sum -c tools/marker3-terminal/SHA256SUMS
work=$(mktemp -d /tmp/sdocx-marker3-terminal.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/project/src"
git archive 5b0f0c9b1b91e8f13353cbfe939f1b86563f8385 crates/sdocx | tar -x -C "$work"
git -C "$work" apply "$root/tools/marker3-terminal/renderer.patch"
mv "$work/crates/sdocx" "$work/project/sdocx"
ln -s "$root/conformance" "$work/conformance"
ln -s "$root/tools" "$work/tools"
cp tools/marker3-terminal/test-Cargo.lock "$work/project/sdocx/Cargo.lock"
cp tools/marker3-terminal/export-Cargo.lock "$work/project/Cargo.lock"
cp tools/marker3-terminal/export.rs "$work/project/src/main.rs"
cat > "$work/project/Cargo.toml" <<'MANIFEST'
[package]
name = "marker3-terminal-export"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = "sdocx", features = ["pdf", "serde"] }
serde_json = "1"
MANIFEST
python_bin=${PYTHON:-python3}
"$python_bin" - "$root" <<'PY'
from pathlib import Path
import gzip,json,sys
root=Path(sys.argv[1]); fixture=json.loads((root/'conformance/marker3-terminal-native.json').read_text())
for name in ('inside','near','two'):
    case=json.loads(gzip.decompress((root/f'tools/marker3-terminal/{name}-caller.json.gz').read_bytes()))['cases']['marker3_linked_saved1:5:tool1:curve1']
    assert fixture[name] == {'input':case['input'],'native_calls':{'source_ObjectStroke_arrays':case['native_calls']['source_ObjectStroke_arrays']},'phase_final_vectors':case['phase_final_vectors']}
for name in ('phase2','phase4','boundary'):
    case=json.loads(gzip.decompress((root/f'tools/marker3-size9-expanded/{name}-caller.json.gz').read_bytes()))
    assert fixture[name] == {'input':case['input'],'native_calls':{'source_ObjectStroke_arrays':case['native_calls']['source_ObjectStroke_arrays']},'phase_final_vectors':case['phase_final_vectors']}
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2
cargo test --locked --offline --manifest-path "$work/project/sdocx/Cargo.toml" --features 'pdf serde' --lib ink::marker3::tests
cargo build --locked --offline --manifest-path "$work/project/Cargo.toml"
for case in inside near two; do
  "$CARGO_TARGET_DIR/debug/marker3-terminal-export" "$case" "$work/$case"
  "$python_bin" -c 'import sys,zlib; open(sys.argv[2],"wb").write(zlib.decompress(open(sys.argv[1],"rb").read()))' "$root/tools/marker3-terminal/$case.rgba.zlib" "$work/$case.rgba"
  "$python_bin" "$root/tools/marker3-terminal/compare.py" "$work/$case.rgba" "$work/$case" "$case" "$root/tools/marker3-terminal/$case.json"
done
