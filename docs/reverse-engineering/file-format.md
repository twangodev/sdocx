# Samsung Notes SDOCX/WDoc file format

Status: reverse-engineering note, based on Samsung Notes 4.4.45.37 APK writers,
JNI/native symbols and three archived SDOCX fixtures. This documents the modern
WDoc/SDOCX path, not the older SDOC `doc.dat`/`content.dat` format.

## Executive summary

An `.sdocx` file is a ZIP archive followed by a second copy of the compact end
tag. Its authoritative object hierarchy is:

```text
archive
├── note.note                         document metadata + flowing rich text
├── pageIdInfo.dat                    note hash + ordered page IDs/hashes
├── <page UUID>.page                  page metadata + layers + object trees
├── media/
│   ├── mediaInfo.dat                 media manifest
│   └── <media files>                 images, PDFs, audio, proprietary data, ...
└── end_tag.bin                       compact document summary/signature

ZIP end-of-central-directory
└── appended end-tag record           quickly readable without inflating ZIP
```

Almost every extensible record starts with offsets and variable-length bit
masks. Declared sizes, offsets and masks define field boundaries; absolute
positions depend on the preceding variable-length data.

All scalar values observed in these structures are little-endian.

## Sources and confidence

Primary writer paths in the decompiled APK:

- `sources/k1/a.java:728-824`: writes all core archive entries.
- `sources/n1/h.java:482-737`: writes `note.note` and its SHA-256 trailer.
- `sources/n1/u.java:961-1342`: writes page header, flexible properties,
  layers, object trees, hashes and the page signature.
- `sources/n1/a.java:49-101`: reads/writes one `mediaInfo.dat` record.
- `sources/r1/w.java:45-116`: populates and writes `end_tag.bin`.
- `sources/f2/a.java:295-366`: primitive little-endian and UTF-16 writers.
- `libSPenWDoc.so`: native WDoc load handlers and object serializers.
- `libSPenModel.so`: native object model and type-specific frame serializers.

Historical fixture validation used the three inputs identified by SHA-256 in
[`fixture-validation.md`](fixture-validation.md):

- Fixture A: 2,769 strokes / 321,776 points.
- Fixture B: 3,228 strokes / 431,933 points.
- Fixture C: 1,185 strokes / 170,733 points.

These inputs are retired. Current corpus coverage is documented in
[`conformance/README.md`](../../conformance/README.md).

The generic frame boundaries and compressed-stroke byte counts matched all
7,182 strokes with zero boundary errors.

## Archive entries

### Required/core entries

| Entry | Purpose |
| --- | --- |
| `note.note` | Document header, title, flowing body text and document extensions. |
| `<UUID>.page` | One physical stored page. There may be more than one. |
| `pageIdInfo.dat` | Authoritative page order and integrity-link manifest. |
| `media/mediaInfo.dat` | Manifest for archive media. Present even when empty in the APK writer. |
| `media/*` | Payloads referenced by objects or document metadata. |
| `end_tag.bin` | Quickly readable document summary ending in the WDoc signature. |

The same schema is appended after the ZIP end-of-central-directory record.
Normal ZIP readers tolerate this trailing data. The inner and outer copies are
usually equivalent, but they are separate serializations and can diverge; one
audited fixture differed only in the display-modified timestamp. The native
whole-file parser treats the post-EOCD copy as authoritative.

ZIP compression choices are not semantic. In fixture A,
metadata/pages are deflated while the `.spi` media payload is stored.

### Related files that are not canonical ZIP members

Several native strings initially look like extra SDOCX entries, but their call
sites place them in unpacked caches, recovery paths or a different generic SPen
container:

| Path | Actual role |
| --- | --- |
| `qsave_state.dat` | WDoc quick-save cache sidecar containing a raw four-byte state enum. |
| `state.dat` | Cache lifecycle state, also a raw four-byte enum. |
| `size.dat` | Cached unpacked-directory size written during close. |
| `refer.dat` | Generic SPen model cache reference count; WDoc does not import that path. |
| `.bak`, `_back` | Save/recovery artifacts. |
| `*.ssf` | Snapshot forms of selected metadata and page cache files; canonical pages use `.page`. See [native source selection](vector-retention-findings.md#native-note-opening-and-recovery-sources). |

`attach/attachInfo.dat` belongs to the generic `NoteDoc`/`FileManager` path,
not the modern WDoc save path. That compatibility form is:

```text
u16 attached_file_count
repeat:
    u16 logical_key_utf8_byte_count
    bytes logical_key_utf8
    utf16_u16 stored_filename
```

with payloads under `attach/<stored_filename>`. Modern WDoc attachments instead
remain in `media/`; `note.note` field-mask bit 14 maps logical attachment keys
to media bind IDs and the media record sets `is_attached`.

## Integrity graph

```text
SHA-256(note.note without final 32 bytes)
    = note.note final 32 bytes
    = pageIdInfo.dat first 32 bytes

page logical hash
    = <UUID>.page bytes at EOF - 58 .. EOF - 26
    = matching pageIdInfo.dat entry hash

<UUID>.page EOF - 26 .. EOF
    = ASCII "Page for SAMSUNG S-Pen SDK"
```

Fixture A satisfies both equality chains byte-for-byte.

Object, layer and page hashes are logical/model hashes, not hashes of their raw
surrounding records. Their exact formulas are:

```text
identity(id, modified_time) =
    SHA256(UTF8(id + decimal_string_of_signed_modified_time))

object_hash = identity(object_uuid, object_modified_time)

layer_hash = SHA256(
    object_hash_0 || object_hash_1 || ...
    || identity(layer_uuid, layer_modified_time)
)

page_hash = SHA256(
    layer_hash_0 || layer_hash_1 || ...
    || identity(page_uuid, page_modified_time)
)
```

Object hashes enter the layer digest in serialized depth-first order, including
descendants. These formulas reproduced all 7,182 object hashes and every layer
and page hash in the three fixtures. `note.note` is different: its trailer is
the SHA-256 of all preceding raw bytes in that file.

## Common primitives

| Name | Encoding |
| --- | --- |
| `u8` | one byte |
| `u16`, `i16` | 2-byte little-endian integer |
| `u32`, `i32`, `f32` | 4-byte little-endian value |
| `u64`, `i64`, `f64` | 8-byte little-endian value |
| `utf16_u16` | `u16` UTF-16 code-unit count, then UTF-16LE units |
| `utf16_u32` | `u32` UTF-16 code-unit count, then UTF-16LE units |
| hash | 32 raw SHA-256 bytes |

## `note.note`

### Top-level layout

```text
0x00  u32 flexible_data_offset
0x04  u8  property_mask_byte_count
0x05  property mask bytes
      u8  field_mask_byte_count
      field mask bytes
      ---- fixed document data begins at offset 14 for current files ----
      u32 format_version
      utf16_u16 note_id
      u32 file_revision
      i64 created_time
      i64 modified_time
      u32 width
      u32 height
      u32 horizontal_page_padding
      u32 vertical_page_padding
      u32 minimum_format_version
      u32 title_object_size
      title object bytes
      u32 body_object_size
      body object bytes
flexible_data_offset:
      optional document fields in ascending field-mask order
EOF-32:
      SHA-256 of all preceding note.note bytes
```

The existing Rust name `integrity_offset` is misleading: offset 0 is the start
of the document-level flexible field area, not the final hash offset.
The SDK exposes its meaning through `flexible_data_offset()` and retains
both complete masks; see [note-header findings](note-header-findings.md).

### Document flexible-field mask

The APK Java writer confirms these currently used bits:

| Bit | Value | Field |
| ---: | ---: | --- |
| 0 | `0x000001` | application name (`utf16_u16`) |
| 1 | `0x000002` | app major, minor (`u32`, `u32`) and patch name |
| 2 | `0x000004` | nullable author name, phone, email and image media ID |
| 3 | `0x000008` | `f64` latitude and longitude |
| 6 | `0x000040` | template URI |
| 7 | `0x000080` | last edited page index |
| 9 | `0x000200` | last edited page image ID and time |
| 10 | `0x000400` | string-ID table |
| 11 | `0x000800` | body-text font-size delta |
| 12 | `0x001000` | older pen-info block |
| 13 | `0x002000` | voice-data list |
| 14 | `0x004000` | attached-file reference map |
| 15 | `0x008000` | length-prefixed current pen-info block |
| 16 | `0x010000` | server checkpoint |
| 17 | `0x020000` | fixed font |
| 18 | `0x040000` | fixed text direction |
| 19 | `0x080000` | fixed background theme |
| 20 | `0x100000` | text summarization |
| 21 | `0x200000` | stroke-group size |
| 22 | `0x400000` | app custom data (`utf16_u32`) |

`StoredNote::metadata` explicitly decodes these fields from a complete
uncompressed note entry. See [note metadata findings](note-metadata-findings.md)
for native evidence, size-prefix conventions, historical optional fields,
unknown-bit handling and allocation limits.

Title and body are themselves object/frame chains. In the current documents
they contain a base object frame, shape frame and type-7 shape-text frame; the
shape-text flexible fields carry text, spans, paragraph metadata, inline object
records and per-page text sections.

The type-7 text-common payload currently decodes as:

```text
u32 text_common_payload_size
utf16_u32 text
u32 style_span_count
repeat style_span_count:
    u16 span_record_size
    u32 span_type
    u32 start_utf16
    u32 end_utf16
    u32 expansion_flag
    bytes type_specific_span_payload
u32 paragraph_count
repeat paragraph_count:
    u16 paragraph_record_size
    u32 paragraph_type
    u32 start_paragraph
    u32 end_paragraph
    bytes type_specific_paragraph_payload
f32 margin_left
f32 margin_top
f32 margin_right
f32 margin_bottom
u8  gravity
u16 page_text_section_count
repeat page_text_section_count:
    i32 start_utf16
    i32 length_utf16
u32 object_span_flags
u32 object_span_reserved
if object_span_flags bit 0:
    u32 object_span_count
    repeat object_span_count:
        u32 span_size
        u32 embedded_object_size
        u32 embedded_object_type
        bytes embedded_object_binary
        i32 text_index_utf16
        u32 layout_option
        u32 layout_constraint
```

Embedded object binaries use the same generic typed-frame convention. Type 22
tables contain column widths, sized row/cell records and rich-text cell
objects; type 23 code blocks contain optional sized rich-text title/body
objects.

Current table decoding also exposes optional minimum sizes, content bounds,
auto-fit mode, size limits, heading/default colors and per-edge border styles.
Row-height constraints use a nonnumerical field order: bit 9 precedes bit 1.
See [native table and code-block records](table-code-findings.md) for the
complete field map and nested record boundaries.

## `pageIdInfo.dat`

```text
hash note_hash                       32 bytes
u16  page_count
repeat page_count:
    utf16_u16 page_uuid
    hash      page_hash              32 bytes
```

The order in this manifest is authoritative; ZIP entry order is not. The APK
and native writers emit no trailing extension. A tolerant parser can preserve
unexpected trailing bytes for forward compatibility.

## `<UUID>.page`

### Top-level hierarchy

```text
page header and page flexible fields
layer_offset:
    u16 layer_count
    u16 current_layer_index
    repeat layer_count:
        layer frame
        u32 top_level_object_count
        recursive object records
        hash layer_hash
    hash page_hash
    ASCII "Page for SAMSUNG S-Pen SDK"   # exactly 26 bytes
```

`current_layer_index` selects a position in the stored layer collection,
independently of each layer's number. Native page loading assigns that layer
to the object handler used by page queries and Standard list-page PDF export.
The SDK decodes its semantic page objects from this layer while retaining and
structurally validating all layers. See
[saved physical-layer selection](page-layer-selection-findings.md) for the
loader and exporter evidence.

### Page header

```text
0x00  u32 layer_offset
0x04  u32 flexible_data_offset
0x08  u8  property_mask_byte_count
0x09  property mask bytes
      u8  field_mask_byte_count
      field mask bytes
      u32 orientation
      u32 width
      u32 height
      u32 offset_x
      u32 offset_y
      utf16_u16 page_uuid
      i64 modified_time
      u32 format_version
      u32 minimum_format_version
      ... page flexible fields until layer_offset ...
```

For current writer output both masks occupy four bytes, making the fixed fields
start at offset `0x12`: orientation at `0x12`, width at `0x16`, height at
`0x1a`, offsets at `0x1e`/`0x22`, UUID at `0x26`, modified time at `0x70`,
format version at `0x78`, minimum version at `0x7c`, and flexible fields at
`0x80`. The masks are length-prefixed; these positions describe the current
writer's four-byte masks.

The page property mask bit 0 marks a text-only page.

Confirmed flexible page field-mask meanings from the Java writer:

| Bit | Value | Field |
| ---: | ---: | --- |
| 0 | `0x000001` | content bounding rectangle (`4 × f64`) |
| 1 | `0x000002` | tag string list |
| 2 | `0x000004` | template URI |
| 3 | `0x000008` | background-image media ID |
| 4 | `0x000010` | background-image mode |
| 5 | `0x000020` | background ARGB color |
| 6 | `0x000040` | background width |
| 7 | `0x000080` | background rotation |
| 8 | `0x000100` | PDF records: IDs plus a four-value rectangle |
| 9 | `0x000200` | template type |
| 10 | `0x000400` | 49-byte canvas-cache metadata records |
| 11 | `0x000800` | imported-data height |
| 12 | `0x001000` | deprecated/unknown `u32` |
| 18 | `0x040000` | [page-owned custom-object list](#page-custom-object-list) |

Bit 10 is `CanvasCacheData`, confirmed by native
`WPageLoadHandler::LoadHeader_CanvasCacheData`, `0xd3b44`, and the Java
`canvasCacheDataMap` reader/writer in `n1/u.java`. Each 49-byte record stores
a map key, media ID, width, height, one byte dark-mode flag, background color,
three version integers, cache version, property, locale-list ID and
system-font-path hash; all other fields are 32-bit integers. The hash is a
cache field, not font-program bytes.

The three fixture field masks are `0x471`, `0xd71`, and `0xd71`; their parsed
flexible fields end exactly at `layer_offset`.

### Page custom-object list

In ARM64 `libSPenWDoc.so`, `WPageLoadHandler::LoadCustomObject` (`0xd49b4`)
tests field 18 at `0xd49e8`; `Save_CustomObject` (`0xd66f0`) serializes the
separate page-owned list at `WPageImpl + 0x210`. Its envelope is:

```text
i32 count
repeat count:
    i32 custom_kind
    i32 own_binary_byte_count
    u8[own_binary_byte_count] own_binary
```

Custom kinds are a separate namespace from layer-object types. The loader
constructs `CustomObject` with the supplied integer (`0xd4ad4`, `0x84d48`),
without the layer factory's known-type mask. This envelope has no layer child
count or object hash. The current own binary is:

```text
u32 zero
u8 property_mask_width = 1; u8 property_mask = 0
u8 flexible_mask_width = 2; u16 flexible_mask = 0
u16 uuid_byte_count = 36; u8[36] uuid_bytes
i32 file_count
repeat file_count:
    i32 key_byte_count; u8[key_byte_count] key_bytes; i32 file_id
i32 string_count
repeat string_count:
    i32 key_byte_count; u8[key_byte_count] key_bytes
    i32 value_byte_count; u8[value_byte_count] value_bytes
f64 left, top, right, bottom
optional pair: u32 format_version; u32 minimum_format_version
```

`CustomObjectImpl::GetOwnBinary` (`0x8a31c`) starts UUID at offset 9,
writes keyed file IDs and string pairs, widens its runtime rectangle floats
to doubles, and appends both version words. The current empty-map size is 95
bytes; Java `n1/u.java:1087–1164` ends after the rectangle, giving 87 bytes.
The native reader defaults both versions to 5303 when fewer than eight bytes
remain (`0x8a990`–`0x8aa38`), otherwise reads two words and ignores extra tail.
There is no version-comparison gate in these inspected own/list methods.

`ApplyOwnBinary` (`0x8a654`) skips the leading word and fully advances both
declared mask widths, copying at most one property byte and two flexible bytes
into local words. It does not dispatch their bits or reject widths above four.
String lengths count raw bytes; arbitrary bytes are not verified as UTF-8.
Runtime maps overwrite duplicate keys. Saving omits file ID -1 and empty
string keys/values and reconstructs the frame; extensions are not copied.

List loading rejects negative count/size and aborts on binary decode failure.
Successful records are attached to the page context, then checked for valid
bound files (`0xd4b80`). Failed bindings remove the record; the operation's
true boolean permits continuing, while false fails. Saving checks bindings
before emitting a record (`0xd67e0`); the same boolean permits skipping invalid
entries or failing, and the writer backpatches the emitted count. These are
static admission rules, not an observed archive round-trip. Custom semantics
and expanded appearance remain unresolved; see [source retention](vector-retention-findings.md#page-custom-objects-and-attached-source).

### Layer record

Each layer begins with another size/offset/mask frame:

```text
layer_start:
    u32 layer_header_size
    u32 layer_flexible_offset       # absolute file offset
    u8  property_mask_byte_count
    property mask bytes
    u8  field_mask_byte_count
    field mask bytes
    fixed layer fields
    flexible layer fields
    u32 top_level_object_count
    object records, recursively including children
    hash layer_hash
```

In current Java output the layer frame starts with a 12-byte reserved header;
after writing its fields, the writer backfills its total header size, absolute
flexible offset and one-byte masks. Property bits 0, 1 and 2 mean invisible,
event-forwardable and locked. Native property bits 3 and 4 mean alpha-locked
and shadow-visible. Flexible-field bits are:

| Bit | Field |
| ---: | --- |
| 0 | transparency (`u8`) |
| 1 | background color (`u32`) |
| 2 | layer name (`utf16_u16`) |
| 3 | layer UUID (`utf16_u16`) |
| 4 | modified time (`i64`) |
| 5 | thumbnail media ID (`u32`) |
| 6 | shadow effect (`u32` byte count followed by payload; currently 20 bytes) |

The native writer and Java reader consume a one-byte transparency value, but
the decompiled Java writer emits four bytes for that field. See
[layer findings](layer-findings.md) for the exact call sites, decoding contract
and unresolved export behavior.

All three fixtures contain one layer with property mask `0x02`, field mask
`0x18`, and a 98-byte header carrying UUID plus modified time.

### Recursive object record

```text
u8   object_type
u16  child_count
u32  declared_size                 # includes the object hash
bytes payload[declared_size - 32]
hash  object_hash                  # 32 bytes
repeat child_count:
    object_record                  # immediately follows parent record
```

The parent `declared_size` does not include recursively serialized child
records. Children immediately follow their parent, before the next sibling.

Known outer object type IDs:

| ID | Object |
| ---: | --- |
| 1 | stroke |
| 2 | text box |
| 3 | image |
| 4 | container |
| 7 | shape |
| 8 | line |
| 9 | deprecated dummy stroke |
| 10 | voice |
| 11 | formula |
| 12 | deprecated table |
| 13 | web |
| 14 | painting |
| 15 | development stroke |
| 16 | video |
| 17 | link |
| 18 | brush stroke |
| 19 | explicit unknown marker |
| 20 | plot |
| 21 | math |
| 22 | current table |
| 23 | code block |
| 24 | attached file |
| 100 | stroke group (logical/container type; skipped by this Java page writer) |

The Rust stored-object model retains unrecognized type IDs, payload ranges and
ordered children.

Recognized outer object types without semantic decoders produce
`UnsupportedObjectType` diagnostics with the page entry, raw type and payload
offset. This includes undecoded container/group payloads even when their child
records can be decoded. `StoredObject::payload(page_bytes)` borrows the matching
original uncompressed page bytes. `StoredObject` owns no payload bytes; opt-in
detailed `StoredArchivePage.source_bytes` can own the matching page buffer. See the
[source ownership boundary](vector-retention-findings.md#original-page-bytes-are-external-to-the-parsed-model).
Child traversal continues. Unknown future IDs retain the separate
`UnknownObjectType` diagnostic. Neither category implies that omitted content
was rendered.

Outer type 21 has the frame chain `0 + 21`. The math frame contains optional
sized formula binaries, four `f64` margins, a `u32` angle mode and
length-prefixed textual UUID references to plots. These embedded formula
binaries differ from the outer page's child records. See
[math-object findings](math-findings.md) for the evidence and explicit
`StoredObject::math_metadata` inspection API.

Outer type 20 uses `0 + 20` and exposes coordinate/color fields, graph
expressions and substitutions, line styles and angle mode through
`StoredObject::plot_metadata`. Graph strings use `u16` byte lengths; graph
records have no enclosing size or field mask. See [plot findings](plot-findings.md).

Outer type 11 uses `0 + 11`. `StoredObject::formula_metadata` exposes LaTeX
inputs/results/substitutions, image and rectangle fields, embedded strokes,
answer text and label graphs. Its physical field order places bit 3 before
bit 2. `FormulaMetadata::parse_bytes` also accepts the sized formula binaries
inside math objects. See [formula findings](formula-findings.md) for the mixed
string encodings, nested limits and unresolved enum meanings.

## Generic object payload frames

An object payload is a chain of one or more typed frames. The common header is:

```text
frame_start:
    u32 frame_size                    # entire frame, including header
    i16 frame_type
    u32 flexible_data_offset          # relative to frame_start
    u8  property_mask_byte_count
    property mask bytes
    u8  field_mask_byte_count
    field mask bytes
    fixed fields for this frame type
frame_start + flexible_data_offset:
    flexible fields in mask-bit order
frame_start + frame_size:
    next frame, if object type requires one
```

The exact frame chain is object-specific. A normal stroke contains:

```text
base object frame (frame_type 0; 121 bytes in every audited stroke)
stroke frame      (frame_type 1; variable size)
```

The current type-0 base-frame fixed layout is:

```text
+0    u32 frame_size
+4    i16 frame_type = 0
+6    u32 flexible_offset            # relative
+10   u8 property_length = 2
+11   u16 property_mask
+13   u8 field_length = 4
+14   u32 field_mask
+18   u32 format_version
+22   u16 UUID byte count = 36
+24   bytes UUID_utf8[36]
+60   i64 modified_time
+68   f64 bbox_left
+76   f64 bbox_top
+84   f64 bbox_right
+92   f64 bbox_bottom
+100  i32 replay_timestamp
+104  u8 resize_mode
+105  flexible fields
```

Every audited stroke uses a 121-byte base frame, flexible offset 105 and field
mask `0x6000`. Confirmed base property bits describe rotatable (0), selectable
(1), movable (2), visible (3), replayable (4), out-of-canvas-enabled (5),
template (6), flip-enabled (7), floating drawn bounds (8), locked (9) and
inverted removable (12) behavior. See
[common object findings](object-base-findings.md) for getter evidence and the
bounded metadata API. Known flexible fields include rotation; attachment/media
IDs; min/max dimensions; append time; owner dimensions; layout type; pivot;
group/page indices; and render-layer ID.

Known frame chains include:

```text
stroke       0 + 1
container    0 + 4
shape        0 + 6 + 7
line         0 + 6 + 8
image        0 + 6 + 7 + 3
text box     0 + 6 + 7 + 2
voice        0 + 10
formula      0 + 11
web          0 + 13
painting     0 + 14
link         0 + 17
unknown      0 + 19
plot         0 + 20
attached     0 + 24
```

Frame type 6 is a shared shape-base component, not an outer object type.

Standalone text boxes follow the `0 + 6 + 7 + 2` chain in the Rust parser.
Frame 7 field bit 0 contains a sized `TextCommon` payload shared with note text;
frame 2 carries text-box settings, including masked border fields. See
[`text-box-findings.md`](text-box-findings.md) for the confirmed native layout,
regressions and remaining semantics. The implementation reads all fields
relative to variable-length masks, rather than using the audited stroke
offsets shown above for other object types.

Shapes and lines follow their native `0 + 6 + 7` / `0 + 6 + 8` chains. Type 6
contains outline effects; type 7 carries shape geometry, text and fill, while
type 8 carries endpoints and optional native path commands. Shape rotation
belongs to type 7, with type 0 normally storing drawn bounds and zero rotation.
Line endpoints already include rotation. Pen fields reference name/settings
strings rather than colors. See [`shape-line-findings.md`](shape-line-findings.md)
for field layouts, supported rendering and unresolved semantics.

## Stroke frame

### Frame header and fixed point data

```text
generic frame header
u16 point_count

if stroke property bit 0 (compressed points):
    f64 first_x
    f64 first_y

    repeat point_count - 1:
        u16 delta_x_signed_magnitude_q10_5
        u16 delta_y_signed_magnitude_q10_5

    f32 first_pressure
    repeat point_count - 1:
        u16 pressure_delta_signed_magnitude_q3_12

    i32 first_timestamp
    repeat point_count - 1:
        u16 timestamp_delta

    if stroke property bit 2:
        f32 first_tilt
        repeat point_count - 1: u16 tilt_delta_signed_magnitude_q3_12

        f32 first_orientation
        repeat point_count - 1: u16 orientation_delta_signed_magnitude_q3_12

    u8 tool_or_input_type_0
    u8 tool_or_input_type_1

else (uncompressed):
    repeat point_count: f64 x, f64 y
    repeat point_count: f32 pressure
    repeat point_count: i32 timestamp
    if property bit 2:
        repeat point_count: f32 tilt
        repeat point_count: f32 orientation
    u8 tool_or_input_type_0
    u8 tool_or_input_type_1

frame_start + flexible_data_offset:
    stroke style fields in field-mask order
```

The coordinate deltas use a sign bit plus Q10.5 magnitude. Pressure, tilt and
orientation deltas use a sign bit plus Q3.12 magnitude. Timestamp deltas are
zero-extended `u16` values.

For a packed little-endian `u16`, the sign is bit 15 and the magnitude is
`raw & 0x7fff`. Divide the signed magnitude by 32 for coordinates or 4096 for
pressure/tilt/orientation, then add it to the previous value. Coordinates start
at the stored absolute X/Y seed; the bounding box does not normalize them.
The high byte also holds magnitude bits, so testing it only for `0x00` or
`0x80` cannot determine where a channel ends. Use the declared point count.

This describes encoded values;
[native restoration and Rust decoded precision](object-transform-findings.md#compressed-format-loss-and-native-restoration-precision-are-separate)
have distinct accumulation widths.

The uncompressed ordering was rechecked directly in
`ObjectStrokeBinaryHandler::NewApplyBinary` at `0x2ee9bc–0x2eea64` in the
4.4.45.37 arm64 library. It reads all coordinate pairs and then copies each
remaining channel as a separate array. Earlier notes incorrectly described
interleaved point structs. The branch at `0x2ee888` also confirms that zero-point
compressed strokes omit every channel seed and contain only the final two
tool/input bytes after the point count. Synthetic regression tests cover both
cases; the three archived fixtures only exercised compressed strokes.

For every audited compressed stroke, the calculated end of these fixed arrays
was exactly `frame_start + flexible_data_offset`.

### Stroke property mask

Confirmed bits:

| Bit | Value | Meaning |
| ---: | ---: | --- |
| 0 | `0x0001` | curve/compressed representation enabled |
| 1 | `0x0002` | replay-only |
| 2 | `0x0004` | tilt and orientation channels are present |
| 3 | `0x0008` | eraser |
| 4 | `0x0010` | fixed-width enabled |
| 5 | `0x0020` | millisecond timestamp mode |
| 6 | `0x0040` | top-layer pen |
| 7 | `0x0080` | alpha lock |
| 8 | `0x0100` | inverted binary-added flag |
| 10 | `0x0400` | inverted generated flag |
| 11 | `0x0800` | fixed opacity |
| 12 | `0x1000` | rainbow effect |
| 13 | `0x2000` | straighten |
| 14 | `0x4000` | reveal mode |

Consequently common mask `0x25` means compressed points + stylus channels +
millisecond timestamps. It is not a point count. `0x05` is the same packed
point/channel structure without millisecond mode; `0x65` also sets top-layer;
`0x425` also sets the inverted generated flag.

Observed masks:

| Fixture | Masks |
| --- | --- |
| A | `0x25` × 2,732; `0x05` × 37 |
| B | `0x25` × 2,578; `0x05` × 644; `0x65` × 6 |
| C | `0x25` × 1,095; `0x425` × 73; `0x05` × 17 |

### Stroke flexible field mask

Confirmed native serializer mappings:

| Bit | Value | Field |
| ---: | ---: | --- |
| 0 | `0x000001` | legacy pen-name string-table ID (`i32`), used as a fallback for field 7 |
| 1 | `0x000002` | advanced pen-setting string-table ID |
| 2 | `0x000004` | ARGB color (`u32`) |
| 3 | `0x000008` | pen size (`f32`) |
| 4 | `0x000010` | one-byte property |
| 5 | `0x000020` | four raw bytes per common partial rectangle; numerical meaning unresolved |
| 7 | `0x000080` | pen-name string-table ID |
| 8 | `0x000100` | fixed width (`f32`) |
| 9 | `0x000200` | size level |
| 10 | `0x000400` | particle density |
| 11 | `0x000800` | rendering level |
| 12 | `0x001000` | original width |
| 13 | `0x002000` | initial tolerance (`f32`) |
| 14 | `0x004000` | line type (`u16`) |
| 15 | `0x008000` | dash offset (`f32`) |
| 16 | `0x010000` | stroke type (`u16`) |
| 17 | `0x020000` | pen repeat distance (`f32`) |
| 18 | `0x040000` | particle size (`f32`) |
| 19 | `0x080000` | pattern index |
| 20 | `0x100000` | pattern scale (`f32`) |
| 21 | `0x200000` | particle level |
| 22 | `0x400000` | rainbow distance |
| 23 | `0x800000` | rainbow offset (`f32`) |
| 24 | `0x1000000` | gradient-color count plus ARGB values |
| 25 | `0x2000000` | color type (`u16`) |

Example: the common `0x258e` mask contains bits 1, 2, 3, 7, 8, 10 and 13.
Its 28 flexible bytes decode as seven four-byte values: advanced-setting ID,
ARGB color, size, pen-name ID, fixed width, particle density and initial tolerance.
The [native getters and writer](pen-selection-findings.md#stored-reference-identity)
confirm the two reference identities; earlier documentation had swapped them.

The little-endian `u32` color is stored as B, G, R, A bytes. Field-mask bit 2
declares its presence regardless of alpha; bit 3 independently declares pen
size. A color-looking byte sequence or an `0xff` alpha byte is not a field
delimiter. The current Rust `Stroke` exposes RGB only, so alpha fidelity remains
a rendering limitation. Explicit `StoredObject::stroke_metadata` inspection
preserves the full ARGB value, native properties and 25 mapped optional
fields. See [stroke metadata findings](stroke-metadata-findings.md) for exact
types, native call sites, unknown-field boundaries and parser validation.

## `media/mediaInfo.dat`

The Java WDoc writer's modern form is:

```text
u32 format_version
u16 media_count
repeat media_count:
    u32 record_payload_size
    u32 bind_id
    utf16_u16 file_name
    if hash exists: bytes file_sha256_hex[64]   # ASCII lowercase hex
    else: u16 zero_length
    u16 reference_count
    i64 modified_time
    u8  is_attached
ASCII "EOFX"
```

The unversioned WDoc form uses populated 64-byte hashes:

```text
u16 media_count
repeat media_count:
    u32 bind_id
    utf16_u16 file_name
    bytes file_hash[64]
    u16 reference_count
    i64 modified_time
ASCII "EOF"
```

In `sources/k1/a.java:66,792–807`, the writer starts with `i = false`, sets
it to true when the note version exceeds 3001, and uses its current value to
select framing and `EOF`/`EOFX`. It does not reset a previously true flag for
a lower version. `sources/n1/a.java:49–103` reads/writes the record fields;
the unversioned reader defaults attachment to true without consuming a byte.

The native WDoc loader selects framing by trailer, rather than comparing a
leading version. In `libSPenModel.so`, `MediaFileManagerNew::Load`, `0x290f0c`,
checks `EOF` at `0x290fac` and `EOFX` at `0x290fcc`. Only the `EOFX` result
enables the version read (`0x290fec`), record size (`0x291098`), attached byte
(`0x29121c`) and record-end seek (`0x291238`). Both routes read bind ID,
filename, fixed 64-byte hash, reference count and timestamp
(`0x2910c4–0x2911f4`). `WNoteLoadHandler::Load` calls this manager through
virtual slot 160 (`libSPenWDoc.so`, `0xa85b8–0xa85dc`), resolving to
`Load(bool,bool)`, `0x292b9c`, then `Load(path,bool)`, `0x292c30`.
The pinned native New writer emits version 5500 (`0x290548–0x290560`) and
`EOFX` (`0x290710`, `0x290924`).

This unversioned WDoc hash-and-timestamp layout differs from the plain
NoteDoc manager's [CRC manifest](painting-source-findings.md#source-media-uses-the-legacy-manifest).
The same archive filename does not identify the document family or record schema.
These are static findings from the pinned APK; no real unversioned WDoc archive
or native execution validated this branch. The Java writer's two-byte empty
hash conflicts with both readers' fixed 64-byte read, so empty-hash legacy
round trips are unconfirmed. The native no-marker recovery branch is outside
the explicit `EOF` contract described here.

Media names are resolved as `media/<file_name>`. Media payloads are not one
format: objects may reference PNG/JPEG/PDF/audio/video or Samsung-specific
formats. Fixture A contains `0@page_0000000.spi`; its pixels remain
undecoded. The [native SPI trace](spi-media-findings.md) identifies
Maetel codec dispatch and two length-prefixed blocks, with an encoded-header
marker after the first length. This framing has not been validated against
that fixture in this investigation. Its manifest's
64-character hash exactly equals the lowercase hexadecimal SHA-256 of that
payload.

The SDK parses modern manifest records and resolves displayed-image IDs
through this mapping. Its parser reads a leading u32 before rejecting values
at or below 3001; an unversioned manifest instead starts with a u16 count and
record bytes, so that check cannot reliably identify the older layout.
Unversioned WDoc and plain NoteDoc manifests remain unsupported. Saved bind IDs,
filenames and recorded hashes are source identities, separate from native
filesystem admission and support for decoding a media payload.
A filename's numeric prefix is only a warned fallback
when the manifest is absent. In native image objects (`0 + 6 + 7 + 3`), the main
ID is inside type 7's bit-5 image fill; type 3's border/original IDs are separate.
See [`image-findings.md`](image-findings.md) for field layouts and limitations.

## `end_tag.bin`

The following is both the `end_tag.bin` member schema and the outer record
appended after ZIP EOCD. Plain current writer layout:

```text
u16 record_size_excluding_this_u16
u32 format_version
utf16_u16 note_id
i64 modified_time
u32 property_flags
utf16_u16 cover_image
u32 note_width
f32 note_height
utf16_u16 application_name
i32 app_major_version
i32 app_minor_version
utf16_u16 app_patch_name
u32 minimum_format_version
i64 created_time
i32 last_viewed_page_index
u16 page_mode
u16 document_type
utf16_u16 owner_id
u32 reserved_blob_length
bytes reserved_blob
u32 encryption_blob_length
bytes encryption_blob
i64 display_created_time
i64 display_modified_time
i64 last_recognized_data_modified_time
utf16_u16 fixed_font
i32 fixed_text_direction
i32 fixed_background_theme
i64 server_checkpoint
i32 new_orientation
i32 minimum_unknown_version
optional utf16_u32 application_custom_data
ASCII "Document for S-Pen SDK"         # exactly 22 bytes
```

These EndTag strings are counted inline values. For the `u16` fields, native
`ReadString2` treats `0xffff` as 65,535 units rather than null; zero means empty.
Generic Java nullable-string helpers and the current Rust sentinel handling
have different admission behavior, as detailed in the
[EndTag findings](end-tag-findings.md#field-boundaries).

The end tag in fixture A is 144 bytes: its first `u16` is 142 and its last 22
bytes are the signature. Its two variable blobs are empty and it predates the
app-custom-data field. The tags in fixtures B and C include a zero-length `u32`
for that optional string and are 148 bytes. Length-prefixed strings/blobs and
the declared size determine the boundaries; fixed offsets such as `0x48` and
`0x50` apply only when all preceding strings are empty.

When `encryption_blob_length != 0`, that blob is:

```text
u32 original_plaintext_size
u32 salt_length
bytes salt
u32 iv_length
bytes iv
u32 wrapped_key_length
bytes wrapped_key
```

The password wrapper encrypts the supplied plaintext file with a random
AES-256 content key and AES-CBC/PKCS7. It derives a 256-bit key-encryption key
with PBKDF2-HMAC-SHA1 (4,000 iterations and a random 32-byte salt), wraps the
content key using the random 16-byte IV, and appends a readable end tag after
a captured 20-byte ZIP EOCD prefix and zero comment length. SDK password save
supplies a fresh current native save; direct-file lock changes the type tag
before encryption. Decryption bounds ciphertext using the saved plaintext size
and subsequently rewrites EndTag. These source-confirmed
[protected-carrier boundaries](vector-retention-findings.md#native-protected-carriers)
do not establish exact original-byte recovery or an encrypted-fixture round trip.

## Why the legacy decoder produced top-right strokes

The former visible-stroke parser did not walk the structural page/layer/object
records. It started one byte into an outer object header and used offsets whose
errors happen to cancel for common files:

- byte value `0x79` is the low byte of the 121-byte base-frame size, not a
  marker with an “extra attributes” bias;
- the value currently named `data_len` is the whole stroke-frame size;
- advancing that many bytes from inside point data lands near the next object
  only because the 32-byte object hash and four bytes of displacement cancel;
- the fallback called `StartPointMinusThree` reads the stroke property mask at
  the wrong position as a `u16` point count;
- common property mask `0x25` therefore becomes a fake count of 37 points.

The fallback chooses whichever candidate produces more points. Any real stroke
with fewer than 37 points is consequently replaced by a misaligned 37-point
stroke, producing the characteristic top-right-corner artifacts.

This was deterministic in all three fixtures:

| Fixture | Bad selected strokes | Correct normal-layout alternative |
| --- | ---: | ---: |
| A | 74 | 74 |
| B | 68 | 68 |
| C | 49 | 49 |

## Parser architecture implied by the APK

The frame boundaries and ordering establish this parse path:

```text
ZIP directory
  -> note.note + pageIdInfo.dat
  -> pages in manifest order
  -> page header/flexible fields
  -> layers
  -> recursive outer object records
  -> generic object payload frames
  -> type-specific fixed/flexible decoders
  -> renderer-facing document model
```

Structural traversal and channel decoding are implemented in
`storage.rs`, `frame.rs`, `page.rs` and `decode.rs`. Page/layer masks are
length-prefixed; existing public header field names are retained for API
compatibility. Color and pen size are read in field-mask order, with later
unexposed style bytes safely left inside their frame. Current object decoding,
end-tag handling and optional integrity checks are documented in
[parser findings](parser-findings.md).

## Still unresolved

- Exact public semantic names for several obfuscated page and layer properties.
- Complete fixed/flexible layouts for every non-stroke object type.
- General `.spi` compatibility and SDK integration; recovered framing and
  experimental codec behavior are documented in
  [SPI media findings](spi-media-findings.md).
- Byte-exact protected/encrypted end-tag variant without an encrypted fixture.

## Separate legacy SDoc family

Do not apply this WDoc/SDOCX map to Samsung's deprecated SDoc container. The
APK and `libSPenSDoc.so` show a separate family built from entries such as:

```text
doc.dat
content.dat
text.dat
fileinfo.dat
searchData.dat
endtag.dat
SPenSDK30/...
```

It does not use the modern `note.note` plus UUID-named `.page` hierarchy.
