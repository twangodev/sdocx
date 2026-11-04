# Marker3 tool 1 name and setting controls

`capture.py` reconstructs three synthetic Model strokes, selecting tool 1 at size 8 via Drawing for both `Marker3` and `StraightMarker`, with literal `5;` and non-null empty settings. It reconnects the same native callback vector to RTV1 after Drawing clears the slots and paints on host Mesa at 2× with quarter-output-pixel translation. `verify.py` checks all four selected V5 drawables, full uploaded stream hashes, draw calls, shader sources, GL errors, and complete 256×256 frame bytes. The compact `native.json` retains hashes and inputs. This bridge does not observe the native SetPenData call site or establish serialized-note or Android-device parity.

```sh
.venv/bin/python tools/marker3-tool1-alias/capture.py /tmp/marker3-tool1-alias
.venv/bin/python tools/marker3-tool1-alias/verify.py /tmp/marker3-tool1-alias tools/marker3-tool1-alias/native.json
```

The paired public SVG and direct PDF comparisons in `fractional-export.json` and `crossing-export.json` use `StraightMarker` with an empty setting, against the fresh native frame. Their baseline is canonical vector output, and their profile selects the explicit 2× scale. Resvg compares transparent premultiplied RGBA; MuPDF and PDFium compare RGB against mathematical composition of the native frame over #fcfcfc. Every reader reduces aggregate error in both controls; this is not native opaque-backdrop or device parity. `export.rs` records the public SDK inputs and exporter. Output and reader versions are captured in the receipts.

The `*-default-export.json` receipts compare default admitted output against a generic fallback of the same stroke, with only the name changed to an unsupported value to force fallback. On these controls default output lowers aggregate error in all three readers; it does not reproduce native pixels. `SDOCX_FALLBACK=1` selects the generic-name probe in `export.rs`. The `profile` receipt key denotes the tested candidate; for the default receipts it is the ordinary route at 2× rasterization, not the explicit output-scale option.

The incremental-commit validation uses identical inputs before and after admission. `same-input-export.json` compares an exporter built from the previous production sources against the isolated candidate, for both default and explicit 2× options. All twelve reader comparisons improve. `validation.json` pins the base commit, relevant build sources, harness and receipts; it records the test-only change present in the before build. The freshly reproduced twelve native alias captures match `native.json` exactly. The two compressed native frames are retained for export replay.

The optional paired comparator has been retired. `validation.json` retains its historical script hash as provenance, not a current executable dependency.

The native controls establish synthetic host alias equivalence at size 8; existing admitted size bounds remain 4–8. StraightMarker 4× is admitted by the [separate 4× controls](../marker3-tool1-scale4-alias/README.md).
