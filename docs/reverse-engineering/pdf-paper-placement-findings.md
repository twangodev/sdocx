# PDF paper placement and clipping

These findings concern Samsung Notes APK 4.4.45.37, arm64. Native addresses
below are library virtual addresses. The investigation used static disassembly,
dynamic symbol tables and decompiled Java; it did not execute the attachment,
preview or capture paths. Neither the numerical examples nor the presence of a
PDF in a note establishes native appearance parity.

| Inspected library | SHA-256 |
| --- | --- |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenPdfViewer.so` | `e910eed95ce0f519f88cdc95b64ffa77b4593157e5c3a44ff08023b86395ac5d` |
| `libSPenView.so` | `c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenPdf.so` | `cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04` |
| `libSPenPdfiumB.so` | `4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854` |

## Placement is separate from the source PDF page box

The saved `SpenWPage.PDFData.rect` is a destination rectangle in document
coordinates. The source PDF page index and media binding accompany it; the
rectangle does not encode the source PDF's MediaBox or CropBox. The attachment
producers establish this distinction directly, rather than merely through a
field name.

The binary field and its float-to-integer version transition are documented in
[page-background-findings.md](page-background-findings.md#pdf-records-change-rectangle-encoding-at-version-2034).
The four coordinates become native `f32` placement values. The private record
initializer at Composer `0x3664ec` stores file ID/page index at offsets 16/20 and
the four supplied floats at offsets 24/28/32/36 (`0x366548–0x366550`). It performs
no unit conversion. This helper has no separately exported name: objdump labels
it relative to `NotePDFAttachList::adjustObjectTextBox`, which is not the helper's
semantic identity.

### Page-list attachment

`NotePDFAttachList::makePdfList(WPage*, int)` (`0x3660ec`) constructs a record
with rectangle `(0, 0, WPage.width, WPage.height)` (`0x36621c–0x366254`). It maps
the supplied ordinal through the selected-PDF-page array when that array is
present (`0x366174–0x366180`), so note-page order and source page index are not
interchangeable. `bindPDF(int,int)` (`0x365a70`) associates the per-page record
lists with note pages through `WNote::SetPDFData` (`0x365c4c–0x365c5c`).

The destination page dimensions are established earlier by
`NotePDFAttachList::insertPage(int,int)` (`0x365568`):

- In its internal flag-at-offset-320 clear branch, it uses note width and
  `source_height / source_width * note_width` to build inserted page heights
  (`0x36587c–0x365928`). If a recovered note page exists and has positive height,
  that height is used instead (`0x3658a4–0x3658c8`).
- In the flag-set branch it gets default note dimensions, collects maximum
  integer source/recovered widths and heights, and chooses the **larger** of
  `default_width / max_width` and `default_height / max_height`
  (`0x36569c–0x3657bc`). It applies this shared scale to integer source rectangles
  before `WNote::InsertPage` (`0x3657f0–0x365864`). The `fcsel` at `0x3657bc`
  establishes that larger-ratio choice; it is not an inferred conventional
  fit-to-box rule. A zero maximum dimension leaves its ratio at `1.0`.

`Rect::Scale(float)` in `libSPenBase.so` (`0xb0bd8`) converts the four signed
integer coordinates to `f32`, multiplies each by the supplied scale, and
truncates them back to `i32` (`0xb0bdc–0xb0be8`). The flag-set branch therefore
truncates source dimensions before scaling and truncates the scaled results
again. This investigation does not assign a public name to the internal flag.
Neither producer branch calls `GetDocumentDensity` or introduces a fixed 72/96
conversion.

### Continuous single-page attachment

`NotePDFAttachSingle::makePdfList()` (`0x366b4c`) copies existing PDF records and
finds the largest scaled bottom coordinate:
`existing.bottom * (f32(WPage.width) / existing.width)`
(`0x366c14–0x366c3c`). This establishes an extent, not a new source PDF crop box.

`getPositionToAttachPdf(float)` (`0x366cb4`) can add whole default-page heights
when the page contains text or physical objects (`0x366cf8–0x366d3c`). It first
subtracts `default_height` from `WPage.height` as integers, then converts that
result to `f32`. If this target exceeds the supplied extent, it subtracts the
extent in `f32`, divides by `f32(default_height)`, and converts upward to an
integer count. It multiplies that count by `default_height` as integers, converts
the product to `f32`, and adds it to the original extent. Otherwise it adds zero.
The extent's fractional or nonzero remainder is preserved until final
truncation to `i32` at `0x366d40`; this is not absolute page-boundary alignment.

`bindPdf()` (`0x366d54`) gets each source page's dimensions, rejects an empty
source rectangle, and scales its height by `WPage.width / source_width`
(`0x366e0c–0x366e60`). The width ratio is evaluated before the height
multiplication. It appends records with:

```text
left   = 0
top    = running document y
right  = WPage.width
bottom = top + source_height * (f32(WPage.width) / source_width)
```

The dimensions and running sum use `f32`. Empty source pages leave the running
position unchanged (`0x366e28–0x366e30`). A separate trailing-empty-page boundary
can suppress record insertion (`0x366e64–0x366e74`); source iteration and the
running position still advance for a nonempty page. The page can grow to the
resulting bottom, truncated to integer height (`0x366f70–0x366f9c`), and the
assembled list is installed through `WPage::SetPDFData` (`0x367014–0x36701c`).

Thus a single stored note page can contain many independently indexed PDF page
placements. Their vertical positions are actual document geometry, not a list
of discrete note-page IDs. The two attachment families produce rectangles from
document dimensions and source aspect ratios; source PDF points are not saved
as unchanged destination coordinates.

## Source dimensions include PDF box and rotation handling

`PdfImporter::GetPageSize(int)` in `libSPenPdf.so` (`0x69738`) returns two floats,
width and height, or zeros when its file path is absent/empty. Its Pdfium
implementation (`0xa3bd8`) subtracts the endpoints of the engine's source
rectangle (`0xa3bf4–0xa3bf8`). `PdfiumImpl::GetPageSize(int)` (`0x88b28`) calls
`FPDF_GetPageSizeByIndexF` and exposes a zero-origin rectangle with the returned
dimensions (`0x88c0c–0x88c24`). No document-density conversion occurs in these
getters.

In `libSPenPdfiumB.so`, `CPDF_Page::GetPageAttr` (`0x472ff0`) first looks for a
direct page attribute, then follows Parent dictionaries with a visited-pointer
loop guard (`0x47309c–0x4731cc`; Parent string relocation `0x655d10`).
`GetBox` (`0x4732ac`) normalizes each coordinate pair with minimum/maximum
selection (`0x473334–0x47334c`), so inverted raw box endpoints are normalized
before validity checks. `UpdateDimensions()` (`0x472c10`) obtains MediaBox and
CropBox through this lookup using string relocations `0x655d20` and `0x655d28`.
An invalid MediaBox becomes `(0,0,612,792)`, from the constant at `0xf9a30`.
A valid CropBox is intersected with MediaBox
(`0x472d1c–0x472d24`); an invalid CropBox falls back to MediaBox. The effective
box and its width/height are stored at `0x472d34–0x472d40`.
`GetPageRotation` (`0x473664`) uses the same inherited attribute lookup for
Rotate (string relocation `0x655d50`), defaults an absent value to zero, and
divides the integer degrees by 90 before masking with 3 (`0x4736dc–0x473700`).
Quarter-turn rotations 1 and 3 swap the returned dimensions
(`0x472d84–0x472dbc`).
`FPDF_GetPageSizeByIndexF` (`0x5a84d8`) copies those dimensions at
`0x5a855c–0x5a8560`.

Consequently the importer sizes represent the rotation-adjusted effective PDF
page box, not necessarily the raw MediaBox dimensions. The original box origin
and rotation are not extra coordinates in the saved destination rectangle.
Vector PDF export has separate object-matrix functions; this preview/import
trace does not establish their transformation contract. Its separate page-copy
and overlay routes are documented in
[pdf-paper-export-findings.md](pdf-paper-export-findings.md).

## Interactive placement and local clipping

Composer `PageBackgroundView::createPdf()` (`0x3e6578`) iterates every saved PDF
record (`0x3e66e8–0x3e670c`), reusing an existing `PDFView` or creating one.
It computes this view position at `0x3e6734–0x3e679c`:

```text
scale = WPage.width / (saved.right - saved.left)
view position = (saved.left, saved.top,
                 saved.right * scale, saved.bottom * scale)
```

The left and top remain unscaled. This is the literal caller behavior, not a
uniform scaling of all four coordinates. `View::SetPosition(float,float,float,float)`
in `libSPenView.so` (`0x70b8c`) confirms that these arguments are stored as
left/top/right/bottom, rather than position plus width/height
(`0x70bcc–0x70bdc`). This distinction matters for nonzero origins and changed
page widths; no execution here establishes the appearance of those cases.

Both reused and newly created views enable `SetClipToBounds(true)`
(`0x3e67a0–0x3e67a8`, `0x3e68c0–0x3e68c8`). `PDFView::Set` in
`libSPenPdfViewer.so` (`0x9f0f0`) copies record data separately from the caller's
view position. `PDFView::onDraw` (`0x9ff34`) draws its document bitmap into local
view bounds and its screen bitmap into a retained local screen rectangle.

The actual clip scope belongs to the shared `View::Draw` wrapper in
`libSPenView.so` (`0x6fd7c`). It saves canvas state, applies the enabled bounds
clip as `(0,0,ceil(width),ceil(height))` for positive dimensions
(`0x6fdb4–0x6fe48`), invokes the background, content, child-dispatch and foreground
drawing callbacks, then restores state (`0x6fec4–0x6fed4`). The scrollbar callback
follows the restore (`0x6fed8–0x6fef8`). Nonpositive bounds bypass that clip call;
the wrapper still calls the content callbacks. This is not a document-wide clip
that leaks into later note objects.

`PageBackgroundView::loadPDF(Rect*,bool)` (`0x3edb2c`) loads only placed views
intersecting the requested document region (`0x3edb90–0x3edbac`) and whose record
is available through `WPage::IsPDFAvailable` (`0x3edbb0–0x3edbcc`). Unavailable
entries activate a coedit placeholder route. `PDFView::loadScreenCache`
(`0x9f3bc`) rejects requested regions whose truncated width or height is below
one, expands an admitted region through `extendRectByAreaRatio(...,2)`, and
requests a clipped screen load (`0x9f4b4–0x9f59c`). These are interactive bitmap
cache policies, not saved-format fields or evidence that a vector export must
flatten PDF paper.

## Capture distinguishes whole placements from partial regions

`NoteCapturePage::drawTemplate` (`0x32e0d8`) bypasses ordinary template drawing
when `WPage::HasPDF` is true. The separate `drawPdf` (`0x32e3e0`) truncates the
requested float rectangle to integer coordinates and invokes
`NotePDFCapture::DrawPDF` (`0x331ec4`).

That function loops over every saved placement and skips non-intersections
(`0x331f84–0x331fa0`). It compares the requested integer rectangle with the saved
rectangle converted to integers (`0x331fa4–0x331fbc`):

- Equality selects the whole-source-page bitmap helper (`0x331fc0–0x331fe8`).
  The bitmap is drawn into the saved placement under the supplied matrix inside
  a save/restore scope (`0x332008–0x33205c`).
- Otherwise it intersects the requested region with the saved placement and
  subtracts the saved left/top to produce placement-local coordinates
  (`0x332068–0x332094`). It invokes `getBitmapClip` (`0x332098–0x3320c8`) and draws
  the resulting region bitmap using the computed region-relative offset
  (`0x33210c–0x332178`). That branch is not an identical replay of the whole-page
  matrix route.

`getBitmapClip` (`0x33253c`) rejects regions whose truncated width is below one
or whose truncated height is nonpositive (`0x33259c–0x3325bc`). File-open
failure, missing source page and renderer failure return a null bitmap. The
outer loop skips null bitmaps and continues; after iterating a nonempty list it
returns true (`0x332184–0x33219c`). A successful aggregate capture return is
therefore not proof that every placed PDF loaded or drew.

At the lower preview renderer boundary, `PDFEnginePdfium::RenderPage` with
source and destination rectangles (`libSPenPdf.so`, `0x6cf50`) derives one
`f32` scale from destination width divided by source width
(`0x6cf80–0x6cf94`); destination height is not an independent scale. Its
`PdfiumImpl::RenderPage` target (`0x892e4`) derives bitmap dimensions by
truncating the absolute scaled source dimensions (`0x89348–0x89370`) and rejects
width below one or nonpositive height. These are resolution and failure
boundaries of the inspected bitmap route.

## Consequences for the current Rust model

[`parse_page_properties`](../../crates/sdocx/src/page.rs) currently reads every
PDF record but discards its media binding and rectangle and keeps only the first
source page index as `PageTemplateSource::CustomPdf`. The parser does not reject
the bytes; its decoded semantic model loses resource identity, later indices
and destination geometry. Source PDF bytes alone cannot recover these choices:
selected source indices, attachment offsets and document page dimensions are
independent inputs to the native producers above.

[`page_background.rs`](../../crates/sdocx/src/page_background.rs) rejects this
template source. The existing seven-document inventory contains two PDF-backed
notes; see [rendering-corpus-findings.md](rendering-corpus-findings.md). That
inventory confirms local resource presence but does not establish rendering or
the unusual nonzero-origin/changed-width view cases.

The demonstrated boundaries are distinct: saved placement, effective source PDF
box, interactive clip/cache region, and capture region. The static caller traces
contain no general finite-value validator before placement width division.
Empty dimensions have narrower checks at specific producer/cache/renderer
boundaries. Those observations describe this APK's behavior; they do not justify
accepting unbounded or nonfinite geometry in a Rust SVG emitter.
