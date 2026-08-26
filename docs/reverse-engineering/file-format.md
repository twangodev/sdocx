# Samsung Notes SDOCX/WDoc file format

Status: reverse-engineering note, based on Samsung Notes 4.4.45.37 APK writers,
JNI/native symbols and three real SDOCX fixtures. This documents the modern
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

The important structural rule is that almost every extensible record starts
with offsets and variable-length bit masks. A reader should obey sizes, offsets
and masks rather than hard-coded absolute positions.

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

Fixture validation used:

- `handwritten.sdocx`: 2,769 strokes / 321,776 points.
- `quiz.sdocx`: 3,228 strokes / 431,933 points.
- `cs61bl_su22.sdocx`: 1,185 strokes / 170,733 points.

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

ZIP compression choices are not semantic. In the handwritten fixture,
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
| `.bak`, `_back` | Atomic-save/recovery artifacts. |
| `*.ssf` | Snapshot/internal page form accepted by loaders; canonical saves use `.page`. |

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

The handwritten fixture satisfies both equality chains byte-for-byte.

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

### Document flexible-field mask

The APK Java writer confirms these currently used bits:

| Bit | Value | Field |
| ---: | ---: | --- |
| 0 | `0x000001` | application name (`utf16_u16`) |
| 1 | `0x000002` | app major, minor (`u32`, `u32`) and patch name |
| 2 | `0x000004` | account/user triplet plus integer metadata |
| 3 | `0x000008` | two `f64` values |
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
`0x80`. They are nevertheless length-prefixed and must not be read as
permanently fixed `u32`s.

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
| 10 | `0x000400` | 49-byte font/cache table records |
| 11 | `0x000800` | imported-data height |
| 12 | `0x001000` | deprecated/unknown `u32` |
| 18 | `0x040000` | custom-object list; internals partly unresolved |

The three fixture field masks are `0x471`, `0xd71`, and `0xd71`; their parsed
flexible fields end exactly at `layer_offset`.

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
event-forwardable and locked. Flexible-field bits are:

| Bit | Field |
| ---: | --- |
| 0 | transparency (`u8`) |
| 1 | background color (`u32`) |
| 2 | layer name (`utf16_u16`) |
| 3 | layer UUID (`utf16_u16`) |
| 4 | modified time (`i64`) |
| 5 | thumbnail media ID (`u32`) |

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
records. Children immediately follow their parent and must be walked before the
next sibling.

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

Unknown future IDs must be retained rather than treated as corruption.

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
mask `0x6000`. Base property bits describe rotatable, selectable, movable,
visible, replayable, clippable, template, flippable, ATT, locked and inverted
removable behavior. Known flexible fields include rotation; attachment/media
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
    repeat point_count:
        f64 x, f64 y, f32 pressure, i32 timestamp
        if property bit 2: f32 tilt, f32 orientation
    u8 tool_or_input_type_0
    u8 tool_or_input_type_1

frame_start + flexible_data_offset:
    stroke style fields in field-mask order
```

The coordinate deltas use a sign bit plus Q10.5 magnitude. Pressure, tilt and
orientation deltas use a sign bit plus Q3.12 magnitude. Timestamp deltas are
zero-extended `u16` values.

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
| handwritten | `0x25` × 2,732; `0x05` × 37 |
| quiz | `0x25` × 2,578; `0x05` × 644; `0x65` × 6 |
| CS61BL | `0x25` × 1,095; `0x425` × 73; `0x05` × 17 |

### Stroke flexible field mask

Confirmed native serializer mappings:

| Bit | Value | Field |
| ---: | ---: | --- |
| 1 | `0x000002` | pen/name string-table ID |
| 2 | `0x000004` | ARGB color (`u32`) |
| 3 | `0x000008` | pen size (`f32`) |
| 4 | `0x000010` | one-byte property |
| 7 | `0x000080` | advanced pen-setting string-table ID |
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
Its 28 flexible bytes decode as seven four-byte values: pen ID, ARGB color,
size, advanced-setting ID, fixed width, particle density and initial tolerance.

## `media/mediaInfo.dat`

Modern form (`format_version > 3001`):

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

Older form omits the top-level version, record-size prefix and attached byte,
then uses the older S Pen end-of-file tag instead of `EOFX`.

Media names are resolved as `media/<file_name>`. Media payloads are not one
format: objects may reference PNG/JPEG/PDF/audio/video or Samsung-specific
formats. The handwritten fixture contains `0@page_0000000.spi`; its payload
does not have a standard image magic and remains undecoded. Its manifest's
64-character hash exactly equals the lowercase hexadecimal SHA-256 of that
payload.

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

The handwritten file is 144 bytes: its first `u16` is 142 and its last 22
bytes are the signature. Its two variable blobs are empty and it predates the
app-custom-data field. Newer quiz/CS61BL tags include a zero-length `u32` for
that optional string and are 148 bytes. A reader must therefore walk the
length-prefixed strings/blobs and use the declared size; fixed offsets such as
`0x48` and `0x50` only work when all preceding strings are empty.

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

Protected-document save paths generate a random AES-256 content key, encrypt
the original file with AES-CBC/PKCS7, derive a 256-bit key-encryption key with
PBKDF2-HMAC-SHA1 (4,000 iterations and a random 32-byte salt), wrap the content
key with AES-CBC using the random 16-byte IV, and append a readable end tag.
The native append path also emits a minimal copied ZIP EOCD header before that
tag, so encrypted files retain a discoverable tail. This path is source-
confirmed but has not yet been checked against an encrypted fixture.

## Why some strokes currently appear in the top-right corner

The current visible-stroke parser does not walk the structural page/layer/object
records. It starts one byte into an outer object header and uses offsets whose
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
| handwritten | 74 | 74 |
| quiz | 68 | 68 |
| CS61BL | 49 | 49 |

## Parser architecture implied by the APK

The robust parse path should be:

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

Concrete changes for this repository:

1. Make the structural `StoredPage` parser authoritative for page traversal.
2. Rename page/note header fields to flexible offsets and variable mask blocks.
3. Add a reusable generic frame parser bounded by `frame_size` and
   `flexible_data_offset`.
4. Decode stroke frames only inside outer type-1 object payloads.
5. Delete `StartPointMinusThree` and all selection-by-larger-point-count logic.
6. Decode point channels according to stroke property bits, never by guessing
   from remaining bytes.
7. Decode style fields in field-mask order and preserve unknown masked bytes.
8. Verify note, page-manifest and page signature/hash links when requested,
   while keeping logical object/layer hash validation separately configurable.
9. Retain unknown object types and unknown flexible fields for forward
   compatibility.

## Still unresolved

- Exact public semantic names for several obfuscated page and layer properties.
- Complete fixed/flexible layouts for every non-stroke object type.
- The proprietary `.spi` media payload format.
- Byte-exact protected/encrypted end-tag variant without an encrypted fixture.

These gaps do not block fixing stroke geometry: the frame and stroke layouts
needed for that are now source-backed and exhaustively boundary-validated on
the available handwritten corpus.

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
