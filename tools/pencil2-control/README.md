# Pencil2 host control

A fresh setter-built Pencil2 object receives the 87 decoded Fountain source channels with x + 3.25, y - 2.5 and pressure * 0.75. Native ARM64 emulation and host Mesa drew it over the retained synthetic prior, with deferred queue intervention and explicit Update/Draw. The source is not a saved Pencil2 stroke. The frame is 192x192, opaque, with explicit 12-pixel translation and captured grain offset. The Rust regression computes the offset and compares its bits with the native receipt, verifies both retained frame hashes, and compares the entire frame to canvas, PNG and resvg SVG in memory. Direct PDF serialization is checked; external MuPDF/PDFium pixel comparisons are historical measurements from the retired exporter. This is not evidence of saved Pencil2 admission, normal queue scheduling, or device parity.

Run the maintained regression with the shared Cargo settings:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --no-default-features --features pdf,serde --test ink_raster changed_input_native_grain_offset_and_canvas_match
```

The experimental export crate and Python comparator have been retired. The source coordinates remain real-note-derived Fountain data with synthetic Pencil2 metadata; this cleanup adds no new source copy.

The capture script is retained for inspection; recapture depends on the project-local native capture harness and DSO files listed in `isolation-receipt.json` under `changed_input_comparison`, as well as its transitive dependencies. The matching native capture is retained in `native-receipt.json`; the historical exporter command referred to a retired prototype. The retained compressed frames make the comparison independent of those native capture dependencies. The second source geometry control has three one-ULP lane differences; this exact raster match does not establish geometry identity generally.

Object metadata with raw flip or rotation-presence flags, unknown property or field bits, or trailing data is rejected. Raw mask checks also reject inconsistent caller-supplied metadata; the captured fresh-object control supplies no object metadata. Source sampling has separate 16,384-point and 16,384-record limits and is not charged to the canvas paint budget.
