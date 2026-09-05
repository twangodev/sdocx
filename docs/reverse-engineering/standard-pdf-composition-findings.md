# Standard PDF export selection and composition

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, using the Java classes in the APK
and ARM64 `libSPenComposer.so` and `libSPenPdf.so`. The APK SHA-256 was
rechecked against the knowledge-base index. The extracted Composer, Drawing
and Pdf libraries also match their archived bytes. JADX fallback mode was
run directly on this APK to verify the constructor instructions independently
of its reconstructed Java control flow.

The public Standard PDF choice reaches `NotePDFExporterRasterListX` for
list-page notes. It does not select the native class named
`NotePDFExporterVectorList`. These implementations have different collectors,
bitmap writers and batch-finalization behavior. The earlier
[native vector stroke findings](native-pdf-stroke-findings.md) describe a
real native implementation, but its class name alone does not connect it to
the public Standard option.

## Java options and the native factory

`com/samsung/android/sdk/composer/pdf/SpenNotePdfExport.java` declares:

| Public value | Constant | Constructor flags: editable, system font |
| ---: | --- | --- |
| 0 | `PDF_EXPORT_TYPE_IMAGE` | true, true |
| 1 | `PDF_EXPORT_TYPE_PDF` | false, false |
| 2 | Deprecated `PDF_EXPORT_TYPE_RASTER` | true, true |
| 3 | Deprecated `PDF_EXPORT_TYPE_VECTOR` | false, false |

The `(Context, int)` constructor at lines 176–191 computes both flags from
`value == 0 || value == 2`. Its `Native_init` call at line 188 passes native
export type **0** regardless of the public value. The fallback instruction
output confirms the zero argument immediately before the companion call;
the companion and synthetic accessor forward it without translation.

`TaskMakePdf.PdfShareType`, in
`com/samsung/android/support/senl/nt/composer/main/base/presenter/task/TaskMakePdf.java`,
maps `NOTES_COMPAT` to 0 and `STANDARD` to 1 at lines 30–32. The task passes
that value to the constructor at line 230. Thus these UI choices select
different options within the same native factory type.

Composer `NotePDFExportGlue::Native_init`, `0x3149e8`, packs the arguments
into the eight-byte `PDFExporterOption` at `0x314a1c`–`0x314a3c`:

| Byte offset | Value derived by this JNI entry |
| ---: | --- |
| 0–3 | Native export type |
| 4 | Editable boolean |
| 5 | System-font boolean |
| 6 | Inverse of editable |
| 7 | Inverse of editable |

The factory call at `0x314a48` reaches `CreateNotePDFExporter`, `0x360610`.
As previously established, native type 0 constructs `NotePDFExporterRaster`
and native type 1 constructs `NotePDFExporterVector`. The normal Java
constructor above requests only the former. This does not prove that the
latter has no other caller.

`NotePDFExporterRaster` saves the option at member 120 at `0x355bd0`.
Its `ExportFile`, `0x355d68`, reads page mode and option byte 6, now member
126, to choose the delegate:

| Page mode | Option byte 6 false | Option byte 6 true |
| ---: | --- | --- |
| 0 | `NotePDFExporterRasterList`, constructor call `0x355ed8` | `NotePDFExporterRasterListX`, call `0x355e1c` |
| 1 | `NotePDFExporterRasterSingle`, call `0x355ef4` | `NotePDFExporterRasterSingleX`, call `0x355ea0` |

Consequently Standard uses the X implementation for both page modes. The
composition below traces the list-page variant; single-page segmentation
must be checked separately before claiming the same complete behavior there.

## Note preparation and layer scope

The ordinary `TaskMakePdf` path reopens the saved note through
`DocumentFileManager.open`, resets selected parameters and passes the note
to the exporter. A separate created-note path uses an existing share note.
`resetParameterInNote`, lines 130–135, copies a missing default page height
and removes custom sticky-memo objects. Immediately before constructing the
exporter, line 228 copies PDF-reader mode. These inspected task methods do
not explicitly flatten physical layers or sort their objects.

In native code, `NotePDFExporter::SetDocument`, `0x3603e4`, stores the note
pointer. The raster dispatcher passes it to its chosen delegate through
virtual slot 16 at `0x355f24`. The X delegate's `SetDocument`, `0x357570`,
stores the same pointer at member 80, updates document width/density, and
calls `NoteTextManager::SetTextSectionData(true)` at `0x357608`.

`SetTextSectionData`, `0x3a253c`, obtains body text, measures a body-text layout
and updates its page text ranges through `SetTextSectionData(int,int,int)`
at `0x3a26bc`. Its inspected body performs text-section preparation, without
an explicit physical-layer merge.

The [saved physical-layer investigation](page-layer-selection-findings.md)
traces the remaining load and list setup. Model `PageImplBase::LoadLayer`
assigns the serialized current-layer index to the page's object handler at
`0x346c78`–`0x346c8c`. X list initialization obtains the note's existing
page pointers at `0x35a488`; WDoc `WNote::GetPageList` and Base
`ArrayList::Copy` copy the pointer array without cloning or flattening pages.
The inspected capture setup preserves those pointers and the layer selection.

`NotePDFExporterRasterListX::getPageObjectList`, `0x35ad54`, gets a page from
the exporter's array, builds its full-page rectangle and calls
`WPage::FindObjectInRectIntersect` at `0x35adbc`. It passes object-type mask
`0x00ffffff` and the supplied render-layer filter. The
[previously traced WPage/Model path](capture-composition-findings.md#clone-state-and-collection-boundaries)
uses the currently assigned physical layer's object manager and existing
list order. It does not call the replay-sorted all-layer collector. Together
with the loader and setup evidence, this supports selecting the saved current
physical layer when constructing the SDK's semantic page.

## Standard list-page paint sequence

The X delegate stores the option at member 200 at `0x357480`. Standard's
option byte 7 therefore makes member 207 true.
`NotePDFExporterRasterListX::exportNotePage`, `0x35aaf4`, reaches
`updatePage(page, false, false)` at `0x35ab44` after creating its PDF page.
On the successful ordinary update path:

| Composer call site | Operation | Render selection / blend |
| --- | --- | --- |
| `0x35b0a8` | Add background |
| `0x35b0b8` | Add body text |
| `0x35b0c8` | Add ordinary page objects | Filter 1; bitmap batches use PDF Normal |
| `0x35b0e4` | Add highlighter pass | Filter 2; bitmap batch uses PDF Darken |
| `0x35b0f4` | Add tape pass | Filter 4; bitmap batch uses PDF Normal |

The last two calls are gated by member 207 at `0x35b0cc`–`0x35b0d8`.
`addPageObject(int)` selects filter 1 when it is true and filter 7 otherwise
at `0x35b538`–`0x35b554`. This prevents double-collecting the later passes
under the Standard option.

`addHighlighter` passes filter 2 at `0x35ac6c`; `addTape` passes filter 4 at
`0x35b63c`. These are the same render-layer bit masks established for page
capture. They select objects by native render-layer matching, including the
stroke top-layer override. The method names do not narrow the collector to
one pen name or to stroke objects only.

## Ordinary objects retain interleaving and flush the tail

`addPageObject(int, ObjectList*)`, Composer `0x35b770`, walks the supplied
list and checks `ObjectBase::IsVisible` at `0x35b888`. Hidden entries advance
without drawing. Visible entries are handled in source-list order:

- Type 3 flushes the accumulated bitmap list at `0x35b8d4`, clears it, and
  calls `ObjectImagePDFWriter::WriteObject` at `0x35b90c`.
- Type 2 flushes at `0x35b924`, clears it, and calls `writeObjectTextBox`
  at `0x35b940`.
- Other types accumulate in the bitmap list through `ObjectList::Add`
  at `0x35b958`.
- After the iterator ends, `0x35b99c` explicitly exports the remaining
  bitmap list before clearing and destroying it.

All three bitmap-flush call sites pass PDF blend value 0. The bitmap helper
returns without writing an empty list. Thus an ordinary list containing
stroke, image, stroke has an explicit final flush in this implementation.
The unresolved stroke-counter tail in `NotePDFExporterVectorList` is a
different loop and must not be used to infer a Standard PDF defect.

## Highlighter blend reaches the PDF image object

`addHighlighter` supplies blend value 4 at `0x35acc8` before calling
`writeObjectListAsBitmap` at `0x35accc`. This is PDF's blend enum; it is not
the graphics paint enum where capture darken was value 16.

The value is retained through these stages:

| Library/address | Operation |
| --- | --- |
| Composer `0x35ae0c`, `0x35ae94`–`0x35ae98` | Save the blend argument and forward it to `ObjectPDFWriter::WriteObjectListAsBitmap` |
| Composer `0x37c3dc`, `0x37c634`–`0x37c648` | Retain that argument and pass it to `PDFWriterUtil::WriteBitmap` |
| Composer `0x383028`–`0x383030` | Pass the value to PDF paint virtual slot 40 |
| PDF relocation `0xb0158` | Resolve slot 40 to `PDFWriter::PDFPaint::SetBlendMode`, `0x665e4` |
| PDF `0x665f4`–`0x665f8`, relocation `0xb06f8` | Forward to `PDFEnginePdfium::SetBlendMode`, `0x73350` |
| PDF `0x73354` | Store the enum at native paint member 12 |
| PDF `0x82eac`–`0x82ebc` | `PdfiumImageHandler::DrawImage` reads that member, resolves its name and calls `FPDFPageObj_SetBlendMode` |

`PDFUtil::GetPDFBlendMode`, PDF `0x68cb4`, indexes the pointer table at
`0xb1b18`. Relocation `0xb1b18` names `pdfium::transparency::kNormal` for
value 0; relocation `0xb1b38` names `kDarken` for value 4. The table independently
confirms both names. The inspected highlighter method supplies 4 directly,
without the dark-background Lighten selection used by `NoteCapturePage`.

The blend applies to the exported bitmap containing the collected top-layer
objects. It is not evidence for applying Darken separately to each primitive
inside that bitmap. Pen drawing, pixel alpha, overlapping highlighter strokes
and PDF composition remain distinct operations.

`ObjectPDFWriter::WriteObjectListAsBitmap`, `0x37c3bc`, creates its bitmap at
`0x37c4c0` and invokes ordinary `ObjectDrawing::DrawObjectList` at `0x37c554`.
It then computes object bounds and calls `WriteBitmap` at `0x37c648`.
That helper creates a PDF paint, sets blend and alpha, saves a bitmap image,
and calls writer slot 168 at `0x38317c`. PDF relocation `0xb00b0` identifies
that slot as `PDFWriter::DrawImage`. This is a separate PDF insertion route
from the `PdfImageAdapter` used by `NotePDFExporterVectorList`.

## Consequences for SDK work

Standard PDF evidence supports explicit base, top and masking passes,
interleaving within the ordinary pass, and a Darken-composited top batch.
The SDK still needs an ordered representation and native pen behavior before
it can reproduce this pipeline. Moving every stroke ahead of text and images,
sorting by replay timestamp, or applying a single per-stroke opacity would
not reproduce the confirmed sequence.

The native exporter name is also insufficient metadata for new fixtures.
Record the actual UI choice, page mode and background kind for each pair.
Useful comparisons include Standard and Notes-compatible exports of the
same light/dark/PDF-background page, with interleaved text boxes, images,
ordinary strokes, highlighters and masking objects. Single-page segmentation,
editor changes to layer selection and pen-level blending remain open APK investigations;
visual fidelity still requires new captured pairs.
