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
| CLI render diagnostics | Selected-page text and object warnings are reported for SVG, PNG, and PDF. | Pending |
| Diagnostic collection | Ordered, source-owned diagnostics use indexed deduplication. | Pending |
| Repeated body preparation | Document exports and browser sessions reuse compatible body plans across pages. | Pending |
| Off-page font warnings | Invalid font geometry remains attributed to the affected source range. | Pending |
| Inline-object font validation | Font metrics contributing to object-anchor leading are validated. | Pending |
| Repeated style scans | Style boundary resolution avoids repeated full-span scans and preserves precedence. | Pending |

Completion requires focused regressions, measured performance checks for the
three scaling findings, an independent review of the combined changes, Rust
feature/build checks, and Chromium verification of selected-face embedding.
