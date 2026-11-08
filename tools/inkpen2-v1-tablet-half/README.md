# InkPen2 V1 tablet half-grid evidence

Six synthetic tool-3 controls match tool-2 native geometry and RGBA bytes at 1440 × 2560 and 0.5×; see the retained [receipt](receipt.json). The scalable radial approximation remains excluded at this scale: historical local reader controls showed regressions, and a local length sweep established no safe threshold. The length-sweep evidence is not packaged in the tracked repository.

## Explicit public half-grid viewport

`RenderOptions::inkpen2_half_grid_viewport` opts into a fixed host-grid approximation for V1 InkPen2 and StraightInkPen2 on the 1440×2560 or 2560×1440 host profile. The caller asserts a whole-page, zero-translation, top-left viewport at exactly 0.5 output pixels per page unit, and supplies its output dimensions. A page whose dimensions do not match twice that viewport, a slice, replay, transformed descendant, translucent ink, enhanced antialiasing, or an unsupported stroke falls through to ordinary rendering. A PDF reader or SVG consumer must use the asserted raster grid; scalable output at other sizes or phases is not covered.

The renderer samples and paints each admitted stroke at its normal traversal position. Stamp, evaluated-fragment and retained-pixel limits are shared per page. Admission and budget failure are atomic per stroke: earlier admitted strokes stay on the grid and the failed stroke follows ordinary dispatch without consuming budget, so a page can contain both approximations. Later strokes can still be admitted; the result depends on traversal order. The limits bound grid work and retained entries, not source geometry allocation, document layout, SVG/PDF serialization, or allocator capacity. The sampler rejects unresolved zero-width boundary cases. This remains experimental; untested paths and scene combinations extrapolate from synthetic host controls and device or saved-note parity is not established.

The `inkpen2_half_grid_public_admission_and_fallback`, `inkpen2_half_grid_mixed_order_exports`, and `retained_host_frames_and_atomic_budget` library tests cover public SVG and direct PDF selection and viewport fallback, painter-order fixture exports, all six retained stroke-only host frames, and each insufficient sampler budget. The [mixed order comparison](public-half-grid-v221-receipt.json) records historical external-reader measurements; the optional artifact hook has been removed. The actual public SVG has the same SHA-256 as the previously injected candidate. On 16 native overlap pixels, candidate versus ordinary RGB L1 is 137 versus 447 in resvg, 132 versus 1,202 in MuPDF, and 129 versus 1,080 in PDFium. This is one host scene; the native line and paper dominate the full-frame error, and the test does not establish arbitrary page or device parity.

Run `cargo test -p sdocx --features pdf,serde --lib inkpen2_half_grid_mixed_order_exports` with the shared Cargo environment to verify the retained native frame hashes and in-memory SVG/PDF assertions. External-reader metrics are historical.

The three compressed native frames and `mixed-scene-order-capture-receipt.json` retain the original capture provenance. Its export patch is historical; the Rust test retains in-memory public SVG/PDF checks. The capture command needs an **absolute** `--input-json` path because its harness changes directory. Fresh native capture additionally requires the local pinned `scratch/straightinkpen2-alias/inkpen2_joined_paint_native.py`, the seven support scripts listed in `overlap_native.py`, the ARM64 native libraries and emulation runtime. These local dependencies are not distributed here. `mixed_scene_order_capture.py` writes fresh frames to a private temporary directory and prints its path; compare decoded hashes with the retained capture receipt.

The source-geometry tests `half_grid_from_source_geometry_matches_retained_phases`
and `half_grid_zero_width_from_source_geometry_matches_retained_phases` check
18 and 26 model grids, respectively. The `explicit_final_grid_source_public_transport`
test exercises source-grid SVG/PDF transport and shifted geometry against two
native color controls. These are bounded controls, not general source-to-device
parity. The [fractional paired control](fractional-v222/README.md) records a
separate row-phase check.

Historical local experiments also compared a direct row mapping with a
reflected mapping in mixed scenes. Their additional capture and comparison
artifacts are not packaged in the tracked repository; they do not establish
application canvas composition or arbitrary transforms.
