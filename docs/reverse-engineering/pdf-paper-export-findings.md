# PDF-backed paper in Samsung PDF export

## Evidence boundary

These findings concern Samsung Notes APK 4.4.45.37. Java paths are relative to
the decompiled `sources/` directory; native addresses are arm64 virtual
addresses. The route conclusions below come from static disassembly, exported
symbols and ELF relocations. No native export execution or paired visual capture
establishes appearance, annotation preservation or arbitrary PDF compatibility.

The inspected APK SHA-256 is
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The extracted libraries were checked byte-for-byte against their APK entries:

| Library | SHA-256 |
| --- | --- |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenPdf.so` | `cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |

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
