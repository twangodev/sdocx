# ColoredPencil V4 host canvas

`InkRasterProfile::ColoredPencilV4Host` reconstructs fresh GLV4 `4;`, tool-2
sampling and RTV1 paint with explicit display dimensions. `paint_with_depth_map`
accepts a caller-owned red-channel bitmap; `InkRasterDepthContext::from_colored_pencil_display`
derives the captured background scale. Source limits and unsupported styles,
transforms, metadata masks and extensions return errors without changing pixels.
Decoded source properties are required; this API does not validate an original
serialized object's unknown property bits. Source sampling has separate limits
of 4,096 input points and 65,536 samples; canvas work is budgeted after sampling.

The retained native receipts cover 856 source/context/backdrop frames, including
both origins, 1x/2x projection, alpha, depth filtering and display scale. The 288
opaque frames match PNG and SVG/direct PDF through MuPDF and PDFium. Default document dispatch, native cached drawing state, mixed-tool pages,
Android RNG/device behavior and saved-note context are outside this claim.
Transparent export controls are measured separately below.

Validation from the repository root (Pillow, PyMuPDF and pypdfium2 required):

```sh
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p sdocx --all-features --lib colored_pencil
SDOCX_COLORED_PENCIL_ARTIFACTS="$PWD/scratch/colored-pencil-public/exports" \
  cargo test -p sdocx --all-features --test colored_pencil_raster
python3 tools/colored_pencil/export_compare.py rust base
python3 tools/colored_pencil/export_compare.py rust context
python3 tools/colored_pencil/export_compare.py rust depth
python3 tools/colored_pencil/export_compare.py rust display
```

Native acquisition scripts retain their input vectors and dependency hashes in
the receipts. Recapture requires the existing project-local emulator/GLES inputs
and pinned helper scripts under `tmp/vector-parity-plan` and `scratch`; these
external dependencies are not included in this commit. Rust replay and export
validation need only the checked-in receipts. Generated PNG/PDF/RGBA exports can
be removed after comparison.

The measurement environment needs existing Pillow, PyMuPDF and pypdfium2;
reuse their installed environment or expose an existing package directory through
`PYTHONPATH` rather than installing another harness.

Generate and measure the transparent artifacts in a fresh directory. The
receipt path must not already exist:

```sh
artifact_dir=$(mktemp -d /tmp/sdocx-colored-transparent-XXXXXX)
SDOCX_COLORED_PENCIL_ARTIFACTS="$artifact_dir" \
  cargo test -p sdocx --all-features --test colored_pencil_raster \
  public_source_to_pixels_matches_all_native_backdrops_and_projection_controls
python3 tools/colored_pencil/transparent_export_compare.py \
  --artifacts "$artifact_dir" --output "$artifact_dir/receipt.json"
```

Use the shared Cargo environment above. This checks the 64 clear and
translucent-prior native frames in the base capture.
The [receipt](../../conformance/colored-pencil-transparent-export.json) verifies
the native frame hash and the PNG straight-alpha conversion, and checks that SVG
embeds the same PNG. Both direct and SVG-page PDFs match that PNG in PDFium.
It records hashes of each measured artifact and selected source inputs; these
source hashes are not complete build provenance.
MuPDF returns premultiplied samples: 56 frames differ from the original native
premultiplied bytes, by at most one channel value. This measures intrinsic-size
transport of synthetic host frames, not saved-note or device fidelity.
