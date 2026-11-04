# AirBrush V1 ordered two-object host control

`capture.py` extends the hash-pinned local AirBrush Drawing harness with two
separately constructed synthetic Model strokes. The second uses the same source
channels and a point offset of (2, 1), with a different color. Drawing submits
each to the same 256×256 Mesa target. The second draw's captured pre-frame has
the first draw's post-frame hash; both issue nine instances. The retained
`native.json` uses a transparent initial target and `native-opaque.json` uses
RGBA (252, 252, 252, 255). They are comparison targets, never renderer inputs.

The public SVG regression renders the two source objects independently through
the existing opt-in `SavedV1` path, and asserts its shared texture appears.
Resvg's opaque full-frame RGBA absolute error against the latter capture is 7.
The direct public PDF for the same inputs has full-frame errors 132980 in
MuPDF 1.28.2 and 2025 in PDFium 150.0.7869.0. MuPDF's background is two channel
levels below the native background in total per pixel; within the 40×40 ink
region the reader errors are 5108 and 2025, respectively. Both have maximum
channel error 30 or less. See `pdf-receipt.json`. This is one order, scale,
background, and pair of strokes; it does not establish general PDF, saved-page,
or device parity. The test requires the `pdf` feature.

Run `cargo test -p sdocx --features pdf --test airbrush_v1_scene_control`
with the shared agent Cargo environment for the native-frame assertions and
in-memory PDF construction. The optional PDF exporter and reader comparator
were removed; the numerical reader findings above remain historical measurements
at 96 dpi against the retained native target.

Capture requires the local native prerequisites pinned by
`tools/pen-capture/airbrush_v1_curved_prerequisites.json` and the runner pinned
in `capture.py`. Run `.venv/bin/python tools/airbrush-v1-scene/capture.py` for
transparent, or set `AIRBRUSH_SCENE_BACKGROUND=252,252,252,255` for opaque.
The `TWO_DRAW` output contains both frame receipts. These local prerequisites
are not included in a clean checkout.
