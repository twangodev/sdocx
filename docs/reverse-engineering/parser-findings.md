# APK-aligned parser findings

## Data flow

```text
SDOCX bytes
  -> appended end tag and ZIP directory
  -> note.note + pageIdInfo.dat
  -> pages in manifest order
  -> page header and flexible fields
  -> layer records
  -> recursive outer object records
  -> bounded typed frames
  -> object-specific decoders
  -> document model
  -> shared Rust layout and rendering
```

`StoredPage` drives traversal. Page and layer masks are length-prefixed;
typed object frames use declared sizes, relative flexible offsets and
variable-length masks. Stroke decoding handles compressed and uncompressed
channels, color and pen size.

## Object decoding

The shared type-0 reader exposes identity, bounds and rotation. Standalone text
uses the `0 + 6 + 7 + 2` chain and bounded `TextCommon` parsing. Embedded text
recursion is limited; unsupported features appear in `ParseReport` and CLI
output. See [text-box findings](text-box-findings.md).

Images use `0 + 6 + 7 + 3` for placement and explicit main, border and original
references. The main ID resolves through the bounded modern media manifest.
Regression cases cover reordered, missing, repeated and ambiguous IDs. See
[image findings](image-findings.md).

Shapes and lines use `0 + 6 + 7` and `0 + 6 + 8` for geometry, styles, native
pen references and embedded shape text. Supported templates and curves render
to SVG. See
[shape/line findings](shape-line-findings.md).

Type-4 containers retain ordered children and root selection. Unreadable common
container metadata produces `UnsupportedContainerFeature`. Known outer types
without semantic decoders produce `UnsupportedObjectType`; unknown future IDs
retain a distinct warning. See [object drawing findings](object-drawing-findings.md).

Table/code inheritance chains and bounded embedded row/cell data are confirmed.
All 14 table fields, row-height constraints and sized borders are decoded, with
complete masks and trailing bytes retained. Parsing those fields does not
establish native layout parity. See [table/code findings](table-code-findings.md)
and [vector text support](../text-vector-support.md) for rendering scope.

Math, formula and plot metadata have bounded inspection APIs. Formula records
retain expressions, answers, image references, embedded strokes and label
graphs. Expression evaluation and automatic formula/graph rendering are not
established. See [math](math-findings.md), [formula](formula-findings.md),
[formula rendering](formula-rendering-findings.md) and [plot](plot-findings.md)
findings.

## Metadata and integrity

Note headers use declared mask lengths and flexible-data boundaries. All 20
mapped flexible fields have explicit bounded decoders. Common object metadata
retains visibility/editing flags, replay/resize values, full masks and frame
extensions; 17 mapped flexible fields include SOR/extra-data bundles. A separate
static extraction format uses a different layout from modern frames.
See [note headers](note-header-findings.md), [note metadata](note-metadata-findings.md),
[object base](object-base-findings.md) and [object flexible](object-flexible-findings.md).

Layer decoding exposes identity, transparency, alpha-lock/shadow flags and
retained shadow payloads. Object and layer visibility bits have different
encodings. Hidden recognized objects and their subtrees are excluded from
semantic decoding. Saved physical-layer selection is documented separately in
[page/layer selection findings](page-layer-selection-findings.md).
The one-byte transparency contract and discrepant Java writer are documented
in [layer findings](layer-findings.md).

Cursor-based inner and appended end tags support historical optional fields,
bounded strings and ZIP comments. Optional note, object, layer, page and
manifest integrity checks distinguish mismatches from unavailable checks.
See [end-tag](end-tag-findings.md) and [integrity](integrity-findings.md) findings.

## Compatibility rules

- Manifest page order overrides ZIP order.
- Media bind IDs override asset encounter order and filename prefixes.
- The post-EOCD end tag is authoritative and can differ from `end_tag.bin`.
- Modern attachments use `note.note` plus `media/`, rather than the generic
  `attach/attachInfo.dat` form.
- Parsing does not require a particular ZIP compression method.
- Unknown object IDs and unexpected trailing manifest bytes are preserved.
- Configured bounds apply before page, object, text or point arrays are allocated.
- Identity hashes are not payload authentication: Samsung's object hash covers
  UUID plus modification time, rather than raw object bytes.

## Evidence limits

Page custom-object internals, some legacy fields and byte-for-byte encrypted
file validation remain unverified. SPI framing and codec investigations have
their own [native media findings](spi-media-findings.md) and
[codec validation findings](spi-codec-validation.md); experimental reconstruction
is separate from SDK integration. Structural decoding and synthetic tests do
not establish visual parity without matching Samsung captures.
