# WDoc note headers

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
No newly captured SDOCX document was required.

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

## SDK changes

`parse_note_bytes` now walks both length-prefixed masks and the UTF-16 note ID.
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
bits. Malformed structural offsets now fail parsing even when integrity checks
are disabled. Missing hash bytes remain an integrity-coverage issue when the
fixed fields and declared flexible boundary are otherwise readable.

Archive metadata now comes from the structured note header. The format version
is never truncated to fit the legacy `FormatVersion(u16)` field. Core note times
provide a fallback when a valid end tag did not supply timestamps; valid end-tag
metadata retains precedence. Flow dimensions and padding use the decoded fields,
and physical page dimensions continue to come from the first ordered page.

The previous container helper searched all note bytes for a color-shaped pattern.
No native note-header field supports that search. It could match text or unknown
data and report that as a document background. That scan is removed. The document
background now reflects the first ordered page's explicit decoded background
field, which the renderer already reads structurally.

## Validation and remaining work

`crates/sdocx/tests/note_header.rs` exercises all combinations of one- through
four-byte masks, Unicode IDs, distinct timestamps, shifted dimensions, wider
future masks, fixed extensions, limits and malformed offsets. It also verifies
end-tag precedence, full-width format versions and a deliberate false color
pattern next to an independently encoded page background.

The two earlier container tests using partial fake note headers were replaced
by complete synthetic archive cases. Integrity tests still cover raw-note hashes;
an offset outside the record now fails structurally instead of reaching optional
integrity reporting. These checks do not replace comparison with future exports.

Document-level flexible fields remain the next note-specific decoding work:
application metadata, pen settings, attachment/voice references and fixed font,
text-direction and background-theme properties. Their mask order is recorded in
the [format map](file-format.md#document-flexible-field-mask).
