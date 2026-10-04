# WDoc end-tag findings

## Evidence

These findings use Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.

| Source | Contract |
| --- | --- |
| Decompiled `r1/w.java`, `b(RandomAccessFile)` | Current field order and the two-byte record-length prefix |
| Decompiled `f2/a.java`, `Y` and `Z` | Generic UTF-16 writers support null sentinels; this is distinct from the native EndTag inline-string reader |
| ARM64 `libSPenModel.so`, `SPen::EndTag::ParseImpl(IInputStream*, bool)`, `0x2a77b4` | ZIP EOCD lookup, comment skipping and outer record extraction |
| ARM64 `libSPenModel.so`, buffer `EndTag::ParseImpl`, `0x2a7d20` | Signature validation, WDoc minimum version and historical extension boundaries |
| `ReadString2`, `0x2788ac` | Counted inline UTF-16 String, without a null sentinel |
| ARM64 `libSPenModel.so`, `EndTag::EncryptionData::ApplyBinary`, `0x2a7308` | Plaintext size and length-prefixed salt, IV and wrapped key |

The buffer reader receives the payload without its two-byte length prefix.
The file representation includes that prefix, and its count includes the final
22-byte `Document for S-Pen SDK` signature. The format version is a full `u32`.
WDoc records require version 2034 or later; older SDoc formats have separate
native branches and signatures.

The [native reader version authority](note-header-findings.md#native-reader-version-authority)
distinguishes the default minimum-format gate from note/page version normalization
and Rust's raw declarations and display metadata.

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

All earlier strings use `u16` UTF-16 counts; zero denotes an empty string.
There is no padding between strings and numbers. An absent extension differs
from a present zero-valued extension.

The native buffer reader uses inline `ReadString2` for note ID (`0x2a7ef0`),
cover image (`0x2a7f40`), application name (`0x2a7fa4`), patch name
(`0x2a7ff4`), owner ID (`0x2a8140`) and fixed font (`0x2a82e8`). The helper
reads an unsigned `u16`, checks `count * 2` bytes, sets the String from those
counted units and advances the cursor (`0x2788d8`–`0x278914`). It does not
interpret `0xffff` as null. Fixed-font helper failure branches directly to
parser failure at `0x2a82ec` → `0x2a7e34`. Application custom data uses the
inline `ReadLongString2` at `0x2a83ac`. These members differ from native
nullable String-pointer readers used by other formats.

Generic Java helpers `f2.a.Y/Z` can write all-ones null sentinels. However,
`r1.w.b` computes its strings' `.length()` values, including fixed font,
before calling those helpers. The generic helper contract does not establish
that this EndTag writer emits null fixed fonts.

## SDK implementation

`parse_end_tag_bytes` and `parse_end_tag_bytes_with_limits` read complete file
records. `ParsedDocument.end_tag` retains the authoritative structured metadata,
and `end_tag_source` distinguishes the appended record from the archive member. Strings,
blobs and extension groups are bounded by the declared payload, excluding the
signature. Unknown bytes after application custom data are retained.

The current Rust decoder additionally accepts all-ones string counts as null;
its optional string fields represent both absence and accepted null as `None`.
The synthetic null-string test pins this SDK behavior, not native inline-string
parity. In particular, a short `0xffff` fixed-font input passes Rust null
handling but fails the native counted-payload check. The native outer record
length is itself `u16` (`0x2a7a30`–`0x2a7a94`), so a 65,535-unit font cannot
fit inside a valid tag. This is an acceptance difference for an input rejected
by the native reader, not evidence of lost valid maximum-length font data.

Document metadata uses display timestamps when present and falls back to core
timestamps for older tags. The detailed API preserves the full `u32`
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

## Validation and evidence limits

`crates/sdocx/tests/end_tag_contracts.rs` reconstructs the Java writer's field
sequence with distinct timestamps and nonempty strings. It checks Unicode
surrogate pairs, nulls, every historical extension boundary, partial groups,
signature/size corruption, future versions, unknown tails, metadata propagation
and resource limits. These are binary-contract tests, not Samsung export or
visual fidelity measurements.

A local Rust/Unicorn probe executes actual `ReadString2` and Base String
construction, getters and destruction, with allocation fills 0, 85, 165, 255
and repeated 0. A second process replay is byte-identical. Native bounds and
String decisions are unpatched; host services supply allocation, memory and
platform/log/error calls. Independent ABI and observer-purity review passed.

| Counted-string input | Native result | Consumed bytes | Remaining bytes |
| --- | --- | ---: | ---: |
| Empty, count 0 | Accepted, empty String | 2 | 0 |
| `Roboto`, count 6 | Accepted, six UTF-16 units | 14 | 0 |
| `ffff` followed by direction/theme 2/2 | Rejected, String remains empty | 2 | 8 |

The probe output SHA-256 is
`d3ad926efe23bb8cfce25e47d6c5873d6547e50fdbadccc316f807ec02264784`.
This is a local counted-string helper probe, not a published full EndTag-parser
capture or a Samsung-exported fixture.

The stream reader skips the ZIP EOCD's variable-length comment before reading
the outer tag. The SDK uses that record in preference to `end_tag.bin`.
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
trailers and limits. A marker in a valid prefixed ZIP does not trigger the
legacy protected-document heuristic; that fallback only applies after ZIP
opening fails. ZIP directory validation remains delegated to the ZIP library.
The appended layout assumes a single-disk archive and a trailer ending at EOF.
The appended layout lacks Samsung-exported or protected-document validation.

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
oversized blob counts and unknown extension bytes. End-to-end cryptographic
behavior remains unverified against a protected Samsung export.
