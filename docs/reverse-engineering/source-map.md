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
| `libSPenComposer.so` | Page capture sequence, base/top/masking pass selection and capture-specific object clones. |
| `libSPenGraphics.so` | Capture blend-mode dispatch and embedded GPU shader equations. |
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
call-site evidence and the remaining export setup and rasterization questions.

The structural stroke implementation also rechecked these arm64 locations in
`libSPenModel.so` 4.4.45.37:

- `ObjectStrokeBinaryHandler::NewApplyBinary`, `0x2ee888`: zero-point compressed
  strokes skip their channel seeds.
- The same function, `0x2ee9bc–0x2eea64`: uncompressed coordinates are followed
  by separate pressure, timestamp, tilt and orientation arrays.
- `m_ApplyBinary_FlexibleData`, `0x2ed780–0x2ed934`: the legacy bit-0 field and
  normal WDoc pen reference are four bytes each, followed by masked ARGB and
  pen size. The alternate coedit string representation is outside this parser's
  normal WDoc archive path.

The extended [stroke metadata inspection](stroke-metadata-findings.md) also
uses the full property reader/writer (`0x2ed138` / `0x2ec080`), named stroke
getters, and flexible reader `0x2ed720`–`0x2ede28`. The call at `0x2ed978`
identifies legacy field 5's count as common partial rectangles. The fallback
at `0x2eda38`–`0x2eda68` identifies field 0 as a legacy advanced-settings ID.

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
