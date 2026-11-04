# Direct document radial alpha control

A synthetic type-8 line uses black ARGB stops `ff000000` at 0.25 and
`00000000` at 0.75, centered at (0.5, 0.5). Its square reference box starts
at (40, 40), with size and stroke width 2 or 64. The SVG projection has a
transparent center and opaque exterior with a hard stop at 0.25.

The maintained Rust test
[`synthetic_radial_line_exports_preserve_alpha_step_and_geometry`](../../crates/sdocx/src/render/gradient/tests.rs)
checks both sizes through public SVG export: gradient geometry, exact stops,
line endpoints, stroke reference/width and empty paint diagnostics. With `pdf`,
it also constructs direct document PDFs in memory at 72 dpi on white.
The standalone exporter and external-reader comparison harness have been retired.
This is a constructed source control, with no native pixel comparison or parity claim.

Historical measurements used resvg 0.45.1, MuPDF 1.28.2 and PDFium
156.0.8076.0. Absolute red-channel error sums versus resvg were 255/255 at
size 2 and 6885/8415 at size 64 for MuPDF/PDFium, respectively; maximum error
was 255 throughout. At size 2, resvg samples were `[[0,0],[0,0]]` and both
PDF readers produced `[[0,0],[0,255]]`. Each PDF contained two radial shadings,
one soft-mask reference and no image objects; these counts did not verify
resource connectivity. Hard-edge sampling confounds attribution, and these
observations establish neither a correction nor current reader parity.
