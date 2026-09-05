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
records. `ParsedDocument.end_tag` retains their structured metadata. Strings,
blobs and extension groups are bounded by the declared payload, excluding the
signature. Unknown bytes after application custom data are retained.

Document metadata uses display timestamps when present and falls back to core
timestamps for older tags. The former fixed-offset reader could interpret
string contents as timestamps. The detailed API preserves the full `u32`
version; the existing `FormatVersion(u16)` metadata field only receives versions
it can represent and otherwise remains eligible for the note-header fallback.

Malformed optional ZIP members produce `InvalidEndTag` diagnostics and do not
populate metadata. Configured byte or text-limit failures remain fatal.
Encryption data is retained as bytes; this change does not decrypt documents.

## Validation and remaining work

`crates/sdocx/tests/end_tag_contracts.rs` reconstructs the Java writer's field
sequence with distinct timestamps and nonempty strings. It checks Unicode
surrogate pairs, nulls, every historical extension boundary, partial groups,
signature/size corruption, future versions, unknown tails, metadata propagation
and resource limits. These are binary-contract tests, not Samsung export or
visual fidelity measurements.

The stream reader skips the ZIP EOCD's variable-length comment before reading
the outer tag. The outer record can differ from `end_tag.bin`, so implementing
its precedence remains the next step. Protected-document encryption appendices
still need structured decoding and validation against a future protected sample.
