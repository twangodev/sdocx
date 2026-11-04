# InkPen V4 5–8× host controls

Twelve synthetic native host-Mesa captures cover straight, bent and crossing
strokes at 5–8×. Inputs and compressed frames remain in
`conformance/inkpen-v4-highscale-native.{json,bin}`. These opaque controls do not
establish device pixels, arbitrary transforms or saved-note parity.

The maintained Rust regression checks frame hashes, exact native geometry bits
and lower SVG alpha error for each scale-aware contour than the 1× contour:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --lib --features render,serde v4_high_output_scales_improve_svg_against_native_frames
```

The public SVG test selects `4;`; native capture uses the empty V4 setting.
The setting pairing and geometry agreement are bounded to these controls.

`comparison.json` retains historical public SVG/resvg alpha and direct-PDF/MuPDF
RGB measurements. Both improved over the 1× contour; native RGBA was composited
over #fcfcfc for PDF comparison, which includes a substantial background/color
conversion baseline. The optional exporter and comparator were retired. MuPDF
pixel measurements are historical, not assertions in the Rust regression.

Capture with the repository's native inputs and dependencies as described in the parent README. For each scale 5 through 8, set `INKPEN_CAPTURE_SCALE` and `INKPEN_CAPTURE_BENT` to `straight`, `1`, and `loop`, and save the capture as `<scale>-<shape>.json` in a temporary directory. Run `python3 tools/inkpen-v4/highscale/pack.py <directory>` to create `packed.json` and `packed.bin`; compare frame and stream hashes to the retained native fixture. Capture JSON hashes and host addresses may vary across runs. `straight` leaves the original source trajectory unchanged. The packer validates each native frame hash and dimensions.
