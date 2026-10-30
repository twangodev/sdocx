#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
sha256sum -c tools/marker3-size9-exploratory/SHA256SUMS
work=$(mktemp -d /tmp/sdocx-marker3-size9.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/work/src"
git -C "$root" archive 4361933850036a3c2e5a333814f04b499f47fe6a crates/sdocx | tar -x -C "$work/work"
mv "$work/work/crates/sdocx" "$work/work/sdocx"
cp "$root/tools/marker3-size9-exploratory/test-Cargo.lock" "$work/work/sdocx/Cargo.lock"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/work/Cargo.lock"
ln -s "$root/conformance" "$work/conformance"
ln -s "$root/tools" "$work/tools"
cp "$root/tools/marker3-size9-exploratory/export.rs" "$work/work/src/main.rs"
cat > "$work/work/Cargo.toml" <<MANIFEST
[package]
name = "sdocx-marker3-size9-replay"
version = "0.1.0"
edition = "2024"
[dependencies]
sdocx = { path = "sdocx", features = ["pdf", "serde"] }
serde_json = "1"
MANIFEST
python_bin=${PYTHON:-python3}
"$python_bin" - "$work" "$root" <<'PY'
from pathlib import Path
import gzip, sys
work, root = map(Path, sys.argv[1:])
p = work/'work/sdocx/src/ink/marker3.rs'
s = p.read_text()
assert s.count('(4.0..=8.0)') == 1, 'unexpected size gate'
s = s.replace('(4.0..=8.0)', '(4.0..=9.0)', 1)
i = s.rfind('}')
s = s[:i] + '''    #[test]
    fn marker3_size9_probe() {
        for name in ["short9", "cross9"] {
            let path = format!("{}/{}.json", std::env::var("MARKER3_SIZE9_INPUT").unwrap(), name);
            let case: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_caller_xy_matches(&prepare_for_page(&caller_stroke(&case)).unwrap(), &case);
        }
    }
''' + s[i:]
p.write_text(s)
for name in ('short9','cross9'):
    (work/f'{name}.json').write_bytes(gzip.decompress((root/f'tools/marker3-size9-exploratory/{name}-caller.json.gz').read_bytes()))
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 MARKER3_SIZE9_INPUT="$work"
cargo test --locked --offline --manifest-path "$work/work/sdocx/Cargo.toml" --features 'pdf serde' --lib marker3_size9_probe
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in short9 cross9; do
  case "$case" in short9) arg=short;; cross9) arg=crossing;; esac
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$arg" "$work/$case"
  "$python_bin" -c 'import sys,zlib; open(sys.argv[2],"wb").write(zlib.decompress(open(sys.argv[1],"rb").read()))' "$root/tools/marker3-size9-exploratory/$case.rgba.zlib" "$work/$case.rgba"
  "$python_bin" "$root/tools/marker3-size9-exploratory/compare.py" "$work/$case.rgba" "$work/$case" "$work/$case-comparison.json" "$root/tools/marker3-size9-exploratory/$case.json"
done
