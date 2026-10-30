# Isolated Pencil2 host fragment regression

`isolated_record_native_comparison` replays one captured 56-byte instance against transparent, opaque and translucent native priors and compares all 192 × 192 RGBA pixels. The test binds the record and decompressed frames to the retained native receipts in `{transparent,opaque,translucent}-receipt.json` alongside these assets.

The capture substituted the 30th instance of a synthetic Fountain-origin payload remapped to Pencil2 into upload slot zero, then reduced the draw to one instance. Native Drawing ran under ARM64 emulation and host Mesa. The original native shader produced `[247, 186, 0, 255]` at bottom-origin pixel `(129, 150)` on all three priors. The rasterizer models Mesa's clamp as numeric-selecting `min` then `max`; changing it to Rust `clamp` fails this pixel regression because NaN then retains the transparent prior.

The explicit replay context is translation `(12, 12)` and grain offset `(132.41741943359375, 138.97479248046875)`. The retained `trace-receipt.json` binds the transparent draw's grain offset upload to active program 3, location 4. Opaque and translucent captures used the same setup but do not independently retain this uniform binding. All three frames match with that context.

These are retained captures, not a fresh native recapture. The retained `fragment-isolated-record-control.py` and `fragment-isolated-record-trace.py` capture scripts depend on additional local harness state, starting with `fragment-nan-control.py`, and are not packaged as a standalone reproducer here. This test covers a substituted single instance and host arithmetic; it does not establish source sampling, general NaN behavior, normal scheduling, saved-note rendering or device parity.

Run `cargo test -p sdocx --lib --features pdf,serde isolated_record_native_comparison`.
