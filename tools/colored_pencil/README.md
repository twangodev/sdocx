# ColoredPencil V4 host canvas

`InkRasterProfile::ColoredPencilV4Host` reconstructs fresh GLV4 `4;`, tool-2
sampling and RTV1 paint with explicit display dimensions. `paint_with_depth_map`
accepts a caller-owned red-channel bitmap; `InkRasterDepthContext::from_colored_pencil_display`
derives `float32(min(display dimensions)) / 1440`. Display dimensions must be
`1..=16384`; admitted depth scale is `1/1440..=16384/1440`.
Source limits and unsupported styles, transforms, metadata masks and extensions
return errors without changing pixels. Decoded source properties are required;
this API does not validate an original serialized object's unknown property bits.
Sampling allows 4,096 input points and 65,536 samples; canvas work is budgeted afterward.

Retained controls cover 964 native paint frames and 332 opaque export controls,
including both origins, 1x/2x projection, alpha, depth and display scale. These
are synthetic host inputs. They do not establish normal document dispatch,
automatic native cache behavior, saved-note/device fidelity, Android RNG behavior,
or higher-level reset/disposal. See the [findings](../../docs/reverse-engineering/colored-pencil-host-raster-findings.md)
for the native model and evidence boundaries.

The export and Node/WASM receipts retain historical measurements and original
script, reader and binary hashes. The experimental artifact writers and their
external comparison callers have been retired; these receipts are not current
reproduction instructions or complete source/build attestations.

## Rust regression checks

Run from the repository root with the shared Cargo settings:

```sh
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p sdocx --all-features --lib colored_pencil
cargo test -p sdocx --all-features --test colored_pencil_raster
```

These tests retain native canvas, SVG and counterexample assertions in memory.
External PDF reader and Node/WASM measurements below remain historical.

The [transparent receipt](../../conformance/colored-pencil-transparent-export.json)
covers 64 clear/translucent-prior base frames: native hashes, PNG straight-alpha
conversion, matching SVG-embedded PNG, and both PDF routes matching PNG in PDFium.
MuPDF's premultiplied output differs from native in 56 frames by at most one
channel value. These are intrinsic-size transport checks; selected source hashes
are not complete build provenance.

## Native capture

Recapture requires existing project-local emulator/GLES inputs and pinned helpers
under `tmp/vector-parity-plan` and `scratch`. Scripts retain dependency hashes,
input vectors and compact receipts; these external dependencies are not bundled.

```sh
for script in capture context_capture context_paint_capture depth_capture \
  scale_capture scale_paint_capture lifecycle_capture drawing_capture mixed_capture; do
  PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python "tools/colored_pencil/$script.py"
done
```

`context_capture.py` captures all controls afresh by default. Its
`--extend-existing` option retains rows with matching producer pins and records
the prior receipt's digest and metadata. Native capture verifies uploaded priors
and preserves the pinned baseline. Keep native inputs, scripts and compact JSON
receipts; generated PNG/PDF/RGBA exports and `/tmp/sdocx-colored-pencil-wasm`
bindings can be removed after comparison. Do not add Python harnesses to conformance.

## Evidence summary

Rust/resvg checks SVG pixels. Historical opaque export comparisons covered PNG and both Rust
PDF routes in MuPDF/PDFium; WASM checks covered SVG carrier equality and retained-scene PDF.
Counts below are additional controls, not cumulative totals.

| Controls | Native paint / opaque exports | Historical comparisons |
| --- | --- | --- |
| base | 100 / 36 | Rust / Node / WASM |
| context | 300 / 100 | Rust / Node / WASM |
| depth | 360 / 120 | Rust / Node / WASM |
| display | 96 / 32 | Rust / Node / WASM |
| drawing | 96 / 32 | Rust / Node / WASM |
| mixed | 12 / 12 | Rust / Node / WASM |

Context controls cover selected size, Drawing/selected Redraw stream equality
and six source aliases. Depth controls use five native bitmap inputs with upload,
sampler, uniform and prior guards. The [display producer](../../conformance/colored-pencil-v4-background-scale-native.json)
and [display paint](../../conformance/colored-pencil-v4-display-depth-native.json)
receipts join recorded scales/streams to native depth replay; this is not a full
Drawing paint pass.

The ten lifecycle controls stop before RT submission and do not prove rendered
depth pixels. Drawing controls execute native Model setters, cached pen selection,
GLV4 redraw, callback task, RT update and GLES draw together. Rust/Node replay
reconstructs that state and supplies the effective depth explicitly. Mixed controls
join separate Pencil2 and ColoredPencil captures through verified retained RGBA
pixels, preserving record order and placement; they do not establish a single
native mixed-tool saved page. These synthetic controls do not establish real
saved ColoredPencil input or parsed-note admission.

## Lifecycle receipt recapture

`lifecycle_capture.py`, `drawing_capture.py`, and `mixed_capture.py` write their
`colored-pencil-v4-*-native.json` receipts under `conformance/`. After recapture,
verify the script and dependency pins, native frame hashes, and recorded scope
before copying the corresponding receipts into
`crates/sdocx/tests/assets/colored-pencil/` and running the lifecycle tests. Keep
the capture scripts unchanged so the receipt script hashes remain meaningful.
These tests supply caller-retained native depth explicitly. The mixed Pencil2
and ColoredPencil captures are joined through retained pixels; they do not
establish automatic cache behavior or a shared native mixed-tool session.
