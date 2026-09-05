# WDoc end-tag findings

## Evidence

These findings use Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
No newly captured document was needed for this analysis.

| Source | Contract |
| --- | --- |
| Decompiled `r1/w.java`, `b(RandomAccessFile)` | Current field order and the two-byte record-length prefix |
| Decompiled `f2/a.java`, `Y` and `Z` | Little-endian UTF-16 unit counts, including null sentinels |
| ARM64 `libSPenModel.so`, `SPen::EndTag::ParseImpl(IInputStream*, bool)`, `0x2a77b4` | ZIP EOCD lookup, comment skipping and outer record extraction |
| ARM64 `libSPenModel.so`, buffer `EndTag::ParseImpl`, `0x2a7d20` | Signature validation, WDoc minimum version and historical extension boundaries |
| ARM64 `libSPenModel.so`, `EndTag::EncryptionData::ApplyBinary`, `0x2a7308` | Plaintext size and length-prefixed salt, IV and wrapped key |

The buffer reader receives the payload without its two-byte length prefix.
The file representation includes that prefix, and its count includes the final
22-byte `Document for S-Pen SDK` signature. The format version is a full `u32`.
WDoc records require version 2034 or later; older SDoc formats have separate
native branches and signatures.

## Field boundaries

The [file-format schema](file-format.md#end_tagbin) describes the current writer.
Its mandatory WDoc core ends at `page_mode`. The native reader permits a record
to end before each following extension:

1. Document type.
2. Owner ID.
3. Reserved blob, including its length.
4. Encryption blob, including its length.
5. Display-created and display-modified timestamps together.
6. Last-recognized-data modification time.
7. Fixed font, text direction and background theme together.
8. Server checkpoint.
9. New orientation.
10. Minimum unknown version.
11. Application custom data, using a `u32` UTF-16 count.

All earlier strings use `u16` UTF-16 counts. A count of all ones denotes null;
zero denotes an empty string. There is no padding between strings and numbers.
An absent extension differs from a present zero-valued extension. Optional
string fields represent both absence and null as `None`.

## SDK implementation

`parse_end_tag_bytes` and `parse_end_tag_bytes_with_limits` read complete file
records. `ParsedDocument.end_tag` retains the authoritative structured metadata,
and `end_tag_source` distinguishes the appended record from the archive member. Strings,
blobs and extension groups are bounded by the declared payload, excluding the
signature. Unknown bytes after application custom data are retained.

Document metadata uses display timestamps when present and falls back to core
timestamps for older tags. The former fixed-offset reader could interpret
string contents as timestamps. The detailed API preserves the full `u32`
version; the existing `FormatVersion(u16)` metadata field only receives versions
it can represent and otherwise remains eligible for the note-header fallback.

Malformed optional ZIP members produce `InvalidEndTag` diagnostics and do not
populate metadata. Configured byte or text-limit failures remain fatal.
Encryption data is retained as bytes. `StoredEndTag::encryption_info()` decodes
the original plaintext size and the salt, initialization vector and wrapped key,
preserving unknown trailing bytes. The method bounds each length against the
encryption blob itself, so fields cannot borrow bytes from later timestamps.
The pure end-tag parser retains opaque encryption bytes; archive parsing also
validates their structure before accepting a tag.

## Validation and remaining work

`crates/sdocx/tests/end_tag_contracts.rs` reconstructs the Java writer's field
sequence with distinct timestamps and nonempty strings. It checks Unicode
surrogate pairs, nulls, every historical extension boundary, partial groups,
signature/size corruption, future versions, unknown tails, metadata propagation
and resource limits. These are binary-contract tests, not Samsung export or
visual fidelity measurements.

The stream reader skips the ZIP EOCD's variable-length comment before reading
the outer tag. The SDK now uses that record in preference to `end_tag.bin`.
It scans a bounded tail large enough for both maximum `u16` lengths: the ZIP
comment and the end-tag payload, plus their fixed headers. The native reader's
65,535-byte scan window is smaller; supporting both maximum lengths together
is a parser extension validated synthetically.

ZIP decoding receives a reader ending immediately before the appended tag.
Consequently an EOCD-shaped byte sequence inside tag metadata cannot replace
the archive directory. A malformed recognized outer tag produces a diagnostic
and permits fallback to the inner member; configured limit violations remain
fatal. A tag inside the ZIP comment is not an appended record.

Tests cover differing inner/outer timestamps, absent inner members, ZIP comments,
ZIP64, preambles, maximum lengths, false footer bytes in metadata, malformed
trailers and limits. A marker in a valid prefixed ZIP no longer triggers the
legacy protected-document heuristic; that fallback only applies after ZIP
opening fails. ZIP directory validation remains delegated to the ZIP library.
The appended layout assumes a single-disk archive and a trailer ending at EOF.
No new Samsung-exported or protected document has been used to validate it.

Native `SPen::EndTag::Append`
at `0x2a9810` writes the saved 20-byte EOCD prefix, a zero comment length, then
the serialized tag; `0x2a9bc4` starts those three writes. This explains why
protected ciphertext retains a discoverable ZIP-shaped tail.

The SDK reports `ProtectedDocument` when the selected tag contains a nonempty,
structurally decodable encryption appendix. It checks appended metadata before
ZIP decoding, including when ciphertext happens to begin with `PK`. This is
conservative classification: it does not authenticate ciphertext, validate AES
parameters, reproduce the native pointer-based `IsEncrypted` predicate, or
decrypt anything. Malformed appendices receive end-tag diagnostics and the
normal metadata fallback behavior.

Synthetic coverage includes a copied footer after an opaque payload, ZIP-like
payload prefixes, inner and outer protection metadata, every truncated appendix,
oversized blob counts and unknown extension bytes. A future protected Samsung
export is still required for end-to-end cryptographic validation.
