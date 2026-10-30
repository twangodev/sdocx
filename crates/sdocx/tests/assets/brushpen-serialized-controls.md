# Synthetic serialized BrushPen geometry controls

The `brushpen-independent` and `brushpen-third` inputs modify the retained
FountainPen-derived GLV10 payload before native `NewApplyBinary`: pressure seeds
0.45/0.70 and path delta words 96/48 respectively. They are synthetic controls,
not saved BrushPen notes. The native capture attaches the object to a Model and
runs Drawing and RTV1 under host adapters at source dimensions 1920 x 1080.

The `serialized_native_tests` unit tests verify both complete geometry streams
against native bytes and verify the third control's 204 decoded points,
pressures, tilts, orientations and timestamps against native getters. They
verify payload reconstruction and input/output SHA-256 against retained capture
receipts. This establishes geometry and decoding for these controls; it does
not establish paint, exported-page, complete-note-loader or device parity.

Capture scripts and receipts are retained in `tools/brushpen-independent/` and
`tools/brushpen-third/`. Native regeneration additionally requires their pinned
project-local native libraries and host adapters, including historical helpers
under `tmp/conformance-cleanup/`; those runtime dependencies are not packaged
here. Transitive adapters and the toolchain are not pinned. Rust regression
checks require no native runtime:

```sh
cargo test -p sdocx --lib --features render,serde serialized_native_tests
```
