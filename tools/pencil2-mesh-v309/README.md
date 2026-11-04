# Pencil2 native geometry controls

The maintained Rust tests in
[`independent_drawing_tests.rs`](../../crates/sdocx/src/ink/pencil2/independent_drawing_tests.rs)
check both synthetic descriptor streams byte-for-byte. The note-derived test in
[`second_source_tests.rs`](../../crates/sdocx/src/ink/pencil2/second_source_tests.rs)
checks 231 source points and 124 native records, allowing only two known one-ULP
lane differences (or their disappearance as fidelity improves).

The note source reuses already tracked Fountain coordinates, pressure, tilt,
orientation and timestamps. These are real note-derived data, **not anonymous**;
normalizing pen metadata does not anonymize handwriting. The source is mapped to
a fresh Pencil2 V14 host profile, not an original saved Pencil2 stroke. The
native fixture is a pre-Update descriptor, not independently verified GPU upload.

The retired Rust exporter and mesh comparators produced these historical RGB
absolute error sums:

| Note backdrop | Mesh PDF MuPDF | Mesh PDF PDFium | Source-cell SVG resvg | Generic PDF MuPDF | Generic PDF PDFium | Generic SVG resvg |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Blue | 3,525 | 3,675 | 22,656 | 25,887 | 28,395 | 29,572 |
| White | 7,696 | 9,106 | 16,911 | 48,690 | 56,229 | 54,929 |

The measurements used MuPDF 1.28.2, PDFium 156.0.8076.0 and resvg 0.45.1.
The mesh used 166,156 triangles; its SVG approximation was about 9.4 MB. A lossy
coarse control reduced the mesh to 24,416 triangles without an error bound.
These are historical experiments, not production rendering or maintained reader
pixel checks. The original receipt retains measurements and provenance. The
manifest labels the archived exporter revision separately from native input pins.

Run the maintained note-derived geometry regression:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features render,serde note_source_preserves_native_geometry_except_two_known_ulp_lanes
```

Native recapture uses the unchanged `capture.py`. On a host with its local
native dependencies, set `SDOCX_PENCIL2_FRESH_FRAME_DIR` to a fresh local directory
and run that script; set `SDOCX_PENCIL2_CLEAR_RGBA=255,255,255,255` for white
instead of the default blue backdrop. The output directory contains note-derived
frames and geometry: keep them local. Native note frames are not retained here.
The capture requires the independent-Drawing harness, mixed-Drawing patch,
local `tmp/vector-parity-plan` helpers, native ARM64 libraries, emulation and Mesa.
The manifest records direct adapter hashes, not every transitive runtime
dependency. Recapture uses synthetic queues and explicit Update/Draw;
it establishes neither normal saved-page scheduling nor device parity.

`first_probe.py` remains because the separate precision control consumes its
pinned source. It approximates a retained synthetic frame; it does not acquire
native data. Its historical horizontal grain shift matched one pixel but
worsened whole-frame error from 344 to 2,134, so it is not a production correction.
