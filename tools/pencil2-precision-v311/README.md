# Pencil2 normalized grain coordinate control

The retired pixel-center probes compared ideal and float32 normalized grain selection using historical Python field/blend approximations. The existing Rust device rasterizer also uses float32 normalized coordinates, but these probes did not execute compiled GPU instructions or measure SVG/PDF output. The following metrics and 192-center index counts are historical model observations, not maintained production invariants.

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

The retained `note_probe.py` now verifies only native receipt/frame/geometry hashes, complete 14-float geometry records, both texture hashes and the observed second grain offset (170.24111938476562,145.2706298828125). It reads private/local outputs without copying or printing note content. It requires Python's standard library only; native recapture still requires the local dependencies described in v309.

Historically, with the captured native first frame as backdrop, the ideal and float32 models both gave 76 / 7 / 41 for the second frame. This was a note-derived synthetic Pencil2 stroke, not an original saved Pencil2 or a public export. Keep native frames local; remove temporary capture outputs after use. Neither the historical metrics nor the grain-index counts establish GPU instruction ordering, a safe source-phase correction or a public-export change. No replacement production assertion is introduced for the retired Python arithmetic model.

A historical replay from `76fd338b0e725b3cba6d1622c28908738c9ca5df` plus the probes matched the synthetic results and a fresh white-note capture; it did not make the local native runtime closure hermetic. Original producer script hashes in receipts remain historical.
