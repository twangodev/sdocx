# WDoc document metadata

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Evidence consists of APK sources and synthetic records.

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
| `loadFlexibleData_FixedProperties`, `0xac0c8` | Reads fixed font/direction/theme copies and compares them with existing native state |
| `loadFlexibleData_TextSummarization`, `0xac450`; `loadFlexibleData_AppCustomData`, `0xac6a8` | UTF-16 strings with 16-bit and 32-bit length prefixes respectively |

All addresses are ARM64 virtual addresses. WNote and voice symbols are in
`libSPenWDoc.so`; shared metadata and the author JNI bridge are in
`libSPenModel.so`. The JNI field-name strings at `0x156717`, `0x12dc3c`,
`0x159ec5` and `0x13970e` confirm the author field names and ordering.
Coedit and page-theme consumers below are in `libSPenComposer.so`; ordinary
body callers are in `libSPenBodytext.so` and `libSPenWidget.so`, and font
measurement/service addresses are in `libSPenText.so`.

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

Native names and the JNI bridge identify bit 2 as author contact information
with an image reference, and bit 3 as latitude/longitude.
The decoder preserves scalar values rather than applying UI defaults or
guessing meanings for text-direction/background-theme enum values.

Bits 4, 5 and 8 have no confirmed payload layout. An encountered unknown bit
stops decoding at that point, including for wider future masks. Known earlier
fields remain available, `first_unparsed_field` identifies the boundary, and
`trailing_data` preserves all remaining bytes. The decoder does not attempt to
locate later fields by searching their contents.

## Fixed-property authority

Native `WNoteImpl::ApplyEndTagData`, `0xa1d50`, copies the EndTag font String
at offset 216 into Impl offset 800 (`0xa1e18`–`0xa1e30`), and its direction
and theme integers at offsets 232/236 into Impl offsets 816/820
(`0xa1e34`–`0xa1e3c`). `ConstructImpl` applies the EndTag at `0xa38f4` before
the note loader at `0xa392c`; Reload uses the same order. `FillEndTagData`
copies those members back at `0xa3d14`–`0xa3d28`.

The flexible-data loader reads temporary values, compares these existing
members and logs differences: font at `0xac110`–`0xac13c`, direction at
`0xac164`–`0xac18c`, and theme at `0xac1b4`–`0xac1dc`. It does not assign
the copies to native state. The font count is limited to 1024 UTF-16 units at
`0xac244`–`0xac24c`. Missing integer fields compare temporary `-1`; that value
is not a native default. The Impl constructor initializes empty font and
numeric direction/theme 2/2 (`0xa0314`–`0xa032c`, String Construct at
`0xa0bd4`–`0xa0bd8`). Model `EndTag::Clear` does
the same (`0x2aa034`–`0x2aa054`); buffer parsing alone does not clear an
existing EndTag when a tail field is absent.

`WNote::SetFixedFont`, `0x9af2c`, copies or clears the String and marks changed
state, without calling a font manager or text layout. `GetFixedFont`,
`0x9b028`, returns null for the empty member. Composer
`preSetCoeditModeInNative`, `0x388e64`, reads fixed direction only after an
actual `IsCoeditMode` check. Values 0/1 reach the context-provided display's
virtual slot 16; other values leave it unchanged (`0x388e74`–`0x388ee8`).
This is a bounded coedit display policy. The
[EndTag findings](end-tag-findings.md#field-boundaries) separately distinguish
native inline-string validation from generic Java null-sentinel support.

The ordinary body has an explicit font policy. Bodytext
`BodyTextLayout::SetBodyTextDocument`, `0xafe54`, constructs a fresh Widget
`ObjectTextLayout` and sets its system-font flag to `!WNote::IsCoeditMode`
(`0xafed4`–`0xafef0`). `BodyTextDocument::IsSystemFontEnabled`,
`0xaaf5c`–`0xaaf84`, repeats this rule; a null note leaves the flag enabled.
Widget's constructor enables it at `0xd3198`–`0xd31a0` and takes layout
direction from context display virtual slot 112 (`0xd31d8`–`0xd3200`).
Bodytext's manager factory, `0xcee14`–`0xcee9c`, transfers note width, density
and body font size delta, without transferring the fixed-font String.

The fresh TextLayout calls `RichText::Construct` at Text `0x8a944`, which
initializes default-name member 160 to null (`0x61de4`). Measurement passes
that member and the system-font flag into `ParagraphMeasure`
(`0x78c70`–`0x78ca4`); its Minikin implementation retains the name in member
32 (`0x766b8`–`0x766ec`). Nonnull saved NAME spans take precedence over that
default at `0x7715c`–`0x77164`, as detailed in the
[font-name findings](text-layout-findings.md#font-name-payload-and-measured-fallback).
With a null/empty name, `FontManager::GetFontFamilyNameByFontName` returns an
empty family for enabled system-font selection (`0x84fd4`–`0x84fec`), or
calls `FontListParser::GetDefaultFontFamily` when disabled (`0x85038`).
This proves the ordinary caller's inputs, not its device-specific physical
face. No fixed-font override was found in this inspected body caller chain;
it does not justify applying retained EndTag font to every ordinary run.

Decompiled `ComposerDocInitialization.setFixedParamCoeditNote` explicitly
gates `isCoeditMode()` before setting direction LTR/theme LIGHT. Shared-note
conversion in `AddNoteToSharedNotebookUseCase.updateDocument` sets those
values; `DeleteCoeditNoteUseCase.updateWNote` resets both to DEFAULT.
Composer's separate page-theme helper, `ContentsView::setPageBackgroundTheme`,
`0x417fb8`–`0x418060`, derives theme from page PDF/background state and context
color conversion before reading/updating `WPage` theme. That inspected helper
does not read WNote's fixed theme. These source paths do not establish a
global fixed-direction/theme override outside coedit.

The font service has a separate caller-byte route. Java
`SpenFontManager.setCustomFallbackFont` supplies a language and byte array;
Text JNI `0x5f484` copies the array at `0x5f588`–`0x5f594`. Actual
`FontListParser::SetCustomFallbackFont`, `0x80a1c`, requires a nonempty native
String comparing equal to `ko` (`0x80a64`–`0x80a7c`, string at `0x25d10`),
a nonnull buffer, positive length and an existing native fallback family
(`0x80adc`–`0x80ae0`). It constructs a font with face index 0,
weight 400, italic false and empty axes (`0x80af4`, `0x80b4c`–`0x80b58`). The
memory-backed font constructor copies bytes into its own storage at
`0x88484`–`0x884a0`; JNI frees its temporary at `0x5f69c`. Native String
construction can terminate at U+0000, so this equality does not prove that
the original Java language had exactly two code units. This runtime service
does not establish serialization of custom font bytes into WDoc.

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

Calling these metadata accessors does not mutate the stored note. A malformed
optional field produces an error from the method while structural note parsing
remains available. Raw author image/media IDs are retained without fetching anything.

`ParsedDocument.end_tag.fixed_style` retains the authoritative fixed-property
carrier. Ordinary `DocumentMetadata` has no corresponding fields;
`container.rs::apply_end_tag_metadata` does not project them. Its optional note
metadata projection keeps body-font-size delta and the stroke string table,
not these fixed-property copies. The detailed document also does not retain
the complete original note entry or a decoded `NoteMetadata`; callers need
their original entry bytes to inspect its flexible data. Font-name span
payload retention and caller-supplied physical font bytes remain separate
source paths.

## Validation and evidence limits

`crates/sdocx/tests/note_metadata.rs` covers all 20 mapped fields individually
and in a single consecutive record, every truncated prefix of those field
payloads, supplementary Unicode, null versus empty author strings, repeated IDs,
sized-record isolation, historical pen/voice boundaries, wider masks and
aggregate allocation limits. Deliberately invalid hash bytes demonstrate that
metadata decoding is independent of integrity verification.

The synthetic cases do not validate combinations emitted by Samsung's UI or
their rendering implications. Fixed properties have the bounded authority and
coedit policy described above; broader enum/appearance semantics, voice actions
and attachment resolution remain unestablished. Fixed-property settings
are exposed for inspection; they are not applied to the renderer.
