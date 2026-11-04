# Pencil2 aggregate SVG staging experiment

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The retired Rust/Python prototype staged the two v199 source-generated SVG fragments and checked aggregate rectangle count and exact byte length before selecting the scene. Below/at/above-boundary and empty-scene checks preserved the fallback bytes on rejection. It also checked accepted bytes against the v198 native-prefix and v199 order controls. Allocation had already occurred before admission.

The measured Rust scene has 6,113 rectangles and 608,273 bytes. This is a staging experiment, not a production resource limit: source stamp frames and individual strings have already been allocated before admission, the fallback selection is local to the harness, and neither total document resources nor PDF object mutation are covered. The v199 PDF form adds resources before its stream check, so this experiment does not establish PDF atomic fallback. The only inputs are the two hash-gated host-Mesa strokes and their one blue-background scene; existing native and reader discrepancies remain. It must not be enabled as a public rendering option on this evidence.
