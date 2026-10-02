# Rust vector text support

One Rust measurement and layout engine serves document
Flow, standalone text boxes, supported shape text, table cells and code blocks.
Preview, replay and document PDF export consume that engine with shared font
resources. Chromium preview and vector SVG/PDF exports are the supported target
for this implementation. Full Samsung Notes visual parity is not claimed.

## Layout and transport

| Area | Implemented scope | Evidence and limits |
| --- | --- | --- |
| Fonts and spans | Pinned fonts, caller-provided fonts, local coverage fallback, typed paint/measurement projections and explicit Widget/Drawing span producers, authoritative parsed heading spans, modern composition, selected-face CBDT metadata and native hyperlink gating | Rust font/Unicode regressions, browser embedded-font checks and [native NAME/default selection](reverse-engineering/text-layout-findings.md#captured-span-font-name-and-default-selection) under supplied XML/four pinned faces. Legacy/incomplete composition and suggestion/correction remain unsupported. Device fallback selection and variable fonts remain outside scope. |
| Text layout | Measured glyph advances, Unicode wrapping, paragraph spacing, density-scaled margins, alignment, placed-text gravity and empty-line metrics | Independent font metrics, captured body/code/table origins and [native ordinary wrap arithmetic](reverse-engineering/text-layout-findings.md#captured-ordinary-wrap-arithmetic), consumed through retained UTF-16 advances for wholly native-measured ordinary paragraphs. A separate [native paragraph-loop capture](reverse-engineering/text-layout-findings.md#captured-paragraph-layout-loop) executes host ICU and wrapping over supplied measured entries. SDK UAX breaks/graphemes/heights, complete Rust/native wrap parity and all standalone native modes remain separate limits. |
| Page text ranges | Saved sections, boundary overlap and a typed measured-line indexing kernel | [Twelve native-produced page profiles](reverse-engineering/text-layout-findings.md#captured-native-produced-page-ranges) match 231 boundary queries and 36 scans; another 47 supplied cases pin integer page arithmetic and retained rescans. Page records are supplied. Production body routing, native page-bound/obstacle producers and document repagination remain separate limits. |
| Bidirectional text | Paragraph context retained across wrapping, native paragraph maps for covered cases, inline objects in visual order | Rust and Chromium regressions cover RTL, isolates and object positions; arbitrary device ICU/locale behavior remains unverified. |
| Shapes | Shared measured text within supported native template/path frames and original rotation pivots | Typed-frame, preview/replay and PDF regressions; unsupported shape frames retain saved bounds and report diagnostics. |
| Embedded content | Images, code title/body, bounded dense unmerged/merged table preparation, measured reservations, staged width/height feedback and page exclusions | Five external native-reference checks cover the locked corpus; table captures establish raw-slot cold sizing, frame-owner warm sizing and endpoint-owner bounds. Saved height limits do not cap the traced export layout. [Code chrome/minimum geometry](reverse-engineering/table-code-findings.md#code-block-chrome-and-split-inputs) matches 18 cold/cleared native cases with supplied child metrics. Live native table/cell geometry and per-line vertical bounds match 18 cold/warm cases and [10 padding cases](reverse-engineering/table-code-findings.md#captured-live-table-padding); native cache lifetime, parent placement beyond the supplied single-table object geometry, sparse preparation and arbitrary nested composition remain unverified. |
| Table painting | Native perimeter styles, heading/default/owned fills, alpha, axis radii, prepared artwork crops and composited text surfaces | Hash-pinned Model/Drawing style selection, [60 complete cell artwork cases](reverse-engineering/table-code-findings.md#complete-cell-artwork-pass) and 78 Composer export-crop cases; SVG/replay/PDF transport tests. Native captures cover 132 per-run clip decisions/transforms, 162 entry/run-bound cases, 22 grouping probes and 230 complete cached-glyph emission cases. Rust line/run vertical bounds match within 0.0001 units. Rust retains a table-wide text clip. Native shaping and admitted pinned Regular metrics are verified separately; [14 actual cell-text producer cases](reverse-engineering/table-code-findings.md#captured-cell-text-measurement) separately connect Model text through native shaping/layout to cached runs and bounds. A [certified whole-source paint plan](reverse-engineering/text-draw-identity-findings.md#certified-whole-source-paint-plans) matches 12 cell producer cases, 20 runs and 50 glyphs and feeds SVG/PDF. Eight source-input LTR profiles add 14 runs/33 glyphs; nonrepresentable SVG shaping or world translations retain explicit fallback. Complete cell-loop profiles outside that certificate, metrics beyond admitted profiles, per-run clip selection and device appearance remain unverified. |
| Decorations | Underline, strikethrough, uniform cluster backgrounds, supported object line bands and vector list markers | [Native entry/retained-run geometry and caller policies](reverse-engineering/text-draw-identity-findings.md#captured-embedded-object-background-geometry), [SVG/PDF regressions](../crates/sdocx/tests/text_styles.rs); document PDF paints ordinary object backgrounds in standalone/table/code contexts and omits them in Body. Backgrounds changing inside a glyph cluster or lacking safe object positions remain conservative. |
| SVG preview/replay | Typed SVG elements, embedded fonts with private physical-face identities, retained text positions where reproducible, source-preserving text fallback elsewhere | [Rust physical-face/usvg/PDF transport and standalone Chromium font identity](reverse-engineering/text-layout-findings.md#svg-physical-face-transport), with separate preview-image pixel tests; a complex-script fallback can preserve text without reproducing native glyph geometry. |
| Document PDF | Retained selected faces, glyph IDs, full XY origins/advances, scoped clipping/transforms, selectable text and logical tagged reading order | Independent PDF/font-outline tests and real WASM downloads; captured stacked-mark common-baseline transport is verified at sizes 17/50. Broader combining-mark Y parity remains unverified. |
| Synthesized styles | Requested styles retained through fallback; native PDF bold pen of 0.25 points and fixed shear for synthesized italic | Regular-only font tests, independent outlines and DPI checks; canvas emboldening and native measurement/face-selection anomalies remain separate. |

Use `render_document_pdf` for a whole document, or
`render_layout_pages_pdf_with_fonts` for selected layout pages with an explicit
`FontBook`. The latter font book controls both measurement and carrier parsing.
CLI and WASM document PDF exports use this retained path.
`PdfOptions::from_font_book` preserves both the database and native NAME
configuration; default options use the default book. `PdfOptions::new` with a
custom database leaves native configuration absent. Supplied-book exports
override both fields, keeping measurement admission consistent with that book.
CLI exports without explicit font files retain the default book/configuration;
explicit font files build a caller database and use compatibility measurement.

`render_svg_pages_pdf` converts arbitrary or serialized SVG. It has no private
Rust glyph registry and can reshape text. Its output remains a compatibility
route; it does not carry the document exporter’s retained-glyph guarantee.
SVG remains responsible for surrounding vector graphics in both routes.
Generated SVG families identify source font bytes plus collection index.
`svg_font_resolver` pairs those families with existing IDs in the same supplied
usvg database; the CLI and PDF converter use that resolver. Raw SVG PDF
conversion rejects a missing private physical alias. Retained PDF plans keep
their original font bytes even when carrier parsing uses another database.

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
The [cached-entry kernel](../crates/sdocx/src/render/text/native_runs.rs) matches
324 supplied snapshot cases and 624 native output records with exact f32
geometry. Those snapshot tests use opaque payload references rather than font
glyph IDs. Compatibility painting uses the boundary predicate with known span
fields, retaining measured-run and paint checks when full identity is
unavailable. The [whole-source certificate](reverse-engineering/text-draw-identity-findings.md#certified-whole-source-paint-plans)
separately supplies dense entry, registered-font and cache fields to the complete
emitter, and compares actual run ranges/maps, glyph IDs, geometry and paint.
The comparisons cover kinds 0/3 and 56 published kind-5 object records,
including nonempty false-drawable/null-font cases. Twenty-six glyphless
precondition controls remain separate from valid emitted records.
Typed source ranges carry consistent character/UTF-8/UTF-16 boundaries from
Rust shaping through retained glyphs. The registry validates all three against
the block text before PDF transport. Within the certificate, comparisons prove
native run UTF-16 source ranges and paragraph maps and actual glyph-ID mapping;
they do not compare every per-glyph source-owner value. Outside it, checked
Rust source transport alone does not establish native ownership or glyph mapping.
Local vertical line bands use native f32 operation order and match all 162
entry/run-bound and 230 cached-run cases exactly. Frame origins are added
separately in f64. The certified cell plan covers its bounded native horizontal
grouping and exact representable world translation; broader world composition
and compatibility ownership/adjacency remain unverified.

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
  [Actual cloned-cell writer capture](reverse-engineering/table-code-findings.md#captured-live-table-text-clipping)
  additionally connects native measurement/cached runs to actual content Model
  source rectangles and writer clip/intersection guards. Its source controls are
  explicit, and upstream document-bound callbacks, parent writer placement and
  final PDF backend clipping remain outside it. The isolated typed decision
  kernel matches 132 supplied controls and all 47 actual runs; production still
  retains a conservative table-wide scope outside certified Model/source
  contexts. Native helper transport evidence remains separate from public
  document clipping admission.
  [Genuine Bodytext placement](reverse-engineering/table-code-findings.md#captured-bodytext-table-placement)
  separately executes document object feedback, real GetTextBound/affine Model
  placement and cell-source updates before cloned writing for three supplied
  document profiles. Nine writer stages retain 54 runs and 30 selected clips;
  full document setup/page-bound production and final PDF backend remain outside
  it. A geometry-only `NativeObjectEntryBounds` adapter matches the three profiles
  in Flow/Capture; kind-5 cached glyph/run/font/style and parent writer parity
  are outside that adapter and the full-source text paint certificate.
  [Ordinary one-page captures](reverse-engineering/table-code-findings.md#captured-ordinary-one-page-measurement)
  additionally execute real `BodyTextDocument(false)` measurement at height zero
  despite a positive physical page rectangle, then native ±10 page-padding
  production, source feedback and cloned writing. The supplied page has null
  `WPage`; an empty parsed object list has source-only equivalence, without
  a nonnull runtime control. Public clipping admission is separate from these
  native producer and recording-writer results.
  [Cell/content rectangle setters](reverse-engineering/table-code-findings.md#cell-content-model-rectangles)
  are captured independently; equal cell frames can preserve different content
  bounds, and native Model/drawn rectangles can diverge.
  [Entry/run-bound captures](reverse-engineering/table-code-findings.md#retained-text-entry-and-run-bounds)
  execute native placement and union producers with supplied metrics.
  [Complete run emission](reverse-engineering/table-code-findings.md#complete-retained-text-run-emission)
  is captured with supplied glyph caches and font interfaces. A certified
  whole-source cell paint plan also connects native measurement, dense UTF-16
  entries and complete run grouping within the default-face 17/50 profile.
  Real nested objects and Rust per-run clip selection remain outside it.
- The typed native Span projection covers ordinary/default fields and retained
  PDF grouping; unsupported correction and malformed/recovered inputs have
  unavailable identity. Admitted default-family/default-book Regular runs retain a registry
  source token, empty language and false bitmap metadata; clones/PDF options
  preserve that instance while database/configuration replacement invalidates it.
  Explicit NAME selections have no certified token; retained entry classification
  and height facts are limited to sizes 17 and 50. Other selected faces retain
  exact CBDT presence. Certified cell painting invokes the complete emitter once
  for the full source and shares its plan between selectable SVG and retained PDF;
  compatibility grouping still uses a partial span boundary. Admitted
  native-measured ordinary paragraphs use captured f32 glyph geometry and
  retained UTF-16 entry slots. The bounded mixed path uses native text slots and
  f32 selection/cursor arithmetic around SDK object anchors. Compatibility
  positioning/ownership and object preparation/height/break policy outside the
  single-table geometry profile remain SDK behavior; native cached-run grouping,
  including horizontal f32 adjacency,
  outside the certificate remains unverified.
  [Draw identity findings](reverse-engineering/text-draw-identity-findings.md)
  establish the native producers with supplied inputs and a separate Chromium
  clip regression. Chromium preserves joined shaping with full span clips, but
  clips follow glyph ownership: clipping the first character of an `ffi`
  ligature hides the entire glyph. Native per-run clip selection remains
  unimplemented, independently of the verified PDF clip transport.
  [Cached snapshots](reverse-engineering/text-draw-identity-findings.md#captured-cached-entry-snapshots)
  preserve supplied codewords and shaping owners; the
  [whole-source certificate](reverse-engineering/text-draw-identity-findings.md#certified-whole-source-paint-plans)
  separately bounds the production font/UTF-16 bridge. Drawable kind-4/emoji emission remains unsupported;
  default-empty controls do not supply a legitimate first glyph word. Actual
  [cell emission](reverse-engineering/table-code-findings.md#captured-cell-text-emission)
  adds nondrawable newline flushing without empty output; drawable kind-4/emoji
  remains unsupported.
  [Actual source-input capture](reverse-engineering/table-code-findings.md#captured-cell-source-inputs)
  additionally records Model getters after producer outputs because paragraph-list
  observation mutates owned ranges. The bounded adapter compares eight LTR
  profiles, including nonzero mark offsets; five
  RTL/`.notdef`/mixed profiles remain outside it. Stacked marks at 17/50 retain
  native glyph IDs/source/origins in PDF while SVG reports unsupported
  positioning. Table world translation retains the plan only when all retained
  line fields exactly match local-plus-origin f64 addition after native f32
  translation; nonrepresentable transforms use compatibility vectors.
  [Nonzero owner-base capture](reverse-engineering/text-draw-identity-findings.md#captured-nonzero-owner-bases)
  verifies native request-relative addition and separate source-vector lookup
  for supplied owners; Minikin/HarfBuzz and chunk normalization remain excluded.
- SVG transport does not reproduce every complex joined script or cluster
  crossing a style boundary.
- [Single-face shaping captures](reverse-engineering/text-layout-findings.md#captured-native-shaping)
  execute native HarfBuzz and Skia/FreeType with supplied Roboto and caller
  paint inputs. [Typed post-shaping geometry](reverse-engineering/text-layout-findings.md#captured-post-shaping-numeric-geometry)
  matches full/owner positions, ink and advances from supplied native shaping
  output and bounds, including [mixed-script chunk arithmetic](reverse-engineering/text-layout-findings.md#captured-mixed-script-chunk-geometry).
  Incompatible chunks and unsupported metric inputs remain explicit errors.
  A [bounded Rust paint-metric provider](reverse-engineering/text-layout-findings.md#bounded-rust-paint-metrics)
  independently matches 401 raw advances and 1604 ink coordinates across
  74 supplied-Roboto cases. It supports horizontal scale one, finite skew and
  static glyf fonts; skewed composite glyphs report an explicit error.
  [Skia metric captures](reverse-engineering/text-layout-findings.md#captured-skia-residual-matrices-and-outline-metrics)
  separately retain native matrices, cached fixed advances and raw outline
  points. The [paint-sized shaping API](reverse-engineering/text-layout-findings.md#bounded-rust-paint-shaping)
  uses Rust-derived metrics and the native GPOS floor policy, preserving typed
  UTF-16 ownership and signed integer positions. Producer regressions match
  all 1453 glyphs across 558 captured shape calls in 151 cases over eight
  fixtures. Public `PaintShapedRun::layout` / `PaintLayout::from_runs` also
  match all 1453 glyphs through full/owner positions, shifted ink and advances.
  Chunk stitching requires the same source, font, paint, scale and direction
  with adjacent source ranges. The [native logical-entry capture](reverse-engineering/text-layout-findings.md#captured-logical-entry-conversion-and-paint-profiles)
  separately executes layout append, entry conversion and actual paint-profile
  setters. Public `PaintLayout::entry_geometry` matches all eight entry cases
  and 25 glyphs, including owner-position division, width division and native
  ink translation/scale/union. [Fractional hinting controls](reverse-engineering/text-layout-findings.md#captured-fractional-paint-hinting)
  separately pin the native Mono hint target and 192 fixed hinted extrema;
  whole SpanRunFunctor and device font-manager resolution remain outside scope.
  [Script itemization](reverse-engineering/text-layout-findings.md#captured-script-itemization)
  matches native plain-Script chunks and full-source context for 44 cases,
  including Common/Inherited absorption. `PaintShaper::shape_text` creates
  exact requests, native feature recipes, shared layout and entry geometry
  through one immutable measured piece across all eight suites.
  The producer accepts Latin, Greek, Cyrillic and Common chunks; unsafe
  positioning, contextual/chained/cursive GPOS and legacy kern/kerx/trak fonts
  report typed unavailability before positioning.
  [Captured GPOS and fused-skew traces](reverse-engineering/text-layout-findings.md#captured-horizontal-gpos-scaling-and-fused-skew)
  verify four horizontal pair-value updates and 12 local skew operations.
  The [complete span helper](reverse-engineering/text-layout-findings.md#captured-complete-span-paint-helper)
  captures 93 paint profiles, including final Typeface weight/italic overriding
  the initial 400/false. `PaintSpanProfile` reproduces scalar source-style
  fields and rejects fake-bold metric inputs; device font resolution and
  physical style-face synthesis remain outside its contract.
  A separate [NAME/default capture](reverse-engineering/text-layout-findings.md#captured-span-font-name-and-default-selection)
  observes 130 native profiles with actual XML parsing and four physical
  Roboto styles. Typed `NativeFontNameRequest`/`FontBook::resolve_native_name`
  matches those profiles, including null/empty names and physical file/style
  identity. Caller configurations define database aliases and matching policy;
  generic Minikin/device font selection remains unverified.
  [Named-face measurement](reverse-engineering/text-layout-findings.md#captured-named-face-measurement)
  captures 110 profiles through actual face/paint selection, shaping, metrics
  and entry conversion. Rust matches 106 whole pieces, 122 calls and 401 glyphs;
  four skewed composite
  profiles remain typed rejections. This four-face API proof does not widen
  production paragraph admission.
  [Consumer metrics](reverse-engineering/text-layout-findings.md#captured-consumer-text-metrics)
  add 52 Regular/normal/LTR profiles and 134 exact glyph/entry records for
  supplied marker/body strings across ten sizes and two `AB` controls. Native
  list numbering and object-width feedback algorithms remain outside the capture.
  The [production paragraph adapter](reverse-engineering/text-layout-findings.md#production-paragraph-measurement-boundary)
  uses paragraph-local [native cache-word views](reverse-engineering/text-layout-findings.md#captured-cache-word-context-windows)
  and admits the pinned Regular face at index zero. It retains native logical
  entries/global owners and shared glyph geometry for PDF, SVG, viewport ink
  and markers. Unsupported font/style/script/tab/budget inputs report typed
  diagnostics and use compatibility geometry. That compatibility producer
  still shapes font units and projects them in f64; independent Chromium
  glyph reproduction, SDK line/grapheme policies and device fallback remain
  separate boundaries.
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
- The whole-source paint certificate admits opaque foreground only; other alpha
  values return typed unavailability at its PDF bridge. Generic retained PDF
  transport supports byte alpha in Fill/Stroke opacity, independently of native
  caller policy. Compatibility span painting retains RGB while measurement
  preserves source alpha.
  The [60-case native PDF alpha capture](reverse-engineering/text-draw-identity-findings.md#captured-pdf-alpha-transport)
  establishes distinct legacy Table, Code and background setter behavior;
  the Rust renderer does not reproduce those route-specific alpha rules.
  Public Standard export also batches ordinary Table/Code page objects, so
  standalone writer evidence does not establish their public-route appearance.
- Native font selection, complete heading editing/runtime lifecycle, opaque native style
  bits, variable-font instances and device-specific fallback selection are not
  established by ordinary measurement-identity coverage.
  [Native XML language capture](reverse-engineering/text-draw-identity-findings.md#captured-font-family-language)
  preserves raw family language, including missing-as-empty, before font loading.
  It does not recover device font configuration or turn shaping script into
  native language metadata; source IDs identify created typeface instances,
  rather than hashes of font-file bytes.
  [Native file-font construction](reverse-engineering/text-draw-identity-findings.md#captured-file-font-source-instances)
  confirms distinct same-file source instances and reference-copy identity with
  pinned test font bytes. The separate [live registry capture](reverse-engineering/text-draw-identity-findings.md#captured-live-font-registry)
  records reuse and native language getters for four supplied physical
  registrations; generic manager reuse and device selection remain unverified.
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

SVG embeds the selected TTC/OTC face used for measurement under its physical
alias; logical source and diagnostic names remain unchanged. CLI exports report
text and object diagnostics for the selected pages. Ordered diagnostics use
indexed deduplication and retain source attribution; font validation includes
metrics contributing to inline-object leading.
Native-support warnings follow painted source visibility. Compatibility runs
retain style/font/shaping/budget/tab reasons; unmeasured fallback lines retain
local source intersections, and wrapping-policy reasons belong to affected
lines. Export excludes all six aggregate native-support warning kinds from
preparation replay while complete producer diagnostics remain available.
Prepared table/code warnings therefore follow visible child source rather
than only the outer object scope.

Document exports and browser sessions reuse compatible body plans across
pages. Cache identity includes the immutable native NAME configuration as well
as database/source/layout identity; changing that configuration invalidates
plans even when the database is shared. Style boundary resolution avoids repeated full-span scans, and fallback
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
- [Physical-face SVG/PDF](../crates/sdocx/tests/svg_face_identity.rs) compares
  selected Regular/Bold outlines under synthetic styling; [missing-alias controls](../crates/sdocx/src/pdf/font_identity_tests.rs)
  distinguish raw SVG failure from retained face preservation.
  [Standalone Chromium identity](../web/tests/e2e/svg-face-identity.spec.ts)
  observes the actual Regular/Bold PostScript faces under CSS weight 700;
  this does not establish inline SVG in the application DOM.
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
