# SimpleBrush tool-1 short movement controls

The experimental public-export generator and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

The note-derived control uses the first 32 positions and pressures from Fountain stroke 29 in `crates/sdocx/tests/assets/fountain-v14/saved-source-29.json`, translated by (-96,-268), with y rounded through the f32 page coordinate. Brush tool 1, size 8, translucent `0x8044aaee`, and empty advanced settings replace the saved pen metadata. It is not a saved Brush object. Separate synthetic three-point paths use x values 32, 32.25, 32.5 and varying pressures 0.1, 0.9, 0.2 at sizes 8 and 12. All use a host Unicorn Drawing capture, explicit scheduled tasks, Mesa RGBA8, and a 2x matrix with translation (0.25, 0.5). They do not establish device or complete note-page behavior.

Native `SimpleBrushOutlineDrawableGLV1::drawLine` at 0x20b10–0x20b2c compares both absolute axis displacements with 1.5 and returns if both are strictly less, before any tool-specific branch. Its input pressure handling for tool 1 uses the fixed 0.5 value. The previous Rust implementation applied this movement filtering only to tools 0, 2 and 3. The new sampler applies it to tool 1 and uses 0.5 for an entirely short tool-1 path. The native vectors in `conformance/simplebrush-note-v300.json` test the 32-point note-derived path, the full 57-point path for tool 2, exact and adjacent f32 threshold positions, a duplicate terminal, a short nonduplicate terminal, and varied pressure, including entirely short paths at sizes 8 and 12 and an all-short two-point control. The threshold samples straddle x=33.5, whose immediate f32 neighbors differ by 0.000003814697265625. Native geometry is tested bit for bit, separately from paint.

`receipt.json` records hashes and comparisons. Generic versus the actual public two-pixel SVG resvg alpha L1 is 69049/1069 for note32, 24807/1332 for short8, and 53528/1906 for short12. Native support intersection/union rises respectively from 37/327, 40/150, and 78/310 to 327/327, 150/150, and 310/310. Direct PDF matched-paper MuPDF RGB L1 changes from 75972 to 4481, 27463 to 3835, and 59090 to 5714. PDFium RGB L1 changes from 75988 to 7574, 27335 to 6679, and 58951 to 9593. PDFium uses an unshifted raster grid relative to the native fractional matrix, so includes phase error. These are aggregate comparisons, not pixel parity. The one-pixel profile remains generic for tool 1, and the two-pixel size/count/context gates are unchanged.

Capture on a prepared host, with the pinned local helper tree and libraries from `tools/simplebrush-long-audit/capture.py`; its complete transitive runtime is not pinned. Create a fresh output directory and run:

```sh
POINT_COUNT=57 PROBE_SIZE=8 python3 tools/simplebrush-note-v300/capture.py fraction12 /tmp/OUT/native.json --tool 1
CASE=short POINT_COUNT=57 PROBE_SIZE=8 python3 tools/simplebrush-note-v300/capture.py fraction12 /tmp/OUT/native.json --tool 1
```

For short12 use `PROBE_SIZE=12`. Boundary controls use `CASE=1.4999961853027344`, `1.5`, or `1.5000038146972656`; each ends with a duplicate point. Use `CASE=two-short` for the two-point control and `CASE=short-terminal` for the nonduplicate terminal. Tool-2 full note input uses `--tool 2` without `CASE`. The script writes native framebuffer bytes under `captures`; compressed three painted control frames remain beside this README. It validates selected capture inputs and GL errors but does not pin all host dependencies.

  The historical PDF comparator used blank-paper composition; retained comparison outputs are not native PDF exports.
