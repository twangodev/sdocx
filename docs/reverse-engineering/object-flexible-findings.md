# Optional common object fields

## Evidence

Confirmed from Samsung Notes 4.4.45.37, `arm64-v8a/libSPenModel.so` and
`libSPenBase.so`, without new SDOCX/PDF samples. The SDK exposes the mapped
fields through `ObjectMetadata::flexible_metadata` and its `_with_limits`
variant. Rotation remains in [common metadata](object-base-findings.md).

The modern typed-frame writer is `ObjectBaseBinaryHandler::GetOwnBinary` at
`0x2daad8`. Its matching loader is `ApplyOwnBinary` at `0x2db0e0`, followed by
`m_ApplyOwnBinary_FlexibleArea` at `0x2db6c4`. The table below follows this pair,
not the separate static base-data extraction format.

## Modern typed-frame field order

The loader reads bits 0–8, then 13–21. Bits 6, 7 and 8 also depend on the
format version stored in the fixed area.

| Bit | Wire payload | Meaning and native evidence |
| --- | --- | --- |
| 0 | `f32` | Rotation; reader `0x2db744`–`0x2db75c`, base offset 68 |
| 1 | `u16 count`, then `count * 16` bytes | Partial-rectangle data; loader retains the count at base offset 104 and skips the bytes at `0x2db784`–`0x2db7c0`; `GetPartialRectCount` at `0x2caacc` |
| 2 | `u16` UTF-16 unit count and text | SOR information; reader `0x2db7c8`, `GetSorInfo` at `0x2cca30`, base offset 40 |
| 3 | Unsized `Bundle` | SOR data; reader `0x2db7ec`, `GetSorDataInt` at `0x2ce26c`, base offset 96 |
| 4 | `u16` UTF-16 unit count and text | SOR package link; reader `0x2db848`, `GetSorPackageLink` at `0x2ccbec`, base offset 48 |
| 5 | Unsized `Bundle` | Extra data; reader `0x2db86c`, `GetExtraDataInt` at `0x2cd1e0`, base offset 88 |
| 6 | `i32`, only when version >= 6 | Attached-file reference; reader `0x2db8b8`–`0x2db8d4`, base offset 108; `GetAttachedFileHash` at `0x2d8f94` uses it at `0x2d8fc8` |
| 7 | Two `f32`, only when version >= 9 | Minimum width/height; reader `0x2db8f4`–`0x2db928`, base offsets 112/116; getters `0x2cb4e8` and `0x2cb550` |
| 8 | Two `f32`, only when version >= 13 | Maximum width/height; reader `0x2db944`–`0x2db978`, base offsets 120/124; getters `0x2cb6cc` and `0x2cb774` |
| 13 | `i64` | Append time; reader `0x2db990`, base offset 160, `GetAppendTime` at `0x2cc774` |
| 14 | Two `i32` | Owner-page width/height; reader `0x2db9b8`–`0x2db9f4`, base offsets 176/180; getters `0x2d2c60` and `0x2d2cbc` |
| 15 | `u8` | Layout type; reader `0x2db9fc`, byte helper `0x2dc794`, base offset 172, `GetLayoutType` at `0x2d1d94` |
| 16 | Four `f32` rectangle components, then `f32` rotation | Saved span snapshot; reader `0x2dba78`–`0x2dbab8`, base offsets 232/248 |
| 17 | `i32` | Captured-thumbnail media ID; reader `0x2dba24`–`0x2dba70` converts it to an `ImageCommon` index stored at base offset 196 |
| 18 | Two `f64` | Pivot x/y; reader `0x2dbac4`–`0x2dbb00` converts to floats at base offset 200; `GetPivot` at `0x2ca044` |
| 19 | `u16` UTF-16 unit count and text | Group ID; reader `0x2dbb08`–`0x2dbb48`, implementation offset 216, `GetGroupId` at `0x2cfcc8` |
| 20 | `i32` | Page index; reader `0x2dbb70`–`0x2dbb8c`, implementation offset 128, `GetPageIndex` at `0x2d3124` |
| 21 | `i32` | Render-layer ID; reader `0x2dbb94`–`0x2dbbb0`, base offset 212, `GetRenderLayerId` at `0x2d1660` |

The attached-file reference is not the user ID. `GetUserId` at `0x2cccf8`
reads implementation offset 48, while field 6 writes base-data offset 108.
Similarly, the stored thumbnail value is a media ID, while the runtime member
holds an index returned by `ImageCommon::AddImage`. The writer converts that
index back through `ImageCommon::GetMediaId` at `0x2daf08`.
`ObjectBaseImpl::GetCapturedThumbnailPath` at `0x2d7ed8` resolves offset 196
through `ImageCommon::GetImagePath` at `0x2d7eec`.

The [current writer does not emit bit 1](eraser-preservation-findings.md#legacy-rectangles-are-skipped-by-modern-native-loading-and-writing), although `GetOwnBinarySize` includes
its `2 + count * 16` contribution at `0x2da8b4`–`0x2da8e4`. The SDK retains
its raw records; its omission from this writer does not make it an empty field.

`ReadString` at `0x2787d4`, used by bits 2, 4 and 19, treats the count as an
unsigned number of UTF-16 units. It consumes `count * 2` bytes even for
`0xffff`. A nullable-string helper would misalign these fields. The group-ID
loader normalizes an empty string to a null pointer after reading it. The SDK
retains the distinction between absent and present-empty.

Java `SpenObjectBase` names layout values normal (0), flow (1), block (2) and
undefined (3). `ObjectBase_getLayoutType` at `0x30ae1c` directly calls the native
getter at `0x30ae30`. The getter returns normal for values >= 4, but the binary
reader stores the raw byte. The inspection API preserves unknown values.

Native minimum-size getters clamp values below 10. Maximum-size getters can
substitute twice a context dimension for nonpositive or oversized values.
These are runtime policies, not transformations to apply when exposing the
serialized dimensions. The modern loader also scales bounds using the owner
dimension and requested load dimension at `0x2db434`–`0x2db4f4`; the SDK's raw
metadata does not apply that scaling.

The SDK retains the 20-byte field-16 payload as `saved_span_data` and exposes its
rectangle and rotation through `saved_span_snapshot()`. Partial-rectangle
records from field 1 remain raw 16-byte arrays.

## Saved span snapshot

`ObjectBase::UpdateAttValue` at `0x2d2268` explains field 16. Without a pending
saved value it clears BaseData offsets 232–248, then, only when `BelongsToSpan`
is true, snapshots `GetRect`/slot 168 and `GetRotation`/slot 136
(`0x2d22c4`–`0x2d22e0`). The base relocations `0x4921b0` and `0x492190`
identify those getters: the five floats are left/top/right/bottom and rotation.
The reader copies 20 raw bytes into this separate snapshot and sets BaseImpl
byte 135 pending (`0x2dba78`–`0x2dbab8`), without applying or scaling geometry.
Current base getters still read rectangle offsets 8/16 and rotation offset 68
(`0x2caa68`–`0x2caa70`, `0x2cbd10`–`0x2cbd18`).

`OnBelongedToSpan` (`0x2d1f90`) compares membership at BaseImpl offset 120;
equal membership bypasses both update and notification (`0x2d1fe4`–`0x2d1fec`).
A change stores membership, calls `UpdateAttValue`/slot 384 (`0x2d2024`), then
optionally notifies the context. True membership can capture current geometry;
false membership clears the snapshot when no saved value is pending. A pending
value instead has its flag cleared without replacement (`0x2d2284`–`0x2d229c`).
It therefore survives the next actual update, not necessarily the next attachment.

Shape and Image dispatch this update to `ObjectShape::UpdateAttValue`
(`0x39c298`, relocations `0x4946e0`/`0x497cd8`). It checks the prior pending
flag before qualified Base update and also preserves its saved path and integer
rectangles on that first update; otherwise it clears or snapshots them according
to membership (`0x39c2c8`–`0x39c324`). Formula inherits the Base update
(`0x497fe0`). These are attribute snapshots, not a generic restoration dispatch.

The Base rectangle-change callable sends its notification at `0x2ca990`, then
preserves the snapshot when belonging and both absolute width/height differences
are strictly below double `0.001` (`0x2ca9a0`–`0x2ca9fc`, constant `0x12d470`).
Differences are computed in f32 before widening. Pure translation and sufficiently
small resizing therefore skip update on this path; other size changes dispatch
slot 384 (`0x2caa18`). Base SetRect calls this directly (`0x2ca7cc`), and the
rotation-change path also dispatches slot 384 whenever belonging (`0x2cbc04`).
The snapshot is consequently a mutable baseline, still subject to the pending-value guard.

`ObjectSpan::OnDetach` changes membership to false before calling the owned
object's detach method (`0x417918`, `0x417934`). On an unpending membership
change, that update clears the snapshot before concrete detach. The inspected
Shape/Image, ShapeBase, Base and Formula detach bodies (`0x39a6d0`, `0x37dd54`,
`0x2d00b4`, `0x42aeac`) do not directly reapply its rectangle or angle.
`RestoreAttributeByCoedit` (`0x2d4798`) retrieves a saved `SPen::String` from
an integer-keyed map; it does not directly apply numeric snapshot geometry.

The common writer requires a context, `IsInitializeCoeditData` false or absent,
`IsCoeditMode` present and true, and `BelongsToSpan` true before copying these
20 bytes (`0x2dae90`–`0x2daeec`; named context getters `0x2ac29c`, `0x2ac2dc`).
Field absence therefore does not prove that an object never belonged to a span.
Rust retains the raw bytes and exposes `saved_span_snapshot()` separately from
current geometry. Applying it as source geometry or final inline placement is
not established; callbacks, full reflow and final page placement remain untraced.

## A different static extraction format

`sm_GetBaseData_FlexibleArea` at `0x2dc1c0` is not interchangeable with the
modern typed-frame loader. It reads additional fields at bits 9, 10 and 12:
an eight-byte value into base offset 80, a UUID into offset 136, and a timestamp
into offset 152. It does not follow the modern field-16/19/20/21 sequence.
The eight-byte field is replay order: `ObjectBase::GetReplayOrder` reads that
member at `0x2cc880`. Its signed comparison and load-time fallback are recorded
in [object drawing findings](object-drawing-findings.md). This is distinct from
the four-byte replay timestamp in the modern fixed area.

The distinction is broader than the function's WDoc name. Its caller
`sm_GetBaseDataImpl_WDoc` at `0x2da6b0` starts with a four-byte flexible offset,
then masks, a 16-byte float rectangle, replay timestamp, resize byte and another
four-byte field. That differs from the modern size/type/offset header and
UUID/timestamp/double-rectangle fixed area. It calls the shared static flexible
extractor at `0x2da794` with document type 2.

The format dispatch for this alternate encoding is not established. Its field
map does not establish the meaning or width of unknown modern fields; the SDK
retains those fields and the later flexible tail without decoding them.

## Bundle boundaries

Fields 3 and 5 have no outer size prefix. In `libSPenBase.so`,
`Bundle::GetBinary` at `0xa1958` writes a one-byte category mask, followed by
the present categories in this order. `Bundle::ApplyBinary` at `0xa2204`
matches that order.

| Category bit | Records after a `u16` category count |
| --- | --- |
| 0 | UTF-8 key, signed 16-bit UTF-16 value length, then value units if nonnegative |
| 1 | UTF-8 key and `i32` value |
| 2 | UTF-8 key, `u16` array count, then unsigned `u16` UTF-16 lengths and values |
| 3 | UTF-8 key, byte count, then raw bytes |

Each key has a `u16` byte count and that many UTF-8 bytes. The string-value
reader checks the sign at `0xa22b8`; negative counts create null values without
consuming text. String-array lengths are unsigned at `0xa24a8`–`0xa24c0`.
Byte counts are `u32` for document types >= 2 and `u16` for 0/1, selected at
`0xa25a8`–`0xa25c0`; the writer mirrors this at `0xa2084`–`0xa20ac`.

The byte-array key `SPEN_SDK_KEY_SYSTEM_RESERVED_EXTRA_DATA` has special handling
at `0xa25ec`–`0xa2664`; the comparison string is at virtual address `0x2e5ca`.
The native reader returns its bytes through optional out-parameters instead of
putting it in the ordinary bundle. The SDK preserves it as a named byte-array
entry. Its payload semantics remain unresolved.

## Explicit SDK decoding

```rust
let base = stored_object.base_metadata(&page_bytes)?;
let details = base.flexible_metadata()?;
```

`ObjectFlexibleMetadata` contains all 17 mapped fields after rotation, including
both bundles. Every optional field preserves absent versus present-empty values.
It exposes raw IDs, dimensions and times without applying native runtime
normalization. Layout values use `ObjectLayoutType`, including `Other(u8)`.
`render_layer()` names IDs 0, 1 and 2 as base, top and masking while preserving
other signed IDs and absence. This identifies the stored field, before any
stroke-specific override described in [capture composition findings](capture-composition-findings.md).
This explicit inspection step leaves ordinary page rendering independent of
optional application metadata.

Each `ObjectBundle` contains ordered `ObjectBundleEntry` records with a typed
`ObjectBundleValue`, its category mask, and its exact bounded `data` bytes.
Repeated keys survive within and across categories. Raw data also preserves
noncanonical negative null lengths. It ends at the bundle boundary and excludes
later fields. Empty bundles, including present categories with zero entries,
remain distinguishable from absent bundles.

The decoder uses the existing flexible tail, which starts after any decoded
rotation and ends at the type-0 frame boundary. It does not consume later typed
frames, child objects or integrity trailers. Unknown object field bits or bundle
category bits set `first_unparsed_field` and preserve that whole field and the
remaining bytes in `trailing_data`. In particular, fields 9–12 are not filled
from the alternate static extractor. A malformed known record returns an error.

`max_object_metadata_entries` defaults to 10,000 and bounds the aggregate count
of partial rectangles, entries in both bundles, and string-array elements.
`max_text_characters` bounds aggregate UTF-16 units across optional strings,
bundle keys and bundle values. Both counters are checked before allocation.
`max_entry_size` bounds the input flexible tail; byte-array lengths are checked
against the bounded remainder before copying. The original base metadata and
its raw tail remain available after explicit decoding.

Thirteen synthetic integration tests cover every mapped field and every truncated
prefix, unknown masks, both bundles, duplicate/reserved keys, signed null lengths,
unsigned 65,535-unit strings, 70,000-byte arrays, old-version gates, empty values,
aggregate limits, malformed encodings and the five-float saved span snapshot.
These cases do not establish rendering behavior or real-file visual conformance.
