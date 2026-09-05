# WDoc integrity verification

## Evidence

This implementation uses Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
No new SDOCX sample was used for this milestone.

| Decompiled source | Confirmed contract |
| --- | --- |
| `h3/d.java:46-59` | SHA-256 of explicitly UTF-8 encoded input |
| `n1/b.java:56-76` | Object UUID concatenated with signed decimal modification time, then hashed |
| `n1/b.java:78-94` | Parent first, then recursive children in serialized order |
| `n1/u.java:1274-1342` | Layer and page digest construction, stored hashes and page signature |
| `n1/h.java:712-737` | Hash all note bytes after header backfill, then append the 32-byte digest |

The [format map](file-format.md#integrity-graph) records the complete equations
and the historical fixture validation. Those historical results are not new
measurements from the current fixture set.

## SDK interface

Set `ParseOptions.verify_integrity` to `true` and use a detailed parse API.
`ParsedDocument.integrity` then contains separate counts for note payloads,
object identities, layers, pages and manifest links. Each group reports
`matched`, `mismatched` and `unavailable` checks. With the default options the
field is `None` and no integrity work runs.

```rust
let options = sdocx::ParseOptions {
    verify_integrity: true,
    ..Default::default()
};
let parsed = sdocx::parse_detailed_with_options(path, &options)?;
let integrity = parsed.integrity.as_ref().unwrap();
```

Mismatches produce `IntegrityMismatch` diagnostics containing the archive entry,
record offset where applicable, and stored/computed hashes. Missing identities,
unsupported base frames and invalid or missing hash trailers produce
`IntegrityUnavailable`. These diagnostics are nonfatal; normal decoding errors
and configured resource-limit violations remain errors. Callers decide whether
the reported coverage and mismatches permit their intended use.

The CLI exposes the same checks through `--verify-integrity`, printing all five
count groups and diagnostics to stderr. Hash findings do not stop conversion or
change its exit status. The CLI integration test exercises mismatched page hashes
and unavailable layer/manifest checks while confirming that PDF export completes.

## What is checked

The note digest covers every raw byte before its final 32 bytes, including
flexible fields. The old `StoredNoteHeader.integrity_offset` name refers to the
start of flexible data, not the hash. Validation checks that the parsed fixed
data and declared flexible start precede the trailer. `StoredNote.fixed_data_end`
retains the boundary needed for that check.

Every object's identity digest is compared with its own stored trailer. Layer
checks hash the recorded object trailers in depth-first order, followed by the
layer's identity digest. Page checks hash the recorded layer trailers followed
by the page identity digest. Recorded child trailers are used so each parent
relationship can be checked independently, even when a child's identity is
unavailable or fails its own check.

`StoredPage.integrity_offset` is the cursor position after the final layer.
The page footer must contain exactly 32 hash bytes and the 26-byte page signature
at that position. A matching signature somewhere else in the data is insufficient.

Manifest checks compare its note hash with the recorded note trailer and each
page hash with the matching recorded page trailer. Repeated page UUIDs consume
physical pages in the same deterministic order used by document parsing, rather
than replacing one another in a map. Missing notes/pages yield unavailable links.
An absent manifest contributes one unavailable manifest check; an absent note
has no note-payload check because no payload was read.

A matched parent or manifest link does not imply that all descendants matched.
Inspect every count group and the diagnostics. The object formula excludes
geometry, stroke samples, rich text, style and media bytes. Consequently these
checks establish consistency of Samsung's stored hash relationships, not payload
authentication, lossless parsing, rendering fidelity or trusted authorship.

## Validation

`crates/sdocx/tests/integrity.rs` uses reference hashes calculated independently
with Python `hashlib` from the Java formulas. The baseline contains four nested
objects, two layers, a note with flexible data, and a manifest. It includes a
Unicode identifier, negative times, both signed 64-bit extremes and an empty
layer. Reference parent/page digests are fixed constants rather than regenerated
by the production verifier.

Mutations cover object, layer, page, note and manifest hashes; unsupported
identities; truncated/displaced signatures; missing links; duplicate UUIDs;
configurable metadata limits; and the default-disabled path. A geometry mutation
deliberately leaves all identity/hash-link checks matched, documenting the
format's limited coverage. These tests validate the binary contract without
claiming new end-to-end Samsung export coverage.
