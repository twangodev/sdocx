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

Preparation commits scene state before painting. Failed source preparation
preserves the previous scene; failed painting preserves canvas pixels and
budgets but does not roll back a successful preparation. Host bounds are
256 objects, 4096 source samples and 8192 generated stamps; these are not
native limits.

Run `cargo test -p sdocx --features 'render serde pdf' --lib scene_tests`
with `SDOCX_FOUNTAIN_SCENE_ARTIFACTS` pointing to a temporary directory, then
run `scene_export_compare.py` on that directory. It compares the direct and
SVG-derived PDF routes in MuPDF and PDFium against all native frames,
allowing at most one channel level after premultiplied-alpha conversion.
The compact export receipt records reader versions and artifact hashes.

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
