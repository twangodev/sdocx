# PDF vector transport findings

These findings concern the optional PDF import path in **Krilla 0.8.2**, with
**Hayro Write 0.7.0**, **Hayro Syntax 0.7.2** and **pdf-writer 0.15.0**. They extend
[the real-paper preservation proof](../pdf-paper-preservation.md). The SDK does
not currently enable Krilla's `pdf` feature. These are temporary Rust transport
probes and inspection of the pinned library sources, not Samsung export parity
or an implemented SDK feature.

## Evidence and inspection boundary

A `lopdf 0.42.0` client constructed a nine-page PDF using typed dictionaries,
streams and content operations. Every page used the same two content streams
and shared resource objects. Krilla imported all nine pages as Forms with a
`(20,30)` translation and explicit `240×270` size; a tenth output page imported
source page zero again. A separate output used `Document::embed_pdf_pages` for
all nine pages.

Independent inspection resolved resource references recursively, compared the
resulting dictionaries and stream payloads, decoded both source and Form
content with lopdf, and enumerated every output Image object. Object numbers
and stream `Length` were excluded from resource equivalence. All source
resource categories had to exist in the imported Form; absent categories were
errors. An independent reviewer rebuilt and reran the temporary Rust client and
reproduced the outputs.

| Artifact | SHA-256 |
| --- | --- |
| Nine-page source PDF | `67410d2e4e8b8bd5f2a53bcb73cbe65cefea313a21a68825c4603511d841af58` |
| Output using page Forms | `2817f815bc1d6cab64fffec6ac08a2c35dff60645285f0b73584310ef8c4ae7f` |
| Output using direct pages | `2348bddf8e416dc286ef99039e692cc0d17b923e2408cc0fd072007b39a19bcd` |

No pixels were rendered or compared. Source-operator equivalence, resource
transport and changes to page/catalog semantics are distinct observations.

## Vector resource transport

Each source page contained 28 operators: clipped axial shading, a tiling-pattern
rectangle, a rectangle using a vector luminosity soft mask and Multiply blend
mode, visible text, invisible text (`Tr 3`), marked content and an optional
content section. Both input content streams were library-generated.

| Source feature | Observed import |
| --- | --- |
| Axial shading with a type-2 function, RGB endpoints and `Extend [true true]` | Shading/function dictionaries were equivalent after resolving references; the `sh` operator survived. |
| Colored tiling pattern with a `10×10` cell, `XStep 13` and `YStep 17` | Pattern dictionary and original vector stream payload were equivalent. |
| Luminosity soft mask whose group contains a vector rectangle | The ExtGState dictionary, nested mask Form and its vector payload were equivalent. |
| Multiply blend mode and stroke/fill opacity | Their ExtGState values survived. |
| Rectangle clipping with `W n` | The original clip operators survived. |
| Helvetica resource, visible text and invisible OCR-like text | The font dictionary and all text operators survived, including `Tr 3`. This synthetic font is unembedded; embedded-font retention is separately demonstrated by the real-paper proof. |
| Same source page imported twice | Both draws referenced the same cached page Form. |

Every Form retained the original 28-operator sequence, and the complete output
contained **zero Image objects**. Multiple source streams were joined with
inter-stream whitespace, so decoded content bytes were not identical even
though their operators were. The real-paper proof's single-stream byte result
must not be generalized to multi-stream pages.

The entire resource graph was **not** equivalent: the Properties category lost
selected keys, as described below. No claim is made about every PDF shading,
font, color space or mask subtype. The resource categories copied by the
extractor are visible in
[Hayro Write's `serialize_resources`](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs).

## Explicit resource scopes

Source inspection found a separate inheritance boundary. PDF 32000-1:2008
Table 30 and sections 7.7.3.4/7.8.3 distinguish an explicitly supplied Resources
dictionary, including an empty one, from an omitted entry that inherits the
attribute. This selects a resource dictionary; it does not merge ancestor names
into a page's explicit dictionary. See the
[Adobe specification](https://github.com/adobe/dc-acrobat-sdk-docs/blob/ab3b42a75df65543025736e9699e1e63af1922b0/docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).

Hayro Syntax instead retains the parent resource chain even when the page
supplies Resources. Hayro Write collects each category from ancestors first,
then replaces duplicate names with current entries; parent-only names survive
in both page and Form export. See [resource construction](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/page.rs)
and [resource collection](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs).

This can introduce ancestor-only DefaultGray, DefaultRGB or DefaultCMYK entries
into an explicit child scope. Section 8.6.5.6 uses those current ColorSpace
entries to remap selected device colors, so unchanged operators can acquire a
different color-space binding. This is a source-observed route and potential
semantic consequence, without an executed fixture, appearance comparison or
observed defect in the available Samsung paper corpus.

## All available original paper pages

A further Rust probe imported all twenty `cs61bl_su22` source pages and the one
`quiz` source page. The 21-page output had 940,671 bytes and SHA-256
`907a19151bb26c8d6b689dec564c22b706b75026bdafd93f82d4d09f4e3bcb66`.
Source identities are pinned in the
[real-document inventory](rendering-corpus-findings.md).

All **3,726 operators** matched, as did decoded content bytes apart from outer
whitespace. The output retained exactly the **27 unique original encoded image
streams**, with none added. Recursive comparisons retained the full font
graphs, including metrics, encodings/Differences, ToUnicode streams and embedded
programs, together with color-space and image/soft-mask dependencies.
Differences were restricted to removed ProcSet arrays and empty
ExtGState/Pattern categories. A separate Hayro Syntax comparison found all
**11,868 visited resource numeric values** bit-identical as `f64`; these are
visits through page graphs, not unique document-wide objects.

For a logical-text check, each imported Form's existing content and Resources
were projected into a temporary page and read with lopdf's text extractor.
Every result matched its source: **22,510 UTF-8 bytes** across the twenty text
pages and zero bytes for quiz. This checks the available source mappings,
not browser text selection. Direct extraction from an unprojected output page
is different because lopdf's page extractor does not traverse Form draws.

None of these source pages declared Annots, UserUnit, Rotate or CropBox, and
neither source catalog declared OCProperties or StructTreeRoot. Ten cs61bl
pages already had the same isolated RGB Group emitted by the importer; the
others had no Group. The real resources contained no nonempty shading, pattern
or ExtGState graph. Those features therefore have synthetic evidence rather
than real-paper coverage. cs61bl's catalog Names/OpenAction/PageMode are outside
the imported resource boundary.

## Source geometry and placement

The ordinary source page had MediaBox and CropBox `[0,0,200,200]`. The probe
changed one page attribute at a time, except the inherited case, whose page
omitted local boxes, rotation and resources.

| Case | Interpreted dimensions | Imported Form BBox | Imported Form Matrix |
| --- | --- | --- | --- |
| Ordinary | `200×200` | `[0,0,200,200]` | `[1,0,0,1,0,0]` |
| CropBox `[10,20,160,170]` | `150×150` | `[10,20,160,170]` | `[1,0,0,1,-10,-20]` |
| Rotate 90 | `200×200` | `[0,0,200,200]` | `[0,-1,1,0,0,200]` |
| Rotate 180 | `200×200` | `[0,0,200,200]` | `[-1,0,0,-1,200,200]` |
| Rotate 270 | `200×200` | `[0,0,200,200]` | `[0,1,-1,0,200,0]` |
| UserUnit 2 | `200×200` | `[0,0,200,200]` | `[1,0,0,1,0,0]` |
| CropBox `[-10,-20,250,240]` outside MediaBox | `200×200` | `[-10,-20,250,240]` | `[1,0,0,1,0,0]` |
| Inherited MediaBox `[15,25,215,225]`, CropBox `[30,50,180,200]`, Rotate 90 | `150×150` | `[30,50,180,200]` | `[0,-1,1,0,-50,180]` |

Hayro Syntax computes dimensions and initial transforms from the intersection
of MediaBox and CropBox. Hayro Write uses the original CropBox for the Form
BBox. Consequently the outside-crop case retains a larger clip boundary than
the dimension calculation uses. Krilla's draw path does not add an explicit
MediaBox-intersection clip. This establishes a structural clipping difference;
the probe does not establish its rendered appearance.

UserUnit was absent from the imported Form and from direct-page output. Hayro
Syntax's dimension and transform calculations did not apply it. This loses
physical page scale for direct-page import. A uniform UserUnit factor can
cancel when scaling a page to an explicit destination size, so omission alone
does not establish a wrong appearance for every Form placement. Source size in
points and fit-to-destination geometry are separate questions.

The relevant pinned code is
[Hayro Syntax's page geometry](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/page.rs),
[Hayro Write's `write_xobject` and `write_page`](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs),
and [Krilla's `draw_pdf_page`](https://docs.rs/krilla/0.8.2/krilla/surface/struct.Surface.html#method.draw_pdf_page).

## Page and catalog semantics that are lost

One source page declared a transparency Group with `CS /DeviceCMYK`, `I false`
and `K true`. Form import replaced it with an isolated RGB transparency Group:
`CS /DeviceRGB`, `I true`, no `K`. Direct-page import retained the original Group.
This difference is produced by Hayro Write's unconditional Form-group creation
and Krilla's supplied group-color-space callback. Source drawing operators can
therefore remain identical while their enclosing transparency semantics change.
See [Hayro Write](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs) and
[Krilla's PDF serializer](https://docs.rs/crate/krilla/0.8.2/source/src/pdf.rs).

Every synthetic source page also had a visible Stamp annotation with a vector
appearance stream. Neither output had page annotations or that appearance
stream. The annotation was omitted rather than flattened into the imported
page content. This can discard vector data located outside the page's Contents
and Resources graph.

Both outputs omitted the supplied `StructTreeRoot` catalog entry and page
`StructParents`. Marked-content operators, `MCID` and `ActualText` remained in
content/resources. The synthetic structure-root entry was minimal, without a
ParentTree or a complete semantic hierarchy; this is a key-retention probe,
not a certified valid tagged-document test. The serializer does not transport
the source accessibility graph merely by retaining its text operators.

The source catalog's optional-content configuration put an OCG in its `OFF`
list. Form output retained the OCG dictionary through the Properties resource
but omitted catalog `OCProperties`. A separate Properties dictionary lost its
`OC`, `Metadata` and `AF` keys while retaining `MCID` and `ActualText`. Preserving
optional-content operators and an OCG dictionary therefore does not preserve
the original visibility configuration.

The recursive serializer intentionally omits `Metadata`, `StructParent`,
`StructParents`, `OC`, `AF`, `PTData`, `Ref`, `LastModified`, `PieceInfo` and `OPI`.
That filter applies to copied resource dictionaries, not just the root page.
The actual exclusion list is in
[Hayro Write's primitive serializer](https://docs.rs/crate/hayro-write/0.7.0/source/src/primitive.rs).

## Bounded annotation and layer transport

A further typed source had one view-OFF OCG, a visible blue path and a printable
Stamp with a green vector normal appearance. Every root-reachable reference
resolved and both lopdf and Hayro read it. No external whole-PDF validator was
installed; this is a structurally complete bounded source rather than
externally certified PDF conformance.

Ordinary import omitted the Stamp appearance and catalog OCProperties. A
bounded Rust transformation instead reused the original appearance Form and
resource graph in the page's XObject dictionary and appended a draw command.
The appearance BBox `[0,0,30,20]` and Matrix `[1,0,0,1,3,4]` give bounds
`[3,4,33,24]`; affine `[2,0,0,2,14,12]` maps them onto Stamp Rect
`[20,20,80,60]`. The appearance payload remained byte-identical, with no new
images. This exercised one Stamp, a direct normal appearance and a translation
Matrix. Other appearance states, annotation flags, NoZoom/NoRotate and arbitrary
inherited graphics state or clipping were outside that transformation.

Another bounded graph operation restored the original OCProperties dictionary,
remapping its one OCG reference through the corresponding source/imported page
Properties binding. Identity came from those explicit resource references,
not the OCG display name. The final PDF retained its original default OFF
configuration, content/appearance payloads and zero Image objects. This was
one known reference mapping, without a generic merger or pixel comparison.
The independently reproduced source and final output SHA-256 values were:

| Artifact | SHA-256 |
| --- | --- |
| Source with OCG and Stamp | `1492f8e9984591dc2e2a5097b5668068df203219be7b963734878db0413c5771` |
| Appearance plus restored single-layer configuration | `0bf9357607e97e04771cee6b883245a4f104984049936a7e0baae6e12bebecf7` |

Krilla's public AnnotationType supports Link only, and its page API does not
separately import an arbitrary annotation appearance Form. The experiment used
typed lopdf graph operations, not an SDK feature or another SVG conversion.
See [Krilla's annotation API](https://docs.rs/crate/krilla/0.8.2/source/src/interactive/annotation.rs)
and [lopdf's graph traversal](https://docs.rs/crate/lopdf/0.42.0/source/src/document.rs).

## Numeric resource rewriting

The primitive serializer reads numbers as `f64`, emits integral values as
`i32`, and emits fractional values as `f32`. Page content is transported as a
stream; numbers inside its drawing operators do not pass through this rewrite.
Numbers in copied resource dictionaries do.

A separate Properties-resource probe retained its content stream byte-for-byte
but observed:

| Source numeric literal | Emitted resource literal |
| --- | --- |
| `1.12345678` | `1.1234568` |
| `2147483648` | `2147483647` |
| `2147483647` | `2147483647` |

The decimal input used a single equal-width replacement in a typed
lopdf-generated PDF because lopdf's `Real` representation is itself `f32`.
Replacing `1234567890` with `1.12345678` preserved all object offsets and stream
lengths. Hayro Syntax independently read the input fractional value as `f64`.
The resulting source SHA-256 was
`2b5c1049bbaea4c0b1babcc38cb7f703075b72af247609de1c2b522046bcd676`;
output SHA-256 was
`cf3fcbfd5da4a196409370eae2eab3ae450924238c2abcdf001ee36853fe025b`.
The large-integer case is an implementation-range boundary, not evidence of
visible damage in the available Samsung paper files.

Adding exactly `-2147483648` as another copied resource value caused a debug
build to panic in pdf-writer's `Limits::register_int`: its `i32::abs()` overflowed.
The source was parsed successfully before `Document::finish` reached that path.
This observation is specific to the executed debug configuration; release-mode
behavior was not established. The source SHA-256 was
`e9d0595a29e08b79221f6dfc528ee681b2eda802ae3889f15585805c8fb076ef`.
See the [primitive conversion](https://docs.rs/crate/hayro-write/0.7.0/source/src/primitive.rs)
and [pdf-writer limit accounting](https://docs.rs/crate/pdf-writer/0.15.0/source/src/buf.rs).

## Successful serialization is not completeness admission

A six-case stream probe combined unfiltered content, a declared
`/Crypt` filter with `/DecodeParms << /Name /Identity >>`, or malformed Flate
bytes with either one stream or a two-stream Contents array. The ordinary
stream contained a red vector rectangle; the second array stream contained a
blue vector rectangle.

| First stream | Single-stream result | Array result | Krilla finish |
| --- | --- | --- | --- |
| Unfiltered vector content | Original operators retained | Both streams retained | Succeeded |
| Declared Crypt Identity | Decoder returned no content; imported Form was empty | First stream skipped; only second stream retained | Succeeded |
| Malformed Flate bytes `[1,2,3,4]` | Decoder returned empty content; imported Form was empty | Only second stream contributed operators | Succeeded |

The Crypt probe was unencrypted and isolates handling of its declared filter;
it does not establish complete encrypted-document or password support. Identity
is a pass-through crypt-filter name in the
[Adobe PDF specification, section 7.6.5](https://raw.githubusercontent.com/adobe/dc-acrobat-sdk-docs/master/docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).
The malformed-Flate cases establish missing error admission, not a requirement
to reconstruct corrupt source drawing data.

In the pinned source, a failed single-stream decode becomes `None`; array
collection skips failed decodes. Extraction then substitutes an empty content
stream when no decoded page stream exists. The extraction error type reports
invalid page indices, not these decode omissions. A valid-looking output PDF
and a successful `finish` result therefore cannot certify complete source
transport. See
[Hayro Syntax's `page_stream`](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/page.rs)
and [Hayro Write's extraction](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs).

## Existing APIs for preservation diagnostics

Hayro Syntax exposes original shared bytes, root IDs/xref lookups, raw page
and annotation dictionaries, raw array members, appearance streams and
per-stream decode results. The OCG/Stamp probe inspected those values through
existing public APIs, without another PDF parser. PdfData accepts Arc-backed
bytes, so placements need not each copy the source file. See the
[PDF API](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/pdf.rs),
[xref API](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/xref.rs), and
[shared data representation](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/data.rs).

`Dict::get_raw::<Object>` retains `MaybeRef<Object>`, `get_ref` exposes the
original object reference, and dictionaries/streams expose object IDs.
`keys` with checked raw lookup distinguishes a lookup failure from an unresolved
reference; `entries` instead unwraps each raw lookup internally. Raw array
iteration remains a parser iterator, not arbitrary malformed-token recovery.
These are existing [dictionary](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/object/dict.rs)
and [reference](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/object/ref.rs)
APIs, separate from a resolving convenience view or complete source validation.

Individual inspection of the Crypt Identity stream returned `DecodeFailure::StreamDecode`,
before aggregate page-stream collection hid the omission. The malformed-Flate
stream still returned `Ok` with empty data. Raw-reference traversal is therefore
stronger than using aggregate results, but it does not establish strict decode
completeness.

A separate probe inserted a dangling reference before, between or after two
valid three-operator streams in a Contents array. These intentionally invalid
inputs successfully exported Forms with **0, 3 and 6 operators**, respectively.
Typed array iteration terminated at the unresolvable member, omitting any valid
suffix. That differs from skipping a failed decode of an already resolved
Stream. Raw iteration plus explicit xref lookup exposed every supplied member.
This demonstrates an invalid-input admission boundary, not damaged transport
of a valid PDF. See the [array iterator](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/object/array.rs).

The library does not expose a complete strict-PDF validator. Its object iterator
skips unparseable objects, parsed filter lists omit unknown filter names, the
Flate decoder uses permissive fallback, and `raw_data` can return empty bytes
after a decryption failure. `has_optional_content_groups` tests for a catalog
OCProperties dictionary, not valid group count or evaluated visibility. These
are source-observed boundaries in the
[stream decoder](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/object/stream.rs),
[filter parser](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/filter/mod.rs), and
[Flate fallback](https://docs.rs/crate/hayro-syntax/0.7.2/source/src/filter/lzw_flate.rs).
Opening or decoding successfully does not replace original source retention
and explicit transport-capability reporting.

The transport's structured errors cover a narrower contract. Hayro Write's
`ExtractionError` contains only `InvalidPageIndex`; Krilla's `PdfError` contains
`InvalidPage` and `VersionMismatch`. An unresolved resource reference is written
as PDF `null`, and failed dependency extraction emits a warning rather than a
corresponding extraction-error variant. Those error enums do not report missing
annotations/catalog context or resource-key/numeric rewriting. See the
[extraction/dependency implementation](https://docs.rs/crate/hayro-write/0.7.0/source/src/lib.rs),
[reference writer](https://docs.rs/crate/hayro-write/0.7.0/source/src/primitive.rs), and
[Krilla PDF errors](https://docs.rs/crate/krilla/0.8.2/source/src/pdf.rs).
Krilla's [validated-output modes](https://docs.rs/crate/krilla/0.8.2/source/src/configure/validate.rs)
reject `ValidationError::EmbeddedPDF` because they cannot verify the embedded
source meets the selected profile; that rejection is not source validation.

Hayro exposes the shared source through public read views; extraction receives
`&Pdf`, while output chunks, reference mappings and visited sets belong to the
extraction context. This differs from the
[executed Samsung link-repair helper](pdf-paper-export-findings.md#successful-graph-rewriting-can-still-lose-forward-link-destinations),
which mutates parsed source destination arrays while leaving the borrowed bytes
unchanged. Original source retention and loaded-document state are distinct
contracts in both cases.

## Consequences for preservation

Direct Form import demonstrably avoids rasterization for the exercised vector
content, including shading, patterns, soft masks and invisible text. It is a
stronger preservation path than conversion through an SVG implementation that
outlines or drops text and samples shading into images.

Its demonstrated boundary is the imported content/resource graph, subject to
resource-key and numeric rewriting. Complete original-document preservation
also includes source bytes, page properties, annotation appearances, optional
content configuration and catalog structure. Those are not interchangeable
claims. Retaining the original PDF resource separately is the lossless source
boundary even when a derived export uses a narrower transport capability.
