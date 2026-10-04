# Rendering support

This document maps the Rust SDK's saved-document rendering surface. It separates
structural inspection, semantic decoding, drawing, and evidence of Samsung
behavior. It is not a claim of complete Samsung Notes compatibility.

“Implemented” means a Rust route exists for the stated scope. “Approximate” means
the route preserves content with geometry or appearance that does not reproduce
the native feature. Native evidence may establish a serialized field, an isolated
arithmetic kernel, a bounded producer, or a real-document comparison; these are
different guarantees. Successful parsing and an empty diagnostic report do not
certify visual parity. Chromium preview and vector SVG/PDF are the supported
rendering target; Firefox fountain-mask appearance remains a known limitation.

## Object types and locations

The [object identifiers](../crates/sdocx/src/types.rs) and
[physical page decoder](../crates/sdocx/src/page.rs) define the first boundary.
Every stored object retains its declared type, payload location and child
structure through [storage](../crates/sdocx/src/storage.rs). Borrowing its original
payload requires the original uncompressed page bytes. Retention does not imply
that the object has semantic content or a renderer.

| Native ID | Object | Semantic decoding or inspection | Rendering |
| --- | --- | --- | --- |
| 0 | None | Stored/common metadata | No dedicated renderer |
| 1 | Stroke | Sample channels, properties and pen resources | Page vectors; supported reconstructions or generic approximation |
| 2 | TextBox | Rich text, styles, paragraphs and object spans | Page text and supported embedded content |
| 3 | Image | Geometry, media references and supported crop fields | Page and text-flow images; effects are partial |
| 4 | Container | Common metadata and ordered children | Nested page objects; supported parent visibility and root selection |
| 7 | Shape | Geometry, styles, saved paths, controls and embedded text | Supported page templates/paths and solid paint; partial styling |
| 8 | Line | Geometry, styles, controls and saved paths | Straight lines and supported paths; partial styling/routing |
| 9 | DeprecatedDummyStroke | Stored/common metadata | No dedicated renderer |
| 10 | Voice | Stored/common metadata; [own attachment/source contract](reverse-engineering/voice-source-findings.md) is distinct from typed note VoiceData | No voice-object renderer |
| 11 | Formula | Bounded expressions, result/image rectangles, media ID, strokes and label graphs | Inspection only; no automatic formula drawing |
| 12 | DeprecatedTable | Stored/common metadata | No dedicated renderer |
| 13 | Web | Stored/common metadata | No dedicated renderer |
| 14 | Painting | Stored/common metadata | No dedicated renderer |
| 15 | StrokeDevelopmentVersion | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 16 | Video | Stored/common metadata | No dedicated renderer |
| 17 | Link | Stored/common metadata; separate rich-text hyperlink spans exist | No link-object renderer |
| 18 | StrokeBrush | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 19 | Unknown | Explicit Samsung unknown marker | No dedicated renderer |
| 20 | Plot | Bounded plot/graph expressions, colors, styles and visibility | Inspection only; no graph evaluation or plotting |
| 21 | Math | Bounded formula envelopes, margins and connected plot UUIDs | Inspection only; no automatic child-formula drawing |
| 22 | Table | Typed table when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 23 | CodeBlock | Typed title/body when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 24 | AttachedFile | Stored/common metadata, including common attachment reference | No attachment-card renderer |
| 100 | StrokeGroup | Stored/common metadata and children | No group renderer; supported children can be traversed separately |
| Other IDs | Future/unrecognized objects | Stored structure and unknown-type diagnostics | No own renderer; supported children can be traversed separately |

Rich-text routing is separate: [initial table/code decoding](../crates/sdocx/src/note.rs),
[image resolution](../crates/sdocx/src/image.rs), and
[embedded rendering](../crates/sdocx/src/render.rs) support only image, table and
code content. A table inside a text box therefore differs from a physical
type-22 page record. Flattened supported children under an unsupported parent
do not establish that parent's transform or composition semantics.

Native [card contracts](reverse-engineering/card-source-findings.md) distinguish
saved Web/Link/AttachedFile text, action metadata and source bindings from
regenerated card artwork. A thumbnail does not retain the contents of a bound
Web or attached document.

The [formula](reverse-engineering/formula-findings.md),
[math](reverse-engineering/math-findings.md), and
[plot](reverse-engineering/plot-findings.md) findings describe inspection APIs.
Native [formula drawing](reverse-engineering/formula-rendering-findings.md)
has separate image/stroke selection rules; decoded expressions do not require
or establish an SDK evaluator.

## Ink

Sample coordinates, pressure, timestamps and optional tilt/orientation are
decoded and retained. A retained channel is not necessarily used by a pen's
drawing route. The [pen registry](../crates/sdocx/src/ink.rs) identifies names and
libraries, not a renderer for every registered pen.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Fountain V14, saved `14;` | Reconstructed stamps/directions; vector directional shading, mask and maximum blending; tools 1–3 and fixed width | Executed native geometry fixtures and real-document comparisons. Native partial-alpha appearance still differs; Firefox masks fail. |
| Fountain V16, saved `18;0;100;` | Reconstructed circular stamps with pressure/tilt/time/movement and fixed width; tools 1–3 | Executed native geometry fixtures; native coverage and zoom compensation are not reproduced. This setting selects drawing V16, not V18; V14 directional shading is not applied. |
| Marker2 | Sampled circular stamps, constant radius and saved alpha | Static native tracing and synthetic arithmetic controls; V2 thin-edge shading and complete device appearance remain unverified. |
| Marker4 V7, saved `7;` | Stylus midpoint sampling and fractional rounded rectangular tip | Executed native geometry fixtures and isolated real-document appearance evidence; public support remains approximate because filtering/zoom coverage differ. |
| Marker4 V8, saved `8;` | Sampled rounded rectangular tip, tool-specific rotation and saved alpha | Static native tracing and one paired Samsung PDF comparison; filtering/zoom coverage remain approximate. |
| Other pens, unknown names or rejected settings | Generic straight segments with pressure width, polyline or tap circle | No pen-specific native texture, smoothing or width-law guarantee. |
| Rainbow, eraser and straighten effects | Metadata retained; native effect semantics not reproduced | Gates vary by profile. V8 accepts straighten/fixed-width bits without implementing their separate effect semantics. |
| Live prediction and temporary tips | Saved samples and saved-vector replay only | Native live-state research does not establish a production live renderer. |

Profile admission and validation are narrower than the names above. See
[stroke rendering](reverse-engineering/stroke-rendering-findings.md),
[fountain parity](reverse-engineering/fountain-parity.md),
[Marker2](reverse-engineering/marker2-rendering-findings.md),
[Marker4 V7](reverse-engineering/marker4-v7.md), and
[Marker4 V8](reverse-engineering/marker4-rendering-findings.md).

The [Pencil3 V1 source trace](reverse-engineering/pencil3-source-findings.md)
separates saved samples from generated particles and configured paper coverage.
Pencil profiles currently use the generic pressure approximation.
The [InkPen V4 source trace](reverse-engineering/inkpen-v4-findings.md) establishes
selected saved curve width/sampling and cache boundaries; ordinary InkPen also
uses the generic approximation, without a shipped native width-law guarantee.

## Text and embedded layout

The detailed [text support table](text-vector-support.md) owns the admission
rules and native capture boundaries. These rows summarize it rather than
extending its guarantees.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Body, standalone and supported shape text | Shared measurement/layout; density, margins, gravity, wrapping and paragraph spacing | Selected native origins/wrap kernels verified; all standalone modes and complete native layout are not established. |
| Text fit and overflow | Saved text retained; no typed auto-fit/ellipsis projection | Present auto-fit/ellipsis fields produce extension diagnostics. Native dots/cached cutoff and separate triangle cues are derived output; full fitting and Free/Path semantics remain unverified. See [saved options](reverse-engineering/text-box-findings.md#persisted-auto-fit-and-ellipsis). |
| Fonts and Unicode | Pinned/caller fonts, coverage fallback, bidi and embedded physical-face identities | Native production measurement has bounded pinned-Regular admission; device fallback order and variable-font instances are unsupported or unverified. |
| Complex shaping and emoji | Source-preserving SVG fallback; retained PDF glyph geometry where available | SVG cannot reproduce every retained cluster; drawable kind-4 states and native emoji resource production remain unsupported. See [resource ownership](reverse-engineering/text-draw-identity-findings.md#line-feeds-and-emoji-resources). |
| Spans, headings, lists and links | Supported styles, underline/strike/background bands, vector list markers and gated SVG hyperlinks | Intra-cluster background changes and unsafe object positions are conservative; PDF hyperlink annotations are absent. |
| Composition/suggestion/correction styles | Supported modern composition appearance; ordinary text preserved elsewhere | Legacy/incomplete composition and special suggestion/correction appearance produce diagnostics. |
| Tables | Dense unmerged/merged preparation, vector fills/borders/artwork and measured content | Sparse/invalid grids use diagnosed fallback; rotated/nested preparation and general split-page composition remain incomplete or unverified. |
| Table text clipping | Conservative table-wide scope; native per-run clipping in certified ordinary one-page contexts | Certification is bounded by source, font metrics, geometry and transforms; arbitrary cell-loop/per-run parity is not established. |
| Code blocks | Shared title/body layout, panels, chrome/minimum geometry and continuation handling | Captured supplied-metric geometry is verified separately from complete native split-page composition. |
| Pagination | Saved sections, full-source fallback reflow and bounded native page-range routing | General native repagination, obstacle production and page-bound construction remain outside the verified route. |

## Graphics, paper and composition

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Shapes and lines | Basic templates, supported saved move/line/quadratic/cubic/close paths, rotation, solid fills/outlines and embedded text | Specialized templates, arc/oval commands, connector routing, pen simulation, gradients, image fills, dash/compound styles and arrowheads remain incomplete. See [shape/line findings](reverse-engineering/shape-line-findings.md). |
| Images | Media resolution, placement, rotation and supported original-placement rectangular cropping | Pixel crops without original placement, border/original references, fill transforms, tiling and active nine-patch remain incomplete. Nonrectangular inherited image paths produce diagnostics; the traced native effect route uses a rectangle. See [image findings](reverse-engineering/image-findings.md) and [image-filled vector shapes](reverse-engineering/image-effects-findings.md#ordinary-image-effect-paths-differ-from-image-filled-vector-shapes). |
| Page paper | Solid color and supported built-in ruled/dotted templates | Image/URI-backed and rotated paper are rejected by [template admission](../crates/sdocx/src/page_background.rs); other templates are retained without drawing. |
| Page custom objects | No typed custom-list decoding or rendering | Separate from layer objects and their diagnostics. Sticky notes can bind nested `.sdocx` source; original page/archive bytes remain required. See [custom source ownership](reverse-engineering/vector-retention-findings.md#page-custom-objects-and-attached-source). |
| Physical layers and visibility | Saved current layer; hidden recognized objects/subtrees omitted | Inactive layers remain structurally available; no captured multilayer appearance comparison. See [layer selection](reverse-engineering/page-layer-selection-findings.md). |
| Object order and containers | Stored order within Base/Top/Masking root passes; selected containers recurse in place | Child render IDs do not reselect root passes. Saved child rotation is not applied again through a parent transform. Full mixed-container pixel parity remains unverified. |
| Root intersection filtering | No native geometric selection filter | Native collector behavior is traced, but comparing stored bounding boxes alone does not implement it. See [selection findings](reverse-engineering/object-selection-findings.md). |
| Light/dark treatment | Shared modes, native lightness reversal, local contrast and alpha-preserving paint | Color conversion is independently verified; paper aliases/contrast selection and some palette decisions remain export policy. See [themes](render-themes.md). |
| Highlighter blending | Whole Top batch uses Darken on light paper and Lighten on dark paper | Capture-route selection/shader math has static evidence; native Standard list PDF uses Darken. SDK shares its SVG policy with PDF; native pixel parity is not claimed. |
| Tape visibility/reveal | Saved note preference and per-stroke reveal flag retained | Generic pen approximation does not implement native presentation controls or destination attenuation. See [Tape findings](reverse-engineering/stroke-metadata-findings.md#tape-visibility-and-reveal-controls). |

The [SVG composition description](svg-rendering.md) and
[native capture findings](reverse-engineering/capture-composition-findings.md)
give the ordering and export-route boundaries. “Masking” is a pass name; it
does not mean the SDK implements eraser subtraction. Layer alpha-lock/shadow
metadata and runtime object draw-state alpha are not a proven general saved
group-opacity contract.

## Output and verification

| Output | Implemented contract | Limits |
| --- | --- | --- |
| SVG preview/export | Typed Rust elements, validated numeric/path values, escaped content, vector ink and embedded fonts | Supported geometry and text transport do not imply complete native appearance. |
| Replay | Reveals the same generated vector geometry at saved sample boundaries | No pen geometry in the UI; no native live prediction or all-layer replay-state guarantee. |
| Document PDF | Shared scene plus retained selected fonts/glyphs/XY, vector masks/shadings/clips and selectable logical source | Inherits rendering gaps; source images remain images; universal PDF accessibility is not established. |
| Arbitrary SVG to PDF | Compatibility conversion through the SVG importer | Can reshape text and rasterize imported filter effects; lacks the document exporter's private retained-glyph guarantee. |

The [conformance guide](../conformance/README.md) records fixture identities,
independent native capture boundaries and real-document reference coverage.
The locked corpus contains four document/PDF pairs, not an exhaustive feature
inventory. Synthetic archives establish SDK behavior and malformed-input
handling; they cannot establish Samsung visual equivalence for absent features.
The browser displays parser diagnostics; its preview/PDF bindings discard the
separate text/object render diagnostics returned by Rust. See the
[diagnostic transport boundary](reverse-engineering/vector-retention-findings.md#diagnostic-interpretation).
The [real-document inventory](reverse-engineering/rendering-corpus-findings.md)
records feature occurrence separately from native research. Its only parse
warnings are PDF-backed paper in two local research notes. Rust currently
retains only the first PDF page index, discarding the resource bindings,
placement rectangles and additional entries; see
[page background findings](reverse-engineering/page-background-findings.md).
This is a retention gap as well as a rendering gap.
The [PDF preservation constraints](pdf-paper-preservation.md) connect the
recovered contracts to the current Rust model and verified vector PDF transport;
the library proof does not implement SDK PDF-paper support.

The native [shape style contracts](reverse-engineering/shape-style-findings.md)
and [image effect contracts](reverse-engineering/image-effects-findings.md)
have static evidence for features absent from these documents; they do not add
real-document appearance coverage. See also
[PDF findings](reverse-engineering/pdf-export-findings.md).

The [vector retention findings](reverse-engineering/vector-retention-findings.md)
separate source-byte ownership, semantic decoding and drawable output. Physical
payload offsets do not own their bytes, and vector output can still quantize or
omit source geometry. Recovered
[path](reverse-engineering/shape-path-findings.md),
[fill](reverse-engineering/shape-fill-findings.md) and
[connector](reverse-engineering/connector-routing-findings.md) contracts identify
additional typed geometry and resource boundaries without adding SDK support.
[Container edits](reverse-engineering/object-transform-findings.md) and
[eraser cuts](reverse-engineering/eraser-preservation-findings.md) can modify saved
child or stroke geometry before drawing. The
[brush and painting findings](reverse-engineering/brush-record-findings.md)
distinguish compatibility dispatch and editable resources from previews.
The [painting source contract](reverse-engineering/painting-source-findings.md)
identifies a separate archive with ordinary stroke packets, replay channels and
bitmap state. The shared native reducer uses different coordinate widths from
WDoc; neither a thumbnail nor decoding WDoc frames preserves that source.
