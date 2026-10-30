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
opaque frames match PNG and SVG/direct PDF through MuPDF and PDFium. Translucent
PDF carriers, default document dispatch, native cached drawing state, mixed-tool
pages, Android RNG/device behavior and saved-note context are outside this claim.

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
