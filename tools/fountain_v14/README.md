# Fountain V14 host canvas

The explicit `FountainPenV14DirectHost` and `FountainPenV14Host` canvas profiles
replay direct Drawing paint and enhanced-AA paint respectively. Source crop,
output scale (1 or 2), origin, premultiplied backdrop and RT color are caller
inputs. Direct paint ignores RT color alpha. Normal document SVG/PDF dispatch
is unchanged. These Mesa host controls do not establish Samsung-device or
complete saved-page fidelity.

The retained native receipts contain the original saved stroke and compressed
RGBA frames. The four integration tests check 360 native frames, including
clipped and outside viewports. Historical measurements of 120 opaque frames matched SVG, PNG, direct PDF and
SVG-derived PDF to native pixels in MuPDF and PDFium. Current tests retain
in-memory native-frame, SVG, PNG and PDF construction checks.
Translucent PDF carriers are not covered.

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test -p sdocx --features render,pdf,serde --test fountain_v14_raster
```

The optional artifact exporter and external reader comparator were removed.
Native recapture is separate from receipt replay: the retained capture controllers require the
hash-pinned archived helpers under `tmp/conformance-cleanup/native-capture-a241793`,
the ColoredPencil host helper under `tmp/vector-parity-plan`, original
`tmp/stroke-conformance/handwritten.sdocx`, proprietary SDK libraries and the
project-local emulator/GLES runtime. Those local resources are not bundled.
Native recapture also requires the `ink_geometry` adapter, built with
`cargo build -p sdocx --features render,serde --example ink_geometry` using the
shared Cargo settings above. Controllers verify helper hashes before running. Receipt replay does not require them.
