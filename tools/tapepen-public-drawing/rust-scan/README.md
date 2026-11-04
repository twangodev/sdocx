# TapePen NanoSVG scan conversion experiment

`scan.rs` is a partial Rust translation of NanoSVG's fixed-point active-edge coverage stage. It accepts sorted, flattened edges, with y coordinates already multiplied by five; it does not parse SVG, tessellate strokes, blend colors, or produce a complete texture. It is not connected to production rendering. The upstream notice is retained in the translated source. The inputs are geometric edges, not captured pixels.

`upstream_edges.c` is a test adapter for the externally supplied upstream NanoSVG headers. It parses the extracted APK pattern SVG, independently flattens every visible stroke, and outputs both the sorted edges and the upstream coverage. `compare.rs` compares the Rust coverage for each stroke, pixel by pixel. This is an upstream intermediate-stage comparison, not a native edge capture; the independent full-texture native/upstream comparison is documented in `../capsule-scan-control-README.md`.

The following commands require the locally extracted pattern and the exact upstream headers described in that document. They validate the original and two source mutations at both sizes:

```bash
set -euo pipefail
work_dir=$(mktemp -d /tmp/sdocx-tape-scan.XXXXXX)
trap 'rm -rf "$work_dir"' EXIT
export work_dir
header_dir=/tmp
printf '%s  %s\n' \
  f1fb86526672f1844af1a30a20007425deab21a195243df5dc9fe9abf23a90e9 "$header_dir/nanosvg.h" \
  2b146cc47d947f55a2ecf0660a424fdc22bd843b22154ba898f2c6bf7b0ee0ae "$header_dir/nanosvgrast.h" | sha256sum -c - || exit 1
cc -O2 -I "$header_dir" tools/tapepen-public-drawing/rust-scan/upstream_edges.c -lm -o "$work_dir/edges"
rustc -O --edition=2024 tools/tapepen-public-drawing/rust-scan/compare.rs -o "$work_dir/compare"
python3 - <<'PY'
from pathlib import Path
import os
work_dir = Path(os.environ['work_dir'])
s = Path('scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg').read_bytes()
assert s.count(b'#3F5480') == s.count(b'stroke-width="1.7"') == 136
(work_dir / 'color.svg').write_bytes(s.replace(b'#3F5480', b'#AD2244'))
(work_dir / 'width.svg').write_bytes(s.replace(b'stroke-width="1.7"', b'stroke-width="2.3"'))
PY
for source in scratch/apk-output/samsung-notes-4.4.45.37/res/raw/tape_pattern_01.svg "$work_dir/color.svg" "$work_dir/width.svg"; do
  for size in 64 128; do
    "$work_dir/edges" "$source" "$size" | "$work_dir/compare" || exit 1
  done
done
```

Each of the six comparisons matched all 136 stroke coverage maps (816 maps total). On the same host, the changed-source native replay matched the upstream full textures for both changes and sizes. This does not establish native intermediate-edge parity, arbitrary SVG coverage, WASM floating-point parity, or end-to-end Rust texture parity. Porting source parsing and stroke tessellation remains necessary.

`texture.rs` additionally translates NanoSVG's solid-color opacity, premultiplied source-over blending, unpremultiplication and defringing. `upstream_texture_edges.c` supplies parsed solid paints and upstream-flattened fill and stroke edges to `texture_compare.rs`; the Rust code computes coverage and composites the texture. This experiment handles solid paints and nonzero fill only, follows parsed paint order and the upstream scaled stroke-width cutoff, and does not yet provide Rust SVG parsing or tessellation. The adapter rejects non-solid paint and even-odd fills. The original input and independently generated color and geometry changes matched the full upstream texture and the existing native control bytes at 64 and 128 on this host (six full textures). This is not a standalone Rust rasterizer or evidence of native intermediate edge parity. The control pixels are comparison outputs, never generation inputs.

Run `python3 tools/tapepen-public-drawing/rust-scan/verify_texture.py` with the pinned headers and source described above. It regenerates both source mutations, compares Rust output byte-for-byte with upstream, and verifies the retained native hashes. The original and changed control hashes are retained in `texture-receipt.json`, with changed hashes copied from the earlier native receipt identified there. The native capture itself is not rerun by this verifier. Python optimization is rejected because verification uses assertions. The Rust coverage stage imposes its documented dimensions and edge bounds, and rejects paths outside them.

`stroke.rs` is a further restricted, altered NanoSVG translation that derives round-cap polygon edges from parsed cubic path coordinates and stroke width. It accepts only a single open, undashed, round-capped cubic whose NanoSVG flatness test succeeds; it rejects curves requiring subdivision and does not implement joins. `upstream_paths.c` parses the actual SVG and verifies those structural restrictions, supplying the path coordinates plus upstream edges for comparison. `path_compare.rs` checks each generated edge's f32 bits and direction against upstream, then uses the Rust-generated edges for stroke coverage and compositing; fill edges still come from upstream. Run `python3 tools/tapepen-public-drawing/rust-scan/verify_paths.py` with the same pinned inputs. All 816 stroke edge lists matched, and the six resulting textures matched upstream bytes and previously retained native hashes on this host. The native hashes are prior observations, not a fresh native capture; upstream edge parity is not native intermediate-edge evidence. This remains a partial experiment, not an SVG parser or general tessellator, and captured texture pixels are never generation inputs.

The restricted stroke experiment rejects non-finite or non-positive width and scale, scaled path coordinates outside ±1000, scaled half-width outside (0, 1000], and non-finite or greater than 128 cap subdivisions. These conservative bounds keep generated edges within the scan converter's coordinate domain. Its standalone tests can be run with `rustc --test --edition 2024 tools/tapepen-public-drawing/rust-scan/path_compare.rs -o /tmp/sdocx-tape-stroke-tests && /tmp/sdocx-tape-stroke-tests`.

`source.rs` closes the upstream intermediate-input dependency for this specific pattern. It strictly recognizes the observed 120×120 document structure, its two full-canvas rectangle fills, and 136 single-segment vertical or explicit line paths with solid colors and round caps. It constructs NanoSVG's cubic line controls from the source numbers, derives stroke edges in Rust, and runs Rust coverage and compositing. It checks the clip definitions and rejects other document structure; it is not a general SVG parser, and assertions mean it is a research executable rather than a public API. The full-canvas fill coverage is derived from the checked geometry. No captured pixels are generation inputs.

Run `python3 tools/tapepen-public-drawing/rust-scan/verify_source.py`. On this host all six original, color, and width outputs at 64 and 128 match independently generated upstream output byte for byte and the retained native SHA-256 receipts. This verifier does not recapture native outputs. The translation modules retain the upstream zlib-style notice and identify themselves as altered; no upstream headers are distributed here. The executable remains a bounded source extraction experiment; general SVG, arbitrary patterns, other platforms and Android parity remain unverified.

The source executable applies the upstream scaled stroke-width cutoff of `<= 0.01` before tessellation. The verifier also changes every width to `0.015`: at 64 this falls below the cutoff, and at 128 it exceeds it. Both results match the upstream executable byte for byte; these extra cases have no native capture. Unsupported or invalid input can panic, including through geometric bounds in the scan and stroke stages, so this remains a controlled-input executable.

Skipped strokes still validate their colors and bounded finite, nondegenerate geometry. A `0.01874999888241291` source width at 64 gives exactly `0.01` in the f32 scaled calculation; its output matches upstream and the below-cutoff control. Invalid color and non-finite geometry controls confirm rejection even when the stroke is skipped.

`asset/mod.rs` extracts a reusable, fallible experimental API from that executable. This experimental version accepts UTF-8 source text and size 64 or 128, returns straight RGBA8 or a typed error, and validates the same exact serialized document grammar and geometry bounds. Geometry and scan conversion checks return errors rather than panicking on rejected inputs; internal buffer shape assertions remain invariants. This is not yet wired into the public crate, WASM, SVG or PDF, and supports only this particular pattern structure. The shared altered NanoSVG translations retain the upstream zlib notice; their source header hashes and uncertain upstream revision are documented above. No packaged SVG or captured pixel data is embedded. Run `python3 tools/tapepen-public-drawing/rust-scan/verify_asset.py` for the same original, modified-color, modified-width and cutoff comparisons; native results are retained hashes, not fresh captures.

The packaged Rust API `sdocx::tapepen_pattern::rasterize` and WASM export `rasterize_tapepen_pattern` accept the same restricted source at sizes 64, 128 or 256 and return straight RGBA8. The source is supplied by the caller; no APK asset or captured pixels are packaged. Unsupported grammar, size, values, and bounded geometry return errors. This is an explicit pattern texture API and does not change document SVG or PDF rendering. Run the crate's retained native-hash controls with `SDOCX_TAPEPEN_SOURCE=/absolute/path/to/tape_pattern_01.svg cargo test -p sdocx --test tapepen_pattern`; without that optional local source, synthetic pixel and invalid-input checks still run. These controls compare to prior native host captures, not a new capture or device parity. The Node WASM runtime matches the same six retained native hashes and rejects four malformed/unsupported inputs. Reproduce with `node tools/tapepen-public-drawing/rust-scan/verify_wasm.cjs /absolute/path/to/node-bindings/sdocx_wasm.js /absolute/path/to/tape_pattern_01.svg` after building `sdocx-wasm` and generating Node bindings with the matching wasm-bindgen CLI. This does not verify browser composition or device pixels.

The [v248 control](../v248-README.md) matches the caller-supplied original SVG to the full 256×256 native host texture upload; Rust synthetic checks and Node WASM native-hash checks also cover 256. Other source changes at 256 and device parity remain unverified.
