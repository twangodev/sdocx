# Unpushed review fixes

Fix all ten findings from the review of `077c3cab..586fee4`, preserving
selectable vector text and the shared Rust implementation. Work in parallel,
add regressions for the observed failures, and commit validated fixes
incrementally.

The earlier broad text-layout goal remains unfinished in the goal tracker.
The tracker cannot replace an unfinished goal; this checklist records the
current, narrower completion criteria.

| Finding | Completion criterion | Status |
| --- | --- | --- |
| NaN source identity | Unchanged recovered geometry retains full-source reflow; source edits still invalidate it. | Verified |
| Caret spans in captures | Native zero-length font spans survive valid captures with their interval semantics. | Verified |
| Empty list markers | Marker font size and reserved width use the empty paragraph's caret formatting. | Verified |
| Collection fonts in SVG | SVG embeds the selected TTC/OTC face; Chromium displays the face Rust measured. | Verified |
| CLI render diagnostics | Selected-page text and object warnings are reported for SVG, PNG, and PDF. | Verified |
| Diagnostic collection | Ordered, source-owned diagnostics use indexed deduplication. | Verified |
| Repeated body preparation | Document exports and browser sessions reuse compatible body plans across pages. | Verified |
| Off-page font warnings | Invalid font geometry remains attributed to the affected source range. | Verified |
| Inline-object font validation | Font metrics contributing to object-anchor leading are validated. | Verified |
| Repeated style scans | Style boundary resolution avoids repeated full-span scans and preserves precedence. | Verified |

All ten criteria are complete. Focused regressions, measured performance
checks, independent review, Rust feature/build checks, and Chromium
selected-face verification passed.

## Verification

- SDK: 1,094 tests passed; five external native corpus checks passed separately.
- CLI: 19 tests passed; WASM bindings: 11 tests passed.
- Strict all-feature, all-target SDK Clippy and CLI/WASM Clippy passed.
- Rust 1.92 host checks passed for minimal, render-only, all-feature, CLI, and
  WASM configurations. The installed stable toolchain passed the WASM target
  check; Rust 1.92's WASM standard library is not installed.
- Formatting passed. Independent review compared 600 exact SVG/diagnostic
  outputs across themes, Unicode, overlapping styles, and bidi controls.
- Web unit tests: 66 passed; typecheck: zero errors and warnings.
- Chromium: 59 passed, with one expected WebKit-only test skipped. The new
  selected-face regression failed on the old WASM and passed on the fresh
  build, matching standalone-font pixels, glyph origins, and ink width.
- Packaged, built, and served WASM bytes matched SHA-256
  `3fcc03ec28e9cb649d6d83081e2ff96d03d9fca8302040cfe4bd2b59b777cec3`.
  All 215 guarded build inputs remained unchanged throughout browser checks.

Release measurements on the review machine:

| Probe | Before | After |
| --- | ---: | ---: |
| 80,000 missing glyphs | 2,233 ms | 142 ms |
| 10,000 style boundaries | 259 ms | 32.5 ms |
| 50-page full-source reflow | 153.76 ms | 11.29 ms |

The missing-glyph fix also caches paragraph bidi contexts: diagnostic indexing
alone left a repeated full-paragraph scan on fallback lines. The combined
result preserves exact source output and restores approximately linear growth.

Logical captures preserve stored caret interval flags. Native CopyText's
separate interval-revision operation remains outside this fix goal.
