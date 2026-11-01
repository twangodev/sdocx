# Pencil2 normalized grain coordinate control

`PYTHONPATH=/tmp/sdocx-marker3-v133-readers .venv/bin/python tools/pencil2-precision-v311/probe.py` repeats the v309 first and v308 second pixel-center diagnostics with a float32 normalized grain selection. It asserts the source script hashes, both original and modified error triples, and the 192-center boundary counts. Run from the repository root without `PENCIL_GRAIN_DX` or `PENCIL_GRAIN_DY` interventions. The existing `device_raster.rs` also calculates the normalized coordinate with f32 before repeat wrapping and selecting a texel. This experiment tests that model, not the compiled GPU instructions. Other field and blend calculations remain the original Python approximations. It does not measure an SVG or PDF.

| Input and backdrop | Original ideal grain selection RGB absolute error sum / max / differing channels | Float32 normalized grain selection |
|---|---:|---:|
| Synthetic first, white 1× | 344 / 29 / 139 | 170 / 3 / 133 |
| Synthetic first, white 2× | 767 / 4 / 607 | 767 / 4 / 607 |
| Synthetic second over native first, white 1× | 2,705 / 26 / 764 | 2,470 / 18 / 758 |
| Synthetic second over native first, white 2× | 10,361 / 24 / 2,992 | 10,361 / 24 / 2,992 |
| Synthetic second over native first, blue 2× | 17,299 / 24 / 3,866 | 17,299 / 24 / 3,866 |

The first white 1× output at top-origin (30,179) changes from (182,201,219) to the native (153,178,205), without a fitted phase shift. Across 192 pixel centers, the 1× phase offsets 6 and 9 select the preceding grain texel instead of the ideal index at 45 coordinates each, and never the following texel. The corresponding 2× offsets select identical indices at all 192 centers. This is a property of this computation and these coordinates, not a general texture sampling rule.

For an off-boundary independent note-derived path, recapture using the native prerequisites and limitations in v309:

```sh
SDOCX_PENCIL2_CLEAR_RGBA=255,255,255,255 SDOCX_PENCIL2_FRESH_FRAME_DIR=/tmp/pencil-v311-note python3 tools/pencil2-mesh-v309/capture.py > /tmp/pencil-v311-note-receipt.json
.venv/bin/python tools/pencil2-precision-v311/note_probe.py /tmp/pencil-v311-note /tmp/pencil-v311-note-receipt.json
```

The probe verifies frame, descriptor, texture and source hashes and the second grain offset (170.24111938476562,145.2706298828125). With the captured native first frame as backdrop, the ideal and f32 models both give 76 / 7 / 41 for the second frame. This is a note-derived synthetic Pencil2 stroke, not an original saved Pencil2 or a public export. Remove the temporary frame directory and receipt after use. These results support f32 normalized sampling as an explanation for part of the boundary discrepancy, but do not establish the exact GPU instruction sequence. The vector mesh's source cell geometry and reader rasterization do not directly encode this destination-dependent finite-precision selection; no source phase correction or public exporter change follows from these tests. See v309/v310 for actual SVG/PDF comparisons and capture limitations.

Validation replay used tracked dependencies archived from `76fd338b0e725b3cba6d1622c28908738c9ca5df` with only these v311 scripts overlaid. Both probes passed, including a fresh white note recapture. Synthetic replay depends on tracked v308/v309 probes, `tools/pencil2-mixed-scene-controls`, `tools/pencil2-worker-v259/native-geometry.vbo`, and Pencil2 texture assets; v310 is background context only. Python requires Pillow plus PyMuPDF and pypdfium2 because v308 imports its scene module; the reader directory above is a local dependency. Native recapture additionally needs the local harness/runtime prerequisites documented in v309, and does not become hermetic through this clean-source replay. No render outputs are retained.
