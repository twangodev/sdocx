# SimpleBrush tool 1 host controls

Synthetic native Drawing/Mesa captures use the pinned script and library inputs checked by `capture.py`; transitive runtime dependencies are not independently pinned. The capture wrapper here adds tool 1, edge translation and short-movement inputs. It needs the project-local scratch inputs named in the wrapper. These controls do not establish device or saved-note behavior. The production gate admits tool 1 only for 3–32 points at sizes 8–12. The 2× profile assumes the caller's axis-aligned output scale assertion.

`receipt.json` records native vector and full-frame hashes and candidate versus generic public SVG and direct PDF comparisons. The candidate used an isolated copy with the same bounded gate. SVG was rasterized by resvg 0.45.1; direct PDF was rasterized by MuPDF 1.28.2 against the matched blank page. The edge vector differs in low f32 bits, while its rendered comparison improves. The short movement is rejected by the existing geometry gate and retains generic rendering. Other counts, sizes and trajectories within the gate extrapolate from these four synthetic paint controls. PDFium and opaque native backdrops were not tested.

Run captures from the repository root, with an existing fresh output directory: `SDOCX_ROOT="$PWD" POINT_COUNT=16 PROBE_SIZE=8 python3 tools/simplebrush-tool1/capture.py fraction12 /tmp/output/native.json --tool 1`. Use `POINT_COUNT=3 PROBE_SIZE=12`, `POINT_COUNT=32 PROBE_SIZE=12 VARIANT=reverse`, `POINT_COUNT=16 PROBE_SIZE=8 EDGE=1`, or `POINT_COUNT=3 PROBE_SIZE=8 SHORT=1` for the remaining controls. The capture relies on ignored scratch source and the named local native library; preserve those prerequisites when reproducing. `compare.py /tmp/output` requires the captured `captures` directory and public fixture exports `generic`, `one`, `two`, and `paper`; the edge and short fixture variants require the temporary fixture modifications below. Full native RGBA frames are retained compressed, and geometry bytes for regression are in the crate fixtures.


For exports, run the existing fixture with the matching `SDOCX_POINTS_COUNT`, `PROBE_SIZE`, `VARIANT` where applicable, `SDOCX_TOOL=1`, `SDOCX_V62_NATIVE=/tmp/output/native.json`, and `SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/output`:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
SDOCX_POINTS_COUNT=16 PROBE_SIZE=8 SDOCX_TOOL=1 \
SDOCX_V62_NATIVE=/tmp/output/native.json SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/output \
cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-tool1/compare.py /tmp/output
```

For the edge export, temporarily add the following after the `VARIANT` match in `two_pixel_native_export_fixture`, before `let default_size`, then use `EDGE=1` and 16 points at size 8 for capture and export. Omit `SDOCX_V62_NATIVE` for this export: the native and Rust vectors differ in low floating-point bits, as recorded above.

```rust
if std::env::var_os("EDGE").is_some() {
    for point in &mut source.points {
        point.x -= 27.0;
    }
}
```

The short capture uses three native points `(32, 32)`, `(32.25, 32)`, `(32.5, 32)` and pressures `0.5`. The generator rejects that movement before public export, so it has no candidate SVG/PDF comparison in the receipt. To reproduce the rejection in the fixture, temporarily set `source.points` to page coordinates `(32, 224)`, `(32.25, 224)`, `(32.5, 224)` at the same position in the fixture and run with `SHORT=1`, `SDOCX_POINTS_COUNT=3`, `PROBE_SIZE=8`, and `SDOCX_V62_NATIVE` set. The expected test result is the movement-gate error. Remove temporary fixture changes after these probes.
