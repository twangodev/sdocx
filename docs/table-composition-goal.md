# Table and embedded-object composition parity

Implement native-backed table and embedded-object composition through one
authoritative Rust layout engine shared by Chromium preview and vector SVG/PDF
export. Preserve selectable text and use Samsung captures as reference evidence.

Status: planned. The thread's goal tracker still holds an unfinished, paused
text-layout goal and rejected creation of this goal. This document records the
new objective and its completion criteria without changing that older goal.

## Milestones

- [ ] Audit current implementations, diagnostics, native findings and available
  captures. Refresh the reverse-engineering index and roadmap, separating
  historical findings from current supported behavior.
- [ ] Recover and document merged/sparse grid rules, cell spans, column sizing,
  row-height constraints and border precedence. Record APK/library identity,
  symbols or addresses, and the evidence supporting each rule.
- [ ] Implement those rules with bounded typed Rust models and shared layout
  abstractions. Preserve source ranges and reject malformed or unsupported
  combinations explicitly.
- [ ] Recover and implement nested embedded-object measurement and placement,
  including text wrapping, size feedback and page-boundary behavior. Define
  supported combinations and limits from evidence before implementing them.
- [ ] Add independent Rust regressions and hash-pinned Samsung reference checks
  for each supported behavior. Verify geometry, wrapping, pagination and vector
  transport separately; synthetic tests alone do not establish visual parity.
- [ ] Verify consistent Chromium preview, SVG and document PDF output, including
  selectable text, transforms, clipping and diagnostic attribution.
- [ ] Publish a support matrix linking implemented cases to evidence and tests,
  with explicit remaining limits and a final code review.

## Working constraints

- Rust is the sole authored composition implementation; no second Python layout
  implementation or raster/GPU rendering work is part of this goal.
- Prefer named abstractions and self-documenting code over extensive comments.
- Commit coherent, validated milestones incrementally with Conventional Commits.
  Push only when explicitly requested.
- If required captures are unavailable, report the missing evidence and keep
  the corresponding parity criterion open rather than substituting guesses.

The goal is complete when every milestone is verified within the documented
supported scope. Any scope reduction must be agreed with the user; documented
unsupported cases alone do not satisfy an implementation milestone.
