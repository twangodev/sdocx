# APK and native source map

This map identifies the source paths used for the current SDOCX/WDoc format
description. Decompiled names are partly obfuscated, so methods and behavior
are more stable evidence than class-field names.

## Java writers

| Source | Evidence |
| --- | --- |
| `sources/k1/a.java:727-823` | Canonical archive entries and write order. |
| `sources/n1/h.java:482-737` | `note.note` fixed data, flexible masks and raw SHA-256 trailer. |
| `sources/n1/u.java:961-1342` | Page header, page fields, layers, object traversal, layer/page hashes and signature. |
| `sources/n1/b.java` | Outer object records, child recursion and object identity hashes. |
| `sources/n1/a.java:49-101` | Modern/legacy `mediaInfo.dat` records. |
| `sources/r1/w.java:45-116` | Plain WDoc end-tag population and serialization. |
| `sources/f2/a.java:295-366` | Little-endian primitives and UTF-16 string encodings. |
| `WLockUtil.java:136-233` | Protected-file content encryption and key wrapping. |

## Native libraries

| Library | Evidence |
| --- | --- |
| `libSPenWDoc.so` | WDoc archive/page loading, `pageIdInfo.dat`, page signatures, quick-save/cache sidecars. |
| `libSPenModel.so` | Generic object frames, object type handlers, packed stroke channels, end-tag parser and encryption appendix. |
| `libSPenComposer.so` | Page capture sequence, base/top/masking passes, object clones and native PDF stroke rasterization. |
| `libSPenDrawing.so` | Object visibility, common-alpha drawing options and stroke pen configuration. |
| `libSPenPdf.so` | Native PDF image insertion and pixel-alpha conversion. |
| `libSPenGraphics.so` | Capture blend-mode dispatch and embedded GPU shader equations. |
| `libSPenPenCommon.so` | Shared pen settings, packed ARGB conversion and render-thread alpha. |
| `libSPenDefaultPen.so`, `libSPenMarker.so` through `libSPenMarker4.so` | Concrete pen interfaces, fixed-opacity bindings and Marker2 coverage shaders. |
| `libSPenRenderer.so` | Blend descriptors, native-to-OpenGL enum tables and state activation. |
| `libSPenView.so`, `libSPenBase.so` | Active color-theme selection and alpha-preserving RGB conversion. |
| `libSPenSDoc.so` | Separate deprecated SDoc container; not the modern SDOCX format. |

Important native functions/symbol families include:

- `SPen::WPageLoadHandler::LoadHeader` and `ReadHash`.
- `SPen::WPageManager::SavePageIdInfo`.
- object `GetBinary`/`ApplyBinary` and flexible-data handlers.
- `SPen::ObjectStroke` setters and stroke binary handlers.
- `SPen::EndTag::{ParseImpl,GetBinarySize,GetBinary,Append}`.
- `SPen::EncryptionData::{GetBinary,Apply}`.

Replay-order assignment and capture composition were traced through these
ARM64 entry points:

- Model `LayerManagerBase::sm_GetReplaceOrderForLayer`, `0x34b44c`: returns
  and increments the manager's shared 64-bit replay-order counter.
- Model `ObjectManager::isMatchLayerFilter`, `0x35e5cc`: per-object render
  layer filtering with a top-layer stroke override.
- Composer `NoteCapturePage::drawPage`, `0x32dd44`, and `drawObject`,
  `0x32e700`: capture sequence and base/top/masking passes.
- Composer `NoteCapturePage::getCloneObjectList`, `0x330494`: intersection
  selection, cloning and selected-object alpha/text-visibility reset.
- Graphics `SPBitmapDrawable::DrawBitmapRT`, `0x95f24`, and advanced shader
  source `0x56fce`: paint modes 16 and 17 select darken and lighten blending.
- Composer `NotePDFExporterVectorList::exportObjects`, `0x361d7c`, and WDoc
  vtable relocation `0x103ae8`: vector export obtains the current-layer
  object list and batches strokes before individual non-stroke exports.

See [object drawing findings](object-drawing-findings.md) and
[capture composition findings](capture-composition-findings.md) for the
call-site evidence and the remaining export setup questions.

[Native PDF stroke findings](native-pdf-stroke-findings.md) continue that
trace through Composer `ObjectStrokePdfExporter::ExportObject`, `0x34f684`,
Drawing `ObjectDrawing::DrawObjectList`, `0x7f098`, and PDF
`PdfiumImageHandler::AddImage`, `0x82810`. PDF `CreateARGBBuffer`, `0x8aa7c`,
preserves pixel alpha while converting premultiplied color channels.

[Pen opacity findings](pen-opacity-findings.md) follow Drawing's morphable
dispatch at `0x81eec` and color setter at `0x81c48` into the pen plugins.
PenCommon `PenDrawableRT::SetPenData`, `0x4a558`, retains normalized ARGB;
Marker2's V1 composite at `0x24180` uses its alpha with mask coverage.
Renderer blend tables `0x308b8` and `0x30880` establish maximum blending
inside the mask and source-over composition for its colored output.

[Marker2 V1/V2 comparison](marker2-rendering-findings.md) checks the V2
mask shader at `0x12447`, antialiasing uniform selection at `0x25fc0`,
composite at `0x260dc` and shared size conversion at PenCommon `0x4a588`.
The V1/V2 composite shader strings are identical; V2's mask changes precision
and the edge ramp for drawing sizes below 3. No static call site for
PenCommon `PenDrawableRT::SetAlpha`, `0x4a5a0`, was identified in the APK audit.

[Marker2 sampling findings](marker2-sampling-findings.md) follow Base
`SmPath::helper_compute_quad_segs`, `0xbcde0`, and distance interpolation
at `0xbc494`. Base's array-based MotionEvent constructor at `0xbfd84`
preserves the last stored point as current input. Marker2 redraw `0x22808`
and end `0x22390` apply midpoint smoothing without a separate terminal stamp.

[Stroke recording findings](stroke-recording-findings.md) trace Drawing
`TouchStrokeDrawing::addEventPointsToObjectStroke`, `0xb7dc0`, into Model
`ObjectStrokeImpl::AddPoint`, `0x2e9d6c`. Repeated coordinates survive the
append path. Marker2's slot-88 provider returns null at `0x20a5c`, while
Base's stored-array MotionEvent constructor leaves source at its default 0.

[Motion-event adapter findings](motion-event-adapter-findings.md) connect
Java `SpenMotionEvent(MotionEvent)` to Base `ConvertMotionEvent`, `0xe1e90`.
The adapter preserves float pressure and axes and promotes X/Y to doubles.
Historical `AddBatch`, `0xc01b4`, duplicates X/Y into raw coordinates.
Base `GetEventTime`, `0xc0634`, and `GetHistoricalEventTime`, `0xc0670`,
subtract down time; their nanosecond counterparts do not. Drawing's
recorder imports the millisecond getters at `0xb7e68` and `0xb7ee8`.

[Pen-action input findings](stroke-input-findings.md) trace Composer
`NoteWritingViewPenAction::handWritingBeautification`, `0x422d14`, which
selects InkPen2 for tool types 2 and 6. The ordinary action's comparison at
`0x422750` triggers stroke splitting at 65501 recorded points. Raster
drawing's slot 208 resolves to `LowLatencyStrokeView`, whose counter at
`0x4d43d8` forwards to Drawing `GetStrokePointCount`, `0xb8160`.

[Stroke prediction findings](stroke-prediction-findings.md) resolve
Composer `TouchPresenter::PresentTouch` at `0x4d76bc` and its event-list
recorder call at `0x4d8bdc`. Marker2 V2's prediction getter at `0x1f13c`
supplies the separate drawable called by `OnPredictTouch`, `0x4d94e0`;
V1 returns null. Drawing appends only the primary event at `0xb7570`.
Composer's presenter constructs `PredStrokeLengthController` at
`0x4d7114`; its member-528 slot 64 resolves to `SetLastEvent`, `0x4d6928`.
That method selects a non-resampled prediction anchor, or uses the last
history/current fallback for state -1, without removing input samples.

[Stroke finalization findings](stroke-finalization-findings.md) resolve
Composer factory `0x4db914` and the stroke-view constructor's null selection
at `0x4d1fc4`. CSAPS transformer `0x4dbbb0` calls Model `ReplacePoint` at
`0x4dc480`; Model `0x2df5f8` requires the original count, and `0x2e9cec`
replaces only the coordinate vector while refreshing object/cache state.

[Stroke insertion findings](stroke-insertion-findings.md) connect Composer
`WritingViewPenAction::addStroke`, `0x500424`, to the page-mode adapters
constructed by `ContentsView::SetDocument`, `0x417940`. Mode 0 selects a
page from the first point at `0x4f683c` and applies its negative offset
at `0x4f6a20`; continuous mode appends to page 0 at `0x4ed234`.
Model `SetMillisecondMode`, `0x2e135c`, changes a flag without rescaling
the recorded timestamp array.

[View input transform findings](view-input-transform-findings.md) resolve
View `ViewGroup::applyTransformToEvent`, `0x73e20`, which removes child
position before applying the inverse child matrix. Base `MotionEvent::Transform`,
`0xc0d2c`, transforms current and historical coordinates through the float
arithmetic at `0xc0e08`. Drawing obtains the new stroke's pen width through
pen slot 24 at `0xb76c4`, separately from these event transformations.

[Zoom scale findings](zoom-scale-findings.md) bind Composer callback
`0x3960a4` to the constructed `ViewZoomScroller`. It copies total axis
scales and integer deltas into the contents-view transform. View getters
`0x7af78` and `0x7aff0` include axis stretch. Composer
`NoteWritingView::SetScale`, `0x4284f0`, reaches cutter/eraser implementation
`0x51e91c` through two concrete vtables; its division by zoom is not an
ordinary saved-pen-width conversion.

[Pen size findings](pen-size-findings.md) trace the note-writing manager's
document-relative conversion to PenCommon `0x53cd8`: pen pixel bounds
are scaled by the shorter document dimension divided by 360. Engine JNI
entry `0xc6454` passes the Java size float to the selected pen's setter
at `0xc6a84`. Composer copies input pen width into recording PenData
at `0x4d3868`–`0x4d3878`, separately from event-coordinate transforms.
The alternative DP utility, PenCommon `0x53fe0`, interpolates different
pen bounds and then multiplies by display `densityDpi / 160` at
`0x541b4`–`0x541b8`; its returned width already includes density scaling.
On down-event initialization, Composer `NoteWritingViewPenAction`
reads ViewCore's selected PenData at `0x4228d4` and forwards it through
raster slot 112, `0x50f7b4`, into the stroke view's member 88 at
`0x4d4220`. This connects the setting bridge to the recording-pen copy.

[Standard PDF composition findings](standard-pdf-composition-findings.md)
resolve the public option through `SpenNotePdfExport(Context,int)`, JNI
`Native_init` at Composer `0x3149e8`, and raster dispatch at `0x355d68`.
Standard list-page export reaches `NotePDFExporterRasterListX::updatePage`,
`0x35af00`, rather than the native class named `VectorList`. Its highlighter
bitmap blend reaches `PdfiumImageHandler::DrawImage` at PDF `0x82ebc`;
the enum-name table relocation at `0xb1b38` confirms Darken.

[Saved physical-layer selection](page-layer-selection-findings.md) follows
Model `PageImplBase::LoadLayer`, `0x346ad4`, through the index lookup at
`0x346c78` and handler assignment at `0x346c8c`. WDoc lazy loading reaches
this method. Composer `NotePDFExporterRasterListX::initializeExport`,
`0x35a40c`, obtains existing note page pointers through WDoc
`WNote::GetPageList`, `0x944f4`, and Base `ArrayList::Copy`, `0xd1ba8`.

[Object order and container selection](object-order-findings.md) links Model
`Load_ObjectList_WDoc` insertion at `0x3585b8`, container-child append at
`0x358bd4`, and Drawing child dispatch at `0x7fd04`. The surrounding
intersection collector at Model `0x35e670` restricts top-only queries to
strokes at `0x35e6cc`–`0x35e6dc`; the layer matcher alone omits this condition.

The structural stroke implementation also rechecked these arm64 locations in
`libSPenModel.so` 4.4.45.37:

- `ObjectStrokeBinaryHandler::NewApplyBinary`, `0x2ee888`: zero-point compressed
  strokes skip their channel seeds.
- The same function, `0x2ee9bc–0x2eea64`: uncompressed coordinates are followed
  by separate pressure, timestamp, tilt and orientation arrays.
- `m_ApplyBinary_FlexibleData`, `0x2ed780–0x2ed934`: the legacy bit-0 pen-name
  ID and bit-1 advanced-settings ID are four bytes each, followed by masked
  ARGB and pen size. The alternate coedit string representation is outside
  this parser's normal WDoc archive path.

The extended [stroke metadata inspection](stroke-metadata-findings.md) also
uses the full property reader/writer (`0x2ed138` / `0x2ec080`), named stroke
getters, and flexible reader `0x2ed720`–`0x2ede28`. The call at `0x2ed978`
identifies legacy field 5's count as common partial rectangles. The fallback
at `0x2eda38`–`0x2eda68` identifies field 0 as a legacy pen-name ID. The
[pen selection trace](pen-selection-findings.md) confirms the reference names
against `ObjectStroke::GetPenName`, `0x2de974`, and `GetAdvancedPenSetting`,
`0x2dec00`. PenCommon's registry at `0x78968` and `SetAdvancedSetting` at
`0x46160` connect the resulting strings to native libraries and versions.

## Standalone text frames

The standalone-text implementation rechecked these arm64 locations in the same
`libSPenModel.so` 4.4.45.37:

| Symbol/address | Evidence |
| --- | --- |
| `ObjectBaseBinaryHandler::ApplyOwnBinary`, `0x2db0e0`; flexible handler `0x2db6c4` | Shared identity/bounds and first flexible rotation field. |
| `ObjectTextBox::NewGetBinary`, `0x3e1850`; `NewApplyBinary`, `0x3e18f8–0x3e1a38` | Inherited shape frames followed by the text-box-specific record. |
| `ObjectShapeText::ApplyBinary_TextData`, `0x3b21d4–0x3b23c8` | Masked, length-prefixed `TextCommon`; optional text-area byte: margin 0, free 1, path 2. |
| `ObjectShape::t_TextboxGetOwnBinarySize`, `0x39a2d0`; writer `0x39a35c`; reader `0x39a3b4` | Delegation to the shared component's text-box serializer. |
| `ComponentImage::TextboxGetOwnBinary`, `0x3a4da0–0x3a4e5c`; reader `0x3a4e5c` | Type-2 header and masked border color/width/type. |

[`text-box-findings.md`](text-box-findings.md) distinguishes these native
contracts from synthetic coverage and the remaining real-fixture gap.

## Image frames and media resolution

The image migration checked the following symbols in arm64 `libSPenModel.so`
4.4.45.37 and the corresponding decompiled Java writers:

| Symbol/source | Evidence |
| --- | --- |
| `ObjectImage::NewGetBinary`, `0x420b00`; `NewApplyBinary`, `0x420ba8` | Shared shape chain followed by the type-3 image component. |
| `ObjectShapeBinaryHandler::GetOwnBinary`, `0x3a8dd0–0x3a9228` | Type-7 fixed fields; text/pen fields before sized fill effect bit 5. |
| `ObjectShapeData::GetBinary_PenData`, `0x3ac284` | Four-byte pen-name and advanced-settings string IDs at bits 2 and 4 before the fill record. |
| `ObjectShapeData::CreateEffect`, `0x3abfe0`; `FillImageEffect::Construct`, `0x3b8030` | Fill effect type 2 selects `FillImageEffect`. |
| `FillImageEffect::GetBinarySize`, `0x3b9060`; writer `0x3b90c8`; reader `0x3b9298` | Normal 62-byte image fill, main media ID, alternate 122-byte representation. |
| `ComponentImage::ImageGetOwnBinary`, `0x3a4538`; flexible reader `0x3a4b20` | Type-3 crop, border and original-image fields; these do not select the displayed image. |
| `sources/k1/a.java:790-808`, `sources/n1/a.java:49-101` | Modern manifest version/count, sized records, bind ID to filename mapping and EOFX. |
| `sources/f2/a.java:143-176` | Fixed-byte hash reader and length-prefixed UTF-16 filenames. |

See [`image-findings.md`](image-findings.md) for implemented semantics and the
separation between native evidence, synthetic tests and real manifest coverage.

## Shape and line geometry, styles and paths

The shape/line migration checked Samsung Notes 4.4.45.37 arm64 writers and
native setters/getters, plus the Java template and effect constants:

| Symbol/source | Evidence |
| --- | --- |
| `ObjectShape::NewGetBinary`, `0x399b40` | Snapshots geometry, drawn bounds and rotation before writing `0 + 6 + 7`; clears only shape type-0 rotation. |
| `ObjectLineBinaryHandler::GetOwnBinary`, `0x38bc34` | Type-8 geometry, native pen references and path order. |
| `ObjectShapeBase` writer helper, `0x37c6b4`; `LineStyleEffect::GetBinary`, `0x395b44` | Sized outline effects and twelve-byte style payload. |
| `ObjectShapeData::GetPenName`, `0x3aba3c`; `SetAdvancedPenSetting`, `0x3aba5c` | Shape fields are string-resource IDs at offsets 272/288. |
| `ObjectLineImpl::SetPenName`, `0x387914`; `SetAdvancedPenSetting`, `0x3879a4` | Line fields are settings/name IDs at offsets 32/16; no pen-color field. |
| `ObjectLineImpl::SetRotation`, `0x387db4` | Stored endpoints and paths include rotation. |
| `Path::GetBinary`, WDoc branch `0x2efe10` | Command-count and double-coordinate path format. |
| `SpenObjectShape`, `SpenObjectLine`, `shapeeffect` Java APIs | Template IDs, line kinds and style enums. |

[`shape-line-findings.md`](shape-line-findings.md) records the complete evidence
table, implemented subset, regression comparison and real-fixture gap.

## Evidence standard

A field is called confirmed when at least one writer/parser path gives its
width/order and fixture boundaries agree. Semantic names derived only from
obfuscated Java members stay qualified unless native public symbols corroborate
them. The exact per-object/layer/page hash chain is confirmed both ways: Java
writer logic and byte-for-byte recomputation across all fixtures.
