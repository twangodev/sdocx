# Bounded Marker2 output-grid approximation

`RenderOptions::marker2_output_scale = Some(2)` selects four nested vector
contours per sampled stamp for eligible Marker2 strokes at an explicitly
asserted axis-aligned two-pixels-per-page-unit output grid. It does not apply
the output transform. Other values preserve the default renderer.

Admission requires at least two finite source points within ±256, tool 2,
non-null empty advanced settings, compressed curves, stroke type 0, and
uniform sampled radius 1 or 1.5 (native widths 2 and 3). The renderer validates
sampled bounds and opacity, limits each stroke to 1,024 stamps, and shares a
65,536-vertex budget across the page. A rejected stroke consumes no budget;
replay and sliced scenes retain fallback. Existing source admission still
applies before these checks.

Width 2 uses a 0.5 fringe; width 3 uses 0.2. An earlier width-3 experiment
using 0.5 regressed in PDFium, so the widths deliberately use different
constants. These are discrete approximations, not an interpolated coverage
model or native pixel parity. Arbitrary zoom, transforms, paths, backgrounds,
saved-page state and Android GPU behavior remain unverified.

The retained controls contain two independently created synthetic native
ObjectStrokes, Drawings, pens and renderers sharing one host Mesa framebuffer.
The native input vectors are bridged explicitly. The comparison checks pinned
input-vector identity and decompressed native-frame hashes, renders the paired
public strokes through SVG and direct PDF, and requires aggregate RGB error
to improve against the default renderer in resvg, MuPDF and PDFium for both
widths. The native transparent framebuffer is composited over each reader's
observed light background for comparison. Capture runners require their
manifest-pinned local native dependencies; retained-frame comparisons do not.

From the repository root, with NumPy, Pillow, PyMuPDF, pypdfium2 and resvg
available:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo build --manifest-path tools/marker2-public/Cargo.toml --offline
python3 tools/marker2-public/compare.py /tmp/sdocx-agent-target/debug/marker2-public-export
```

The isolated validation receipt records source hashes and both readers' error
sums. Rust `marker2_` tests cover admission, replay/slices, mixed source order,
atomic page-budget fallback, smaller strokes after budget rejection, and
budget reset between pages, including direct PDF operators.
