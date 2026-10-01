# Rust vector text support

One Rust measurement and layout engine serves document
Flow, standalone text boxes, supported shape text, table cells and code blocks.
Preview, replay and document PDF export consume that engine with shared font
resources. Chromium preview and vector SVG/PDF exports are the supported target
for this implementation. Full Samsung Notes visual parity is not claimed.

## Layout and transport

| Area | Implemented scope | Evidence and limits |
| --- | --- | --- |
| Fonts and spans | Pinned fonts, caller-provided fonts, local coverage fallback, per-span families, sizes, colors and styles | Rust font/Unicode regressions and browser embedded-font checks; device fallback selection and variable fonts remain outside scope. |
| Text layout | Measured glyph advances, Unicode wrapping, paragraph spacing, density-scaled margins, alignment, placed-text gravity and empty-line metrics | Independent font metrics and captured body/code/table origins; emergency breaking, RTL justification and all standalone native modes are not established. |
| Bidirectional text | Paragraph context retained across wrapping, native paragraph maps for covered cases, inline objects in visual order | Rust and Chromium regressions cover RTL, isolates and object positions; arbitrary device ICU/locale behavior remains unverified. |
| Shapes | Shared measured text within supported native template/path frames and original rotation pivots | Typed-frame, preview/replay and PDF regressions; unsupported shape frames retain saved bounds and report diagnostics. |
| Embedded content | Images, code title/body, unmerged table preparation, native merged-cell visibility with saved frames, measured reservations, staged width/height feedback and page exclusions | Five external native-reference checks cover the locked corpus; saved row maxima do not cap the traced table export layout. Merged frame sizing, sparse preparation and arbitrary nested composition remain incomplete. |
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

## Known limits

- Merged/sparse table preparation, rowspan growth, and nested or rotated object
  feedback are incomplete. Merged saved-frame painting follows native visibility;
  merged sizing and sparse/invalid-grid fallbacks report `UnsupportedContent`.
- SVG transport does not reproduce every complex joined script or cluster
  crossing a style boundary.
- Native font-selection and measurement-style anomalies, variable-font
  instances, and device-specific fallback selection are not established.
- Standalone text modes, RTL justification, separator-only clipping, and
  unusual page/composition behavior need additional captured evidence.
- Extreme frame/page geometry and unsupported glyph/effect combinations retain
  explicit validation and transport limits.

Samsung output remains reference evidence. The implementation and regression
contracts stay in Rust; no second authored Python layout engine is required.
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
lines reuse paragraph bidi contexts. Release measurements from the review
machine showed the following changes; these are historical observations, not
portable performance thresholds:

| Probe | Before | After |
| --- | ---: | ---: |
| 80,000 missing glyphs | 2,233 ms | 142 ms |
| 10,000 style boundaries | 259 ms | 32.5 ms |
| 50-page full-source reflow | 153.76 ms | 11.29 ms |

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
- [Native table ownership and visibility](../conformance/table-ownership.json)
  and [saved-frame vector outputs](../crates/sdocx/tests/embedded_text_layout.rs)
  distinguish frame owners from paint-visible cells; merged geometry remains
  outside that evidence.

The external reference tests require the local corpus described in
[Conformance testing](../conformance/README.md). A passing synthetic regression
or transport test is not a captured visual comparison.
