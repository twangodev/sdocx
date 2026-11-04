# OilBrush3 source model public export control

The `oilbrush3_source_model_public_canvas_export` regression joins the public
`Stroke` adapter to the bounded GLV1 sampler and CPU P3/P4 feedback model.
It regenerates all 15 AddPoint records, derives each draw's matrices and depth
scale from those records, applies level-0 feedback after every four draws, and
compares the final transparent RGBA frame to the retained native V17 readback.
It then composites that frame on white, constructs a public `InkRasterCanvas`,
and checks SVG equality and in-memory direct and SVG-page PDF construction.
Historical comparisons found both PDF routes matched native RGB exactly in
MuPDF 1.28.2 and PDFium 150.0.7869.0. The artifact exporter, reader comparators,
and derived receipts were removed.

The explicit `InkRasterCanvas::paint_oilbrush3_cold_host` method now exposes this source-driven model. It requires a transparent canvas and caller-supplied source textures matching the captured hashes; it rejects unsupported metadata and contexts before changing the canvas. Its concrete model
contract is GLV1, source 0, identity model matrix, 1920x1080 screen and bitmap,
size 8, ARGB 0x8044aaee, selected ordinary stylus metadata, glibc RNG and host
atan2, the retained 16x16 mask and 297x372 depth texture, transparent initial
bitmap, opaque source-RGB initial feedback brush, and rendering level 0. The
source adapter rejects styles outside its pinned profile; the textures, initial
state, cadence and host arithmetic are assertions of the control rather than
inferences about arbitrary saved strokes. The opaque white output is a separate
post-paint composite. Native captures beyond this test constrain some of these
assumptions but do not establish arbitrary contexts, lifecycle, device, scaling,
or saved-note fidelity. Further native controls are needed for broader admission and compositing. A captured output is not substituted for source-derived paint.

Run `cargo test -p sdocx --lib --features pdf,serde oilbrush3_source_model_public_canvas_export`
with the shared agent Cargo environment for the native-frame and transport checks.

The Rust fixture inputs under `conformance/oilbrush3-native.json`,
`conformance/oilbrush3-v17-native-paint/`, and
`conformance/oilbrush3-v23-native-paint/` are tracked repository files.
Native capture inputs remain independent of the removed export workflow.
