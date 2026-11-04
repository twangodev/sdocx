# Historical radial alpha mask reader control

The retired harness compared resvg, MuPDF and PDFium for a 2 × 2 rectangle with an object-bounding-box radial alpha mask, then repeated at 64 × 64. It used `render_svg_pages_pdf`, not direct document PDF export or native pixels.

With resvg 0.45.1, MuPDF 1.28.2 and PDFium 156.0.8076.0, the four covered alpha
values were respectively `[[75, 75], [75, 75]]`, `[[0, 0], [0, 255]]` and
`[[1, 1], [1, 255]]`. The 64 × 64 maximum errors were 8 for both PDF readers,
with error sums 11,137 and 12,722. Results are reported, not asserted across
versions; this does not attribute the difference exclusively to a reader or
the converter, or establish Pencil2 or device parity.

The maintained `svg_radial_alpha_mask_preserves_vector_pdf_structure` test in `crates/sdocx/tests/pdf_export.rs` checks page dimensions, radial-shading coordinates and its referenced stop function, a mask, a form with expected bounds, and absence of image objects. These structural checks do not prove mask/form/shading connectivity or reproduce the historical external-reader measurements.

## Historical width and source-stamp experiments

A synthetic host comparison found opposing width adjustments, so it did not establish a general width correction; a source-stamp SVG mask prototype also retained substantial reader-dependent PDF errors and was not promoted. The historical native alpha captures recorded uncompressed SHA-256 values `8b1a2ab915b32265aa8e4898d8ed08d539b5bcf5ad5df0ebe62790d43198576f` (pressure ramp) and `699cb08853a9f08cd1b39b27b99de36add28e8a6545a3d7654f02245d7c6062b` (constant size 8).
