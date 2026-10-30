# Saved Marker4 V7 paint control

This is quiz.sdocx stroke index 69: 824 original samples, stylus tool 2,
width 35.50513458251953, ARGB `733396ff`. The retained `source.json` has
its original parsed stroke and the 504 native V7 redraw stamps. `verify_source.py`
re-parses the pinned source note and asserts exact stroke and native stamp equality.
The capture replays those stamps through native RTV3 with host Unicorn/Mesa
adapters. It is an isolated native paint control, not archive Drawing or device
output. Its uncompressed frame hash is in `native.json`; the compact frame is
`frame.rgba.zlib`.

The current public SVG and direct PDF are compared over white with native paint;
forced generic output changes only advanced settings to select fallback. RGB L1
is 42,275 / 60,364 / 95,966 for resvg / MuPDF / PDFium, versus 2,701,500 /
2,682,620 / 2,693,820 for fallback. These are reader-specific approximation
measurements, not pixel parity. The exporter translates the input coordinates to
the captured viewport, so float32 rounding can differ from the native matrix.
`receipt.json` identifies input, scripts, outputs and reader versions. The parser
binary's build provenance was not independently attested. The capture pins
the listed DSOs and helpers, but its transitive native dependencies and runtime
environment are not fully pinned.

Review replay on 2026-10-30 reproduced the original stroke, all 504 native
stamps, the producer hash, and the native frame hash. An exporter built from
clean Git source `fcbeb1fb3b7073f6b5b4f26d9d767eb21845c660` reproduced all
four SVG/PDF hashes and all six RGB L1 values in `receipt.json`. This confirms
the public comparison without depending on unrelated workspace edits; it does
not attest the historical parser binary's build provenance.

Run from the repository root with the retained note and local native inputs:

```sh
export PYTHONPATH=tools/marker4_v7:tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance:scratch/apk-analysis-runtime/python
.venv/bin/python tools/marker4_v7/saved-v197/verify_source.py /tmp/sdocx-agent-target/debug/examples/ink_geometry
PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/conformance-cleanup/native-capture-a241793/conformance:scratch/defaultpen-native .venv/bin/python tools/marker4_v7/saved-v197/capture.py --case saved69 --capture-output /tmp/marker4-v197-final.json
```

The capture requires a new output path and checks the pinned native DSOs and
helpers. For exports, put `export.rs` in a temporary Cargo binary depending on
this repository's `sdocx` with `serde`, `render`, `pdf` and on `serde_json`.
Run the binary once with a caller-selected new output directory as its only
argument; it exclusively creates that directory and writes `public.svg`,
`public.pdf`, `generic.svg`, and `generic.pdf`. Use the shared agent Cargo
settings. Run `compare.py OUTPUT_DIR` with Pillow, PyMuPDF, pypdfium2 and
resvg; its PNG intermediates use a private temporary directory. The comparator verifies the native receipt, frame, and export hashes and checks
every raster dimension and byte length against the retained receipt. No production change follows from this one control.
