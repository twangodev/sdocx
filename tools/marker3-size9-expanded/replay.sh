#!/usr/bin/env bash
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
sha256sum -c tools/marker3-size9-expanded/SHA256SUMS

work=$(mktemp -d /tmp/sdocx-marker3-size9.XXXXXX)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/work/src"
git -C "$root" archive 4361933850036a3c2e5a333814f04b499f47fe6a crates/sdocx | tar -x -C "$work/work"
mv "$work/work/crates/sdocx" "$work/work/sdocx"
cp "$root/tools/marker3-size9-exploratory/test-Cargo.lock" "$work/work/sdocx/Cargo.lock"
cp "$root/tools/marker3-size9-exploratory/replay-Cargo.lock" "$work/work/Cargo.lock"
ln -s "$root/conformance" "$work/conformance"
ln -s "$root/tools" "$work/tools"
cp "$root/tools/marker3-size9-expanded/export.rs" "$work/work/src/main.rs"
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
import gzip, sys, json, struct
work, root = map(Path, sys.argv[1:])
p = work/'work/sdocx/src/ink/marker3.rs'
s = p.read_text()
assert s.count('(4.0..=8.0)') == 1, 'unexpected size gate'
s = s.replace('(4.0..=8.0)', '(4.0..=9.0001)', 1)
i = s.rfind('}')
s = s[:i] + '''    #[test]
    fn marker3_size9_probe() {
        for name in ["phase2", "phase4", "boundary"] {
            let path = format!("{}/{}.json", std::env::var("MARKER3_SIZE9_INPUT").unwrap(), name);
            let case: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_caller_xy_matches(&prepare_for_page(&caller_stroke(&case)).unwrap(), &case);
        }
    }
''' + s[i:]
p.write_text(s)
inputs = {
 'phase2': ([[22.125,25.375],[31.125,36.375],[43.125,26.375],[53.125,39.375]],[1.0,0.0,0.65,0.25],9.0),
 'phase4': ([[18.375,19.125],[25.375,28.125],[34.375,20.125],[43.375,29.125]],[0.02,0.95,0.15,0.8],9.0),
 'boundary': ([[21.75,30.25],[39.75,43.25],[54.75,27.25],[67.75,40.25]],[0.9,0.1,0.7,0.3],9.0001),
}
export = (root/'tools/marker3-size9-expanded/export.rs').read_text().replace(' ', '')
for name, (points, pressure, size) in inputs.items():
    case=json.loads(gzip.decompress((root/f'tools/marker3-size9-expanded/{name}-caller.json.gz').read_bytes()))
    data=case['input']; bits=lambda x: struct.pack('<f',x).hex()
    assert data['points_f32_bits'] == [[bits(x) for x in pair] for pair in points]
    assert data['pressure_f32_bits'] == [bits(x) for x in pressure]
    assert data['pen_size_f32_bits'] == bits(size)
    assert data['tilt_f32_bits'] == data['orientation_f32_bits'] == [bits(0)]*len(points)
    assert data['timestamps_i32'] == [8*i for i in range(len(points))]
    source=case['native_calls']['source_ObjectStroke_arrays']
    assert source['points']['data_hex'] == ''.join(bits(x) for pair in points for x in pair)
    assert source['pressures']['data_hex'] == ''.join(bits(x) for x in pressure)
    assert source['tilts']['data_hex'] == source['orientations']['data_hex'] == bits(0)*len(points)
    assert source['timestamps']['data_hex'] == ''.join(struct.pack('<i',8*i).hex() for i in range(len(points)))
    assert f'"{name}"=>(vec!{json.dumps(points,separators=(",",":"))},vec!{json.dumps(pressure,separators=(",",":"))},' in export
    assert 'timestamps:(0..coords.len()).map(|i|(8*i)asi64).collect()' in export
    assert 'tilts:vec![0.0;coords.len()],orientations:vec![0.0;coords.len()]' in export
for name in ('phase2','phase4','boundary'):
    (work/f'{name}.json').write_bytes(gzip.decompress((root/f'tools/marker3-size9-expanded/{name}-caller.json.gz').read_bytes()))
PY
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 MARKER3_SIZE9_INPUT="$work"
if cargo test --locked --offline --manifest-path "$work/work/sdocx/Cargo.toml" --features 'pdf serde' --lib marker3_size9_probe > "$work/geometry.log" 2>&1; then
  cat "$work/geometry.log"; echo 'unexpected geometry match'; exit 1
else
  cat "$work/geometry.log"; grep -q 'left: 12' "$work/geometry.log"; grep -q 'right: 37' "$work/geometry.log"; echo 'expected geometry counterexample observed; continuing to output comparisons'
fi
cargo build --locked --offline --manifest-path "$work/work/Cargo.toml"
for case in phase2 phase4 boundary; do
  "$CARGO_TARGET_DIR/debug/sdocx-marker3-size9-replay" "$case" "$work/$case"
  "$python_bin" -c 'import sys,zlib; open(sys.argv[2],"wb").write(zlib.decompress(open(sys.argv[1],"rb").read()))' "$root/tools/marker3-size9-expanded/$case.rgba.zlib" "$work/$case.rgba"
  "$python_bin" "$root/tools/marker3-size9-expanded/compare.py" "$work/$case.rgba" "$work/$case" "$case" "$root/tools/marker3-size9-expanded/$case.json"
done
