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

## Evidence standard

A field is called confirmed when at least one writer/parser path gives its
width/order and fixture boundaries agree. Semantic names derived only from
obfuscated Java members stay qualified unless native public symbols corroborate
them. The exact per-object/layer/page hash chain is confirmed both ways: Java
writer logic and byte-for-byte recomputation across all fixtures.
