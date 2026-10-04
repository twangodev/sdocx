# PDF paper preservation constraints

Preserving PDF-backed paper includes its original vector paths, text operators,
font resources and placement. A source PDF can also contain images; keeping
those original resources does not introduce new rasterization. The available
`quiz` paper is already image-only, while `cs61bl_su22` contains text, fonts,
vector content and some images. These source differences are recorded in the
[real-document inventory](reverse-engineering/rendering-corpus-findings.md).

## Recovered contracts and codebase consequences

| Recovered behavior | Consequence for the Rust codebase |
| --- | --- |
| A continuous note can contain twenty ordered source-PDF placements on one physical page. | A first-page index is an incomplete background model. Physical note pages, source PDF pages and output pages have separate identities. |
| `HasPDF(int)` searches media bindings; each record also stores a separate signed source-page index. | Resource IDs and source-page indices cannot share an interpretation. Raw values and resolved asset positions are different domains. |
| The media manifest supplies authoritative bindings; filenames, content hashes and runtime paths have separate roles. | PDF resources have the same binding boundary as images. Reconstructing identity from a filename prefix loses that contract. |
| Saved presence, an empty list, template type, file availability and successful PDF opening are separate states. | Retention, resource resolution and rendering admission need separate results. PDF presence does not establish a usable page. |
| Destination rectangles are note-space geometry; source size comes from inherited PDF boxes and rotation. | Stored placement must survive independently of PDF interpretation. Source box handling belongs to the PDF library. |
| Preview, list export and continuous export have different selection and scaling paths. | Preview bitmap behavior does not define vector-export geometry or output-page topology. |
| Native Standard export imports source PDF pages before painting note overlays. | Source content can remain in its PDF resource graph instead of being reshaped through the note's SVG text pipeline. |

The detailed evidence is in the
[storage](reverse-engineering/pdf-paper-storage-findings.md),
[resource](reverse-engineering/pdf-paper-resource-findings.md),
[placement](reverse-engineering/pdf-paper-placement-findings.md), and
[export](reverse-engineering/pdf-paper-export-findings.md) findings. Static
traces, bounded native execution and source-document observations are identified
separately there. None establishes complete Samsung export appearance parity.

## Current Rust boundaries

The [page decoder](../crates/sdocx/src/page.rs) retains all ordered signed records
and version-dependent rectangles in `PageBackground.pdf_paper`, independently of
raw `template_type`. `PageTemplateSource::CustomPdf` remains a first-record
compatibility summary. Document metadata retains modern manifest bindings and
uncompressed `media/` source bytes, including opaque PDFs. Source resolution does
not open a PDF or certify its indices. Structural offsets require matching original
page bytes, retained by opt-in detailed parsing or supplied separately.

`PageBackground` participates in
[trailing compatibility-page equality](../crates/sdocx/src/layout.rs). Complete
saved paper identity therefore affects visible-page selection, not just paint.
Its existing equality is exact; older floating rectangle encodings also have
bit-level identity and invalid-value concerns. The native loader's filtering of
bad bindings is distinct from preserving original saved records for inspection.

Layout pages clone the page model, while document resources are separate.
Resource references carry placement identity without copying the PDF bytes into
every page. Replay's background request uses the same Rust page renderer after
removing strokes; there is no separate JavaScript placement implementation.
The body-text preparation cache is not a paper-rendering cache. A source asset
index alone does not identify immutable bytes when public document assets can
be replaced.

Current PDF export consumes an SVG carrier plus retained Rust text operations.
The SVG begins with an opaque paper rectangle. Composition order for imported
paper is consequently **canvas → imported paper → note content**; drawing an
imported page behind that opaque rectangle would hide it. Imported PDF font
resources are independent of the `FontBook` used to measure note text.
Retaining metadata alone does not establish drawing support: the existing
`UnsupportedPageTemplate` diagnostic still describes omitted PDF paper.

## Existing vector PDF transport

The pinned Krilla 0.8.2 dependency has an optional `pdf` feature exposing
`Surface::draw_pdf_page`. It imports a source page as a Form XObject, retaining
its PDF content/resources and using `f32` placement. The repository currently
enables only `raster-images`, so this import API is not active in the SDK.
See the [versioned surface API](https://docs.rs/krilla/0.8.2/krilla/surface/struct.Surface.html)
and [Hayro Write 0.7.0](https://docs.rs/hayro-write/0.7.0/hayro_write/).

A temporary Rust proof enabled that feature, imported three real source pages,
and drew a new vector triangle after each background. Independent `lopdf 0.42.0`
checks observed:

| Source | Zero-based page | Preserved content operators | Preserved embedded font programs | Original image streams |
| --- | ---: | ---: | ---: | ---: |
| `cs61bl_su22` | 0 | 287 | 5 | 0 |
| `cs61bl_su22` | 2 | 123 | 6 | 2 |
| `quiz` | 0 | 4 | 0 | 1 |

Decoded imported Form content matched the original page content except outer
whitespace. Decoded font-program hash sets, subset font names and encoded image
hashes matched. The output had exactly the three unique original image streams
and no new images; note path operators followed the background draw operator.
The 499,401-byte output SHA-256 was
`574d79543aa74b761be94bb6f7ad5c1ebf10097fa8993ecd54d3efeebcb404fa`.
The proof pinned Krilla `=0.8.2` and resolved Hayro Write 0.7.0 and
Hayro Syntax 0.7.2. Rust 1.92 compilation passed; a separate transport-only
crate compiled for `wasm32-unknown-unknown`.

The proof is a temporary research client, not an SDK feature or repository
command. It used explicit `(20,20)` placement and `612×792` dimensions, not
recovered Samsung placement. Browser execution, main-project integration,
annotation/tag preservation, arbitrary PDF compatibility and conforming
PDF/A/PDF/UA imports remain outside that proof. Content preservation is not a
byte-identical copy of the entire source file.

The [PDF vector transport findings](reverse-engineering/pdf-vector-transport-findings.md)
extend this with adversarial source properties. Tested shading, patterns and
vector soft masks survive, but Form import changes page transparency-group
semantics and omits annotation appearances and optional-content configuration.
Resource numbers are rewritten, and successful serialization can omit
undecodable streams. Original resource retention and derived export capability
therefore remain separate contracts; matching content operators alone does not
certify complete transport.

## SVG is a separate conversion boundary

The installed usvg 0.47 has no PDF image kind; putting PDF bytes into the current
SVG image route cannot carry this paper. Direct PDF embedding produces no SVG.
The released Hayro SVG 0.7.0 emits text outlines, drops invisible glyphs such as
hidden OCR text, and samples shading patterns into images. Its current behavior
therefore does not satisfy a strict
vector-preservation contract for general PDF paper. These are release-specific
findings from [glyph transport](https://github.com/LaurenzV/hayro/blob/0c98904d2ced8e37b63ed2bd1e60f95bc0c7c774/hayro-svg/src/glyph.rs)
and [shading transport](https://github.com/LaurenzV/hayro/blob/0c98904d2ced8e37b63ed2bd1e60f95bc0c7c774/hayro-svg/src/paint.rs).

PDF import and SVG conversion can consume the same Rust resource/placement
contract while having different output capabilities. Passing PDF export through
that SVG conversion would sacrifice the demonstrated preservation of original
text and shading resources. Complete SVG transport remains unresolved; a
successful generated file is insufficient evidence that vectors survived.
