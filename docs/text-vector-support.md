# Rust vector text support

This milestone provides one Rust measurement and layout engine for document
Flow, standalone text boxes, supported shape text, table cells and code blocks.
Preview, replay and document PDF export consume that engine with shared font
resources. Chromium preview and vector SVG/PDF exports are the supported target
for this milestone. Full Samsung Notes visual parity is not claimed.

## Layout and transport

| Area | Implemented scope | Evidence and limits |
| --- | --- | --- |
| Fonts and spans | Pinned fonts, caller-provided fonts, local coverage fallback, per-span families, sizes, colors and styles | Rust font/Unicode regressions and browser embedded-font checks; device fallback selection and variable fonts remain outside scope. |
| Text layout | Measured glyph advances, Unicode wrapping, paragraph spacing, density-scaled margins, alignment, placed-text gravity and empty-line metrics | Independent font metrics and captured body/code/table origins; emergency breaking, RTL justification and all standalone native modes are not established. |
| Bidirectional text | Paragraph context retained across wrapping, native paragraph maps for covered cases, inline objects in visual order | Rust and Chromium regressions cover RTL, isolates and object positions; arbitrary device ICU/locale behavior remains unverified. |
| Shapes | Shared measured text within supported native template/path frames and original rotation pivots | Typed-frame, preview/replay and PDF regressions; unsupported shape frames retain saved bounds and report diagnostics. |
| Embedded content | Images, code title/body, unmerged table cells, measured reservations, staged width/height feedback and page exclusions | Five external native-reference checks cover the locked corpus; merged/sparse grids and arbitrary nested composition remain incomplete. |
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

The closing consistency audit added native-document PDF checks alongside
generic SVG conversion for mixed styled text, regular-only synthesized shape
text checks through preview/replay/PDF, and styled Arabic WASM downloads.
It also fixed synthesized italic leaking into an adjacent plain Arabic SVG
fallback span; those fallback runs now respect synthesis boundaries.

## Closing scope and follow-up work

The milestone closes with the synthesis implementation committed, the shared
text contexts checked through preview/replay and PDF, the native corpus checks
passing, and the final built WASM verified against the served browser artifact.
New font, table or script features are separate work rather than prerequisites
for calling this bounded pipeline implemented.

Follow-up areas:

1. Merged/sparse table preparation, rowspan growth, and unsupported nested or
   rotated object feedback.
2. A complete SVG transport for complex joined scripts and clusters crossing
   style boundaries, while preserving selectable source.
3. Native font-selection and measurement-style anomalies, variable-font
   instances, and device-specific fallback fonts.
4. Captured standalone text modes, RTL justification, separator-only clipping,
   and unusual page/composition behavior.
5. Extreme frame/page validation and unsupported glyph/effect boundaries.

Samsung output remains reference evidence. The implementation and regression
contracts stay in Rust; no second authored Python layout engine is required.
Exact native addresses, historical comparisons and evidence limits are in
[Text layout findings](reverse-engineering/text-layout-findings.md).

## Validation

The closure checks include workspace tests with all features, strict Clippy,
formatting, render-only and minimal builds, the five external native-reference
checks, web typecheck/unit tests, and Chromium against freshly built WASM.
Focused regressions cover retained PDF glyphs, mixed styles in all shared
text contexts, shape frames and synthesized styles.

Closure results on 2026-09-30, implementation and tests at `0a06b1a`:

| Check | Result |
| --- | --- |
| Workspace tests, all features | Passed, including 496 core tests, 26 retained-PDF tests, 27 text-style tests and 10 shape-frame tests. |
| Strict workspace/all-target Clippy, formatting | Passed. |
| Render-only tests and minimal-feature build | Passed. |
| External native references | Five passed. |
| Web typecheck and unit tests | Zero errors/warnings; 66 tests passed. |
| Chromium preview/export tests | 58 passed; one WebKit-only test skipped. |
| Final WASM and application build | Passed; packaged, built and served WASM bytes matched. |

The matched WASM SHA-256 was
`10d47282f36cd2bfc89a30e7e861839354e7ab7dae42570984cf5748896ea734`.

The external reference tests require the local corpus described in
[Conformance testing](../conformance/README.md). A passing synthetic regression
or transport test is not a captured visual comparison.
