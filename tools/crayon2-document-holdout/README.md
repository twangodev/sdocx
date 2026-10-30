# Crayon2 document grid holdout

This fresh synthetic native Model/Drawing and Mesa capture uses a third path
`(33,58), (57,39), (82,72)`, pressure `0.3, 0.6, 1`, ARGB `c23c92d7`,
and a 160×160 opaque target with background RGB `(64,128,192)`. The view
translation is zero. The native texture-scale uniform is
`1.4222222566604614` on both axes. The 160-pixel projection changes the
scale from the previous 192- and 256-pixel controls; this is not a nonidentity
view-transform control. The capture and its pinned native dependencies are
host-only, not evidence of Android or saved-note parity.

The ignored `write_fresh_document_grid` test independently samples this source,
asserts that its vector SHA-256 matches the native vector, and renders the
actual public document SVG and direct document PDF with the opt-in profile.
The retained native frame is not passed to the renderer. Against that frame,
resvg differs in 342 pixels with maximum RGB channel error 3 and total absolute
RGB error 478. Direct PDF differs in 443/4/1115 for MuPDF and 442/4/1111 for
PDFium. These residuals provide no justification for a paint correction. The
receipt retains hashes and metrics; `frame.rgba.zlib` is the native bottom-up
RGBA8 readback and `vector.bin` is the native uploaded record stream.

To reproduce, use the local native prerequisites described in
[the independent control](../crayon2-independent-control/README.md), then:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-holdout.XXXXXX)
export PYTHONPATH="$PWD/tmp/conformance-cleanup/native-capture-a241793/conformance:$PWD/conformance:$PWD/scratch/apk-analysis-runtime/python"
export LD_LIBRARY_PATH="$PWD/scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
.venv/bin/python tools/crayon2-document-holdout/capture.py > "$work_dir/native.json"
.venv/bin/python tools/crayon2-document-holdout/verify_native.py "$work_dir/native.json"
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
SDOCX_CRAYON2_DOCUMENT_DIR="$work_dir" cargo test -p sdocx --features pdf,serde --lib write_fresh_document_grid -- --ignored
cargo run -p sdocx --features pdf --example crayon2_host_export -- "$work_dir/document.svg" "$work_dir/document.png" "$work_dir/svg-page.pdf"
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-document-holdout/compare.py "$work_dir/document.svg" "$work_dir/document.png" "$work_dir/document.pdf"
```

The comparator uses the retained frame and verifies its hash; it checks reader
metrics and verifies the export hashes against the receipt. The native verifier
checks source, uniforms, texture uploads, blend and draw evidence as well as
the frame and vector bytes. The helper-generated SVG-page PDF is discarded;
the comparison uses the direct document PDF. The texture and helper inputs are
local, hash-gated native dependencies, not supplied by a clean checkout.
