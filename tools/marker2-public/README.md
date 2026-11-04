# Bounded Marker2 output-grid approximation

`RenderOptions::marker2_output_scale = Some(2)` selects a bounded vector
approximation for eligible width-2 and width-3 strokes. The caller must assert
an axis-aligned two-pixels-per-page-unit output grid; the option does not apply
that transform. Width 2 uses a 0.5 fringe and width 3 uses 0.2. Unsupported
sources, replay and slices retain fallback, with atomic fallback at the shared
65,536-vertex page budget.

The maintained Rust regression
[`marker2_both_widths_improve_native_overlap_rgb`](../../crates/sdocx/src/render/marker2_guard_tests.rs)
reconstructs paired translucent strokes from pinned native inputs and checks
that both widths improve aggregate RGB error over the default renderer using
in-memory resvg at 2×. It verifies native frame hashes and independent lifecycle
provenance. Other tests in that module cover admission, source order and page
budgets, including direct PDF operators.

The exporter, external reader comparator and generated reports were retired.
Their historical aggregate RGB absolute errors were:

| Width | resvg default → opt-in | MuPDF default → opt-in | PDFium default → opt-in |
| --- | ---: | ---: | ---: |
| 2 | 47,455 → 7,019 | 47,222 → 7,584 | 42,824 → 7,757 |
| 3 | 24,434 → 7,787 | 22,333 → 8,555 | 18,977 → 10,752 |

The MuPDF and PDFium measurements are historical external reader evidence;
the Rust regression does not rerun those pixel checks. Native controls remain
in [independent-lifecycle](../marker2-scale-investigation/expanded/independent-lifecycle/)
and [width3-controls](../marker2-scale-investigation/width3-controls/), with
[source width 2](../marker2-scale-investigation/v61/bend/w2-translated-a255.json)
and [source width 3](../marker2-scale-investigation/fringe-sweep/bend/w3-translated-a255.json).
They contain independently created synthetic native ObjectStrokes, Drawings,
pens and renderers sharing one host Mesa framebuffer. Native recapture requires
the capture runners' pinned local dependencies; the Rust regression uses retained
frames. Other paths, colors, backgrounds, phases and transforms remain
extrapolations. These controls establish neither device nor saved-page parity.
