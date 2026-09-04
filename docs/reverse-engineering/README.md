# Reverse-engineering knowledge base

This directory is the maintained Markdown memory for Samsung Notes SDOCX/WDoc
reverse engineering. It records conclusions that are backed by the Samsung
Notes APK, native serializers, or real compatibility fixtures.

## Documents

- [`file-format.md`](file-format.md) — authoritative archive and binary-format
  map: `note.note`, pages, layers, objects, frames, strokes, media, hashes and
  end tags.
- [`source-map.md`](source-map.md) — where each conclusion comes from in the
  decompiled Java and native libraries.
- [`fixture-validation.md`](fixture-validation.md) — repeatable facts measured
  across the handwritten, quiz and CS61BL documents.
- [`stroke-rendering-findings.md`](stroke-rendering-findings.md) — root cause of
  the stray top-right strokes and the exact packed-point layout.
- [`text-box-findings.md`](text-box-findings.md) — native standalone-text frames,
  bounded rich-text decoding, diagnostics, regressions and rendering limits.
- [`image-findings.md`](image-findings.md) — displayed-image versus border/original
  references, authoritative media bindings and image regression coverage.
- [`parser-roadmap.md`](parser-roadmap.md) — implementation sequence and
  compatibility rules for the Rust parser.

## Maintenance rules

- Mark facts as confirmed, inferred or unresolved.
- Prefer declared record sizes, offsets and masks over fixture-specific magic
  numbers.
- Record the APK version and fixture set used for a conclusion.
- Keep unknown fields and object types round-trippable where practical.
- Do not commit the APK, decompiled sources or compatibility documents here.
  Store the test corpus externally and keep only measurements/expectations in
  the repository.
- Keep the knowledge base Markdown-only. One-off disassembly/audit programs can
  remain disposable local tooling; durable conclusions belong in these files.

## Current baseline

- Samsung Notes APK: 4.4.45.37 (`arm64-v8a`/`armeabi-v7a`), SHA-256
  `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
- Fixtures: handwritten, quiz and CS61BL.
- Validated: 7,182 stroke objects, 924,442 points, three layer hashes and three
  page hashes with zero structural/hash mismatches.
