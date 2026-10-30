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
