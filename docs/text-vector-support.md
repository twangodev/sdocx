# Rust vector text support

One Rust measurement and layout engine serves document
Flow, standalone text boxes, supported shape text, table cells and code blocks.
Preview, replay and document PDF export consume that engine with shared font
resources. Chromium preview and vector SVG/PDF exports are the supported target
for this implementation. Full Samsung Notes visual parity is not claimed.

## Layout and transport

| Area | Implemented scope | Evidence and limits |
| --- | --- | --- |
| Fonts and spans | Pinned fonts, caller-provided fonts, local coverage fallback, typed paint/measurement projections and explicit Widget/Drawing span producers, authoritative parsed heading spans, modern composition, selected-face CBDT metadata and native hyperlink gating | Rust font/Unicode regressions and browser embedded-font checks. Legacy/incomplete composition and suggestion/correction remain unsupported. Device fallback selection and variable fonts remain outside scope. |
| Text layout | Measured glyph advances, Unicode wrapping, paragraph spacing, density-scaled margins, alignment, placed-text gravity and empty-line metrics | Independent font metrics and captured body/code/table origins; emergency breaking, RTL justification and all standalone native modes are not established. |
| Bidirectional text | Paragraph context retained across wrapping, native paragraph maps for covered cases, inline objects in visual order | Rust and Chromium regressions cover RTL, isolates and object positions; arbitrary device ICU/locale behavior remains unverified. |
| Shapes | Shared measured text within supported native template/path frames and original rotation pivots | Typed-frame, preview/replay and PDF regressions; unsupported shape frames retain saved bounds and report diagnostics. |
| Embedded content | Images, code title/body, bounded dense unmerged/merged table preparation, measured reservations, staged width/height feedback and page exclusions | Five external native-reference checks cover the locked corpus; table captures establish raw-slot cold sizing, frame-owner warm sizing and endpoint-owner bounds. Saved height limits do not cap the traced export layout. Native merged shaping/parent placement, sparse preparation and arbitrary nested composition remain unverified. |
| Table painting | Native perimeter styles, heading/default/owned fills, alpha, axis radii, prepared artwork crops and composited text surfaces | Hash-pinned Model/Drawing style selection and 78 Composer export-crop cases; SVG/replay/PDF transport tests. Native captures cover 132 per-run clip decisions/transforms, 162 entry/run-bound cases, 22 grouping probes and 230 complete cached-glyph emission cases. Rust line/run vertical bounds match within 0.0001 units. Rust retains a table-wide text clip; native shaping and device appearance remain unverified. |
| Decorations | Underline, strikethrough, uniform cluster backgrounds, supported object line bands and vector list markers | [Native entry/retained-run geometry and caller policies](reverse-engineering/text-draw-identity-findings.md#captured-embedded-object-background-geometry), [SVG/PDF regressions](../crates/sdocx/tests/text_styles.rs); document PDF paints ordinary object backgrounds in standalone/table/code contexts and omits them in Body. Backgrounds changing inside a glyph cluster or lacking safe object positions remain conservative. |
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
| `UnsupportedBackgroundPositioning` | A background cannot be placed safely at retained cluster boundaries or measured object positions. Object failures retain the object's source and owner. |
| `UnsupportedCompositionStyle` / `UnsupportedSuggestionStyle` / `UnsupportedCorrectionStyle` | An appearance-affecting source range contains legacy/incomplete composition or unsupported suggestion/correction style. Diagnostics retain the source range or enclosing object owner. |
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
Valid modern composition spans resolve typed backgrounds and underline/bold/
italic flags. Preview and replay select nonzero composing ARGB after theme
mapping before ordinary background; retained document PDF selects ordinary
background, matching the traced native consumer distinction. Generic SVG-to-PDF conversion preserves
the supplied SVG's preview background. Supported inline/block objects paint
SVG/replay line bands. Document PDF paints their ordinary backgrounds in Frame
contexts (standalone/table/code), while Body omits them according to its native
caller policy. Valid composing tags on supported objects have no unsupported-style
diagnostic. Legacy/incomplete composition and
suggestion/correction appearance remain unsupported. Typed suggestion metadata
decoding does not implement its special decoration. Native binary methods do
not make every in-memory span writable; the
[binary capture](reverse-engineering/text-draw-identity-findings.md#native-binary-boundaries)
records those distinct contracts.

Body/capture/table text uses Widget span rules; placed/code text uses Drawing
rules, independently of flow versus placed geometry. Only Widget excludes
ordinary/composing backgrounds from validated object slots; composing tags
remain unguarded in both producers. Parsed heading size/bold spans remain
authoritative, Heading 3 fallback is 15, and rendering does not synthesize the
native Model's editing spans from paragraph metadata alone.

Ordinary measurement identity retains resolved f32 size, native-theme-mapped
foreground ARGB, nullable font name and supported bold/italic bits, separately
from display color and decorations. Rust regressions match 89 supported native
predicate cases and independently verify kerned versus separate `AV` advances.
The [291-case native kernel capture](reverse-engineering/text-draw-identity-findings.md#measurement-identity)
does not include full shaping, font selection or heading-style producers.
Six identity cases across five text contexts separately verify glyph advances,
retained PDF origins/colors, midpoint-width wrapping and stored selectable source.
Retained PDF paint groups preserve different link/plain colors within one
measured run and logical text block.

Available typed `NativeDrawSpan` identity also participates in retained PDF
grouping. It preserves full foreground ARGB before link paint, separate mapped
backgrounds, nullable font names, source style/underline fields and producer
flags. Measurement derives from the same projection. Active unsupported
correction, malformed recognized payloads and incompatible font-metric recovery
leave identity unavailable; their adjacent shaped clusters do not coalesce.
The [projection tests](../crates/sdocx/src/render/text/native_identity.rs)
classify all 70 native fixture pairs, including 59 complete field/equality
comparisons and explicit unavailable/control cases. Retained-run tests preserve
shared shaping, glyph positions and exact logical source across supported
identity boundaries; they do not subdivide a single shaped source cluster.

## Known limits

- Dense merged preparation uses captured frame-owner sizing and native visibility.
  Sparse/invalid grids retain saved-frame painting with `UnsupportedContent`.
  Rotated/nested preparation and native merged shaping/parent pagination remain
  incomplete or unverified.
  [Native editor construction](reverse-engineering/table-code-findings.md#dense-editor-construction-and-sparse-transport)
  creates dense rows; separate serialized counts do not establish safe sparse
  document topology or native rendering admission.
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
- The typed native Span projection covers ordinary/default fields and retained
  PDF grouping; unsupported correction and malformed/recovered inputs have
  unavailable identity. Native font language metadata is not retained.
  Selected faces retain exact CBDT presence, but the native
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
- Native preview background geometry for embedded objects is captured through
  measurement, placement and rectangle commands for 40 supplied cases. Inline
  backgrounds include margins; block backgrounds use visible width despite a
  wider reservation. SVG/replay paints supported bands in source order;
  missing or ambiguous reordered placements report object-owned positioning
  diagnostics. Document PDF paints ordinary object backgrounds for standalone,
  table and code text, while Body omits them. Native Widget conversion and
  full line-metric production remain outside the
  [preview capture](reverse-engineering/text-draw-identity-findings.md#captured-embedded-object-background-geometry).
  The [80-case retained-run capture](reverse-engineering/text-draw-identity-findings.md#captured-retained-object-runs)
  supplies shaped owner records and cached fonts; it does not establish actual
  native object shaping, complete Drawing object conversion or PDF pixels.
  Native table Composer and newer body PDF background writers use different
  alpha gates. The [204-case export policy capture](reverse-engineering/text-draw-identity-findings.md#captured-object-export-caller-policy)
  verifies Body object exclusion, Frame caller requests and the separate
  foreground/background-alpha decisions; it does not execute native PDF painting
  or establish complete output for newer foreground-alpha edge cases.
- Foreground painting retains RGB rather than complete native span ARGB.
  Measurement preserves source alpha, but that does not implement glyph alpha.
  The [60-case native PDF alpha capture](reverse-engineering/text-draw-identity-findings.md#captured-pdf-alpha-transport)
  establishes distinct legacy Table, Code and background setter behavior;
  the Rust renderer does not reproduce those route-specific alpha rules.
  Public Standard export also batches ordinary Table/Code page objects, so
  standalone writer evidence does not establish their public-route appearance.
- Native font selection, complete heading editing/runtime lifecycle, opaque native style
  bits, variable-font instances and device-specific fallback selection are not
  established by ordinary measurement-identity coverage.
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
