# WDoc document metadata

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
This work uses the APK and synthetic records; no new SDOCX exports were needed.

| Source | Confirmed behavior |
| --- | --- |
| Decompiled `n1/h.java:536-711` | Ordered document fields, masks, collection counts and pen serialization |
| `libSPenWDoc.so`, `WNoteLoadHandler::loadNoteFile_FlexibleData`, `0xa9644` | Native dispatch order through bit 22 |
| `libSPenModel.so`, `MetaData::Load`, `0x2c2338` | Application, author and geographic metadata |
| `MetaData::m_Load_AuthorInfo`, `0x2c2b18` | Three strings followed by an image media ID; `0xffffffff` means no image |
| `MetaData::m_Load_AuthorInfo_Str`, `0x2c3120` | Signed `-1` string length means null; zero means an empty string |
| `NoteDoc_getAuthorInfo`, `0x2f752c` | Native string slots map to Java `name`, `phoneNumber`, `email`, then resolved `imageUri` |
| `WNoteLoadHandler::loadFlexibleData_LastPenInfo`, `0xab8a8` | Pen size includes its four-byte prefix; a trailing three-field extension is optional |
| `VoiceData::GetBinary`, `0x8e194`; `ApplyBinary`, `0x8e38c` | Voice identity, strings, timestamps and action records; recording time is optional in older records |
| `loadFlexibleData_FixedProperties`, `0xac0c8` | Fixed font, text direction and background theme |
| `loadFlexibleData_TextSummarization`, `0xac450`; `loadFlexibleData_AppCustomData`, `0xac6a8` | UTF-16 strings with 16-bit and 32-bit length prefixes respectively |

All addresses are ARM64 virtual addresses. WNote and voice symbols are in
`libSPenWDoc.so`; shared metadata and the author JNI bridge are in
`libSPenModel.so`. The JNI field-name strings at `0x156717`, `0x12dc3c`,
`0x159ec5` and `0x13970e` confirm the author field names and ordering.

## Ordered fields

The note header's first offset locates flexible data. Its field mask controls
the following sequence; unset fields occupy no bytes. All numeric values are
little-endian. Unless otherwise specified, strings contain a `u16` count of
UTF-16 code units followed by those units.

| Bit | Layout | SDK field |
| ---: | --- | --- |
| 0 | String | `application_name` |
| 1 | `i32` major, `i32` minor, string patch | `application_version` |
| 2 | Nullable name, phone and email strings; `u32` image ID | `author` |
| 3 | `f64` latitude, `f64` longitude | `location` |
| 6 | String | `template_uri` |
| 7 | `i32` | `last_edited_page_index` |
| 9 | `u32` image ID, `i64` time | `last_edited_page` |
| 10 | `u32` payload size, string table | `string_table` |
| 11 | `i32` | `body_font_size_delta` |
| 12 | Unsized compatibility pen settings | `compatible_pen` |
| 13 | `u32` count, individually sized voice records | `voices` |
| 14 | `u16` count, string name and `u32` media ID per entry | `attachments` |
| 15 | `u32` total size including prefix, current pen settings | `pen` |
| 16 | `i64` | `server_checkpoint` |
| 17 | String | `fixed_font` |
| 18 | `i32` | `fixed_text_direction` |
| 19 | `i32` | `fixed_background_theme` |
| 20 | String | `text_summarization` |
| 21 | `i32` | `stroke_group_size` |
| 22 | String with `u32` code-unit count | `app_custom_data` |

The earlier format map described bit 2 as account/user data and bit 3 as two
unidentified doubles. Native names and the JNI bridge establish that these are
author contact information with an image reference, and latitude/longitude.
The decoder preserves scalar values rather than applying UI defaults or
guessing meanings for text-direction/background-theme enum values.

Bits 4, 5 and 8 have no confirmed payload layout. An encountered unknown bit
stops decoding at that point, including for wider future masks. Known earlier
fields remain available, `first_unparsed_field` identifies the boundary, and
`trailing_data` preserves all remaining bytes. The decoder does not attempt to
locate later fields by searching their contents.

## Sized records

String tables contain a `u16` entry count and ordered pairs of `u32` ID plus
string. Their outer `u32` size excludes that prefix. Duplicate IDs and unknown
bytes after the entries are retained. Attachment names and IDs are also kept
in stored order, including duplicates; they are references, not extracted files.

Each voice record is preceded by a `u32` payload length excluding its prefix:

```text
u32 media_id
string name
string play_time
i64 created_time
u32 event_count
repeat event_count: i32 action, i64 time
optional i64 recording_time
remaining extension bytes
```

`VoiceData::ApplyBinary` checks whether it has reached the record end before
reading recording time (`0x8e4cc`–`0x8e4f0`). The decoder follows this boundary:
absence is represented as `None`, partial timestamps fail, and bytes following
a complete recording time are retained. The two time representations and event
actions are exposed as stored; no playback or synchronization behavior is
implemented. Decompiled `r1/z.java` confirms the display names.

Both pen forms begin with:

```text
string name
f32 size
u32 ARGB color
u32 curvable
string advanced_setting
u32 eraser_enabled
i32 size_level
i32 particle_density
```

The compatibility form then stores three `f32` HSV values and `u32` color UI
information. The current form inserts `f32` particle size and `u32` fixed-width
flag before HSV. Its optional extension consists of `u32` fixed-opacity flag,
`u32` automatic-size flag and `f32` fit ratio. The native reader checks the
declared end once before this complete extension (`0xabe04`–`0xabe60`) and seeks
to `block_start + total_size` afterward (`0xabf4c`–`0xabf60`). The SDK rejects
partial extensions and retains bytes after a complete extension. Compatibility
and current settings remain separate values; neither silently replaces the other.

## SDK access and bounds

`StoredNote::metadata` explicitly decodes metadata from the same complete,
uncompressed `note.note` entry used for `parse_note_bytes`:

```rust
fn inspect_note(note_bytes: &[u8]) -> sdocx::Result<sdocx::NoteMetadata> {
    let note = sdocx::parse_note_bytes(note_bytes)?;
    note.metadata(note_bytes)
}
```

`metadata_with_limits` accepts `ParseLimits`. Entry size and each string's
UTF-16 code-unit count are bounded. `max_note_metadata_entries` limits the
combined number of string-table entries, voices, voice events and attachments
across one metadata decode, with a default of 10,000. Counts are checked against
both this budget and the minimum possible bytes before reserving arrays.

Sized tables, voice records and pen blocks each have independent readers.
A malformed record cannot consume the next field to satisfy its declared
contents. The decoder excludes the final 32 bytes as the note hash trailer;
callers must supply a complete entry. Metadata decoding does not verify that
hash. Use the separate [integrity checks](integrity-findings.md) for verification.

Metadata decoding is explicit and does not alter ordinary document parsing,
rendering defaults, media resolution or network behavior. A malformed optional
field produces an error from this method while structural note parsing remains
available. Raw author image/media IDs are retained without fetching anything.

## Validation and remaining work

`crates/sdocx/tests/note_metadata.rs` covers all 20 mapped fields individually
and in a single consecutive record, every truncated prefix of those field
payloads, supplementary Unicode, null versus empty author strings, repeated IDs,
sized-record isolation, historical pen/voice boundaries, wider masks and
aggregate allocation limits. Deliberately invalid hash bytes demonstrate that
metadata decoding is independent of integrity verification.

Real exports are still needed to validate combinations emitted by Samsung's UI
and their rendering implications. Further APK work can map fixed-property enum
values, voice actions and attachment resolution. Document-level style settings
are exposed for inspection; they are not yet applied to the renderer.
