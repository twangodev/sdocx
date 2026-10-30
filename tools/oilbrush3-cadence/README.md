# OilBrush3 source-driven feedback controls

The Rust conformance model reconstructs P4 and feedback model matrices from
AddPoint records, checks native rendering-level blend uniforms, and replays two
nonuniform page controls through P4, P3 feedback, and the following P4 draw.
Compressed native pixels are expected outputs only. These are test-only models;
production dispatch is unchanged. A public SVG regression separately records the
remaining difference from the V17 native frame.

Run the retained source regressions with:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde ink::oilbrush3
```

The two nonuniform controls inject an opaque 64x64 pattern into the initialized
native page before drawing. They establish the captured host feedback behavior,
not normal page initialization, Android rendering, or saved-note parity. The
independent control takes its P3 ratio from the earlier V17 state; its own uniform
list does not capture that value. Details and hashes are retained in each
`conformance/oilbrush3-*-feedback/receipt.json`.

Capture scripts retain their pinned input checks but require the original
`scratch/oilbrush3-public-drawing-v17` runner, preflight manifest, native libraries,
and transitive helper tree. Those dependencies are not all packaged here. The
Rust regression command uses only repository fixtures and does not execute a
native library. `matrix_capture.py` and `ratio_trace.py` reproduce the corresponding
compact receipts when that original capture environment is available.
