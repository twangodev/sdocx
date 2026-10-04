# Saved page backgrounds and template sources

These findings concern Samsung Notes APK 4.4.45.37. Java paths below are relative
to the decompiled `sources/` directory. Native addresses are virtual addresses in
the APK's arm64 libraries. The native drawing conclusions are from static
disassembly and constant/relocation tables; no new native execution or visual
capture establishes image/PDF-background appearance parity here.

The inspected ARM64 libraries have these SHA-256 hashes:

| Library | SHA-256 |
| --- | --- |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenView.so` | `c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8` |
| `libSPenGraphics.so` | `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621` |

## Serialized fields and defaults

`n1/u.java:1007–1060` writes background fields in ascending flexible-field bit
order. `collectVariables` at line 202 identifies the obfuscated members by their
original roles. The matching reader at lines 332–373 establishes omission
behavior for the fields it resets; PDF records and template type are updated
only when present.

| Bit | Field | Encoding | Omission behavior |
| ---: | --- | --- | --- |
| 2 | Template URI | `u16` UTF-16 code-unit count, followed by UTF-16LE | `null` |
| 3 | Background-image media binding ID | Little-endian `i32` | `-1` |
| 4 | Background-image mode | Little-endian `i32` | `0` |
| 5 | Background color | Little-endian ARGB word | `-1` |
| 6 | Background width | Little-endian `i32` | `0` |
| 7 | Background rotation | Little-endian `i32` | `0` |
| 8 | PDF data list | `u16` count, then the records described below | Writer omits an empty list; reader updates only when present |
| 9 | Template type | Little-endian `i32` | Writer omits type `0` |

`f2/a.java:118–140` and `295–366` establish the primitive encodings. The Java
string reader treats the 16-bit count as signed: `0` is empty and `-1` is null.
The page writer emits a URI only when its member is non-null, so the null string
sentinel is separate from ordinary omitted-field behavior.

These are separate properties. A template type is not sufficient to resolve an
image asset, and an image's media binding is not its template URI.
The archive's media binding contract is described in
[image-findings.md](image-findings.md).

The saved width is an image reference width. Model `GetBackgroundRatio`,
`0x344378`, returns page width / saved image width in f32, or 1 when the
saved width is zero (`0x34437c–0x344398`). Background image setters supply
page width to that image state (`0x343e44–0x343e54`, `0x344118–0x344128`).
This establishes a logical page-width reference, not generated line thickness
or original bitmap pixel width; a universal physical unit remains unproven.

### PDF records change rectangle encoding at version 2034

Each bit-8 record contains:

1. A little-endian `i32` media binding ID.
2. A little-endian `i32` PDF page index.
3. Four rectangle values: left, top, right and bottom.

The rectangle is **four `f32` values for page format versions below 2034**, and
**four `i32` values for versions 2034 and later**. This branch is explicit in
`n1/u.java:356–366`; the older branch calls `f2/a.java:129–135`, which reconstructs
floats with `Float.intBitsToFloat`.

The current Java writer at `n1/u.java:1042–1051` constructs an Android `Rect` by
casting each `RectF` coordinate to `int`, then writes those integers. Fractional
coordinates therefore truncate toward zero on this write path. Both rectangle
encodings occupy 16 bytes, but interpreting all versions as floats would be
incorrect. The synchronization XML path also writes `pageIndex`, a media hash,
and integer `pdfRect` (`n1/u.java:237–248`).

`SpenWPage.PDFData` exposes an index, rectangle, password, bound file ID and file
path (`com/samsung/android/sdk/pen/worddoc/SpenWPage.java:458–471`). The password
and file path are not additional fields in this bit-8 binary record. Their
presence on the API object does not establish serialization beside the rectangle.

## Template identifiers and activation

`SpenWPage.java:35–55` defines these identifiers:

| ID | Native API name | Native template drawing factory |
| ---: | --- | --- |
| 0 | None | No generated drawing |
| 1–3 | Narrow, medium, wide line | `LineTemplateDrawing`, variants 0–2 |
| 4–6 | Narrow, medium, wide grid | `GridTemplateDrawing`, variants 0–2 |
| 7–9 | Narrow, medium, wide dot | `DotTemplateDrawing`, variants 0–2 |
| 10 | Todo | `TodoTemplateDrawing` |
| 11 | Oxford paper | `OxfordTemplateDrawing` |
| 12 | Custom | No generated drawing |
| 13 | Weekly | `WeeklyTemplateDrawing` |
| 14 | Monthly | `MonthlyTemplateDrawing` |
| 15 | Manuscript | `ManuScriptTemplateDrawing` |
| 16 | PDF | Outside generated drawing factory's admitted range |
| 17 | Dynamic | Outside generated drawing factory's admitted range |

The factory is `TemplateDrawingFactory::CreateTemplateDrawing` at
`0x3f3e7c`. It admits IDs 1–15 using the byte jump table at `0x20d231`; the custom
ID branches directly to the null return. Constructor vtable relocations at
`0x5a1dc8–0x5a1e08` identify the classes selected by the branches. The factory
alone does not establish how dynamic templates are expanded or saved.

Application activation distinguishes the backing sources:

- `NoteManager::setCustomTemplate` at
  `com/samsung/android/support/senl/nt/composer/main/base/model/composer/NoteManager.java:622–633`
  sets type 12, image mode, background image by media ID, and template URI.
- `NoteManager::setPdfTemplate`, lines 648–652, sets type 16 and the entire
  `ArrayList<SpenWPage.PDFData>`.
- The ordinary template setters at lines 789–795 and 909–917 independently set
  type, mode, image binding and URI. The media-ID overload skips image rebinding
  when the supplied ID is `-1`.

Consequently an unsupported template cannot generally be reconstructed by
inventing geometry for its numeric ID. Some IDs activate resources or separate
document content instead of the built-in drawing factory.

## Image modes and tiling

`SpenWPage.java:29–32` declares center = 0, stretch = 1, fit = 2 and tile = 3.
`NoteManager::getImageMode`, lines 87–89, returns tile for `mIsSingleMode` and fit
otherwise. `DocumentInitializer::setPageBackground` sets these properties
separately. Its new-page caller supplies tile for a single page of twice the
default height, or fit for two pages of the default height
(`com/samsung/android/support/senl/nt/model/document/DocumentInitializer.java:41–47,73–78`).

`BackgroundDrawing::SetBackgroundImageMode` (`0x32d470`) maps modes 1/2/3 through
the three-word table at `0x1fb10c`, whose values are 1/0/0, into the drawable's
gravity field. Mode 0 and other values select gravity 6. The independent tile
flag is true only for mode 3. The same translation occurs in page view setup
(`PageBackgroundView::SetWPage`, `0x3e6420`, table `0x20d0c8`) and page capture
(`NoteCapturePage::drawTemplate`, `0x32e0d8`, table `0x1fb390`). The gravity numbers
are internal drawable values, not additional saved modes.

View `BitmapDrawable::update`, `0x8bff0`, establishes the rectangle mapping:
gravity 1 stretches the full source into bounds (`0x8c20c–0x8c214`);
gravity 0 fits by the smaller width/height scale and centers the result
(`0x8c218–0x8c270`, `0x8c41c–0x8c458`). Gravity 6 normally centers the
unscaled image (`0x8c2b0–0x8c308`), but when **both** source dimensions exceed
bounds it replaces the destination with the complete bounds (`0x8c30c–0x8c34c`).
CENTER therefore does not universally mean an unscaled crop. Tile axes bypass
gravity (`0x8c03c–0x8c0f4`); the single paint tile-mode setter sets both axes
(Graphics `0xbdb8c`). These calculations use native f32 fused margin operations.
`BackgroundDrawing::Draw`, `0x32d3f8`, clips to (0,0,ceil(width),ceil(height))
before drawing (`0x32d42c–0x32d450`); Graphics SPCanvas relocations
`0xd0308/0xd0310/0xd0318` resolve its Save/Restore/SetClipRect slots.

`BackgroundDrawing::UpdateBitmap` (`0x32d4a4`) receives the bitmap and target
width/height as `f32`. With positive dimensions it retains ceiling-rounded
integer clip dimensions, creates a `BitmapDrawable`, installs the selected
gravity, and invokes the tile adjustment before setting final drawable bounds.

Tile adjustment at `0x32d5ac` creates an intermediate bitmap with integer
dimensions `trunc(target_width)` by `trunc(target_width * 4 / 3)`, draws the
source drawable into it, replaces the drawable with that bitmap, and calls
`BitmapDrawable::SetTileMode` with value 2. The non-tile branch sets value 0.
This is not merely repetition at the original asset's pixel size. It introduces
a page-width-derived 3:4 intermediate tile before repetition. Native filtering
and exact edge coverage have not been measured in this investigation.

## Drawing callers and PDF-backed paper

`NoteCapturePage::drawTemplate` first tests `WPage::HasPDF`, then
`HasBackgroundImage`, then the generated-template route (`0x32e184–0x32e268`).
Its image route resolves `GetBackgroundImagePath`, loads a bitmap, configures
image mode, and supplies the page width/height to `BackgroundDrawing::UpdateBitmap`
(`0x32e19c–0x32e2b8`). URI text is not the direct bitmap load argument in this
route.

The interactive view follows the same resource boundary:
`PageBackgroundView::setBackgroundImage` (`0x3ed8bc`) resolves the bound image
path and requests `BitmapCacheLoader::RequestLoad`. Its loaded-bitmap callback
at `0x3ede7c` supplies the view width/height to `BackgroundDrawing::UpdateBitmap`.
Resource availability also has a separate coedit placeholder route; a saved
binding is not proof that the image bytes are locally available.

Saved rotation has a concrete consumer on a separate Model bitmap-getter route.
With an attached context, `PageImplBase::GetBackgroundImage` passes current
note rotation minus saved image rotation to `GetImage` (`0x344324–0x344330`);
without a context it passes zero (`0x344334–0x34433c`). The clone getter uses
the same subtraction. On the uncached path `GetImage` loads the resolved image
then may rotate its pixels (`0x2b0a74–0x2b0afc`); its cached branch returns the
existing bitmap (`0x2b0a44–0x2b0a70`). Base `CreateRotatedBitmap`, `0xa6014`,
reduces the angle modulo 360 and delegates quarter turns to pixel-buffer
permutation (`0xa6048–0xa6068`, `0xa5950–0xa5aac`), exchanging dimensions
for 90/270 degrees. This establishes no arbitrary SVG rotation pivot.
The inspected Composer preview/capture image chains above and older image
exporter (`BackgroundPdfExporter::exportBackgroundImage`, `0x3471a0–0x34742c`)
load the resolved path directly and supply target extents; they do not fetch
this saved rotation/ratio or invoke that rotated-bitmap getter. This is a bound
on those chains, not proof that indirect or external consumers ignore the fields.

Template URI is separate from that resolved image path: Model `SetTemplateUri`
stores page impl `+0x40` (`0x3438cc`), while `GetBackgroundImagePath` reads
image state through impl `+0x70` (`0x34436c`). The Java media-ID overload can
change URI/type while preserving an existing image binding when given -1
(`NoteManager.java:909–917`). URI is not a file-load fallback in the inspected
Composer branches. Generated paper remains generated geometry even when its
native preview caches a bitmap; an original background image remains image
content. Neither backing source substitutes for the other.

`PageBackgroundView::createPdf` (`0x3e6578`) obtains the PDF data list and iterates
its entries (`0x3e66e8–0x3e670c`). Each entry supplies a PDF view with its
placement, document identity and bounds clipping (`0x3e673c–0x3e67c0` and the
corresponding new-view branch). `loadPDF` (`0x3edb2c`) checks each view's
intersection with the requested region and `WPage::IsPDFAvailable` before loading
it. A page can therefore hold multiple placed PDF records; the first PDF index
does not describe that composition.

The older native vector export boundary is separate.
`NotePDFExporterVectorList::exportBackground` (`0x361b1c`) bypasses
`BackgroundPdfExporter` when `WPage::HasPDF` is true. For non-PDF paper,
`BackgroundPdfExporter::ExportBackground` (`0x346e9c`) emits background color
first, then chooses the image route if present, otherwise the generated template
route. These observations identify caller branches; they do not establish parity
with every public Standard export path.

## Current Rust retention and rendering limits

[`page.rs`](../../crates/sdocx/src/page.rs) retains the template URI, image ID,
mode, width and rotation as optional values. Unlike the Java in-memory model,
these preserve omission separately from a present default value. The raw words
are exposed as `u32`; native `-1` image IDs appear as `u32::MAX`.

For PDF records, `parse_page_properties` consumes every entry, retains **only the
first page index**, and skips every media ID and rectangle. Remaining page
indices are also discarded. A nonempty PDF list overrides `Page.template` with
`CustomPdf { page_index }`. The decoded model is therefore insufficient to
resolve the PDF resource or reproduce multiple-record placement. Neither the
decoded page nor `StoredPage` exposes these PDF fields as typed data.

Without a PDF record, any nonzero raw template ID is currently marked `BuiltIn`,
including custom/PDF/dynamic IDs. That source label is a parser classification,
not proof that the native drawing factory generates it.

[`page_background.rs`](../../crates/sdocx/src/page_background.rs) admits only
line IDs 1–3 and dot IDs 7–9, with known native document dimensions/orientation
and zero background rotation. Any nonempty URI or image ID other than
`u32::MAX` rejects the template route before template selection. Unsupported
backgrounds keep the solid background and produce `UnsupportedPageTemplate`.
Mode and width do not alter supported built-in spacing. Image/PDF-backed paper,
grids, calendar/worksheet patterns, and rotated templates are not rendered.

The independently recovered line/dot density and spacing rules, visual evidence
and zoom-dependent limits remain in
[shapes-dot-calibration-findings.md](shapes-dot-calibration-findings.md).
The image reference-width and Model bitmap-orientation contracts above do not
establish rotated generated-template geometry or every consumer of these fields.
Filtering, exact edge coverage and image-backed SVG/PDF appearance parity remain
unmeasured; the Composer and Model image routes retain their separate scope.
