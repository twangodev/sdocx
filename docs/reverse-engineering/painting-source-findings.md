# Painting source attachments and replay

## Evidence boundary

The archive and replay findings are static traces in Samsung Notes 4.4.45.37.
A bounded native stroke-reducer capture is identified separately below. No `.spp`
was generated, loaded or replayed during this investigation. These findings do
not establish complete SVG or brush appearance parity.

The APK is
`com.samsung.android.app.notes_4.4.45.37-444537000_minAPI29(arm64-v8a,armeabi-v7a)(nodpi)_apkmirror.com.apk`,
SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The extracted ARM64 libraries were compared byte-for-byte against the APK entries.

| APK library | SHA-256 |
| --- | --- |
| `lib/arm64-v8a/libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `lib/arm64-v8a/libSPenPaintingCompat.so` | `fb2f8cd46c45cc4b6ef79f7e7d45c7eac9f12583bfc70ff48e1244f9c861d4a8` |
| `lib/arm64-v8a/libSPenPaintingCore.so` | `56b386228e9b4217a08e16afd7d8f65482bc6e66e4c76f0951a640e1a703c3fa` |

Native addresses below are ELF virtual addresses. Decompiled Java paths are
relative to `scratch/apk-analysis-decompiled/sources/`.

## The app saves an editable source separately from its thumbnail

`com/samsung/android/support/senl/nt/brush/model/canvas/BrushSaveExecutor.java`
executes `setPaintDocForegroundImage`, `updateSavePath`, `makeThumbnail`, then
`save` in `doInBackground` (around line 170). The first step captures the
foreground page bitmap and calls `paintingDoc.setForegroundImage` (301–326).
`makeThumbnail` captures the drawn rectangle and writes a JPEG (133–142).
`save` calls `SpenPaintingDoc.save(mDocPath)` (145–153).

`PlatformUtil.getBrushFileExtension()` supplies `spp` in the app delegate
`com/samsung/android/support/senl/nt/app/addons/PlatformUtilImpl.java:65`.
`BrushSaveModel.setPathInfo` assigns distinct source and thumbnail output paths.

`com/samsung/android/support/senl/nt/composer/main/base/model/composer/util/ObjectManagerHelper.java:453–495`
binds both outputs to `SpenObjectPainting`: `setThumbnailPath(outThumbPath)` and
`setAttachedFile(outBrushPath)`, then deletes the temporary producer paths.
These are two bound resources. A thumbnail is not a replacement for the source.

The matching edit route is
`.../presenter/menu/contextmenu/ContextMenuDrawing.java:69–78`: for a type-14
selection it passes the painting's attached-file and thumbnail paths to
`executeBrush`. `BrushFacade.setDocument` constructs `BrushDocModel` with the
source path; `brush/view/canvas/BrushCanvas.java:85–99` constructs
`SpenPaintingDoc(context, width, height, documentFilePath)`.

That Java constructor (`sdk/pen/document/SpenPaintingDoc.java:205–234`) loads an
existing path through `PaintingDoc_Construct2` only if `INSTANCE.isValid(path)`.
An absent or invalid path instead constructs a new document. Therefore opening
this app view successfully is insufficient evidence that a source was recovered.

## Source serialization uses the model's NoteDoc archive

`SPen::PaintingDoc::Construct(cache, width, height, background-path)`, Model `0x466b00`,
constructs `NoteDoc` with `NoteType=1` at `0x466c2c` and appends one page with
`NoteType=1` at `0x466c50`. The path constructor, `0x466d88`, passes that type to
`NoteDoc::Construct` at `0x466ec4` and rejects a loaded page count other than one
at `0x466ed4–0x466ed8`. These checks concern the painting source, not outer
SDOCX object type 14.

`PaintingDoc::Save(path)`, `0x467248`, calls `NoteDoc::Save(path, bool)` at
`0x467258`. The latter reaches `NoteDocImpl::SaveImpl`, `0x2bb6cc`.
Its archive pipeline is explicit:

| Call site | Operation |
| --- | --- |
| `0x2bb744–0x2bb750` | Construct `NoteZip` |
| `0x2bb770` | `PageDocManager::SavePage` |
| `0x2bb780` | `NoteDocImpl::SaveMedia` |
| `0x2bb790` | `NoteDocImpl::SaveNote` |
| `0x2bb7ac–0x2bb7e0` | Fill and serialize `EndTag` |
| `0x2bb810–0x2bb824` | Write and add `end_tag.bin` |
| `0x2bb854` | `NoteZip::Zip(IOutputStream, ...)` |

`SaveNoteImpl`, `0x2bab60`, first writes page-ID information (`0x2bab88`) and
then builds `<internal-directory>/note.note` (`0x2babc8–0x2babd4`; literal at
`0x147277`). `PageDocManager::SavePage`, `0x2c6860`, saves changed pages,
adds their ID plus the page extension to the ZIP (`0x2c6998–0x2c69c0`), saves
history (`0x2c69d0`) and includes existing packed source data (`0x2c69dc`).
The page extension is `.page` (literal at `0x162198`).
`EndTag::GetBinary`, `0x2a891c`, selects the signature from its stored note type
at `+276` (`0x2a8db8–0x2a8df4`). NoteType 1 selects the 39-byte
`Document for SAMSUNG S-Pen PAINTING SDK`, via the relocated table entry at
`0x49a9f0` pointing to literal `0x1605ba`, with length at `0x164ab0`.
`SaveImpl` also appends the serialized end tag after ZIP output at
`0x2bb85c–0x2bb878`. Archive resemblance to SDOCX does not establish identical
note/page schemas: this route uses generic `NoteDoc`, not WDoc's writer.

The foreground setter is itself raster storage.
`ForegroundHandler::SetForegroundImage(Bitmap*)`, `0x336334`, saves the bitmap
to a temporary `.mem` path through `BitmapFactory::SaveBitmap` at `0x336400`,
then calls the path setter at `0x336410`. Keeping that resource preserves an
image, not the original point channels. It coexists with saved layer objects.

## Layer records and the 10,000-object split

`PageDocSaveHandler::Save`, `0x33750c`, calls `Save_PaintingPage` at `0x337c50`
and subsequently `Save_LayerDoc` at `0x337e54`.
`Save_LayerDoc`, `0x338d54`, writes a two-byte layer count, a two-byte current
layer index and each `LayerDoc::Save` (`0x338dc0`, `0x338df0`, `0x338e20`).
It ends with the 26-byte `Page for SAMSUNG S-Pen SDK` marker (`0x338e50`).

`Save_PaintingPage`, `0x33842c`, is gated by the painting flag at page-impl
`+521` and sums `LayerDoc::GetObjectCountToSave`. It emits a new external packet
only when that sum is **greater than 10,000** (`0x3384d8–0x3384e8`).
The filename is `<page ID>_<counter>.dat`, subsequently ZIP-packed into
`<page ID>_<counter>.dat.pack`; `PageDocImpl::GetPackedData`, `0x361ce0`, adds
all packet indices 1 through the saved counter to the outer NoteZip.
For an already split source, the page's field bit `0x800000` stores the packet
counter and total count (`0x33885c–0x3388a0`).

The external `.dat` writer has a concrete boundary before layer data:

| Offset | Static writer operation |
| --- | --- |
| 0 | `u32 32` |
| 4 | `u32` layer-data offset, backpatched to 44 |
| 8 | same backpatched offset |
| 12 | `u32 0` |
| 16, 20 | two page-impl integer dimensions (`+36`, `+40`) |
| 24 | `u32` summed object count |
| 28 | four `f32` values of the supplied rectangle |
| 44 | `u16` layer count |
| per layer | `i32` layer ID, then `LayerDoc::Save` |
| end | 34-byte `Package Data for SAMSUNG S-Pen SDK` marker |

Offsets follow the successful straight-line writer at `0x3385a8–0x338758`;
there is no executed packet capture yet. `LoadAllObjects`, `0x33c130`, reads
this header, seeks to its layer offset (`0x33c6a4–0x33c6b0`), finds each layer
by its stored ID and calls `LayerDoc::Load` at `0x33c770`. Ignoring `.dat.pack`
can therefore lose earlier source objects even when the final `.page` loads.

`LayerDoc::Save`, `0x341754`, calls `Save_LayerData` then `Save_Objects` with
**DocumentType=0** (`0x3417e4–0x3417e8`). `Save_Objects`, `0x354868`, writes
object type as one byte and delegates non-container records to
`WriteDefaultObject` (`0x3549b0`, `0x354a40`). The latter writes a two-byte child
count, a four-byte payload size and the payload (`0x354fd4`, `0x3551d8`,
`0x3551ec`). A type-1 object resolves the ordinary virtual slots to
`ObjectStroke::GetBinarySize/GetBinary` or the compatible variants, rather than
`NewGetBinary` used by current WDoc saving. Dynamic relocations in the
`ObjectStroke` vtable at `0x493010` establish those slot names.

The ordinary stroke block has a 14-byte prefix: flexible offset u32 at +0,
property width byte 2 at +4 and two property bytes at +5, field width byte 4
at +7 and four field bytes at +8, then point count u16 at +12.
`GetBinary` writes offsets relative to the complete object output buffer
(base plus stroke), rather than to a standalone new-style own frame.
The WDoc own block adds total size and kind before the corresponding fields
and starts samples at +20. These layouts follow `0x2ebe88–0x2ebea0` and
`0x2ec008–0x2ec048`, compared with `0x2ee5b0–0x2ee614`.

`ObjectStroke::GetBinary`, `0x2e4b14`, serializes ordinary base data then calls
`ObjectStrokeBinaryHandler::GetBinary` at `0x2e4b9c`. Both ordinary `GetBinary` and WDoc `NewGetBinary` use the same
`sm_ReduceStroke`, property writer and flexible-data writer, but the reducer
mode differs: ordinary passes false (`0x2ebecc`) and WDoc passes true
(`0x2ee424`). The mode branch (`0x2ec46c–0x2ec490`) stores the first compressed
point as two f32 values for ordinary packets, or widens that PointF pair into
two f64 values for WDoc. The ordinary uncompressed writer likewise copies
eight-byte PointF pairs (`0x2ebf18–0x2ebf28`); WDoc stores widened coordinates.
An adapter that only removes/replaces frame headers would misread the channels.
The common channel model, compression algorithm and geometry remain reusable
with explicit coordinate-width handling; current Rust `StrokeChannels::read`
and `decode_stroke` assume WDoc byte counts and f64 point storage.

### Executed reducer boundary

A Rust probe reused the existing conformance `Machine` loader and executed
Model `sm_ReduceStroke` at `0x2ec1a8`, including native `sm_ReduceTilt` at
`0x2eed34`. Only allocation, deletion, `memset` and bounded `memcpy` imports
were supplied by the host; no compression or decoder was implemented there.
The synthetic handler contained a non-null implementation and five vector
triples. Points were `[(1.25, -2.5), (1.5, -2), (2, -1.75)]` as f32, pressure
`[0.5, 0.75, 0.625]`, timestamps `[100, 116, 132]` as i32, and optional tilt
`[0.1, 0.2, 0.3]` / orientation `[1, 1.1, 1.2]` as f32. No record mask or
complete object serializer was involved.

| Channels present | Ordinary mode | WDoc mode |
| --- | --- | --- |
| Points, pressure, timestamps | 32 bytes | 40 bytes |
| Same plus tilt and orientation | 48 bytes | 56 bytes |

Both captures stored the exact first PointF pair, with WDoc widening it to f64.
The entire remaining byte suffix was identical between modes. Output stayed
identical under heap fills `0x00`, `0xa5` and `0xff`; buffer canaries, vector
triples, input arrays and synthetic implementation remained unchanged. This
confirms the coordinate-width seam for these compressed samples, not complete
ordinary object framing, malformed-input handling or archive/replay execution.
The emitted bytes encode native quantized deltas; they are not the original
PointF arrays. Retaining the original stored block and exposing decoded channels
therefore serve different preservation purposes.

## Replay consumes original stroke channels and also uses bitmaps

`PaintingCompatView::SetDocument`, Compat `0x66af8`, gets the painting's PageDoc
and calls `LoadObjectEngine` at `0x66b4c`. Replay initialization
`PaintingCompatReplay::InitializeData`, `0x6021c`, explicitly calls
`PageDoc::LoadAllObjects` at `0x602fc` before traversing source state.

`PaintingCompatReplayDrawing::DrawObject`, `0x6451c`, checks visibility,
`IsReplayOnlyEnabled` for hidden strokes and `IsReplayable` before
`DrawRecordedObject`. Hidden replay-only strokes can therefore be needed for
replay even when ordinary final-page visibility would exclude them. Its type-1 branch calls `DrawObjectStroke` at `0x654e8`.
The latter consumes these model channels:

| Call site | Getter |
| --- | --- |
| `0x64b6c` | `ObjectStroke::GetPoint` |
| `0x64b78` | `GetPressure` |
| `0x64b84` | `GetTimeStamp` |
| `0x64b90` | `GetPointCount` |
| `0x64cfc`, `0x64d08` | `GetTilt`, `GetOrientation` |
| `0x64d5c` | `GetToolType` |

At `0x64d90` these values feed the `MotionEvent` constructor, rather than
being inferred from the foreground image. Pen identity and settings are used
at `0x64ac8`, `0x64b10`, `0x64b64`. Replay still draws into native pen canvases
and bitmaps. Preserving the samples is different from reproducing every
brush's pigment, wet/dry state, texture or blend appearance as SVG.

There are explicit raster-state producers as well.
`PaintingCompatViewBitmapManager::SetLayerBitmap`, `0x6a23c` and `0x6aed8`,
sets or clears `PageDoc::SetLayerDirtyBitmap` and commits history.
Its `isClear=true` branch also calls `PageDoc::RemoveAllObject`
(`0x6a390`, `0x6b014`). This removes the current layer's objects:
`m_SetCurrentLayer` writes the same layer pointer to manager `+40` and its
ObjectHandler at `0x3476e8–0x3476ec`; `ObjectHandlerBase::RemoveAllObject`
calls that layer's removal at `0x364770`. Whether replay/history retains
removed geometry in a subsequently saved source requires separate evidence.
`PaintingCompatViewFillColorAction::fillColor`, `0x7441c`, sets dirty bitmap
at `0x74740` and commits history at `0x7476c`. Model
`LayerDocImpl::SetDirtyBitmap`, `0x351424`, stores a path at impl `+184`, not a
sample array. These traces alone do not establish whether each history action
also has independently serializable vector geometry; dirty bitmap presence
must not be reported as a vector object.

## Corpus and Rust consequences

The seven real files in `hf/` and `tmp/stroke-conformance/` were inspected by
ZIP member name. None has `.spp`, `.spd` or `.dat.pack` members. A bounded
check of every member's first 4 KiB / last 64 KiB and each archive's last
64 KiB also found no Painting SDK end-tag signature. This does not rule out
other encodings or deeply nested sources. Existing SPI and embedded PDF media
belong to different contracts. The corpus does not cover Painting attachments,
packet splitting, fills or replay-only strokes.

Current Rust media admission retains JPEG/PNG/WebP resources; preserving an
outer type-14 source requires retaining the bound non-image payload and its
media metadata first. Current page conversion marks Painting unsupported.
A native attachment retained intact would avoid irreversible loss before a
future decoder is available.

A later source decoder can share `StrokeChannels` and the authoritative Rust
stroke geometry after separately decoding ordinary base/stroke packets, explicitly widening
f32 coordinates without inventing precision, and preserving layer order,
visibility and replay metadata. Flattening `.page`
alone or exporting the JPEG thumbnail cannot establish vector preservation.
The remaining significant unknown is the full ordinary
`ObjectStrokeBinaryHandler` wire layout and how source packet objects,
anchor-image operations and dirty bitmap changes combine during replay.
