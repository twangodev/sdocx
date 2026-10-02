# Rust vector text support

One Rust measurement and layout engine serves document
Flow, standalone text boxes, supported shape text, table cells and code blocks.
Preview, replay and document PDF export consume that engine with shared font
resources. Chromium preview and vector SVG/PDF exports are the supported target
for this implementation. Full Samsung Notes visual parity is not claimed.

## Layout and transport

| Area | Implemented scope | Evidence and limits |
| --- | --- | --- |
| Fonts and spans | Pinned fonts, caller-provided fonts, local coverage fallback, per-span families, sizes, colors and styles, selected-face CBDT metadata and native hyperlink type gating | Rust font/Unicode regressions and browser embedded-font checks; composition, suggestion and correction appearance report source-owned diagnostics. Device fallback selection and variable fonts remain outside scope. |
| Text layout | Measured glyph advances, Unicode wrapping, paragraph spacing, density-scaled margins, alignment, placed-text gravity and empty-line metrics | Independent font metrics and captured body/code/table origins; emergency breaking, RTL justification and all standalone native modes are not established. |
| Bidirectional text | Paragraph context retained across wrapping, native paragraph maps for covered cases, inline objects in visual order | Rust and Chromium regressions cover RTL, isolates and object positions; arbitrary device ICU/locale behavior remains unverified. |
| Shapes | Shared measured text within supported native template/path frames and original rotation pivots | Typed-frame, preview/replay and PDF regressions; unsupported shape frames retain saved bounds and report diagnostics. |
| Embedded content | Images, code title/body, bounded dense unmerged/merged table preparation, measured reservations, staged width/height feedback and page exclusions | Five external native-reference checks cover the locked corpus; table captures establish raw-slot cold sizing, frame-owner warm sizing and endpoint-owner bounds. Saved height limits do not cap the traced export layout. Native merged shaping/parent placement, sparse preparation and arbitrary nested composition remain unverified. |
| Table painting | Native perimeter styles, heading/default/owned fills, alpha, axis radii, prepared artwork crops and composited text surfaces | Hash-pinned Model/Drawing style selection and 78 Composer export-crop cases; SVG/replay/PDF transport tests. Native captures cover 132 per-run clip decisions/transforms, 162 entry/run-bound cases, 22 grouping probes and 230 complete cached-glyph emission cases. Rust line/run vertical bounds match within 0.0001 units. Rust retains a table-wide text clip; native shaping and device appearance remain unverified. |
| Decorations | Underline, strikethrough, uniform cluster backgrounds and vector list markers | Retained layout and native endpoint contracts; backgrounds changing inside a glyph cluster remain conservative. |
| SVG preview/replay | Typed SVG elements, embedded fonts, retained text positions where reproducible, source-preserving text fallback elsewhere | Chromium tests; a complex-script fallback can preserve text without reproducing native glyph geometry. |
| Document PDF | Retained selected faces, glyph IDs, full XY origins/advances, scoped clipping/transforms, selectable text and logical tagged reading order | Independent PDF/font-outline tests and real WASM downloads; combining-mark Y parity with Samsung's common-baseline PDF route remains unverified. |
| Synthesized styles | Requested styles retained through fallback; native PDF bold pen of 0.25 points and fixed shear for synthesized italic | Regular-only font tests, independent outlines and DPI checks; canvas emboldening and native measurement/face-selection anomalies remain separate. |

Use `render_document_pdf` for a whole document, or
`render_layout_pages_pdf_with_fonts` for selected layout pages with an explicit
`FontBook`. The latter font book controls both measurement and carrier parsing.
CLI and WASM document PDF exports use this retained path.

`render_svg_pages_pdf` converts arbitrary or serialized SVG. It has no private
Rust glyph registry and can reshape text. Its output remains a compatibility
route; it does not carry the document exporter’s retained-glyph guarantee.
SVG remains responsible for surrounding vector graphics in both routes.

PDF logical source is recorded through `ActualText` and structure-tree order,
separately from paint order. Readers that ignore those structures may extract
ligatures, bidi runs or embedded objects differently. Selectable text does not
imply universal PDF accessibility conformance.

## Diagnostics and explicit failures

Rendered pages expose text and object diagnostics. Check them when assessing
fidelity; successful parsing or a generated file is not a parity certificate.

| Signal | Meaning |
| --- | --- |
| `MissingGlyphs` / `MeasurementFailure` | Available fonts cannot cover or measure the requested source. |
| `UnsupportedGlyphPositioning` | SVG text cannot reproduce the retained geometry; source is preserved through its fallback. |
| `UnsupportedBackgroundPositioning` | A background cannot be placed safely at the retained cluster boundaries. |
| `UnsupportedCompositionStyle` / `UnsupportedSuggestionStyle` / `UnsupportedCorrectionStyle` | The source includes a valid composition, suggestion or correction span whose appearance the engine does not implement. Diagnostics retain the source range or enclosing object owner. |
| `UnsupportedTextFrame` | A shape lacks a supported native text frame. |
| `UnsupportedContent` / `UnsupportedWidthLimitContext` | Embedded composition or a required runtime width context is unsupported. |
| `InvalidGeometry` / `InvalidBounds` | Geometry is unusable; the relevant adapter rejects it or uses its documented recovery. |

Retained PDF export explicitly fails on variable-font instances, unsupported
bold glyph outlines, invalid embedding/geometry, or effects that bypass its
text hook. It does not silently replace those glyphs with raster text. Arbitrary
object-bounding-box effects can still depend on carrier geometry; their native
equivalence is not established. SVG/browser synthesis and its existing Flow
bold stroke are distinct from the native PDF pen contract.

SVG fallback runs respect synthesis boundaries, including adjacent italic and
plain Arabic spans.

Font names accept native UTF-8 and CESU-8 without changing retained raw span
payloads. Hyperlink action types 1–9 enable native hypertext styling; unknown
types outside that gate do not add link color, underline or an anchor.
Composition, suggestion and correction appearance remains unimplemented,
including legacy records and transient in-memory attributes. Native binary
methods do not make every in-memory span writable; the
[binary capture](reverse-engineering/text-draw-identity-findings.md#native-binary-boundaries)
records those distinct contracts.

## Known limits

- Dense merged preparation uses captured frame-owner sizing and native visibility.
  Sparse/invalid grids retain saved-frame painting with `UnsupportedContent`.
  Rotated/nested preparation and native merged shaping/parent pagination remain
  incomplete or unverified.
- Table border paths match the [native capture](../conformance/table-border-paths.json).
  Outline color, thickness and axis radii also match
  [native Drawing aggregation](../conformance/table-border-drawing.json) at seven
  canvas scales. Document vectors use the unit-scale result; screen coverage,
  complete split-page clipping and device-rendered appearance remain unverified.
  See [border painting evidence](reverse-engineering/table-code-findings.md#border-painting).
  [Native clip commands](reverse-engineering/table-code-findings.md#visible-rectangles-and-canvas-clipping)
  establish separate raw-endpoint visible bounds and outward-rounded canvas clips;
  the native PDF route bypasses that display clip and uses the
  [captured export artwork crop](reverse-engineering/table-code-findings.md#export-artwork-crop).
  The SDK's measured-table text clip remains conservative. The
  [native per-run clip capture](reverse-engineering/table-code-findings.md#export-text-clips)
  establishes decisions and transforms with supplied run rectangles.
  [Final native path capture](reverse-engineering/table-code-findings.md#final-pdf-text-clip-paths)
  extends those inputs through 69 Pdfium clip paths, before installation;
  Rust PDF regressions separately verify supplied-clip geometry, selectable
  text and restoration of neighboring text's clip state.
  [Cell/content rectangle setters](reverse-engineering/table-code-findings.md#cell-content-model-rectangles)
  are captured independently; equal cell frames can preserve different content
  bounds, and native Model/drawn rectangles can diverge.
  [Entry/run-bound captures](reverse-engineering/table-code-findings.md#retained-text-entry-and-run-bounds)
  execute native placement and union producers with supplied metrics.
  [Complete run emission](reverse-engineering/table-code-findings.md#complete-retained-text-run-emission)
  is captured with supplied glyph caches and font interfaces; native shaping,
  real nested objects and Rust per-run clipping remain unverified/unimplemented.
- Rust measured runs do not retain the native raw Span equality fields or font
  language metadata. Selected faces retain exact CBDT presence, but the native
  bitmap/language grouping gates remain unimplemented. Their f64 cluster positions
  also differ from native f32 entry adjacency.
  [Draw identity findings](reverse-engineering/text-draw-identity-findings.md)
  establish the native producers with supplied inputs and a separate Chromium
  clip regression. Chromium preserves joined shaping with full span clips, but
  clips follow glyph ownership: clipping the first character of an `ffi`
  ligature hides the entire glyph. Native per-run clip selection remains
  unimplemented, independently of the verified PDF clip transport.
- SVG transport does not reproduce every complex joined script or cluster
  crossing a style boundary.
- Native font-selection and measurement-style anomalies, variable-font
  instances, and device-specific fallback selection are not established.
- Standalone text modes, RTL justification, separator-only clipping, and
  unusual page/composition behavior remain unverified against native captures.
- Extreme frame/page geometry and unsupported glyph/effect combinations retain
  explicit validation and transport limits.

Exact native addresses, historical comparisons and evidence limits are in
[Text layout findings](reverse-engineering/text-layout-findings.md).

## Preparation and diagnostic contracts

Full-source reflow recognizes unchanged recovered geometry, including retained
NaN values; source edits invalidate that identity. Captures preserve native
zero-length font-span intervals. Empty list markers use caret formatting for
their font size and reserved width. Native CopyText interval revision is a
separate operation.

SVG embeds the selected TTC/OTC face used for measurement. CLI exports report
text and object diagnostics for the selected pages. Ordered diagnostics use
indexed deduplication and retain source attribution; font validation includes
metrics contributing to inline-object leading.

Document exports and browser sessions reuse compatible body plans across
pages. Style boundary resolution avoids repeated full-span scans, and fallback
lines reuse paragraph bidi contexts.

## Regression evidence

- [Source identity](../crates/sdocx/tests/source_identity_review.rs) and
  [capture intervals](../crates/sdocx/tests/layout_review_regressions.rs) cover
  recovered geometry, source edits and caret formatting.
- [Body preparation](../crates/sdocx/tests/body_preparation_review.rs) and
  [geometry diagnostics](../crates/sdocx/tests/text_geometry_review_regressions.rs)
  cover compatible plan reuse and source attribution.
- [Collection-font SVG](../crates/sdocx/tests/font_collection_svg.rs) and
  [Chromium selected-face checks](../web/tests/e2e/font-collection.spec.ts)
  compare the embedded face with the face used for Rust measurement.
- [Retained PDF glyphs](../crates/sdocx/tests/pdf_retained_glyphs.rs),
  [text styles](../crates/sdocx/tests/text_styles.rs) and
  [shape text frames](../crates/sdocx/tests/shape_text_frame.rs) cover shared
  layout, synthesis, selectable source and vector transport.
- [Embedded layout regressions](../crates/sdocx/tests/embedded_text_layout.rs)
  check dense merged preparation, saved fallbacks, code panels, themes,
  selectable text and vector transport. [Paged table regressions](../crates/sdocx/tests/prepared_tables.rs)
  compare merged owner placement with an unmerged projection at identical saved
  bounds. These transport checks do not establish device appearance parity.
- The [native table capture matrix](../conformance/README.md#document-composition-regressions)
  records each hash-pinned fixture's coverage and isolated dependencies. It
  distinguishes ownership, merge construction, borders/fills, column minima,
  cold/warm row sizing, bounds, split caches, compression and cell inputs from
  shaping and complete pagination. Exact native contracts are in
  [table/code findings](reverse-engineering/table-code-findings.md).

The external reference tests require the local corpus described in
[Conformance testing](../conformance/README.md). A passing synthetic regression
or transport test is not a captured visual comparison.
