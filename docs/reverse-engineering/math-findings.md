# Native math-object envelopes

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
This pass uses native serializers, readers and getters, decompiled SDK constants,
and synthetic records. It does not use new Samsung-generated SDOCX files.

Unless a library is named explicitly, addresses are ARM64 virtual addresses in
`libSPenModel.so`.

| Symbol | Address | Confirmed behavior |
| --- | --- | --- |
| `ObjectMath::NewGetBinary` | `0x455f00` | Writes the base frame followed by its own math frame |
| `ObjectMathImpl::GetOwnBinary` | `0x45a034` | Type-21 frame with variable masks and no current fixed fields |
| `ObjectMathImpl::GetBinary_Property` | `0x45a0f4` | Property bit 0 comes from implementation byte 80 |
| `ObjectMath::SetEditable` | `0x458998` | Changes byte 80, confirming the editable flag |
| `ObjectMathImpl::GetBinary_FlexibleData` | `0x45a10c` | Four optional fields in ascending bit order |
| `ObjectMathImpl::ApplyOwnBinary` | `0x45a4c8` | Validates frame type 21 and seeks to its flexible offset |
| `ObjectMathImpl::ApplyBinary_FlexibleData` | `0x45a6d0` | Reads formula objects, margins, angle mode and connected plot UUIDs |
| `ObjectMathImpl::GetMargin` | `0x45b3d8` | Rectangle at implementation offset 32 |
| `ObjectMathImpl::GetAngleType` | `0x45b93c` | Four-byte angle value at implementation offset 84 |
| `ObjectMathImpl::SetAngleType` | `0x45b818` | Stores that value and propagates non-2 values to formulas |
| `ObjectMath::ConnectPlot` | `0x458a44` | Accepts objects of outer type 20 |

## Frame and field layout

The frame chain is **0 + 21**. Calls at `0x455f40` and `0x455f58` serialize
`ObjectBase` and `ObjectMathImpl::GetOwnBinary`, respectively. Math does not
inherit the shape frames used by tables. The current math frame header is 15
bytes: the generic size/type/offset header, one property-mask byte and two
field-mask bytes. The writer sets the flexible offset to 15 when fields exist,
or zero when they do not. Wider masks are structurally representable.

Property bit 0 means editable. Remaining property bits have no mapped semantics.

| Field bit | Encoding | Meaning |
| ---: | --- | --- |
| 0 | `u32` count; repeated `u32` payload size plus object bytes | Embedded formula objects |
| 1 | Four `f64` values | Left, top, right and bottom margins |
| 2 | `u32` | Angle type |
| 3 | `u32` count; repeated native UUID records | Connected plot references |

Field 0 traverses the formula list at implementation offset 16. The writer
queries each object's binary size, writes that size, writes the object binary,
then advances by the payload size. The size excludes its own four-byte prefix.
The reader creates an outer-type-11 formula object at `0x45a7c0`–`0x45a7c8`
and applies the sized binary. These records are contained inside the math
payload; they do not use the outer page object's type/child-count/hash wrapper.
Envelope inspection preserves formula internals as raw binaries. They can be
decoded explicitly with `FormulaMetadata::parse_bytes`; see
[formula findings](formula-findings.md).

Field 1 widens four in-memory `f32` margins to `f64`. The reader consumes 32
bytes and narrows them back to the native rectangle. These values are margins,
distinct from the placement bounds in the base frame.

Field 2 is four bytes, unlike the table's one-byte auto-fit field.
`SpenObjectMath.java` defines `TYPE_DEGREE = 0`, `TYPE_RADIAN = 1` and
`TYPE_ALL = 2`. The writer omits value 2. `SetAngleType` propagates values
other than 2 to the embedded formulas; the SDK retains `All` without inventing
a combined rendering or evaluation behavior.

Field 3 traverses the connected-plot list at implementation offset 88, obtains
each plot's UUID and calls `Uuid::GetBinary`. It stores references, not plot
object binaries. Resolving those references requires looking up separate plot
objects; the inspection API does not resolve or evaluate them.

## UUID encoding

In `libSPenBase.so`, `Uuid::GetBinarySize` at `0xaaa4c` returns 38.
`Uuid::GetBinary` at `0xaaa54` writes a `u16` value of 36 followed by 36 bytes
of textual UUID data. This is byte text, not UTF-16 or a 16-byte UUID.

The bounded `Uuid::ApplyBinary` at `0xaacdc` reads the prefix and checks the
available payload before validation. Its validator permits shorter identifiers
and checks hexadecimal characters and the usual hyphen positions. The SDK
inspection API decodes the length-prefixed UTF-8 text without normalizing or
requiring UUID syntax, consistent with existing base-object identity decoding.

## SDK inspection and limits

`StoredObject::math_metadata(page_bytes)` and `math_metadata_with_limits`
explicitly decode an outer-type-21 object from the original uncompressed page
bytes. `MathMetadata` exposes:

- Common base identity, placement and rotation.
- Editability and optional `MathMargins` / `MathAngleType`.
- Separately sized raw `formula_objects` and ordered `connected_plot_uuids`.
- Complete property/field masks and separate fixed, flexible and post-frame
  trailing bytes.

Absent optional values remain absent; unknown angle values retain their raw
`u32`. The known fields occupy contiguous bits 0 through 3, so unknown later
fields remain in the flexible trailing data. The complete source payload stays
accessible through `StoredObject::payload`, including base-frame extensions.

The object payload must fit `ParseLimits::max_entry_size`. Formula and plot
counts share one per-math-object budget derived from `max_objects_per_page`.
Before allocating each vector, the decoder also checks that the remaining bytes
can hold at least the corresponding size/length prefixes. Every embedded
formula read remains inside its declared payload and the containing math frame.
No recursive formula decoding or formula execution occurs.

This is envelope inspection, not validation of the formula binaries or math
rendering. The ordinary document model still omits standalone math objects and
reports `UnsupportedObjectType`; calling the inspection method does not remove
that warning. Stored outer child records continue through the existing traversal.

## Validation and remaining work

Seven synthetic integration tests cover all fields together and individually,
every truncated field prefix with a later decoy frame, absent fields and zero
offsets, known/unknown angle values, wider masks and trailing bytes, cumulative
entry limits, invalid lengths, non-finite margins, invalid UUID text encoding,
wrong outer/frame types and out-of-bounds stored payload offsets. Formula bytes
are checked for exact preservation rather than interpreted as valid formulas.

Type-20 plot fields and graph expressions now have their own bounded inspection
API; see [plot findings](plot-findings.md). Type-11 formulas also expose their
expressions, embedded strokes and label graphs; see
[formula findings](formula-findings.md). Samsung-generated
math/formula/plot documents and matching PDF exports are still needed to check
real writer variants, layout and visual fidelity.
