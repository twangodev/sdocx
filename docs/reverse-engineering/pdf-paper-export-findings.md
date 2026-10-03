# PDF-backed paper in Samsung PDF export

## Evidence boundary

These findings concern Samsung Notes APK 4.4.45.37. Java paths are relative to
the decompiled `sources/` directory; native addresses are arm64 virtual
addresses. The route conclusions below come from static disassembly, exported
symbols and ELF relocations. A separate bounded native PDFium import/save probe
establishes the explicitly reported object-graph results below. Neither that
probe nor the static routes establish complete Samsung export appearance or
arbitrary PDF compatibility.

The inspected APK SHA-256 is
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The extracted libraries were checked byte-for-byte against their APK entries:

| Library | SHA-256 |
| --- | --- |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenPdf.so` | `cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04` |
| `libSPenPdfiumB.so` | `4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libc++_shared.so` | `4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4` |

The [saved-background findings](page-background-findings.md) establish the
versioned PDF record encoding and viewer's multiple-record composition. The
[Standard composition findings](standard-pdf-composition-findings.md) establish
the public export option and the native X delegate. This document traces what
those export delegates do with PDF paper.

## Public Standard reaches a page import operation

`TaskMakePdf.java:30–32` maps `STANDARD` to public type 1. Its `exportPdf` method
copies PDF-reader mode and constructs `SpenNotePdfExport` at lines 228–232.
`SpenNotePdfExport.java:176–191` passes native type 0 with editable and
system-font flags false. As established in the Standard findings, list mode
therefore selects `NotePDFExporterRasterListX`; single mode selects
`NotePDFExporterRasterSingleX`.

For list mode, the imported-paper call chain is:

| Library / address | Operation |
| --- | --- |
| Composer `0x35a48c` | `checkPdfContinuity` selects source identities and page indexes |
| Composer `0x35ab74`, call `0x35abbc` | `exportPDFPages` calls export virtual slot 88 |
| Pdf relocation `0xb0fd0` | Slot 88 resolves to `PDFExport::AddPages`, `0xa74fc` |
| Pdf `0xa7588–0xa75a0`, relocation `0xb05b0` | Forward to engine slot 432, `PDFEnginePdfium::Import`, `0x71a38` |
| Pdf `0x7a1d4`, call `0x7a39c` | `PdfiumExportHandler::Import` loads the source with `FPDF_LoadDocument` on a source-cache miss |
| Pdf `0x7a400` | Call `PdfiumExportHandler::importPages`, `0x7a9f4` |
| Pdf `0x7ab64` | Call `FPDF_ImportPages` |

The imported-paper portion uses PDF page import. It does not render the source
PDF to a bitmap, convert it to SVG, or feed it to the ordinary image-background
writer. The same export can still rasterize note-object batches as described in
the Standard composition findings. Preservation of source PDF graphics and
rasterization of newly drawn note objects are separate operations.

`PDFExport::AddPages` uses the current output page count as the insertion index.
On success it increments that count by `last - first + 1`
(`0xa75f4–0xa7604`). The bounds are inclusive source-page indexes.

### Stored PDF page indexes are zero-based

`importPages` rejects a negative first index or a first index greater than or
equal to `FPDF_GetPageCount` (`0x7aa3c–0x7aa48`). The wrapper adds one to the
first index at `0x7aa68` and to the last index at `0x7aa88` when constructing the
range string supplied to `FPDF_ImportPages`. This is the conversion to the
callee's textual page-range numbering; the saved index itself is zero-based.
The helper separately supports a last-index sentinel of `-1` for its single-page
case. That sentinel is not evidence for a saved negative page index.

## List mode selects the first record and groups contiguous source pages

`NotePDFExporterRasterListX::initializeExport` (`0x35a40c`) copies the note's
page pointers and allocates parallel arrays for group IDs, source indexes and
page information. `checkPdfContinuity` walks all but the final stored page
(`0x35a4b8`, `0x35a62c`); it assigns the final page a group sentinel of `-1` at
`0x35a634–0x35a644`.

For each ordinary note page it obtains `WPage::GetPDFData`
(`0x35a54c`). When the list is nonempty, it calls `ArrayList::Get(0)`
(`0x35a564–0x35a56c`). It compares that record's leading resolved source string
with the previous source identity, then checks whether its source page index
at member 20 equals the previous index plus one
(`0x35a578–0x35a5a0`). Matching entries share a group; a changed identity,
nonconsecutive index or ordinary non-PDF page starts a new group.

At `exportPDFPages`, the first group's resolved source string is passed as the
file argument, password is null, and the first and last source indexes come
from the parallel index array (`0x35ab90–0x35abbc`). The placement rectangle,
media binding integer, and additional PDF records are not arguments to this
list-page import call.

This is a property of this exporter, not the full saved-document contract.
The viewer iterates all records and applies their placements. In particular,
`checkPdfContinuity` is not evidence that a page can contain only one PDF record
or that its stored rectangle can be discarded by a parser.

## Imported pages receive note overlays afterward

After a successful `AddPages`, `exportPDFPages` iterates the corresponding note
pages and invokes `updatePage` with the last boolean true and the preceding
boolean equal to option member 204 (`0x35abcc–0x35abdc`). For Standard that
member is the editable flag and is false.

`updatePage` first loads the imported output page through export virtual slot
104 (`0x35af34–0x35af40`, Pdf relocation `0xb0fe0`). Before writing it, the
true final boolean enables removal of matching existing overlays for the
exporter's application tag and, when enabled, highlighter tag
(`0x35af84–0x35b08c`). The two virtual calls resolve to
`PDFExport::RemoveOverlayImage` and `PDFExport::RemovePDFOverlay`
(Pdf relocations `0xb1010` and `0xb0fb0`). The separate editable-flag branch
would call `RemoveOverlayImageTags` through slot 40; Standard does not take it.

The writer then adds background, body text, ordinary objects, highlighters and
tape in the order documented for Standard (`0x35b090–0x35b0f4`). On the imported
paper route, `captureBackground` returns without producing a background image
when the PDF data list is nonempty (`0x35a9d8–0x35a9ec`). Consequently
`addBackground` sees an empty captured-background path and returns before
inserting an image (`0x35b218–0x35b224`). The imported source supplies the paper;
this branch does not place a new solid-color rectangle over it.

This is not a global claim that every PDF template suppresses every background
color under every option. It is the inspected Standard list-page import route.

`openPDFWriter` opens the destination page and an application-tagged writer
group (`0x358788–0x3587c4`). In Pdfium,
`PdfiumExportHandler::CloseWriter` (`0x7c93c`) checks the group's form-object
content, calls `FPDFFormObj_GenarateStream` at `0x7c980`, inserts it with
`FPDFPage_InsertObject` at `0x7c98c`, and generates page content at `0x7c994`.
Here the form object is a PDF drawing group, not evidence of an interactive
AcroForm field or of flattening every source-page annotation.

The import helper also calls `restorePageLink` immediately after successful
`FPDF_ImportPages` (`0x7ab6c–0x7ab80`). The native export performs extra link
repair after page copying. The inspected caller alone does not prove preservation
of every destination, widget, annotation, structure tree or document-level
resource. Those behaviors have not been executed against a PDF feature corpus.

### PDFium copies page entries, with a narrower reference and catalog contract

The pinned `libSPenPdfiumB.so` supplies `FPDF_ImportPages` at `0x59bc8c`.
It calls `CPDF_PageExporter::ExportPages` (`0x406fb4`) at `0x59bd34`.
Independent bounded disassembly and ELF relocation checks establish the
following copier policy; this is not a full Samsung export execution result.

| Boundary | Pinned native behavior |
| --- | --- |
| Local page entries | Clone every key except `/Type` and `/Parent` (`0x4070a8–0x4070f4`) |
| Inherited page values | Copy `/MediaBox`, `/Resources`, `/CropBox`, `/Rotate` if the destination lacks them (`0x4071c4`, `0x4073c8`, `0x4074d4`, `0x407578`) |
| Missing media box | Fall back to inherited `/CropBox`, then `[0, 0, 612, 792]` (`0x407274`, `0x407334`, constant `0x1b04c0`) |
| Missing resources | Create an empty resource dictionary (`0x407440`) |
| Stream cloning | Clone the stream dictionary and obtain raw stream data through `CPDF_StreamAcc::ProcessRawData` (`0x4450a8`, `0x44512c`, `0x445184`) |
| Destination catalog | Initialize destination info, catalog and page tree; the helper does not read the source-document field (`InitDestDoc`, `0x407a04`) |

The first loop admits local `/Annots`, annotation appearance resources, `/Group`
and `/UserUnit` to cloning. It does not establish intact appearance, physical
scale or annotation semantics. The stream clone path has no page-to-bitmap
operation; subsequent serialization still passes through `CPDF_FlateEncoder`
(`CPDF_Stream::WriteTo`, `0x445cdc`), so raw cloning alone does not establish
identical encoded output bytes. This entry policy differs from the pinned Rust
library's explicit page/Form writer, whose executed boundaries are recorded in
the [PDF vector transport findings](pdf-vector-transport-findings.md).

`UpdateReference` (`0x40844c`) rewrites nested references, but dictionary traversal
skips `/Parent`, `/Prev` and `/First` (`0x408504–0x408548`). Plain reference
cloning retains the original holder and object number (`0x43f654–0x43f668`);
these skipped backlinks are therefore not demonstrated to be rebound safely.
For other dictionary keys, a failed child rewrite queues the key for removal,
then the dictionary returns success (`0x4085a4–0x4085ac`, `0x408790`,
`0x4087bc`). Arrays instead return failure at the first unsuccessful child
(`0x4086b0–0x4086c8`). `ExportPages` ignores the page-level rewrite result at
`0x407618`. A successful import return consequently does not certify a complete
rewritten object graph.

The current source page's object number is mapped to its destination number
before rewriting (`0x4075bc–0x4075f4`). An annotation `/P` pointing to that
page has a cached resolution route. For an uncached reference, `GetNewObjId`
(`0x4088ec`) refuses targets whose dictionary type is `/Page`, and returns
literal object number `4` for `/Pages` (`0x408aa0–0x408acc`). This does not prove
that an uncached destination page or that literal page-tree number is correct
for every import context. Other objects enter the destination and identity cache
before their children are rewritten (`0x408b14`, `0x408b58`, `0x408b6c`). A
failed rewrite returns zero without a visible rollback in this helper, while
later nonzero cache hits return immediately (`0x408970–0x408974`).

The copier starts traversal from selected page entries. Catalog-only
`/OCProperties`, `/StructTreeRoot`, names, outlines and actions are outside
`InitDestDoc`'s source traversal. Retaining a page's optional-content resource or
`/StructParents` integer cannot establish the corresponding catalog visibility
or tagging context. The wrapper's separately observed link repair remains an
additional operation; these local copier findings do not substitute for a
complete Samsung export capture.

### A native import/save probe retains one vector Stamp but drops layer defaults

A temporary Rust/Unicorn client executed the APK's actual `libSPenPdfiumB.so`
and `libc++_shared.so`, loading the one-page typed source from the
[annotation and layer experiment](pdf-vector-transport-findings.md#bounded-annotation-and-layer-transport).
It called `FPDF_InitLibrary`, `FPDF_LoadMemDocument64`,
`FPDF_CreateNewDocument`, `FPDF_ImportPages` and `FPDF_SaveAsCopy`. The pinned
Samsung save entry also reads a fourth argument; the client supplied a mapped,
zeroed empty callable facade. This is a capture of the public PDFium operations,
not the Composer/`libSPenPdf` wrapper or its additional link repair.

The client supplied explicit host boundaries for allocation, memory/string
operations, integer formatting, fixed time, single-thread synchronization and
zlib compression. No unknown host import was allowed to silently succeed.
System-font directory lookup returned absent; the supplied source contains no
font. There was no page rendering or bitmap creation. An independent reviewer
reran the native client and inspected the saved bytes through Rust `lopdf`.

The source SHA-256 is
`1492f8e9984591dc2e2a5097b5668068df203219be7b963734878db0413c5771`.
The saved output is 1,319 bytes with SHA-256
`44a839c4803a696e3f8e448066f202fbd4959ffb518d326d4c23fab7687a60bd`.
Five fresh executions with ordinary allocation filled with `0x00`, `0x55`,
`0xa5`, `0xff`, then `0x00` produced identical output; `calloc` remained zeroed.

| Supplied feature | Independently inspected native result |
| --- | --- |
| Eight page-content operators | Decoded content bytes and operators unchanged |
| Stamp annotation | Fields unchanged except `/P` rebound to the imported current page |
| Direct normal appearance Form | Decoded vector stream, bounding box, matrix and resources unchanged |
| Optional-content resource group | Resolved group dictionary unchanged |
| Catalog layer configuration with default `/OFF` | `/OCProperties` absent from the output catalog |
| Images and object references | Zero Image objects; all references reachable from the output root resolve |

This confirms direct appearance-resource retention for the supplied Stamp,
where the pinned Krilla Form route omits it. It also demonstrates that retaining
the OCG dictionary does not retain the catalog's default visibility configuration.
There was no rendered visibility comparison. Other annotation states, widgets,
page groups, `/UserUnit`, tags and cross-page destinations were not supplied by
this probe, and its successful return is not a general import-completeness check.

### All available original paper pages survive the native content transport

The same client imported all 20 `cs61bl_su22` pages and the one `quiz` page into
one destination. These are the embedded originals identified in the
[original-paper inspection](pdf-vector-transport-findings.md#all-available-original-paper-pages),
not paired Samsung-export reference files. The output is 937,425 bytes,
SHA-256 `2cb0a9a9327cf190546a9b9df661155648ce4fa4108e66a57553972de0320b0f`.
Five fresh allocation-fill executions and an independent replay produced the
same bytes. The real-source client additionally supplied host zlib inflate with
pointer/count marshalling and host allocation in place of guest codec allocation
callbacks, plus locale, math and single-thread services. Source parsing, page
copying, C++ numeric serialization and saving still executed the APK code.

Independent Rust inspection found all 3,726 operators and decoded page-content
bytes unchanged. Direct `lopdf` text extraction succeeded and matched: 22,510
UTF-8 bytes across `cs61bl`, zero for `quiz`. All 27 unique original encoded
image payloads survived, with no added or missing image payloads. Resolved
resource graphs matched after stream-encoding normalization and `f32` number
comparison; font programs, encodings, ToUnicode, ColorSpaces and image masks
remained present. Page dictionary comparisons and separate `f64` inspection of
page-dictionary numbers found no supplied changes. All references reachable
from the output root resolved.

The `f32` comparison conceals a numeric boundary: direct Hayro `f64` inspection
found 5,600 changed values among 11,868 resource-number **visits** through the
page graphs. Every observed change is a font `/Widths` value, for example
`777.8 → 777.79999` and `694.4 → 694.40002`. The maximum absolute change is
approximately `0.00005` font units, not page coordinates or pixels. These are
repeated graph visits, not counts of unique PDF objects. The
[Rust-library original-paper result](pdf-vector-transport-findings.md#all-available-original-paper-pages)
has no such observed `f64` resource-number changes. Native precision loss is an
observed serializer behavior, not a requirement for Rust preservation outputs.

These real sources do not supply annotations, `/UserUnit`, rotations, optional
content catalogs or tagging trees, or nonempty shading, pattern and graphics
state graphs. Their absence is not coverage. Source `cs61bl` catalog `/Names`,
`/OpenAction` and `/PageMode` remain outside the imported page graph. This result
still concerns the public PDFium import/save route, not the full Samsung wrapper
or rendered appearance.

### A geometry variant retains direct page fields and a CMYK group

A typed variant of the one-page source adds `/CropBox [10 20 180 190]`,
`/Rotate 90`, `/UserUnit 2` and a transparency Group with `/CS /DeviceCMYK`,
`/I false` and `/K true`. Its source SHA-256 is
`b3e9efc46dd232d2fe014df18bc677f1df67c633449f460be076af72b49d15d0`.
The native output is 1,430 bytes with SHA-256
`4ee607178794ae2e447cd605e49afa4f5673f48faf3ce533edc5f13ee18cfd8e`,
identical across the five allocation fills and an independent replay.

All supplied page fields and the complete Group dictionary survive unchanged;
separate `f64` page-dictionary number inspection agrees. The eight content operators,
annotation appearance, resources and current-page `/P` reference retain the
earlier checkpoint's behavior. There are zero Image objects and all root
references resolve; catalog `/OCProperties` is still absent. This establishes
field and object-graph retention, without a rotated, physically scaled or CMYK
appearance comparison. It does not expand the real papers' supplied feature set.

## Source size controls the overlay scale

The imported-page operation receives no requested output width or height.
`addBodyText` reads the loaded PDF writer's page rectangle and divides its width
by the note page width (`0x35b3cc–0x35b3fc`). That `f32` ratio becomes the
legacy writer's scale. Note height remains separately supplied as the note's
logical height at `0x35b458–0x35b45c`.

The other native vector delegate makes the distinction especially explicit:
`NotePDFExporterVectorList::exportPages` imports the first PDF record with
`PdfDocumentAdapter::AddPage` (`0x361924–0x361948`). For ordinary paper it
instead creates a new page of width 595 and integer-computed height
`note_height * 595 / note_width` (`0x36196c–0x361990`). After loading either
page, it obtains the actual PDF page rectangle and computes
`pdf_width / note_width` (`0x3619a4–0x3619d4`).

`PdfDocumentAdapter::AddPage` at `0x343910` forwards through `PdfExporter::AddPage`
at Pdf `0xa8198` to the same `PDFExport` source-page import implementation.
`exportBackground` bypasses `BackgroundPdfExporter` when `WPage::HasPDF` is
true (`0x361b50–0x361b54`), because the page has already been imported in
`exportPages`. The bypass does not mean PDF paper disappears from this native
export.

Neither inspected list import call applies the saved PDF placement rectangle as
an import transform or clip. Source page boxes, PDF rotations, and the viewer's
note-space placements are distinct contracts. A complete mapping between them
is not established by these export calls. The
[source-size trace](pdf-paper-placement-findings.md#source-dimensions-include-pdf-box-and-rotation-handling)
separately establishes effective CropBox/MediaBox dimensions and rotated width
and height; those PDF dimensions are not the saved note-space rectangle.

## Single mode has a separate segment selector

`NotePDFExporterRasterSingleX::initializeExport` obtains the first note page's
whole PDF list and builds `PdfPageSegmentBuilder` using note width and the
default export height (`0x35daa0–0x35dadc`). `buildPageSegments` repeatedly
calls `AccumulatePage` with the current vertical offset and PDF-record cursor
(`0x35db58–0x35db68`).

`AccumulatePage` forms a full-width vertical strip and calls `getPdfDataInRect`
(`0x35fa0c–0x35fa4c`). That helper inspects the supplied list index, not all
records, and tests its `RectF` at member 24 with `RectF::IsIntersect`
(`0x35facc–0x35faf8`). `accumulatePdfPage` compares the current strip top with
the record's top. It truncates the absolute difference and admits the aligned
PDF branch when that integer is less than 2 (`0x35fb40–0x35fb60`). On that
branch, height is the truncated difference between the record's bottom and the
strip top (`0x35fb84–0x35fb98`). Grouping also checks source identity and
consecutive source-page indexes (`0x35fbb0–0x35fbc4`, `0x35fc0c–0x35fc18`).

`addPageToSegment` stores vertical offset and selected height separately from
the optional PDF record; a nonpositive height substitutes the default height
(`0x35fff8–0x36002c`). `exportPdfPages` imports the segment's first-to-last
source indexes through `AddPages` at `0x35e208`, then passes each offset/height
pair to `updatePage` (`0x35e220–0x35e23c`). PDF-record rectangles therefore
participate in single-mode segmentation even though they are not arguments to
the final PDF import operation.

The loop advances the vertical offset by the chosen slice height, increments
the output page index, and increments the PDF-record cursor only when the
current segment contains PDF records (`0x35dbb8–0x35dbf8`). A single physical
continuous note page can produce many PDF output pages. Its records are
consumed through a cursor, rather than independently finding every rectangle
that intersects each strip.

For ordinary strips, `updateHeightByBodyText` checks the full-width band between
`y + nominal_height - 1` and `y + nominal_height`, finds the body line at the
band's top, and can return `trunc(line_bottom - f32(y))`
(`0x35fd50–0x35fe68`). Absent visible text, a missing line index and the last-line
branches that lie wholly above or below the cut retain the nominal height.
`accumulateNormalPage` accepts a changed candidate only when it is at most
`f32(nominal_height) * 1.2f`; the constant at `0x1f9108` is
`1.2000000476837158`. If the extended band intersects the current PDF record,
it clips that candidate to `trunc(pdf_top - f32(y))`
(`0x35fc54–0x35fcc4`). This is evidence of content-aware slice selection, not
fixed-height division of the continuous canvas.

Single-mode placed text and images also rebase their note-space bounds before
writing. `writeObjectTextBox` offsets a copied rectangle by
`(0, f32(neg_i32(slice_y)))` at `0x35f46c–0x35f488`; `writeObjectImage` does the
same at `0x35f5c0–0x35f5dc`. Their writer scale uses the loaded PDF page width
divided by the first note page's width (`getPageRatio`,
`0x35f348–0x35f3b4`). Continuous-note placement and source PDF page indexes are
therefore different coordinate systems.

These static branches do not establish arbitrary overlap, unsorted-record,
integer-overflow or malformed-rectangle handling. They do establish that
list-mode first-record selection cannot be generalized to single-mode notes.

## Consequences for the Rust model and vector outputs

The current Rust parser consumes PDF records but keeps only the first source
page index; it discards binding IDs, placements and subsequent records. As
recorded in the saved-background findings, the resulting `CustomPdf` value
cannot identify the PDF bytes or reproduce viewer composition. Knowing the
template ID and first page number is insufficient for either use.

The evidence distinguishes three operations:

| Operation | Input needed | Native boundary |
| --- | --- | --- |
| Render PDF paper in a note view | Every resolved PDF record and its placement | Viewer creates placed PDF views |
| Export PDF paper through list mode | First resolved source and page index per note page | Import source pages, then write note overlays |
| Export a continuous note | Ordered PDF records, placements and segment state | Choose vertical segments, import matching source ranges, then write overlays |

Samsung's PDF export does not provide an SVG representation of the imported
paper. Preserving a source PDF page in PDF output and representing that page in
SVG are different capabilities. The current Rust document-to-PDF transport
starts from its SVG graphics plus retained text; it has no imported-source-page
operation. Its vector guarantee for supported note graphics does not establish
support for arbitrary PDF paper or for PDF-to-SVG conversion.

Likewise, turning a PDF page into a bitmap would change the retained content
contract, independently of whether note strokes remain vectors. Source content,
note geometry, resource resolution, placement and export transport are separate
data requirements demonstrated by these native routes.
