# Brush, development stroke, group and painting records

## Evidence and scope

These findings concern Samsung Notes 4.4.45.37 ARM64, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Offsets below are ELF virtual addresses, before relocation.

| APK library | SHA-256 |
| --- | --- |
| `lib/arm64-v8a/libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `lib/arm64-v8a/libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `lib/arm64-v8a/libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `lib/arm64-v8a/libSPenMarker2.so` | `f1b2ceea921baac78b722cbd5423732f42d7bec19ccb63d428cf429582412269` |
| `lib/arm64-v8a/libSPenPaintingCore.so` | `56b386228e9b4217a08e16afd7d8f65482bc6e66e4c76f0951a640e1a703c3fa` |
| `lib/arm64-v8a/libSPenPaintingCompat.so` | `fb2f8cd46c45cc4b6ef79f7e7d45c7eac9f12583bfc70ff48e1244f9c861d4a8` |

The Java SDK names types 14, 15, 18 and 100 in
`com/samsung/android/sdk/pen/document/SpenObjectBase.java`.
`SpenObjectPainting.java` exposes attachment, thumbnail, ratio, state, crop
and original-rectangle accessors. Java declarations establish names and API
boundaries; native branches below establish the recovered behavior.

All native claims here are **static traces**, not execution of a saved brush
record or proof of visual parity. None of these four outer object types occurs
in the seven-document [inspected corpus](rendering-corpus-findings.md).

## The names do not imply four independent vector formats

`ObjectFactory::CreateObject`, Model `0x36d6cc`, accepts an unsigned dispatch
index for types 1–24. Its `u16` jump table begins at `0x16768c`; targets are
`0x36d718 + 4 * table[type - 1]`.

| Outer type | Factory target | Meaning in this APK |
| ---: | --- | --- |
| 1, Stroke | `0x36d718` | Allocate ordinary `ObjectStroke`, its vtable and stroke construction |
| 15, StrokeDevelopmentVersion | `0x36d718` | Same construction branch as type 1; saved-record loading has an additional dispatch step |
| 14, Painting | `0x36dc3c` | Allocate `ObjectPainting`; `Construct(bool)` at `0x442268` constructs common type 14 |
| 18, StrokeBrush | `0x36d9f0` | Unsupported-type error 7 and null result |
| 100, StrokeGroup | Range rejection to `0x36d9f0` | Outside factory range; no group construction branch |

The type-15 factory alias alone does **not** establish that every type-15
payload is an ordinary type-1 stroke. The WDoc reader examines a reserved
common-metadata key before choosing its effective object class.

Likewise, the type-18 name does not certify coordinates, a brush engine, or
bitmap content. This APK's recovered factory cannot construct that type.
There is no basis here for treating all such records as ordinary strokes or
for asserting they are intrinsically raster data.

## Type 15 is a compatibility dispatch envelope

`LayerDocLoadHandler::Load_ObjectList_WDoc`, Model `0x358410`, reads the outer
type as one byte. For format versions at least 7 it additionally reads the
two-byte child count. Its default-object admission mask at
`0x35854c`–`0x358560` is `0x01cfe58f`, which includes types 14, 15 and 18.
Only type 4 uses `ReadObjectContainer_WDoc`; other out-of-mask types use
`ReadUnknownObject_WDoc`.

`ReadDefaultObjectImpl_WDoc`, `0x3596c0`, special-cases type 15 at `0x359708`:

1. Parse the common base with `ObjectBaseBinaryHandler::GetBaseData_WDoc`
   (`0x35975c` call).
2. Inspect the base bundle for integer key
   `SPEN_SDK_RESERVED_KEY_OBJECT_TYPE`, whose literal is at `0x14d9c9`
   (`HasInt`/`GetInt` calls at `0x359788` and `0x359798`).
3. Use that integer as the factory type if present. Otherwise retain type 15
   (`0x359888`), whose factory branch constructs an ordinary stroke.
4. Apply the compatibility representation using the already decoded base.
   When compatibility bytes exist, use virtual slot 328; otherwise slot 320
   (`0x3598b4`–`0x3598d8`, `0x359958`–`0x35997c`).

Consequently, outer type, resolved native class and inner-frame kinds are
separate identities. A type-15 envelope can select a different supported
factory class. Dispatching it directly to `decode_stroke` without checking
the bundle would erase that distinction and could misread non-stroke data.

The ordinary stroke serializer/reader already provides sample channels:
`ObjectStroke::GetBinary` at `0x2e4b14` calls the common-base writer followed
by `ObjectStrokeBinaryHandler::GetBinary` (`0x2e4b68`, `0x2e4b9c`). Its reader
calls the corresponding handlers (`0x2e4e84`, `0x2e4ed0`). The existing
[stroke metadata](stroke-metadata-findings.md) and
[file-format channels](file-format.md) describe the ordinary point, pressure,
timestamp, tilt and orientation representation. These channels are reusable
only after effective-type and compatible-payload boundaries are known.
Construction aliasing does not prove every compatibility payload has the
ordinary frame layout.

Type 18 reaches the default-loader path but fails the recovered factory.
Type 100 instead reaches `ReadUnknownObject_WDoc`, `0x359038`, which explicitly
constructs type 19 at `0x359100` and calls
`ObjectUnknown::NewApplyUnknownBinary` with the original type at `0x35912c`.
That is opaque unknown-object handling, not the type-4 child-container route.
No type-100 group geometry or child-transform contract is established by it.

## Grouping ordinary strokes does not manufacture type-100 objects

`ObjectUtil::GroupObjectStrokes`, Model `0x464fa8`, traverses an existing
`ObjectList`, selects runtime type 1 (`0x46505c`–`0x465064`) and reads existing
group IDs. It marks selected stroke boundaries with `ObjectBase::SetGroupId`
(`0x4650c4`, `0x465100`). `RegroupObjectStrokes`, `0x4651e0`, clears existing
IDs and marks new boundaries; its zero-size branch clears IDs directly.

Neither recovered routine constructs a type-100 parent, moves points into a
new child record, or calls `ObjectContainer::Construct`. These operations
establish metadata on ordinary strokes. They do not explain the wire payload
of the separately named `StrokeGroup` type. Rust already exposes a common
`group_id` in `ObjectFlexibleMetadata`; this is a separate preservation
channel from `StoredObject.children`.

## Sync grouping and ordinary drawing have distinct controls

The decompiled sync listener `sources/e4/e.java`, `onOpen`, calls
`SpenWNote.setStrokeGroupSize(100)` and requests save strategy 1024. Its
caller, `sources/e4/f.java`, method `o`, opens the document through
`UpdateDocumentHelper` and waits for the save callback. WDoc
`WNote::SetStrokeGroupSize`, `0x9ed24`, routes a changed size through page
and layer regrouping to Model's routine above. It requires regrouping to
succeed before storing the size at `0x9eda0` and state flags at `0x9eda4`.
In this regroup routine, UUIDs mark selected boundaries rather than assigning
one shared identifier to every stroke in a run.

Drawing `ObjectDrawing::drawObject` supplies literal `w4 = 0` at `0x7fd48`
to its ordinary type-1 `drawObjectStroke` call at `0x7fd64`. That adapter
reads `GetAdvancedPenSetting` at `0x81aec` and searches for `WETDRY` at
`0x81b00`; the literal begins at Drawing `0x4ffb5`. A successful search
sets the queue flag; an absent setting sets it to false at `0x81b8c`.
The true branch adds the stroke to the renderer's pending list at `0x8256c`.
The false branch adds it at `0x8257c`, calls `redrawIPen` at `0x82590`, then
clears that list at `0x82598`. This queue branch uses a saved pen setting,
not the common group UUID. Its redraw dispatch additionally distinguishes
list count and stroke type (`0x82af4`–`0x82b70`).

A concrete saved-stroke consumer, Marker2 V1
`RedrawPen(ObjectStroke const*, RectF*)`, `0x21c58`, builds its `MotionEvent`
at `0x21d04` from tool type, sample count, XY, pressure, timestamps, tilt and
orientation. It passes the event through drawable slot 144 at `0x21d1c`;
the original stroke pointer and common UUID do not cross that callback.
This result concerns this wrapper, not every plugin or multi-object renderer.

These static paths support retaining `group_id` independently of geometry and
physical children. They do not justify merging strokes into one SVG opacity
group by UUID. They also do not establish a metadata-only role throughout the
app or grouped-stroke appearance parity. The recovered
[Painting continuation and watercolor list composition](painting-source-findings.md#saved-wetdry-continuation-changes-shared-brush-composition)
use saved settings and shared coverage independently of UUID boundaries.
Other plugins and [pen-opacity controls](pen-opacity-findings.md) remain separate.

## Painting stores source and preview resources separately

`ObjectPainting` derives from the common object base. Its implementation has
two separate `FileAttacher` instances: attached source at implementation
offset `0x10`, thumbnail at `0x80`.
`ObjectPaintingImpl::GetAttachedFile`, Model `0x446c80`, uses the former;
`GetThumbnailPath`, `0x446dc0`, uses the latter. The binding IDs reside at
implementation offsets 88 and 200 respectively. They are not interchangeable
with an index in Rust's filtered image-asset array.

`NewGetBinary`, `0x442b28`, writes the common base and then painting own data.
`ObjectPaintingImpl::GetOwnBinary`, `0x447138`, writes a 15-byte frame header:
four-byte total frame size, two-byte inner kind **14**, four-byte flexible
offset, one property-mask-size byte of 1 and one zero property byte, then one
field-mask-size byte of 2 and two field-mask bytes. The flexible offset is 15
when fields are present and zero otherwise. This inner kind is distinct from
the outer tree-record type.
`ApplyOwnBinary`, `0x4471ec`, explicitly checks kind 14 at `0x44728c`.

The flexible writer is `0x4466ec`; the reader is `0x446a0c`:

| Field bit | Stored representation | Meaning / member | Reader evidence |
| ---: | --- | --- | --- |
| 0 | `i32` | Attached source binding ID, offset 88 | `0x446a48`–`0x446a68` |
| 1 | `i32` | Thumbnail binding ID, offset 200 | `0x446a6c`–`0x446a8c` |
| 2 | `f32` | Ratio, offset 240 | `0x446a90`–`0x446aac` |
| 3 | Four `i32` coordinates | Crop `Rect`, offset 248 | `0x446ab0`–`0x446acc` |
| 4 | Four `f64` coordinates | Original `RectD`, offset 264 | `0x446ad0`–`0x446b10` |
| 5 | `Rect` then `RectD` | Saved span attachment crop/original rectangle, offsets 348 and 368 | `0x446b14`–`0x446b74` |

Fields are consumed in ascending bit order. IDs are written only when not
`-1`; ratio only when different from 1. The writer emits crop/original data
only when `IsEmpty()` is false. The own-size estimator uses `IsNull()` for
these rectangles (`0x4470a8`, `0x4470bc`); degenerate or inverted rectangles
therefore need separate treatment rather than assuming size-estimator and
writer predicates are identical.

The original `RectD` remains double precision; load magnification promotes
the supplied `f32` factor to `f64` and multiplies its four coordinates
(`0x446b00`–`0x446b10`). The crop is copied as integers in this reader.
Absent field 5 explicitly empties the saved attachment rectangles; present
field 5 marks `SetSavedAttValue(true)`. This is attachment placement metadata,
not a brush-point array or a path-verb stream.

The traced normal flexible block has no stored painting-state field or
inline point-channel sequence. Successful `ApplyOwnBinary` assigns state 4
at implementation offset 244 (`0x447348`–`0x44734c`); Java's
`getState`/`setState` API does not establish a corresponding wire field. The
attached file's content remains a separate format question: its identity must
survive even when a thumbnail is available.

## Painting display uses the thumbnail bitmap

`ObjectDrawing::drawObjectPainting`, Drawing `0x8263c`, constructs a
`ObjectPaintingDrawing` and calls `DrawObject`, `0x88ae4`. That method obtains
`getThumbnailBitmap`, `0x88e14`, before drawing. The helper fetches
`ObjectPainting::GetCacheThumbnail`; it copies the bitmap buffer and creates
a graphics bitmap. `ObjectPaintingImpl::GetThumbnail`, Model `0x446dc8`,
loads the thumbnail path through `BitmapFactory::CreateSharedBitmap`
(`0x446e24`–`0x446e30`).

Normal painting drawing maps the full bitmap rectangle into the object's
drawn rectangle in `drawPaintingObject`, Drawing `0x88f64`, then issues the
canvas bitmap operation at `0x89034`–`0x8904c`. A nonempty crop selects
`drawCroppedPaintingObject` instead (`0x88bb4`–`0x88c94`). Rotation and object
placement are applied before these calls.

This establishes an existing raster preview resource; it does not establish
that the attached editable source is raster-only. `PaintingCore` and
`PaintingCompat` import ordinary `ObjectStroke` construction/access and
stroke drawing APIs. The companion painting APIs expose recorded objects,
layers and dirty bitmaps together. Both vector samples and already-raster
state can therefore participate in the painting subsystem; imported symbols
alone do not recover an attached painting file's complete storage contract.
The [painting source findings](painting-source-findings.md) recover the `.spp`
archive route, ordinary layer/object framing and shared compression seam,
including a bounded native reducer capture. Complete source replay and a real
editable-source sample remain unverified.

## Rust retention and vector-preservation consequences

`storage.rs::parse_object` retains each outer type, payload byte range,
integrity trailer and child tree independent of semantic support. The payload
is borrowed from caller-supplied original `.page` bytes; `StoredObject` does
not own those bytes. Structural retention is not a self-contained round-trip
archive or a decoded vector model.

`page.rs::decode_objects` admits only type 1 as a stroke. Types 14, 15, 18 and
100 emit `UnsupportedObjectType` and retain no parent in the high-level
`Page.objects`. Their supported child records are traversed separately unless
common parent visibility suppresses the record. A type-100 child tree can
therefore yield ordinary strokes in Rust without proving native group
semantics, transforms or composition. `StoredObject::decode_stroke` and
`stroke_metadata` also reject type 15 because they check outer type 1.

Rust's existing bounded frames, common bundle reader, stored record tree and
stroke-channel decoder are the relevant authoritative pieces. The recovered
type-15 envelope requires retaining outer identity while resolving its
integer dispatch key and compatible bytes; it does not justify a second
stroke implementation.

For Painting, `container.rs::parse_media_assets` admits only `.jpg`, `.jpeg`,
`.png` and `.webp` entries. Even if a painting thumbnail is retained as an
image asset, that does not retain its separately bound editable source. A
vector-preserving document needs the original attached resource identity and
bytes as well as placement/crop and preview identity. Rendering the thumbnail
alone is a preview fallback, with no demonstrated preservation of editable
painting vectors. None of these findings implements brush or painting
rendering in the SDK.
