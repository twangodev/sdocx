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
- [`fixture-validation.md`](fixture-validation.md) — historical measurements
  from three retired fixtures, preserving the evidence behind the format map.
- [`stroke-rendering-findings.md`](stroke-rendering-findings.md) — root cause of
  the stray top-right strokes and the exact packed-point layout.
- [`text-box-findings.md`](text-box-findings.md) — native standalone-text frames,
  bounded rich-text decoding, diagnostics, regressions and rendering limits.
- [`image-findings.md`](image-findings.md) — displayed-image versus border/original
  references, authoritative media bindings and image regression coverage.
- [`shape-line-findings.md`](shape-line-findings.md) — native geometry and effects,
  pen references, bounded paths, rendering coverage and remaining fidelity gaps.
- [`visual-conformance-findings.md`](visual-conformance-findings.md) — measured
  Samsung PDF comparison, explicit PNG fonts and remaining visual gaps.
- [`pdf-export-findings.md`](pdf-export-findings.md) — shared SVG-to-PDF export,
  page units, embedded text/fonts and measured PDF validation.
- [`native-pdf-stroke-findings.md`](native-pdf-stroke-findings.md) — native
  vector-export stroke bitmaps, PDF image handoff and separate opacity inputs.
- [`standard-pdf-composition-findings.md`](standard-pdf-composition-findings.md)
  — public export option selection, Standard paint order, Darken highlighters
  and final object-batch flushing.
- [`end-tag-findings.md`](end-tag-findings.md) — native metadata boundaries,
  appended trailer precedence, bounded decoding and synthetic regressions.
- [`layer-findings.md`](layer-findings.md) — native layer identity, alpha-lock
  and shadow fields, bounded decoding and the Java transparency discrepancy.
- [`page-layer-selection-findings.md`](page-layer-selection-findings.md) —
  saved current-layer assignment, Standard PDF page pointers and semantic
  selection with complete structural retention.
- [`object-base-findings.md`](object-base-findings.md) — shared object visibility,
  editing flags, replay/resize values and preserved frame extensions.
- [`object-flexible-findings.md`](object-flexible-findings.md) — optional common
  fields, bundle boundaries and the distinct static extraction format.
- [`object-drawing-findings.md`](object-drawing-findings.md) — common visibility,
  container traversal, replay-order assignment and layer collection boundaries.
- [`object-order-findings.md`](object-order-findings.md) — file-order insertion,
  nested container order, stroke-only top selection and grouping boundaries.
- [`capture-composition-findings.md`](capture-composition-findings.md) — base,
  top and masking passes, object layer filters and capture clone state.
- [`stroke-metadata-findings.md`](stroke-metadata-findings.md) — stroke property
  polarity, ARGB colors, pen settings and legacy partial-rectangle records.
- [`pen-opacity-findings.md`](pen-opacity-findings.md) — pen-specific fixed
  opacity dispatch, theme-preserved alpha and Marker2 mask/composite equations.
- [`pen-selection-findings.md`](pen-selection-findings.md) — corrected stroke
  string IDs, native pen registry, fallback lookup and Marker2 version selection.
- [`integrity-findings.md`](integrity-findings.md) — optional hash verification,
  exact coverage, unavailable checks and independent synthetic reference hashes.
- [`note-header-findings.md`](note-header-findings.md) — variable note masks,
  bounded fixed data and structured document metadata.
- [`note-metadata-findings.md`](note-metadata-findings.md) — optional application,
  author, pen, voice, attachment and fixed-style fields with bounded records.
- [`table-code-findings.md`](table-code-findings.md) — native inheritance chains,
  table styles, per-edge borders, bounded row/cell data and the nonnumerical
  row-height field order.
- [`math-findings.md`](math-findings.md) — native math envelopes, embedded
  formula boundaries, angle modes and connected plot references.
- [`plot-findings.md`](plot-findings.md) — plot coordinates, graph expressions,
  substitutions, colors, widths and visibility.
- [`formula-findings.md`](formula-findings.md) — formula expressions, answers,
  embedded strokes, image references and label graphs.
- [`formula-rendering-findings.md`](formula-rendering-findings.md) — image/ink
  precedence, image placement, visible-stroke bounds and expression-type limits.
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

## Sources and validation

- Samsung Notes APK: 4.4.45.37 (`arm64-v8a`/`armeabi-v7a`), SHA-256
  `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
- Historical audit: 7,182 stroke objects, 924,442 points, three layer hashes
  and three page hashes with zero structural/hash mismatches. The retired
  fixtures are identified by digest in [fixture validation](fixture-validation.md).
- Current corpus and test commands: [`conformance/README.md`](../../conformance/README.md).
  Historical audit totals do not describe current corpus coverage.
