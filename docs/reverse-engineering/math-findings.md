# Native math-object envelopes

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Evidence consists of native serializers, readers and getters, decompiled SDK
constants and synthetic records. Samsung-generated math documents are not covered.

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

## Connected plot references after loading

Field 3 loads UUID strings into a pending vector at implementation +104,
whose end is +112 (`0x45a884`–`0x45a8e8`). This retains their input order;
it does not immediately populate the connected-pointer list at +88. The
writer instead traverses +88 and writes each current pointer's own UUID
(`0x45a340`–`0x45a3e4`), without refreshing or consulting pending strings.
An empty live list omits the field even if pending UUIDs remain.

`UpdatePlotList` (`0x4590cc`) requires the current context and its +440
registry. Math OnAttach registers its own identity in the registry's tree
at +0 (`0x4556f4`–`0x455780`); Plot OnAttach uses a separate tree at +24
(`0x4513b8`–`0x451444`). Plot registration can be skipped by the context
+608 callback returning nonzero (`0x451398`–`0x4513a8`). Existing keys keep
their registered pointers on these insertion paths; document-level collision
admission remains unverified. Math attachment performs no direct resolution
of pending plot UUIDs.

The resolver looks up each pending UUID in the Plot tree, adds the found
pointer to +88 and binds it (`0x459150`–`0x45917c`). It has no direct type,
visibility, clone or UUID-remapping operation. Public ConnectPlot/SetList
do check type20 (`0x458a90`, `0x458ec8`); this resolver bypasses those checks.
ObjectList::Add delegates to Base List::Add (`0x2dcd84`, `0x9d000`), which
appends without deduplication. Repeated saved UUIDs can append/bind the same
pointer repeatedly in pending order. Base SHA-256 is
`e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb`.

Missing targets log and continue. Normal completion clears all pending
strings, including misses (`0x4591b8`–`0x4591c0`); the subsequent writer has
no fallback for those identities. Add failure instead releases/clears the
live list and returns false without clearing pending strings (`0x45921c`–
`0x459270`). Missing context also returns before consumption.

A verified caller exists in `libSPenWDoc.so`, SHA-256
`1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6`,
freshly matched to the APK member. WNoteLoadHandler::Load (`0x0a8584`)
calls LoadPage (`0x0a8824`); on true it iterates WNoteImpl's map at +648
and passes each node's object pointer to UpdatePlotList (`0x0a8844`). Its
return is unchecked. The map's construction/alias to the context registry
was not independently established; neither complete page availability nor
every other reopen route is implied by this load pass.

GetConnectedPlotCount/GetConnectedPlotList refresh first (`0x458d84`,
`0x45907c`), whereas indexed GetConnectedPlot does not (`0x458cfc`).
With a context registry available, refresh checks each live pointer's own UUID
for existence in the Plot tree and removes misses (`0x45baa4`, `0x45bab8`).
It does not compare the current pointer with the map's value or rebind it.
Plot OnDetach erases its own key before Base OnDetach (`0x4515dc`–`0x4515fc`);
later refresh can therefore prune an association. These are static conditional
paths, not captured deletion/save.

MathImpl::Copy directly neither clears nor copies live +88 or pending +104
associations (`0x45b40c`–`0x45b568`); a fresh destination starts both empty.
Base copy, callbacks and higher-level reconnect flows remain separate scopes.
Math's own OnTransfer is a literal return (`0x455840`), which does not prove
the complete framework's cross-document outcome. Connector reconnection
findings for type8 lines do not establish Math restoration.

Rust inspection retains the ordered saved `connected_plot_uuids` independently
of this current pointer projection. Original payload access requires separately
supplied original uncompressed page bytes; StoredObject offsets do not own
them. A reference neither supplies nor owns its separate
[Plot source](plot-findings.md#saved-source-and-generated-appearance).
No real connected Math/Plot copy, collision, partial load or archive round trip
was executed, and no native cleanup is applied to the Rust inspection data.

## Referred strokes and recognition identity

The runtime referred-stroke list at implementation +136 holds the supplied
object pointers: ReferStroke adds and binds them (`0x45c218`, `0x45c228`),
UnReferStroke removes/releases them (`0x45c354`, `0x45c35c`), and the getter
returns the same pointers (`0x459620`–`0x459664`). These loops have no direct
visibility gate or cloning. The list differs from formulas at +16 and plots
at +88. Current own writer/reader (`0x45a10c`, `0x45a6d0`) omit +136; Copy
(`0x45b40c`) performs no direct clear/copy of the destination's referred list.
Attachment registers Math's own UUID (`0x4556f4`–`0x455780`); application
population and saved reconstruction remain unproven by these direct bodies.

Separate saved recognition groups appear in `libSPenRecogUIFeature.so`
(SHA-256 `53ecf90eb45d0a09ecaebb367cf75b3ad327b34386b8a0ca674b0247fa47f276`,
matching the APK member byte for byte). Its native key getters identify:

| Getter | Common ExtraData string-array key |
| --- | --- |
| `0x11f52c` | `RecogUIFeature_MathStrokeUuidStringArray` |
| `0x11f574` | `RecogUIFeature_AnswerStrokeUuidStringArray` |

`updateMathResult` resolves recognition input handles to existing objects
(`0x1b08a0`) and writes the source UUID array to each source stroke (`0x1b0a54`).
The UUID producer uses each pointer's own GetUuid/ToString (`0x120aa0`,
`0x120aa4`), without remapping to an original-object identity. Answer UUIDs
are written to source and answer strokes (`0x1b0dd4`, `0x1b1090`); answers are
marked generated (`0x1b103c`).

In RecogUIFeature, `getUuidList` (`0x1b2454`) selects the source array with flag
bit 0 and the answer array with bit 1, reading their common StringArray entries (`0x1b2518`,
`0x1b26f4`). An absent source array can still reach the answer branch.
`findStrokeSetByUuid` (`0x1abb94`) obtains candidates from the current document
using the supplied page or seed-relative query, compares each candidate's own
GetUuid/ToString with those saved strings (`0x1ac068`–`0x1ac100`), and inserts
the same matching pointer into a set. No direct UUID remap, clone, visibility
check or Math +136 append appears here; candidate filtering remains defined
by document callbacks. `getAnswerRect` reads the answer array and unions
matched objects' current rectangles with the seed's (`0x1ad17c`–`0x1ad318`).

The conditional refresh route with `OnObjectSelected`'s boolean false
(`0x1aa878`) uses nongenerated input handles as fresh-recognition seeds
(`0x1aaae8`–`0x1aab08`); both generated and nongenerated resolved inputs still
reach the flag3 source/answer UUID query (`0x1aab20`). It calls
`removeExistingAnswerList` (`0x1aab60`), which offers generated matches to a
removal queue and erases their temporary set nodes without checking queue
insertion success. The caller then invokes removers on remaining matches for
both UUID arrays and `RecogUIFeature_MathExpressionString` (`0x1aaba0`,
`0x1aabc8`, `0x1aabf0`), before requesting recognition with the separate seed
handle set (`0x1aac3c`). This establishes cleanup calls and recognition requests;
it does not prove completed deletion or replacement of the original stroke data.

`SetDocument` (`0x1a9ed0`) registers document callbacks; indirect callback and
complete archive-reopen paths remain untraced. These consumers do not establish
a bridge from saved UUID groups into Math +136 or replace source vectors.

Model SetExtraDataStringArray uses common BaseData +88 (`0x2cd02c`–`0x2cd03c`),
serialized as [common flexible bit5](object-flexible-findings.md#modern-typed-frame-field-order).
Explicit Rust inspection of common metadata retains these entries through
`ObjectFlexibleMetadata::extra_data` / `ObjectBundleValue::StringArray`, alongside
the stored payload. This is distinct from ownership in the high-level Document.

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

## Validation and evidence limits

Seven synthetic integration tests cover all fields together and individually,
every truncated field prefix with a later decoy frame, absent fields and zero
offsets, known/unknown angle values, wider masks and trailing bytes, cumulative
entry limits, invalid lengths, non-finite margins, invalid UUID text encoding,
wrong outer/frame types and out-of-bounds stored payload offsets. Formula bytes
are checked for exact preservation rather than interpreted as valid formulas.

Type-20 plot fields and graph expressions have their own bounded inspection
API; see [plot findings](plot-findings.md). Type-11 formulas also expose their
expressions, embedded strokes and label graphs; see
[formula findings](formula-findings.md). Real writer variants, layout and visual
fidelity remain unverified against Samsung-generated math/formula/plot documents
and matching PDF exports.
