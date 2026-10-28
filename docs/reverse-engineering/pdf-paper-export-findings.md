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

SingleX uses a separate background predicate (`Composer 0x35e51c`): without a
background image or template, it captures color only when `ARGB + 0x30304` is
nonzero, making `0xfffcfcfc` (RGB 252) the default sentinel. That false branch
skips `SetPageContents` and `saveThumbnailByRect`; when a custom color is
captured, `drawPage` paints it before `drawPdf`, and `drawTemplate` returns for
`HasPDF`. The bounded probe69 PDF has no image, form or content stream on its
ordinary tail pages 2–10; this establishes serialized emptiness, not page
pixels or a device-backdrop color. The inspected Composer binary has SHA-256
`52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f`.

`openPDFWriter` loads the destination page before opening its application-tagged
writer group (`0x358788–0x3587c4`). `PDFManager::CreateWriter` (`Pdf 0x60104`)
initializes the writer's page index at `+36` to `-1`; `PDFWriter::LoadPage`
(`0x65b88`) calls the engine loader and stores the index. `OpenPDFWriter`
(`0x65bfc`) rejects `-1`. A direct object-drawing probe that substitutes success
for this lifecycle can report a destination rectangle without serializing an
overlay; it does not establish final PDF placement. In Pdfium,
`PdfiumExportHandler::CloseWriter` (`0x7c93c`) checks the group's form-object
content, calls `FPDFFormObj_GenarateStream` at `0x7c980`, inserts it with
`FPDFPage_InsertObject` at `0x7c98c`, and generates page content at `0x7c994`.
Here the form object is a PDF drawing group, not evidence of an interactive
AcroForm field or of flattening every source-page annotation.

The import helper also calls `restorePageLink` immediately after successful
`FPDF_ImportPages` (`0x7ab6c–0x7ab80`). The native export performs extra link
repair after page copying. The inspected caller alone does not prove preservation
of every destination, widget, annotation, structure tree or document-level
resource. The bounded experiments below cover only their supplied cases.

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

### Successful graph rewriting can still lose forward link destinations

A typed two-page source supplies five Link annotations: forward and backward
`/Dest` arrays, forward and backward GoTo action `/A /D` arrays, and a current-page
destination. Each page has three vector operators; all supplied root references
resolve and both Rust parsers accept the source. Its SHA-256 is
`6eff059e1d0a767af48ae0725e9379d8813f1c27ee56a0753d5a9843da765098`.

Raw `FPDF_ImportPages` succeeds when importing both pages, but removes the first
page's forward `/Dest`; its forward GoTo action retains `/S /GoTo` while losing
`/D`. Current/backward references map correctly. All five annotation `/P` values
identify their actual output pages, all root references resolve, and vector
content remains unchanged. Resolving every graph reference does not establish
retained destination semantics: a reference to a not-yet-mapped page can disappear even
when that page is also requested in the import. This source has no intentionally
dangling references.

The client then executed the actual `libSPenPdf.so` `restorePageLink` helper
(`0x7cea8`) before saving, without additional host substitutions. It restored
both forward destinations and emitted integer output page indexes for every
supplied destination: `1` for forward, `0` for current/backward, retaining `/Fit`.
The raw and repaired captures match across five allocation fills and independent
replay; the later reuse control matches three fresh processes.

| Native route | Bytes | SHA-256 |
| --- | ---: | --- |
| Raw import/save | 1,738 | `eb080b94f707f2caf0d9d016fc23adf541ede1261427a908111da7c772e2c9e3` |
| Import, actual link repair, save | 1,746 | `167fcef77ae26c1ced6ff1d534cecf1802116c4615be1ec1e90ddaeff39e2d4b` |
| Three imports reusing the parsed source | 4,380 | `d5af31265bf06cd863bedcd483473ac315c68b6d02e02758ebf6157d9572561d` |

Independent static review confirms repair pairs annotations by array index and
requires equal source/output annotation-array lengths (`0x7cfdc–0x7cfe0`). It
admits Link/Widget annotations with action type at most `1`, bounds the resolved
destination to the imported source range, and computes
`insertion_index + source_index - first_imported_index`
(`0x7d168–0x7d180`, `0x7d248–0x7d260`). The helper at `0x7d754` replaces
destination-array element zero with an integer, then clones the array into the
output `/Dest` or action `/D`. Saving the source model after repair independently
confirms this also mutates the parsed source object; exact guest-buffer checks
confirm the borrowed source bytes remain unchanged.

One bounded control reused that source handle: raw prefix import at insertion
index `0`, import plus repair at `2`, then import plus repair at `4`. The first
repaired copy stores forward index `3` and current/backward index `2`. The later
copy still stores `3`/`2`, rather than the newly inserted `5`/`4`. Read-only native
instruction hooks observe destination-index evaluation returning
`[1, 1, 0, 0, 0]`, then `[3, 3, 2, 2, 2]`; the latter values exceed the helper's
source range `0..1`, so its bounds check skips conversion. All 18 vector
operators/content bytes, resources and 15 current-page `/P` references remain
intact, with zero images and resolving root references. The original borrowed
buffer remains unchanged after each operation.

This distinguishes immutable source bytes from mutable loaded-document state
and reproduces stale destinations in one explicit reuse sequence. It does not
certify emitted integer destinations in arbitrary viewers, widget behavior or
the full Samsung source-cache lifecycle. The local helper executes without the
complete import wrapper, handler constructor, Composer or navigation rendering.

### Cached source handles persist between imports within a loaded document

Source inspection distinguishes the temporary export handler from its parsed
PDF inputs. `PDFEnginePdfium::Import` wraps the existing PdfiumImpl pointer in
a lightweight handler (`0x71a94–0x71ab0`); its constructor only stores that
pointer (`0x7a100`). Handler Import caches source handles by exact supplied
filename bytes. A hit bypasses `FPDF_LoadDocument` (`0x7a354–0x7a3c4`), and the
common path passes the cached handle to `importPages` (`0x7a3e0–0x7a400`) and
then `restorePageLink` (`0x7ab6c–0x7ab80`). It does not reopen the source before
each repair.

A bounded capture executed that actual wrapper for the same two-page Link
source, appending at zero-based positions 0, 2 and 4. It then executed actual
`ClearImportReferences` (`0x856d8`) and appended at 6. Native load, cache,
import and repair instructions ran unchanged. The lightweight handler
constructor ran; the full PdfiumImpl constructor and Composer did not.
Relevant state was supplied in a zeroed 2,048-byte object: an empty source map
at offset 1288, load factor `1f32` at 1320, empty page-info vectors at 1496/1520,
zero page count at 352 and an actual new destination handle at 1544. This does
not establish equivalence to the complete constructed object or application.

Additional host services admitted only the exact source fixture and read-only
open flags `0x20000`. They supplied the native 128-byte `fstat` layout with file
size at byte 48, checked seek/read bounds and tracked file positions and close.
A log-string hook passed through its pointer. Actual native file loading and
PDF parsing ran, subject to the earlier allocator/compression/ABI boundaries;
no hook supplied a preloaded document or destination semantics.

| Append position | Source state observed | Stored forward / current-backward destination integers |
| --- | --- | --- |
| 0 | First native filename load | 1 / 0 |
| 2 | Same cached parsed handle | 3 / 2 |
| 4 | Same handle after prior source mutation | 3 / 2; not the newly appended pages' 5 / 4 |
| 6, after cache reset | Second native filename load | 7 / 6 |

The first three imports shared one parsed source handle and one native load.
Reset closed that source/file, emptied map entries and retained bucket storage;
the next import loaded a fresh source. Both native page-info vector lengths and
the page-count field matched the output's 2/4/6/8 pages. A separate process
stopped after the second import and confirmed its parsed source already stored
3/2. No serialization occurred between the full route's first three imports.
The provider bytes and on-disk source stayed unchanged. The final cached source
remained open at isolated-process exit; full teardown was not captured.

The final eight-page PDF retained all 24 original vector operators and decoded
content/resources/page fields, all twenty annotation page bindings, zero images
and resolving root references. It had 5,709 bytes and SHA-256
`1545c704c7ffd864c96766c8e3d6042dd65eaa012ac4d71014a945e9672ffa20`.
Five fresh allocator-fill runs reproduced the outputs. An independent reviewer
verified the frozen Rust client/executable hashes, replayed the full route and
terminal two-import control in fresh processes and inspected their graphs.
This establishes the controlled actual cache-wrapper sequence, including
resolving references despite stale link destinations. It does not establish
full Samsung UI behavior, arbitrary-reader integer-destination compatibility
or every annotation subtype.

The normal lifecycle has reset boundaries. Handler `NewDocument` calls
PdfiumImpl::Close before creating a destination (`0x7a120–0x7a124`), and both
Open overloads call Close before loading. With a nonnull main document, Close
calls `ClearImportReferences` (`0x84da0`) before resetting the main handle. Its
null-main guard skips that teardown (`0x849bc–0x849c0`), so unconditional reset
wording would be inaccurate. `CloseWriter` does not clear the import cache.
Separately, handler Export creates a temporary output, uses the existing main
document as the source of the same import/repair helper, and closes the
temporary output (`0x7cab0–0x7cad4`, `0x7ccb0`). This last route is statically
traced, rather than executed by the cached Import capture: a fresh output does
not itself provide a fresh parsed source.

At the unprotected final-save branch, Standard list/single `saveFile`
(`0x35a99c`, `0x35dee4`) reaches RasterDelegateX `savePDF` and slot 184
(`0x357cc4`). Vector `savePDF` calls `PdfDocumentAdapter::Save`
(`0x361598`, `0x343d14`), then `PdfExporter::Save` (`0xa8820`) and the same
slot. Relocation `0xb1030` resolves it to `PDFExport::Save()`. These concrete
callers establish final Save dispatch separately from ranged Export; they do
not establish an additional ranged import/repair stage in ordinary note save.

### Ranged Export trims a runtime font cache

Source-only Pdf inspection identifies member 1000 as the linked head of an
integer-keyed font map at PdfiumImpl member 984. `GetFont` (`0x8d17c`) reads
the supplied `PDFFontData` ID; on a miss it loads that data's program bytes
into the main document through `FPDFText_LoadFont` (`0x8d1d4`) and stores the
returned font at node member 24 (`0x8d1fc`). A cache hit returns that handle
without comparing or reloading supplied bytes. The separate bitmap-font map
at member 1024 is outside this head-1000 traversal. These runtime programs have
the [font provenance](note-metadata-findings.md#font-program-provenance) described
for newly written text; no direct population from imported font-resource
dictionaries was found in the inspected Import/Open bodies.

After page import, ranged Export walks this list and passes each node's font
to `FPDFText_TrimFontData` (`0x7cc10–0x7cc24`), ignoring its result. The pinned
PdfiumB trimmer (`0x40bc40`) requires program data and a nonempty usage set.
Those values seed HarfBuzz's glyph set; explicit input flags enable
`desubroutinize` and `retain_gids`.

For ordinary shaped text, the [glyph-owner producer](text-draw-identity-findings.md#utf-16-entry-ownership)
and Text's retained-run transfer (`0x67330–0x673b0`, `0x67f84–0x67fb4`) carry glyph
IDs unchanged into `DrawnText`. Both modern and legacy PDF writers forward
that code vector to the custom `SetCharcodesAndPositions`, which inserts each
u32 unchanged into the font's usage set (`0x58b650–0x58b684`). This establishes
glyph-ID-valued subset inputs for those ordinary paths; it does not establish
the value kind of every OCR, bitmap or public text caller, correct face selection
or recording of every required glyph.

The inspected `GetFont` loader supplies type 2 and CID mode 1, creating a
Type0 `Identity-H` font with a `CIDFontType2` descendant (`0x58b8ec–0x58b900`).
For its embedded face with no `CIDToGID` stream, the normal identity branch
uses the codeword's low 16 bits as the glyph ID (`0x40e840–0x40e980`). Identity
encoding emits two big-endian bytes (`0x40fed4–0x40fefc`). These conditions
connect ordinary drawing codes to glyphs; they do not establish that arbitrary
imported font encodings or supplied u32 values use the same mapping.

Generated `ToUnicode` has a separate origin: the loader enumerates the font
charmap's character/glyph pairs, keys them by glyph ID (`0x58ba64–0x58ba78`),
and installs the resulting CMap (`0x58cb20`). It receives no original shaping
clusters. Thus retained glyph IDs and subset seeds do not prove reconstruction
of ligatures, contextual alternates or multi-character source sequences.
The public Unicode `SetText` route separately maps Unicode to font codewords
before encoding them (`0x58b344–0x58b38c`). Existing source-paper `ToUnicode`
and encodings follow resource copying, rather than this runtime font creation.

The subset update starts from the supplied font's dictionary and requires
exactly one `DescendantFonts` member, its `FontDescriptor` and an existing
selected `FontFile`/`FontFile2` stream (`0x40bed8–0x40c0c4`). It replaces that
stream's program bytes through `SetData` (`0x40c0d4`); the non-Type1 branch also
updates `Length1`. Failed prerequisites return false rather than synthesizing
a complete replacement font dictionary.

This establishes a guarded runtime-font resource operation, not wholesale
subsetting of the imported source-paper graph. No font-trimming capture,
source-paper glyph loss or complete logical-text parity was established.

The successful stream-copy path establishes a more specific storage boundary.
`CPDF_Stream::CloneNonCyclic` (`0x445024`) clones the dictionary (`0x44512c`)
and calls `DetachData` (`0x445184`). A memory-backed source is initially borrowed,
but detach allocates and copies its bytes (`0x446cb0–0x446cc4`); a file-backed
source transfers its separately allocated read buffer. The new stream owns
that buffer and cloned dictionary (`0x445324–0x445358`, `0x44481c–0x444854`).
Successful import also rebinds cloned reference holder/ID fields to the
destination (`0x408644–0x408648`). `SetData` copies replacement bytes and
replaces only the receiver's storage (`0x4449c0–0x4449e4`, `0x44565c`). Thus a
later source-stream update cannot change an already copied and rebound target
font program through shared byte or dictionary storage. This conditional
source finding excludes failed or skipped graph rewriting; it is not an
executed subset, glyph-coverage or complete resource-graph preservation proof.

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

The [parsed indent controls](../../conformance/pdf-paper-parsed-indent-native.json)
feed a synthetic type-2 record, ordinal range `[0,1)`, level 1 and LTR direction
to native `ApplyBinary_TextData`. At text scale 3 the wide, wrapped and narrow
cases place the first nonempty line background left edge at x=16; wrapped and
narrow cases also produce overlapping page sections. The Rust measured-layout
line origin for the wide control is x=48. The harness uses the default device
profile with density 1 and no text manager, then independently sets the Widget
text scale to 3. The native indent increment uses document density rather than
that text scale. Rust currently derives its text scale from document metadata
density and uses the same setting for indent width, so the x=48 comparison does
not by itself establish a mismatch for a production document with density 3.
The [density-3 manager controls](../../conformance/pdf-paper-parsed-indent-density3-native.json)
use local text scale 1 and place the indented line at x=48. Rust matches their
line rectangles, ownership and page sections for three single-paragraph controls.
The PDF-paper certificate admits this exact level-1 LTR record with an implicit
font and either a single nonempty ASCII paragraph, including wrapped text, or a
trailing empty second paragraph or an empty middle paragraph among three. The
latter boundary follows the retained
`AV\n\nTo` and `AV\n` controls: line geometry, source ownership, first-empty
and cursor rectangles, and page sections match the native capture. In this
indent profile the implicit empty-line caret is scaled; the default profile uses
an unscaled caret. The leading-empty control remains rejected because its native
line background starts at x=0 while the measured Rust line starts at x=48. The
[additional density-3 capture](../../conformance/pdf-paper-parsed-indent-multiple-native.json)
adds three short nonempty paragraphs and two wrapped paragraphs, producing 18
lines in the latter. Their rectangles and UTF-16 ownership match Rust, as do
the six and 23 supplied page sections, respectively. The wrapped sections
cover the second paragraph transition and the end of the text. The certificate also admits two nonempty paragraphs and
three with a nonempty first and last paragraph for the same record. The controls
are synthetic. The hash-verified real `01-basic-formatting` note has the same
level-1 LTR payload at paragraph ordinals 54 and 57 among many other records;
its full content is not admitted by this certificate. Other indent records and
more paragraphs remain unsupported.
Admitting other text and widths is an inference from the limited captures. A public PDF control is readable by MuPDF and PDFium. The
controls supply page records and do not execute full archive loading, the native
PDF writer or a device. The host libc digest differs from the pinned earlier
controls and is recorded in the receipt. Capture mode sets `SDOCX_CAPTURE_PARSED_DEFAULT=1` and
`SDOCX_CAPTURE_PARSED_INDENT=1` for `--bodytext-page-ranges`; this replay used
a temporary harness copy with its host-libc digest check updated to the
recorded digest. Native Model getters verify the parsed level and direction.

Blank-body admission has a separate native predicate: Bodytext `HasVisibleText`
(`0xcbe84`) obtains a line range, then Widget `HasVisibleTextNParagraph`
(`0xde170`) scans source UTF-16 and paragraph bullet fields. LF-only text with
no bullets is invisible regardless of font size, foreground or margins. Widget
`ConvertParagraphs` zeroes the bullet field at record `+40` (`0xdadc8`,
`0xdae6c`); only paragraph type 5 writes it (`convertParagraphImpl`, `0xdba5c`).
The Rust adapter admits known nonpainting styles on LF-only bodies and excludes
bullet paragraphs, visible characters and inline objects. This predicate does
not establish body-object drawing or general text placement.

Ordinary output surfaces use a separate integer size. The delegate constructor
stores the constant pair `(1800,600)` at offsets `40/44` (`0x3573c4–0x3573d8`,
constant bytes `0807000058020000` at `0x1f94a0`). SingleX setup retains that
600-point width. `exportNormalPages` computes height with signed integer
division, `width * slice_height / note_width` (`0x35e364–0x35e380`). An executed
synthetic-note control with the real quiz PDF therefore produces `600×505`
continuations from `1527`-unit bands at note width `1812`; imported source
pages retain their own physical boxes. Query bands and output clips differ.

The loop advances the vertical offset by the chosen slice height, increments
the output page index, and increments the PDF-record cursor only when the
current segment contains PDF records (`0x35dbb8–0x35dbf8`). A single physical
continuous note page can produce many PDF output pages. Its records are
consumed through a cursor, rather than independently finding every rectangle
that intersects each strip.

An executed `ConstructLoad` → `SingleX::initializeExport` → `buildPageSegments`
check on the quiz archive (SHA-256
`38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a`)
confirms this topology on its actual loaded WNote: one PDF record at
`[0,0,1812,1485]`, a PDF segment with slice `(0,1485)`, then ten ordinary
slices starting at `1485,3012,4539,6066,7593,9120,10647,12174,13701,15228`,
each height `1527`. The Rust planner regression checks all eleven slices.
This execution uses host platform/display/math services; it does not establish
final PDF placement or native device pixels. A separate
[archive-backed query comparison](object-selection-findings.md) verifies
ordinary stroke admission and order for this sample.

An archive-backed SingleX export capture also opened this WNote's actual
cached PDF: `FPDF_LoadDocument` returned a non-null handle, `importPages`
returned zero (success), and its caller continued. The run then stopped at
`PenManagerST::loadlibrary` requesting `libSPenFountainPen.so`, before completed
PDF output. This verifies native page import in the host harness, not ink export
or final appearance (receipt SHA-256
`26da043153a62af7d44f2b2d19a7fdf9aecce0991147a550d2f37833f0c2e075`).

A later archived v88 attempt advanced beyond that earlier FountainPen stop. Its
[compact receipt](../../conformance/pdf-paper-singlex-v88-boundary.json),
extracted from the hash-checked event journal, records a non-null
PDFium document, successful import return, all 56 FountainPen initialization
entries and the exact exported factory lookup. After 7,323,017 guest instructions,
the host Unicorn/Mesa run stopped at renderer `0x55248` with LR `0x40afee4`:
the harness rejected a return address that differed from its pinned ClearRT
virtual-call expectation. The original v88 harness is unavailable in the retained
attempt; the journal alone does not prove whether the guard or its caller model
is wrong. The capture did not reach its return sentinel or establish completed
PDF output, ink appearance, or device parity. The [byte-checked caller receipt](../../conformance/pdf-singlex-clear-caller.json)
identifies `0xafee0` in `SPPenCanvasImpl::ClearAllRT` as a virtual call through
slot +56, with return at `0xafee4`; the older `ClearRT` slot call returns at
`0xafd98`. This identifies a valid second call site under the historical
`0x4000000` graphics mapping, but does not by itself validate a host callback.
The original journal records the callback receiver as the attached complete
second framebuffer owner, while the previous clear owner was deactivated. A
safe reconstruction must check the current owner, context and call state rather
than accept the return address alone. A subsequent v90 journal stops earlier
at a separate sub-bitmap guard and does not demonstrate a completed export.
Reconstructing and retaining the executable harness remain prerequisites to
advancing this route.

A published mutation archive (SHA-256
`1f16a027df8f767ecb7387edeb290905e06b44248b4fb78b5fc4f8d200742ffb`)
also completes a fresh-cache native `ConstructLoad` at explicit fixed axis 1812.
Public getters report 3,228 objects and retain object
`e2c775ce-3ece-11ee-b26c-efb5498813c4` at 30° rotation. No export or draw runs in
this reload control; it proves loadable published bytes and the mutation, not
normal `SaveAsFile` completion or PDF appearance (observation SHA-256
`50e927ae1152204902e3578adac2624b9471927cc5c50bf6978794735eec12da`).

A separate loader control executes all 47 PenCommon and 56 FountainPen init
entries, resolves `createPenInst` at FountainPen `0x621ec`, and executes the
PenCommon `BLR x0` at `0x4df78`. Its returned 184-byte object has the source-pinned
primary vptr `0xd0988`. The loader uses a harness-owned module token; this proves
the bounded factory route, not full `createPen`, Android loader behavior or PDF export
(receipt SHA-256 `0eb7c7b2afef8eae206d237485b3da5d8ac7beae9c8e5aeceeb934b01121d33b`).

Those archive probes supply `ConstructLoad` fixed-axis size 1812 and exporter
default height 1527. The app's common portrait PDF open paths instead supply
fixed-axis size 1080 (`DocumentFileUtil.Document`, `TaskMakePdf`). Native
[`LoadHeader_Scale`](vector-retention-findings.md#native-opening-normalizes-the-requested-fixed-axis)
normalizes page/object geometry using the requested and saved page-header axes.
The probes establish the supplied profile, rather than the app's load defaults.

A follow-up with the same 1812 profile measures 116 body lines. Native
`BodyTextContext::HasVisibleText` returns true for the full note and the
`9120–10647` band; the other ten observed page bands are false. Line 112 spans
`[80.5333,9182.8164,952.1088,10347.1309]`, approximately `1812/1080` times the
SDK's flow-space geometry. This executes measurement and visibility without
body/query injection, but does not establish an SDK geometry bug or resolve
and draw the image object.
The event receipt SHA-256 is
`1f2237fc0f7288b913d0fa2260b0fa68dd5c1b750d599ae49c422f0412bc2c6e`.

An app-equivalent 1080 load of the same archive yields a 1080×9162 note,
default pages 1080×1527, density 3, and load scale 1. Its 116 body lines include
line 112 at `[48,5473.2061,567.4821,6167.1689]`, matching the SDK body layout.
The inline image's saved bounds also match the SDK; native drawn bounds add
0.0048828125 vertically. Page-space strokes and PDF-paper records instead
normalize from the saved 1812-wide page header: the first record ends at
885.099304, and native slices start at `0,885,2412,3939,5466,6993,8520`.
The first slice has height 885; the others have height 1527. These are distinct
page and body coordinate domains, not a reason to rescale the SDK body.
This executes loaded-model measurement and queries, without drawing or pixel
proof. The summary receipt SHA-256 is
`2e6067ec4c9c60bae9f76ece3d98345faefbc9623a2222fbfbc681656761b215`.

Across all 3228 strokes, saved bounds, sample coordinates, and pen widths match
`f32(1080/1812)` scaling exactly. Recomputing native selection bounds after
scaling reproduces each of the seven query sequences, including source order.
Derived bounds retain fixed-unit margins; scaling already-expanded bounds is
incorrect. The comparison receipt SHA-256 is
`304afbad7abef675101aabc439062d942efb0f570662d51ee02dc5747b56debb`.

The SDK's explicit 1080 route exports this quiz as seven pages without
diagnostics. MuPDF and PDFium render all seven; the inline PNG's decoded RGB
pixels survive unchanged on page 5, with bounds within 0.00002 points of the
native context rectangle after page offset and physical scaling. The source
PDF page box, text and original image stream also survive. This checks SDK
transport against native model placement, not native export pixels. Consumer
receipt SHA-256:
`62319db0b18ed8f3a4571c36cec8c1199c82d9fc78634bfdba69b685d8721243`.

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

A native LF-plus-image control at width 1080, nominal height 1300 and y=4785
queries `[6084,6085]` and returns height 1382 from line 112's bottom
6167.1689453125. It executes native height selection; segment-list append was
not executed. Receipt SHA-256:
`232d64ac1144ccbba1c167390af2df1da2b6ea77dd9d88a20cc15aa19330b9fa`.

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

The Rust model retains all ordered signed PDF records and version-dependent placement
rectangles, with manifest/source-byte resolution separate from the `CustomPdf`
compatibility summary. The private PDF adapter now imports source pages for the
bounded continuous-note class listed in [rendering support](../rendering-support.md).
General viewer composition and body-aware export remain open; the first-index
summary alone remains insufficient.

The evidence distinguishes three operations:

| Operation | Input needed | Native boundary |
| --- | --- | --- |
| Render PDF paper in a note view | Every resolved PDF record and its placement | Viewer creates placed PDF views |
| Export PDF paper through list mode | First resolved source and page index per note page | Import source pages, then write note overlays |
| Export a continuous note | Ordered PDF records, placements and segment state | Choose vertical segments, import matching source ranges, then write overlays |

Samsung's PDF export does not provide an SVG representation of the imported
paper. Preserving a source PDF page in PDF output and representing that page in
SVG are different capabilities. The current Rust document-to-PDF transport
uses SVG graphics plus retained text for overlays and preserves source pages
through a separate imported-page operation. This bounded implementation does not establish
support for arbitrary PDF paper or for PDF-to-SVG conversion.

Likewise, turning a PDF page into a bitmap would change the retained content
contract, independently of whether note strokes remain vectors. Source content,
note geometry, resource resolution, placement and export transport are separate
data requirements demonstrated by these native routes.

The [native serialization capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-pdfium-create-page.json)
executes PDFium page/form/path generation and saving for a synthetic 200×150
page rotated 90°. Its counterrotated form has BBox `[0,0,150,200]` and placement
`[0,1,-1,0,200,0]`; the emitted PDF renders a vector path. Allocation, formatting,
C locale/time, host libm and the file-write sink are explicit providers; unknown
imports abort. This does not load a source PDF or execute the full Samsung
writer wrapper, and establishes no source CropBox/UserUnit or pixel parity.

The [loaded-source capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-pdfium-source-overlay.json)
executes the real PDFium parser and overlay/save sequence at all four rotations.
Its nonzero MediaBox/CropBox, UserUnit 2 and original vector path survive; native
page dimensions use the rotated CropBox without multiplying by UserUnit. The
form receives literal `4 - rotation`, including 4 for an unrotated source.
This remains a synthetic PDFium API sequence, not the Samsung writer wrapper.

The [scale-three body controls](../../conformance/pdf-paper-body-scale3-native.json)
execute Widget `SetTextScale(3)` before native shaping, layout and page-index
queries. Their ASCII line rectangles and source owners match the SDK for the
admitted nonempty-paragraph cases. Native line-range and substring-visibility
primitives also match the tested boundary bands; the complete BodyTextContext
visibility method does not execute in this control. A missing default font size
produces unscaled empty-paragraph height, while explicit sizes 17 and 50 scale;
a parsed implicit-default body is admitted for one leading, interior, or trailing empty
paragraph; [parsed controls](../../conformance/pdf-paper-parsed-default-native.json)
confirm the unscaled empty-line height and page-index boundaries using the body
constructor and binary text parser. A narrow parsed 24-byte whole-text font span
profile is admitted for one
leading empty paragraph at size 50 or one interior empty paragraph at size 17
or 50. The [parsed span receipt](../../conformance/pdf-paper-parsed-font-span-native.json)
records the native lines, page-index boundaries, and rectangular visibility
controls, and a 20-byte fallback that behaves differently. Its host libc hash
differs from the earlier pinned capture; the prevalence of the 24-byte
encoding in saved notes is unverified.
The [entry controls](../../conformance/native-resolved-entry-facts-51-150.json)
measure direct resolved sizes 51 and 150 on a native table-cell producer, not
through the document scale setter. They cover letters, digits, punctuation,
spaces, wrapping and LF; they do not establish arbitrary fonts or scripts.
Both captures used a host libc with a different digest from the earlier pin.
The density-one baseline geometry agrees with the prior capture, and the
receipt records the affected boundary. The public PDF regression checks line
baselines across source-page cuts. Independent MuPDF and PDFium reads of that
synthetic output extracted 81 characters on the same four pages. Neither check
establishes native export pixels or saved-device parity.
