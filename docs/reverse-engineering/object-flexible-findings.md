# Optional common object fields

## Evidence and status

Confirmed from Samsung Notes 4.4.45.37, `arm64-v8a/libSPenModel.so` and
`libSPenBase.so`, without new SDOCX/PDF samples. This is a field map for further
implementation. The SDK currently decodes rotation and preserves the remaining
common flexible bytes; see [common metadata](object-base-findings.md).

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
| 16 | 16-byte rectangle and four-byte value | Saved span data; reader `0x2dba78`–`0x2dbab8`, base offsets 232/248; detailed coordinate/value semantics unresolved |
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

The current writer does not emit bit 1, although `GetOwnBinarySize` includes
its `2 + count * 16` contribution at `0x2da8b4`–`0x2da8e4`. Retain its raw
records when decoding older data; its omission from this writer does not make
it an empty field.

`ReadString` at `0x2787d4`, used by bits 2, 4 and 19, treats the count as an
unsigned number of UTF-16 units. It consumes `count * 2` bytes even for
`0xffff`. A nullable-string helper would misalign these fields. The group-ID
loader normalizes an empty string to a null pointer after reading it; a
structural SDK should retain the distinction between absent and present-empty.

Java `SpenObjectBase` names layout values normal (0), flow (1), block (2) and
undefined (3). `ObjectBase_getLayoutType` at `0x30ae1c` directly calls the native
getter at `0x30ae30`. The getter returns normal for values >= 4, but the binary
reader stores the raw byte. Preserve unknown values in inspection APIs.

Native minimum-size getters clamp values below 10. Maximum-size getters can
substitute twice a context dimension for nonpositive or oversized values.
These are runtime policies, not transformations to apply when exposing the
serialized dimensions. The modern loader also scales bounds using the owner
dimension and requested load dimension at `0x2db434`–`0x2db4f4`; the SDK's raw
metadata does not apply that scaling.

The field-16 writer calls `BelongsToSpan` at `0x2daecc` and emits 20 bytes only
under additional context conditions. The loader marks saved ATT state after
reading them. The rectangle's coordinate system and trailing value need more
tracing before a public semantic type is chosen.

## A different static extraction format

`sm_GetBaseData_FlexibleArea` at `0x2dc1c0` is not interchangeable with the
modern typed-frame loader. It reads additional fields at bits 9, 10 and 12:
an eight-byte value into base offset 80, a UUID into offset 136, and a timestamp
into offset 152. It does not follow the modern field-16/19/20/21 sequence.

The distinction is broader than the function's WDoc name. Its caller
`sm_GetBaseDataImpl_WDoc` at `0x2da6b0` starts with a four-byte flexible offset,
then masks, a 16-byte float rectangle, replay timestamp, resize byte and another
four-byte field. That differs from the modern size/type/offset header and
UUID/timestamp/double-rectangle fixed area. It calls the shared static flexible
extractor at `0x2da794` with document type 2.

Before supporting this alternate encoding, trace the format dispatch and add
separate bounded parsing. Do not fill the modern mask's gaps from a similarly
named native function. Until historical behavior is established, retain an
unknown modern field and the later flexible tail without guessing its width.

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

A bounded bundle decoder is therefore a prerequisite to locating later object
fields when either bundle is present. It must limit aggregate record counts,
text and byte allocations; retain duplicate records until key replacement
semantics are deliberately chosen; and stop on unknown category bits rather
than assume they consume no bytes. A byte-array key also has special handling
at `0xa25ec`–`0xa2664`; its meaning remains to be traced. These constraints
provide the next implementation work without requiring new documents.
