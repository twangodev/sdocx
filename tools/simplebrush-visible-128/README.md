# Fully visible 128-point SimpleBrush control

The experimental public-export generator and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

A folded and reversed tool-0 size-3 trajectory has 128 points and varying pressure. Native Drawing and SimpleBrush execute with host-scheduled tasks and Mesa at 2× with fractional translation. The frame has no nontransparent edge pixels. The retained [receipt](../../conformance/simplebrush-visible-128/tool0-receipt.json), native geometry and compressed frame record this synthetic host control; they do not establish device or saved-note parity.

At revision `a0157777`, the explicit two-pixel profile reduces resvg alpha, MuPDF paper-matched RGB, and PDFium RGB error relative to generic output. Native geometry differs from Rust in one of 1,425 float lanes by 0.00000762939453125; the cause is unverified. This does not establish exact geometry parity. The GL verdict covers observed errors only. Capture dependencies include project-local scratch inputs and are not fully pinned.

For recapture, create an output directory and run `POINT_COUNT=128 PROBE_SIZE=3 VARIANT=reverse SDOCX_ROOT="$PWD" python3 tools/simplebrush-visible-128/capture.py fraction12 /tmp/OUTPUT/native.json --tool 0`.  The retained native geometry is the decoded `drawing_vector.vector_bytes`.  The native recapture checks the local `native-inputs.json` inventory of available scratch scripts and libraries by SHA-256 (extracted from the previous long-audit inventory); it does not pin every transitive host library, driver, or runtime component.
