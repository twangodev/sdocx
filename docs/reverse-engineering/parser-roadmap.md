# APK-aligned parser roadmap

## Target data flow

```text
SDOCX bytes
  -> appended end tag and ZIP directory
  -> note.note + pageIdInfo.dat
  -> pages in manifest order
  -> page header and flexible fields
  -> layer records
  -> recursive outer object records
  -> generic typed frames
  -> object-specific decoders
  -> stable document model
  -> SVG/PNG renderer
```

## Implementation sequence

The structural stroke milestone is implemented: `StoredPage` drives traversal,
page/layer masks are length-prefixed, and a bounded typed-frame reader handles
compressed/uncompressed stroke channels plus color and pen size. The legacy
stroke walker and shifted fallback are removed. Public header field names are
retained for compatibility. The numbered sequence below remains the overall
roadmap; full common-object metadata, remaining styles and
non-stroke semantics are still incomplete.

The structural text-box milestone is also implemented. A shared type-0 reader
exposes identity/bounds/rotation; standalone text follows `0 + 6 + 7 + 2` and
reuses bounded `TextCommon` parsing. Embedded text recursion is limited, and
detected unsupported features appear in `ParseReport` and CLI output. See
[`text-box-findings.md`](text-box-findings.md) for test evidence and limits.

The structural image milestone is implemented: `0 + 6 + 7 + 3` supplies bounds,
rotation and explicit main/border/original references; the main ID resolves
through the bounded modern media manifest. Reordered, missing, repeated and
ambiguous IDs have regression coverage. See [`image-findings.md`](image-findings.md).

The structural shape/line milestone is implemented: `0 + 6 + 7` and `0 + 6 + 8`
expose geometry, styles, native pen references and embedded shape text. Common
templates, straight lines and supported native curves render to SVG. The
remaining UUID/text heuristics have been removed from page parsing. See
[`shape-line-findings.md`](shape-line-findings.md) for evidence and limits.

The visual comparison runner is implemented and measured against the existing
five-page formatting export. Explicit PNG font selection reduces the observed
font-substitution mismatch; see [visual findings](visual-conformance-findings.md).
The next priority is real Samsung standalone-text, image and shape/line fixtures
using the [capture checklist](../../conformance/fixture-capture.md), followed by
measured crop, wrapping and style fixes. Controlled Unicode font coverage is the
next improvement measurable on the existing pair.
Cursor-based inner and appended end tags are implemented, including historical
optional fields, bounded strings, ZIP comments and authoritative outer metadata.
See [end-tag findings](end-tag-findings.md). Optional note, object, layer, page
and manifest integrity checks are implemented with separate mismatch and
unavailable counts; see [integrity findings](integrity-findings.md). Advanced
outer objects still need native frame research.

Layer identity and style metadata are available through bounded explicit
decoding, including native alpha-lock/shadow flags and retained shadow payloads.
See [layer findings](layer-findings.md) for the Java transparency discrepancy and
the remaining rendering work. Layer identity supplies the inputs used by logical
hash verification.

Common object metadata exposes confirmed visibility/editing flags, replay and
resize values, full masks and bounded frame extensions. Object and layer
visibility have different bit encodings. Applying visibility during rendering
and decoding later common flexible fields remain; see
[common object findings](object-base-findings.md).

Note headers now use their declared mask lengths and flexible-data boundary.
Container metadata uses the decoded header and page background rather than
fixed offsets or color-pattern searches. See [note-header findings](note-header-findings.md).
All 20 mapped note flexible fields have an explicit bounded metadata decoder,
with null author strings, pen variants, voice/attachment references and fixed
style properties. See [note metadata findings](note-metadata-findings.md).
Native table/code-block inheritance chains are confirmed, and embedded table
row/cell fixed data is bounded by its declared flexible offsets. All 14 table
fields, row-height constraints and sized borders are decoded, with complete
masks and trailing bytes retained. Applying those styles to rendering and
standalone support remain; see
[table/code-block findings](table-code-findings.md).
Known outer object types without semantic decoders now produce
`UnsupportedObjectType` diagnostics, including container/group payloads whose
children are still traversed. Unknown future IDs retain their distinct warning.
Math objects have explicit envelope inspection for sized formula binaries,
margins, angle mode and connected plot references; see [math findings](math-findings.md).
Formula metadata exposes LaTeX inputs/results/substitutions, answer text,
image references, embedded strokes and label graphs with named relation kinds,
recognition stroke indices and start/end labels.
Expression-type semantics and math rendering remain open; see
[formula findings](formula-findings.md).
Native formula image/ink precedence and placement dependencies are traced in
[formula rendering findings](formula-rendering-findings.md); visible-stroke
bounds and enclosing transforms remain before automatic rendering.
Plot metadata and graph expressions/styles also have explicit bounded inspection;
evaluation and graph rendering remain open. See [plot findings](plot-findings.md).

1. Correct page and note header names: flexible offsets and variable-length
   property/field masks.
2. Make `StoredPage` traversal authoritative for layers and recursive objects.
3. Add a generic typed-frame reader using `frame_size`, relative
   `flexible_offset`, and variable-length masks.
4. Add the type-0 base-frame decoder and expose UUID, modified time, bbox and
   common flexible fields.
5. Add the type-1 stroke decoder for compressed and uncompressed channels.
6. Delete the magic-offset stroke walker and shifted fallback.
7. Decode stroke style fields in mask order; retain unknown fields safely.
8. Cursor-parse the variable-length inner and appended end tags.
9. Add optional validation for note, manifest, object, layer and page hashes.
10. Move text/image/shape/table/code-block parsing onto the same frame model.

## Compatibility rules

- Manifest page order overrides ZIP order.
- Media bind IDs override asset encounter order and filename prefixes.
- The post-EOCD end tag is authoritative and can differ from `end_tag.bin`.
- Modern attachments use `note.note` plus `media/`; do not generate the generic
  `attach/attachInfo.dat` form.
- Do not require a particular ZIP compression method.
- Preserve unknown object IDs and unexpected trailing manifest bytes.
- Apply configured bounds before allocating page, object, text or point arrays.
- Do not equate identity hashes with payload authentication: Samsung's object
  hash covers UUID plus modification time, not raw object bytes.

## Remaining research

- Multiple layers and recursive child/container fixtures.
- Formula label-graph semantics, expression and graph evaluation, native
  table/code layout, and standalone decoding for remaining non-stroke types.
- Page custom-object internals and a few legacy common fields.
- Proprietary `.spi` payload semantics.
- Byte-for-byte encrypted-file validation with a protected fixture.
