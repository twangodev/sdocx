# Painting source attachments and replay

## Evidence boundary

The archive and replay findings are static traces in Samsung Notes 4.4.45.37.
Bounded native stroke-reducer, anchor-metadata, media-manifest and memory-image
captures are identified separately below. No `.spp` was generated, loaded or replayed during this
investigation. These findings do not establish complete SVG or brush appearance
parity.

The APK is
`com.samsung.android.app.notes_4.4.45.37-444537000_minAPI29(arm64-v8a,armeabi-v7a)(nodpi)_apkmirror.com.apk`,
SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The extracted ARM64 libraries were compared byte-for-byte against the APK entries.

| APK library | SHA-256 |
| --- | --- |
| `lib/arm64-v8a/libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `lib/arm64-v8a/libSPenPaintingCompat.so` | `fb2f8cd46c45cc4b6ef79f7e7d45c7eac9f12583bfc70ff48e1244f9c861d4a8` |
| `lib/arm64-v8a/libSPenPaintingCore.so` | `56b386228e9b4217a08e16afd7d8f65482bc6e66e4c76f0951a640e1a703c3fa` |
| `lib/arm64-v8a/libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `lib/arm64-v8a/libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `lib/arm64-v8a/libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| `lib/arm64-v8a/libSPenRenderer.so` | `f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed` |
| `lib/arm64-v8a/libSPenWaterColorBrush.so` | `ae17e7e63abdc0f5bac9ef4d78676d096779f3586f41c88b029dacfa157020ea` |
| `lib/arm64-v8a/libc++_shared.so` | `4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4` |

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
adds their ID plus the page extension to the ZIP (`0x2c6998–0x2c69c0`), calls
the conditional `SaveHistory` route (`0x2c69d0`) and includes existing packed
source data (`0x2c69dc`). This captured history route cannot add an undo file:
`PageDoc::GetSavedHistoryFileName`, `0x3361e4`, returns false on every branch,
preventing `SaveHistory` from reaching `NoteZip::Add` at `0x2c6c04`.
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

This `.mem` resource has its own raw-image framing. Base
`BitmapFactory::SaveBitmap`, `0xa94fc`, matches extension `mem` at `0xa95f0`
and calls `write_mem_argb`, `0xbc208`, at `0xa99bc`, before the other formats'
unpremultiplication at `0xa960c`. The writer emits three ASCII bytes `mem`,
then little-endian u32 width, height and row-byte count at offsets 3, 7 and
11, followed at offset 15 by exactly height × row-byte-count input bytes.
Its bit-depth argument does not participate in those writes. This is a
pixel cache, distinct from the editable source's layer-object records.

A conformance `Machine` probe executed that writer with width/height 2,
row-byte counts 8/12 and supplied consecutive pixel bytes. Output lengths
were 31/39, retaining the exact 16/24 input bytes including row padding;
input and surrounding canaries remained unchanged. Both cases matched across
allocation fills `0x00/0xa5/0xff` (six writer invocations). File construction
and destruction plus a bounded `fwrite` sink were hosted. The probe was
independently compiled with warnings denied and replayed; output SHA-256 was
`7484ed15b7feced3bfa45a0d898cbd00404b2609fc6248e604851f253a73c346`.
This covers supplied raw-image framing, not foreground capture, pixel
interpretation, native filesystem/archive writes or Painting replay.

## Source media uses the legacy manifest

This NoteDoc route constructs plain `MediaFileManager`, not
`MediaFileManagerNew`: `NoteDocImpl` constructor `0x2ba354` selects vtable
`0x491920` through GOT `0x4a3c00` and stores the facade at implementation
offset 96 (`0x2ba594`). Fresh/loaded constructors use that manager without
replacement. `SaveMedia` calls virtual slot 104 at `0x2bb5e0`, resolving to
plain `Save`, `0x2899ac`; loaded construction calls slot 112 at `0x2bf27c`,
resolving to plain `Load(bool)`, `0x28a784`.

Both manifest generations use `media/mediaInfo.dat`, but this legacy writer
begins with a u16 selected-entry count, rather than a modern u32 format version.
Its successful record layout is:

| Field | Legacy encoding | Writer |
| --- | --- | --- |
| Media ID | u32 | `0x289e60` |
| Filename length | u16 UTF16 code-unit count | `0x289e74` |
| Filename | twice that count of bytes | `0x289e94` |
| File CRC | u32 | `0x289eb0` |
| Reference count | u16 | `0x289f4c` |

`Bind`, `0x287db4`, computes the file CRC and increments the CRC-keyed
record's reference count (`0x287f50–0x287f58`). Save skips zero-reference
records (`0x289d1c–0x289d20`), adds `media/<stored filename>` and backpatches
the selected count. Original archive presence alone therefore does not imply
retention through a later native save.

Binding and archive compression are separate operations. Bind calls
`Image::GetInfo` at `0x28868c`; nonimages or images within its configured
dimension bound use byte-copy calls at `0x2886cc` / `0x2886bc`, while oversized
images use `ResizeImage` at `0x288914` (failure falls back to copying).
Model `SPenCopyFile`, `0x285268`, reads an 8192-byte buffer and writes the same
buffer/count (`0x285314`, `0x285338`), with no object decoder. These copy/resize
branches remain static evidence; no real `.spp` classification was executed.
Save's special true flag for `.mem` (`0x289f04–0x289f20`) controls ZIP
compression, not rasterization: Base's adapter selects method 8 versus 0 at
`0x9b6a8–0x9b6b4`.

### Executed native manifest preservation boundary

A Rust probe reused the conformance `Machine` to execute native Save, then
passed its exact output to native `Load(String*)`, `0x28a0c4`. Initial writer
maps were synthetic, with IDs `[0,1,2]`, CRCs `[111,222,333]` and names
`data0.spp`, `data1.bin`, `image2.mem`; they were not admitted by Bind. The
loaded implementation used the actual native map initializer and insertion
helpers. Native Base/libc++ String helpers were used throughout.

| Initial map | Reference counts | Manifest size | Selected IDs |
| --- | --- | --- | --- |
| empty | none | 2 bytes | none |
| three entries | `[0,0,0]` | 2 bytes | none |
| three entries | `[1,0,2]` | 64 bytes | 0, 2 |
| three entries | `[1,1,2]` | 94 bytes | 0, 1, 2 |

Actual `GetCRCById`, `0x289770`, and `GetFileNameById`, `0x289820`, recovered
the selected original ID/CRC/name triples. Native resave reproduced the
initial bytes and Add arguments exactly. Four cases agreed across allocation
fills `0x00/0xa5/0xff`: twelve loader and twenty-four writer invocations. Initial maps and
implementation headers remained unchanged. No manifest decoder was supplied
by the host. The probe was independently compiled with warnings denied and
replayed; output SHA-256 was
`d9532519541d7ba41c587e73f0c2ab5ad62d3a1e7e4f879b1a659f28adeb552d`.

File access/open, bounded read/seek/write callbacks and logging were hosted;
`NoteZip::Add` arguments were recorded rather than creating a ZIP. This
establishes selected valid metadata framing and native resave, not media file
contents, binding, source-object recovery, malformed-state handling or replay.
The static loader inserts ID/name before CRC (`0x28a280`); its zero-CRC branch
skips the reference count and CRC mappings, so that case is a separate
unexecuted admission boundary.

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

The complete ordinary stroke payload starts with a u32 base-block size, then
that ordinary base block (`ObjectStroke::GetBinary`, `0x2e4b60–0x2e4b68`).
The own block starts at payload +4 +base size (`0x2e4b80`, `0x2e4b9c`);
`GetBinarySize` sums those blocks plus the four-byte size word (`0x2e49f4`).
The compatible wrapper uses the compatible base writer, then the same stroke
writer, passing the base writer's returned integer rather than the compatibility
argument directly (`0x2e4c98–0x2e4ccc`).

The current ordinary own block has a 14-byte prefix: flexible offset u32 at +0,
property width byte 2 at +4 and two property bytes at +5, field width byte 4
at +7 and four field bytes at +8, then point count u16 at +12.
The flexible offset is relative to the complete payload, including its base-size
word; it points after the sample channels and tool field, or is zero when the
field mask is zero (`0x2ebfdc–0x2ec004`). There is no own-size/kind pair here.
The WDoc own block adds total size and kind before the corresponding fields
and starts samples at +20. These layouts follow `0x2ebe88–0x2ebea0` and
`0x2ec008–0x2ec048`, compared with `0x2ee5b0–0x2ee614`.

Uncompressed samples are grouped arrays: N f32 XY pairs (8N bytes), N f32
pressures (4N), N i32 timestamps (4N), then optional N f32 tilts and N f32
orientations (4N each). Only the paired stylus arrays are optional; the writer
tests whether the tilt vector is nonempty. The five memcpy calls are
`0x2ebf28`, `0x2ebf44`, `0x2ebf60`, `0x2ebf84`, `0x2ebfa0`. Raw tool/input
u16 follows those channels (`0x2ebfc8–0x2ebfd8`), before flexible style fields.

Both ordinary `GetBinary` and WDoc `NewGetBinary` use the same
`sm_ReduceStroke`, property writer and flexible-data writer, but the reducer
mode differs: ordinary passes false (`0x2ebecc`) and WDoc passes true
(`0x2ee424`). The mode branch (`0x2ec46c–0x2ec490`) stores the first compressed
point as two f32 values for ordinary packets, or widens that PointF pair into
two f64 values for WDoc. Ordinary reading calls the shared `sm_RestoreStroke`
with mode false (`0x2ecf1c–0x2ecf20`).
An adapter that only removes/replaces frame headers would misread the channels.
Current Rust `StrokeChannels::read` and `decode_stroke` assume WDoc byte counts
and f64 point storage; the five-channel representation remains reusable with
explicit ordinary framing and coordinate-width handling.

Ordinary handlers explicitly use false string mode (`0x2e4b90`, `0x2e4ec0`):
advanced-settings and pen-name fields contain i32 string-table IDs, using the
same [style map](stroke-metadata-findings.md#optional-pen-fields) as normal WDoc
(`0x2ed898–0x2ed8d0`, `0x2ed9fc–0x2eda68`). The reader gets/binds those
references through the context's StringIDManager; the writer requires that
manager, with a detached-history fallback (`0x2ec614–0x2ec6b0`). Decoding
samples alone cannot resolve the saved pen without its source string table.

The reader accepts variable mask widths, keeping their low property two/field
four bytes while advancing the declared widths (`0x2ecdbc–0x2ece7c`). Versions
>=7 use the leading base-size envelope (`0x2e4e40–0x2e4e8c`); versions <=6
instead place an inline base block after the masks, before the u16 count
(`0x2ece78–0x2ececc`). The current 14-byte prefix is not every legacy layout.
Native loading can also change state: nonunit scale multiplies XY
(`0x2ecfcc–0x2ed000`), versions <=19 replace owner bounds from samples
(`0x2ed008–0x2ed034`), and supplied orientation later transforms points
(`0x2ed068–0x2ed0f0`). Those are load transformations, not stored channels;
original bytes and transformed native state remain separate preservation inputs.
These framing and reader findings are source-only, without a complete ordinary
packet or archive load execution.

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
The probe was independently rebuilt and replayed; its output SHA-256 was
`cf9347195b0db00e4d685a4f2999ca4d78ca708bb9724d00b9474237bdeaba7d`.

## Physical-layer properties in the painting editor

These static traces concern the PaintingDoc/PageDoc editor route, not ordinary
WDoc capture. Model `PageImplBase::GetLayerTransparency`, `0x344eb0`, resolves
the requested layer and calls `LayerDocBase::GetTransparency`, which reads
the unsigned byte at impl `+24` (`0x33d258`), the same runtime byte mapped by
the [layer metadata writer](layer-findings.md).

`PaintingCompatLayerManager::UpdateLayer`, Compat `0x5d944`, builds separate
composites for physical indices below and above the current layer, each in
ascending order (`0x5db04`, `0x5dd1c`). The current layer is excluded.
`IsLayerVisible` returning false skips both shadow and contents for that
sibling (`0x5db30–0x5db34`, `0x5dd48–0x5dd4c`). A visible sibling's shadow
also requires its shadow-visible flag (`0x5dbb4`, `0x5ddcc`).

Each admitted sibling's shadow merge precedes its drawing-bitmap merge
(`0x5dc28`, `0x5dcac`; upper composite `0x5de3c`, `0x5deb8`). Each separately
fetches the transparency byte and computes `f32(value) / 255.0f` with `scvtf`
then `fdiv` (`0x5dbfc`, `0x5dc24`; `0x5dc84`, `0x5dca8`). `mergeInLayer`,
`0x5e02c`, supplies that direct alpha to `SPPaint::SetAlpha` (`0x5e0a8`) for
each bitmap draw (`0x5e104`), without inversion. Shadow and contents receive
the same alpha in distinct draws, rather than one opacity after combining them.

`PaintingCompatViewDrawing::drawCompositeLayer`, `0x6c244`, draws the lower
composite (`0x6c2c0`), admitted current shadow (`0x6c4d4`), current canvas and
floating stage (`0x6c648` on the non-alpha-lock route), then upper composite
(`0x6c680`). Current alpha uses the same f32 division (`0x6c414–0x6c438`).
Its current bitmap and shadow getters pass false for the second argument
(`0x6c30c`, `0x6c480`); those getters return null for hidden requested layers
(`0x5cd00–0x5cd08`, `0x5ce74–0x5ce78`). Current selection therefore splits
the live drawing stage from sibling composites; it does not exclude siblings.

`UpdateShadowBitmapOfCurrentLayer`, `0x6e108`, requires the shadow-visible flag
and both visibility-aware bitmap resources (`0x6e148–0x6e188`). It obtains
the layer's saved runtime `ShadowEffect` (`0x6e1a4`), copies all 20 bytes into
`ShadowDrawing::EffectOption` (`0x6e1a8–0x6e1cc`), then calls `DrawShadow`
(`0x6e1d0`). This establishes a saved-effect consumer without identifying its
numeric fields or pixel appearance. The foreground image alone does not retain
these physical-layer properties or their separate drawing order.

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
calls that layer's removal at `0x364770`. Successful Painting removal also
clears page-impl packet counter `+552` and accumulated object count `+560`
at `0x32e49c–0x32e4a4`; earlier packed-source reuse cannot be inferred afterward.
`PaintingCompatViewFillColorAction::fillColor`, `0x7441c`, sets dirty bitmap
at `0x74740` and commits history at `0x7476c`. Model
`LayerDocImpl::SetDirtyBitmap`, `0x351424`, stores a path at impl `+184`, not a
sample array. These traces alone do not establish whether each history action
also has independently serializable vector geometry; dirty bitmap presence
must not be reported as a vector object.

`PageDocImpl::SetHistoryManager`, `0x3616f4`, constructs a plain runtime
`HistoryManager`. History-enabled removal binds existing objects and packs their
handles for undo/redo (`0x35a988`, `0x35a99c`). `PackObjectHandleList`,
`0x368a48`, appends object pointers to history impl `+48/+64`; it invokes no
stroke serializer. These references can retain live objects after removal from
the active layer. The generic manager's save/load/filename APIs return false,
so this runtime undo state does not establish serialized vector history.

Source-only packet selection distinguishes clearing from individual removal.
Successful painting `PageDoc::RemoveAllObject`, `0x32e444`, resets loaded packet
count 552 and packed-object total 560 (`0x32e4a0–0x32e4a4`), leaving selected
count 556 unchanged. When the page writer actually runs successfully,
`Save_PaintingPage` copies 552 into 556 before its 10,000-object threshold
(`0x3384d4–0x3384e8`). After this reset, at most 10,000 saveable objects selects
no packet names and omits the packet header field; above 10,000 generates new
packet number 1 from current saveable objects, without selecting old higher
indices. Saveable counts and writing exclude each layer's packed prefix 176
(`0x3416fc–0x341708`, `0x354938–0x354950`); clearing does not itself prove all
old packet vectors are materialized or other layers are safely rebuilt.

This outcome requires the writer: `SavePage` checks `IsChanged` before saving
(`0x2c6930`, `0x2c6984`). Empty-layer clearing can succeed without marking the
Model layer changed (`0x35a7ac`), and `SetDirtyBitmap`, `0x351424`, does not itself
mark those change bytes. Before a writer updates 556, `GetPackedData` can still
select its prior packet names. Single `RemoveObject`, `0x32e354`, leaves page
packet counters unchanged; the inspected no-history list-removal branch leaves
prefix 176 unchanged (`0x34e6a4`). Selected packets are not filtered against that list.
This does not establish valid arbitrary deletion of an already-packed object
or preservation of every removed sample through ordinary editing.

`GetPackedData`, `0x361ce0`, adds exact packet names through selected count 556.
The fresh outer NoteZip writes explicit entries (`0x2bb744–0x2bb854`, Base
`0xcb30c–0xcb414`), rather than sweeping leftover packet files from the work
root. Unselected old packets may remain working-file orphans; their presence
there does not establish new archive membership. Retaining original archive
bytes preserves their original source bytes even when edited saves omit them.

## Saved wet/dry continuation changes shared brush composition

Compat `commitStroke(bool,bool)`, `0x76268`, records continuation when its first
boolean is true and the saved stroke color equals the current pen color
(`0x76398–0x763bc`): it copies advanced settings, appends `WETDRY` (`0x763f4`),
saves the string/millisecond mode and appends to the page (`0x76400–0x76420`),
skipping terminal bitmap/history close-out. Pending finalization (`0x77214`)
and the deferred callback `0x77330` supply first boolean false; the request
supplies delay 500 (`0x76888`). The terminal merge selects
the current layer and supplies paint alpha 1, clear-before-copy false
(`0x5e204/0x5e218`), independently of the brush's internal color/coverage.

Replay `initialWetDryObjectlist`, `0x60ff4`, derives run indices from saved
settings and source positions. `DrawObjectStroke` computes dry from absent
`WETDRY` or null settings (`0x64b24–0x64b34`). At a nonzero last sample
(`0x64e34–0x64e40`), after successful `GetStrokeInfo`, wet retains dirty bounds;
dry closes the floating result. Drawable bitmap type 1 selects the floating
bitmap (`0x65398`); its terminal copy uses the **source stroke's layer ID** and
helper `0x82c8c`, boolean false (`0x65034–0x65064`), then clears floating state
and bounds (`0x65070/0x65078`). Type 2 instead draws into the requested layer
(`0x653a0–0x653ac`). None of these controls supplies the common group UUID.

Live alpha lock is separate: `onTouchDown` reads the current layer's lock,
gated by action member 275 being zero (`0x75dfc–0x75e1c`), and saves
`SetAlphaLock` (`0x76390`). `StartAlphaLock` captures the layer background;
`FinishAlphaLock` draws its updated result with XFermode 8 (`0x6e094/0x6e0a4`).
Replay supplies false to its helper's XFermode-8 selector; the live path does
not establish saved replay alpha-lock parity.

The [ordinary Drawing queue](brush-record-findings.md#sync-grouping-and-ordinary-drawing-have-distinct-controls)
has a concrete multi-stroke consumer. Its drawable slot 48 binds WaterColor
V1/V2 ObjectList redraw through relocations `0x760a8/0x761b8`, entering
`0x62240/0x64248`. Each creates one callback/shared buffer before traversing
strokes, reads separate point counts, XY, pressure and timestamps, and resets
per-stroke geometry state; V2 reads `IsShape` (`0x643f0`). Pen size/color are
queued once and the accumulated rectangle after the complete list.
That `SetRect` task does not render: PenCommon's callback destructor `0x50424`
queues buffer upload and `PenGLRenderMsg`; the message calls drawable slot 104
for intersecting tiles (`0x46970`), bound to WaterColor `Draw`, `0x65a08`.

This Draw uses one shared additive mask, then source-over color composition
(`0x64e40/0x64e54`, `0x64df8/0x64e0c`; Renderer enum tables `0x308b8/0x30880`).
Its color alpha is fused f32 `savedAlpha * f32(0.9) + f32(0.1)` (`0x65cdc`).
In shader `0x52c7a`'s normal noneraser branch with positive mask red, green
below 0.7 scales alpha toward a maximum of `0.8 * colorAlpha`; above threshold
it holds that alpha and adds a mask-dependent RGB term. Nonpositive mask red
is discarded; its eraser branch has a separate equation and original texture.

Continuation/settings, source order/layer IDs, original channels, color and
alpha lock therefore remain separate preservation inputs from regroup UUIDs.
Rust's `StrokeStyle`, `StrokeRendering` and `StrokeProperties` already retain
advanced-setting IDs/resolved strings and alpha lock. A uniform-opacity union
or independent fill per stroke lacks this shared-coverage contract;
retaining its source does not require adopting the native GPU/bitmap backend.
Static findings do not establish appearance parity, universal plugin behavior,
or a flush for malformed trailing wet runs.

## Anchor images checkpoint replay without replacing original strokes

Compat `SetReplayAnchorBitmapEnabled`, `0x70b0c`, creates anchor bitmaps then
stores their interval/list in PageDoc (`0x70bc0`, `0x70bd8`, `0x70bf0`). Disabling
stores threshold -1 and a null list. Neither branch removes source objects.
`CreateReplayAnchorBitmap`, `0x63370`, copies visible layers' drawing bitmaps
and saves them at quality 100 (`0x636a0`) while advancing the existing stroke
list through `drawOrSkipStroke` (`0x63718`). Model `SetAnchorImageList`,
`0x336e14`, binds these paths and keeps their media IDs separately from vectors.

Model `Save_AnchorImage`, `0x3389cc`, writes threshold i32 under mask bit 25
when threshold >=1. A nonempty list sets bits 26/27 and writes count i32,
then each anchor's stroke index, timeline field and media ID as three i32
values, followed by a second pass writing layer IDs. It contains no sample
arrays. `LoadHeader_AnchorImage`, `0x33b138`, restores paths through media IDs;
the layer-ID pass is optional under bit 27, otherwise IDs remain initialized -1.
The timeline field's precise unit remains unverified.

A Rust probe reused the conformance `Machine` and executed this native writer
with actual Base `List` construction, insertion, count and traversal. Host
support supplied bounded allocation/deletion, a `File::Write` byte sink and
no-op recursive-mutex imports for single-threaded execution. Two supplied
anchors `[layer,index,timeline,mediaID]` were `[7,29,250,17]` and `[-1,59,500,19]`;
these were metadata inputs, not validated media bindings. The initial mask was
`0x81`.

| Threshold | Bound list | Resulting mask | Written bytes |
| --- | --- | --- | --- |
| -1 | absent | `0x00000081` | 0 |
| 0 | absent | `0x00000081` | 0 |
| 30 | absent | `0x02000081` | 4 |
| 0 | two anchors | `0x0c000081` | 36 |
| 30 | two anchors | `0x0e000081` | 40 |

All five cases agreed across heap fills `0x00/0xa5/0xff`; supplied records and
native list counts remained unchanged. This capture covers anchor metadata
writing, not complete page framing, resource binding, archives or replay.
The probe was independently rebuilt and replayed; its output SHA-256 was
`ce537a822d43e494d43301d684a3bb6f0d754e1ea6614309b587bbbd73882671`.

On fresh replay initialization, Compat `InitializeData`, `0x6021c`, calls
`PageDoc::LoadAllObjects` before copying stored anchors (`0x60304–0x60360`);
the caller does not check that load's return value.
`FindAnchorBitmap`, `0x61b68`, chooses an anchor strictly before the requested
frame's stroke index. The bound-list branch of `GetAnchorFileName`, `0x64940`,
accepts a matching layer ID or -1 for a matching checkpoint index; its other
branch derives temporary filenames.
`SetReplayPositionWithAnchorBitmap`, `0x61fd8`, loads visible layers' checkpoint
bitmaps (`0x6216c`), then calls `drawObjects` from checkpoint +1 (`0x62358`) and
`drawOneFrame` (`0x62398`). The bitmap covers an already-drawn prefix; remaining
replay uses original stroke channels. This shortcut does not justify discarding
the prefix vectors or treating dirty-bitmap clears as equivalent checkpoints.

## Corpus and Rust consequences

The seven real files in `hf/` and `tmp/stroke-conformance/` were inspected by
ZIP member name. None has `.spp`, `.spd` or `.dat.pack` members. A bounded
check of every member's first 4 KiB / last 64 KiB and each archive's last
64 KiB also found no Painting SDK end-tag signature. This does not rule out
other encodings or deeply nested sources. Existing SPI and embedded PDF media
belong to different contracts. The corpus does not cover Painting attachments,
packet splitting, fills or replay-only strokes.

Current Rust retains `archive_id` and modern manifest bindings, while the
legacy manifest above remains unsupported. Image admission retains
JPEG/PNG/WebP resources; preserving an
outer type-14 source requires retaining the bound non-image payload and its
media metadata first. Current page conversion marks Painting unsupported.
A native attachment retained intact would avoid irreversible loss before a
future decoder is available.

A later source decoder can share `StrokeChannels` and the authoritative Rust
stroke geometry after separately decoding ordinary base/stroke packets, explicitly widening
f32 coordinates without inventing precision, and preserving layer order,
visibility and replay metadata. Flattening `.page`
alone or exporting the JPEG thumbnail cannot establish vector preservation.
The remaining significant unknowns are the ordinary base block's complete
optional layout and source-archive string-table loading contract, an actual
packet/archive load, full materialization of old packed vectors before edits,
valid arbitrary packed-prefix mutation, and whether separate recorded-operation
contracts retain removed original samples. Packet selection after clearing is
conditional on the writer's change gate and successful execution, as above.
Runtime undo handles and anchor images alone do not establish that preservation.
