# Explicit Fountain V14 scene replay

`FountainV14Prepared` owns sampled source geometry. `FountainV14Scene` keeps
caller-supplied object/revision keys and the direction used by a following
single-stamp stroke. Change the revision after changing source data, color,
size or rectangle; reset the scene for a new Drawing. Output-scale changes
regenerate geometry. A cache hit preserves direction. These are explicit
host APIs, not automatic saved-page rendering or inferred native cache keys.

The retained original inputs in `conformance/fountain-v14-saved-scene-inputs.json`
include serialized object payloads and decoded sources. `scene_capture.py`
reapplies these through native Drawing. The scene test replays 108 cumulative
frames at scales 1 and 2, both row orientations, and clear, white and checker
priors. Its synthetic mixed-page wrapper tests caller selection/order only;
the image is not painted. All frames must equal the native capture and SVG
rasterization byte for byte. Cache lifecycle controls cover repeated objects,
a following short stroke and a new Drawing using three retained captures.

A separate retained capture replaces the pen canvas after source 29 while
keeping the Drawing and destination. Its following source 30 uses a different
instance stream from a fresh Drawing, and a repeated draw reuses that stream
without new AddPoint arguments. The scene API reproduces the three captured
frames with one caller-managed scene and output canvas; the Rust replay does
not itself replace a pen canvas. This observation covers this sequence and does not infer general native cache keys.

Preparation commits scene state before painting. Failed source preparation
preserves the previous scene; failed painting preserves canvas pixels and
budgets but does not roll back a successful preparation. Host bounds are
256 objects, 4096 source samples and 8192 generated stamps; these are not
native limits.

Run `cargo test -p sdocx --features 'render serde pdf' --lib scene_tests`
for native-frame/SVG assertions and in-memory direct and SVG-derived PDF
construction. Historical MuPDF/PDFium comparisons against all 108 native frames
had at most one channel level of error after premultiplied-alpha conversion.
The optional artifact exporter and comparator were removed; the historical
export receipt retains reader versions and artifact hashes.

`scene-native.json` under the crate test assets preserves the native scene receipt.
The capture script emits `conformance/fountain-v14-saved-scene-native.json`; copy
that output to the asset after recapture. `scene-sources.json` retains only
stroke IDs and decoded sources, with the original input file hash. The cache
lifecycle fixture retains the source strokes, native frames and original
receipt hashes from `cache_state_capture.py`, `cache_state_pair_capture.py`
and `fresh_drawing_capture.py`.

After an authorized native recapture, synchronize and verify the receipt before
running the Rust test and export comparator (from the repository root):

```sh
cp conformance/fountain-v14-saved-scene-native.json crates/sdocx/tests/assets/fountain-v14/scene-native.json
cmp conformance/fountain-v14-saved-scene-native.json crates/sdocx/tests/assets/fountain-v14/scene-native.json
sha256sum conformance/fountain-v14-saved-scene-native.json crates/sdocx/tests/assets/fountain-v14/scene-native.json
```

The comparator records the SHA-256 of the crate-local receipt it actually reads.
Rebuild the test artifacts after changing either fixture; do not reuse prior PDFs.

The fresh-canvas regression uses crate-local `fountain-v14-fresh-canvas-native.json`
and `fountain-v14-fresh-drawing-native.json` receipts beside the lifecycle fixture.
After recapturing with `fresh_canvas_capture.py` or `fresh_drawing_capture.py`,
copy the matching receipt and verify its bytes before replay:

```sh
cp conformance/fountain-v14-fresh-canvas-native.json crates/sdocx/tests/assets/fountain-v14/fountain-v14-fresh-canvas-native.json
cp conformance/fountain-v14-fresh-drawing-native.json crates/sdocx/tests/assets/fountain-v14/fountain-v14-fresh-drawing-native.json
```

Run `cargo test -p sdocx --features "render serde" --test fountain_v14_cache_lifecycle`
with the shared Cargo settings in [README.md](README.md). The scene export
comparator reads the crate-local assets. Historical scripts requiring generated
conformance paths need temporary copies of the corresponding retained assets.
