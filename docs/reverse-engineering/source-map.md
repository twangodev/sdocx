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
| `libSPenSDoc.so` | Separate deprecated SDoc container; not the modern SDOCX format. |

Important native functions/symbol families include:

- `SPen::WPageLoadHandler::LoadHeader` and `ReadHash`.
- `SPen::WPageManager::SavePageIdInfo`.
- object `GetBinary`/`ApplyBinary` and flexible-data handlers.
- `SPen::ObjectStroke` setters and stroke binary handlers.
- `SPen::EndTag::{ParseImpl,GetBinarySize,GetBinary,Append}`.
- `SPen::EncryptionData::{GetBinary,Apply}`.

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

## Standalone text frames

The standalone-text implementation rechecked these arm64 locations in the same
`libSPenModel.so` 4.4.45.37:

| Symbol/address | Evidence |
| --- | --- |
| `ObjectBaseBinaryHandler::ApplyOwnBinary`, `0x2db0e0`; flexible handler `0x2db6c4` | Shared identity/bounds and first flexible rotation field. |
| `ObjectTextBox::NewGetBinary`, `0x3e1850`; `NewApplyBinary`, `0x3e18f8–0x3e1a38` | Inherited shape frames followed by the text-box-specific record. |
| `ObjectShapeText::ApplyBinary_TextData`, `0x3b21d4–0x3b23c8` | Masked, length-prefixed `TextCommon`; optional next byte. |
| `ObjectShape::t_TextboxGetOwnBinarySize`, `0x39a2d0`; writer `0x39a35c`; reader `0x39a3b4` | Delegation to the shared component's text-box serializer. |
| `ComponentImage::TextboxGetOwnBinary`, `0x3a4da0–0x3a4e5c`; reader `0x3a4e5c` | Type-2 header and masked border color/width/type. |

[`text-box-findings.md`](text-box-findings.md) distinguishes these native
contracts from synthetic coverage and the remaining real-fixture gap.

## Evidence standard

A field is called confirmed when at least one writer/parser path gives its
width/order and fixture boundaries agree. Semantic names derived only from
obfuscated Java members stay qualified unless native public symbols corroborate
them. The exact per-object/layer/page hash chain is confirmed both ways: Java
writer logic and byte-for-byte recomputation across all fixtures.
