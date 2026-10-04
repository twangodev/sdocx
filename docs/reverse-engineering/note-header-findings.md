# WDoc note headers

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.

| Source | Confirmed behavior |
| --- | --- |
| ARM64 `libSPenWDoc.so`, `WNoteLoadHandler::loadNoteFile`, `0xa88c0` | Reads the first offset, masks and fixed data, then seeks to flexible data |
| `loadNoteFile_PropertyFlag`, `0xa8bfc` | Reads a byte count followed by that many property bytes |
| `loadNoteFile_FieldCheckFlag`, `0xa8dec` | Reads a separate byte count and field mask |
| `0xa8d64`–`0xa8d78` | Property bit 3 controls background inversion; bit 4 disables tape visibility |
| Decompiled `n1/h.java:478-530` | Fixed field order and sized title/body objects |
| Decompiled `n1/h.java:712-737` | Backfills the flexible offset and masks before hashing the complete note payload |

The native mask readers support widths up to four bytes and consume the declared
width. Current Java output uses two four-byte masks, which places the version at
offset 14. That offset is a property of the current writer, not an invariant of
the reader. The native property getters and Java names identify background
inversion and tape visibility; they do not encode a background RGB color there.

## Native reader version authority

These are static ARM64 traces in the pinned Model and WDoc libraries.

Default Model `EndTag::Parse` overloads supply true (`0x2a77a4–0x2a77b0`).
Its buffer reader, `0x2a7d20`, rejects WDoc type 2 EndTag format versions below
2034, as described in [EndTag findings](end-tag-findings.md). With that boolean
true and type 2, it also rejects unsigned **minimum-format versions above 5500**
(`0x2a801c–0x2a808c`, Error 12); false bypasses this minimum gate.
WDoc stream `ConstructLoad` calls `ConfigureCache` (`0x90c54`), whose
`CheckEndTag` route invokes default Parse before loading cache files
(`0xa22f8`, `0xbcb60`). The minimum is therefore an admission control in this
route, rather than only display metadata.

The note's fixed record supplies a separate format authority.
`loadNoteFile_FixedData`, `0xa8fb4`, reads format into note implementation +248,
rejects unsigned values below 2034 (`0xa9034–0xa9038`), and changes values above
5500 to 5500 before loading its body (`0xa9158–0xa918c`).
`loadFixedData_NoteData`, `0xaa180`, reads the note minimum into +252
(`0xaa47c–0xaa498`) without a value comparison in that function.
`ApplyEndTagData`, `0xa1d50`, does not replace these two members.

Initial page-header admission receives the normalized **note** version:
Note Load passes its +248 pointer in `PageLoadInfo` (`0xa87fc–0xa8800`);
manager `loadPage` dereferences it before `WPage::LoadHeader`
(`0xb1e44–0xb1e5c`). Handler `LoadHeader`, `0xd234c`, requires incoming unsigned
version at least 2034 (`0xd2380–0xd2384`) before reading the page header.
Page `LoadHeader_FixedArea`, `0xd2c1c`, then reads its own format into page +312
and changes values above 5500 to 5500 (`0xd2d38–0xd2d7c`). It reads minimum
into +292 (`0xd2d90`) without a value gate in that function. Later `LoadObject`
checks the **page's own** format is at least 2034 (`0xd50b4–0xd50bc`).
`LoadLayerForChild`, `0xd0848`, supplies that own format to `WLayer::Load`
(`0xd085c–0xd0894`) and subsequent layer/default-record dispatch.
This does not make it an explicit argument to every ordinary object codec:
Model's ordinary slot +408 receives buffer, size, orientation and width; type-15
compatibility decoding separately receives the enclosing version
(`0x359818–0x359834`, `0x3598c0–0x3598d8`).

File `WNoteFile::GetFormatVersion`/`GetMinFormatVersion` (`0xbf070`/`0xc0688`)
parse the EndTag with the default gate, return its declarations, and return zero
on failure. Live WNote getters (`0x92864`/`0x928bc`) instead read the normalized
note format and retained fixed minimum. `GetMinUnknownVersion`, `0x92914`,
reads the separate opaque-version member +256 and maps `INT_MAX` to zero;
it is not either saved minimum field. SDK `GetSdkFormatVersion`, `0xc00f0`,
returns 5500. Native save backfills the live note format/minimum
(`0xaf748`/`0xaf7d8`); `FillEndTagData` copies them to EndTag +8/+112
(`0xa3c20–0xa3c30`, `0xa3cc0–0xa3cd4`). Independent declarations need not agree.

Rust retains raw full-width note and optional page versions without those
native high-version changes. Its EndTag decoder already rejects format below
2034, but retains the minimum without the native upper gate. Optional invalid
EndTags become diagnostics and no tag, except fatal limit errors.
The legacy metadata version projection and EndTag/note fallback below govern
display metadata, not object codec selection. Raw declarations, normalized
native state and original source bytes remain distinct; structural parsing
does not certify native admission or future semantic compatibility.

## Rust decoding

`parse_note_bytes` walks both length-prefixed masks and the UTF-16 note ID.
Fixed fields, title and body are read within the declared flexible-data offset.
A corrupt string or object length cannot consume bytes beyond that boundary.
Unknown fixed bytes after the body are retained in `StoredNote.fixed_trailing_data`.
Input size and note-ID length honor the configured limits before allocation.

`StoredNoteHeader.property_mask` and `field_mask` retain all bytes. The existing
`header_flags` and `property_flags` fields preserve their low 32-bit views, while
the legacy `header_constant_1` and `header_constant_2` values are the actual mask
byte counts. `flexible_data_offset()` exposes the meaning of the legacy
`integrity_offset` field. Named `inverts_background_color()` and `tape_visible()`
accessors expose the confirmed property bits.

The parser also retains masks wider than four bytes, consistent with its other
forward-compatible mask readers. This exceeds the analyzed native reader's
accepted width and is tested synthetically; no meaning is assigned to unknown
bits. Malformed structural offsets fail parsing even when integrity checks
are disabled. Missing hash bytes remain an integrity-coverage issue when the
fixed fields and declared flexible boundary are otherwise readable.

Archive metadata comes from the structured note header. The format version
is never truncated to fit the legacy `FormatVersion(u16)` field. Core note times
provide a fallback when a valid end tag did not supply timestamps; valid end-tag
metadata retains precedence. Flow dimensions and padding use the decoded fields,
and physical page dimensions continue to come from the first ordered page.

The document background comes from the first ordered page's explicit decoded
background field. No native note-header field encodes that RGB color.

## Validation and evidence limits

`crates/sdocx/tests/note_header.rs` exercises all combinations of one- through
four-byte masks, Unicode IDs, distinct timestamps, shifted dimensions, wider
future masks, fixed extensions, limits and malformed offsets. It also verifies
end-tag precedence, full-width format versions and a deliberate false color
pattern next to an independently encoded page background.

Integrity tests cover raw-note hashes; an offset outside the record fails
structurally before optional integrity reporting. Synthetic checks do not
establish compatibility with unobserved writer variants.

Document-level flexible fields have a bounded decoder, including
application metadata, pen settings, attachment/voice references and fixed font,
text-direction and background-theme properties. See
[note metadata findings](note-metadata-findings.md) for its evidence and limits.
