# Structural standalone text boxes

## Native evidence

Confirmed from Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`:

- Outer object type 2 uses frames `0 + 6 + 7 + 2`. These are the object base,
  shared shape, shared shape text, and text-box-specific settings respectively.
  `ObjectTextBox::NewApplyBinary` delegates to the inherited shape reader and
  then applies the text-box remainder.
- Frame 7 field bit 0 contains a `u32` byte length followed by `TextCommon`.
  `ObjectShapeText::ApplyBinary_TextData` advances by that declared length.
  Its optional field bit 1 consumes a further byte; its meaning remains
  unresolved here.
- `ComponentImage::TextboxGetOwnBinary` writes final frame type 2 with a
  one-byte property mask and a two-byte field mask. Its minimum header is 15
  bytes, with no fixed data. Flexible field bits 1, 2 and 3 store a four-byte
  border color, `f32` border width and `u16` border type. With no fields, the
  writer can use a zero flexible offset.
- Type 0 provides the format version, length-prefixed UTF-8 identity, raw
  modification timestamp and four `f64` bounds. Its first flexible field,
  selected by bit 0, is `f32` rotation.

See [`source-map.md`](source-map.md) for the disassembly addresses. These are
serializer findings; the available real-document corpus does not yet contain
a Samsung-exported standalone-text-box case.

## Implemented decoding

`StoredPage` traversal now dispatches outer type 2 directly to a bounded frame
reader. It no longer searches that payload for UUID-shaped strings, plausible
rectangles or ASCII-looking text. It retains empty text, whitespace, a single
character, Unicode, and small or negative placement coordinates.

The existing note rich-text decoder now reads `TextCommon` inside frame 7's
flexible slice. A declared text length cannot borrow bytes from frame 2, the
object hash or a sibling. The same bounded frame reader is used for note text,
embedded tables and code blocks. Text/style/paragraph/object-span limits still
apply. `max_object_nesting_depth` also bounds embedded rich-text object chains,
independently of the physical child-record tree.

The document model receives bounds, rotation, text, spans, paragraphs, sections,
margins, gravity and embedded content. UTF-16 style offsets remain available;
bold/italic runs are converted to character offsets without splitting emoji.
`StoredObject::base_metadata(page_bytes)` exposes common identity and placement
through the public `ObjectMetadata` type.

Malformed required text frames fail with the page ID and object payload offset.
Unknown style/paragraph kinds retain their payloads. Unsupported inline objects
retain their bytes. Detected unsupported shape settings, text extensions,
borders and additional frames produce `UnsupportedTextBoxFeature` diagnostics
in `parse_detailed` / `parse_bytes_detailed`. The CLI prints diagnostics on
stderr; WASM's existing inspection report exposes the same category. Simple
`parse` APIs continue to return only the document.

## Regression evidence

Twelve synthetic tests in `crates/sdocx/tests/structural_text_boxes.rs` cover:

- Empty/short/Unicode text, whitespace, explicit bounds and rotation.
- UTF-16 style ranges, paragraph records, sections, margins and gravity.
- Multiple layers, child objects and mixed text/stroke pages, with an unknown
  object containing a decoy text payload.
- Every truncation of a text-box payload, wrong frame types, inflated lengths,
  non-finite placement/margins and incomplete declared border fields.
- Unsupported inline content, future mask bytes/frames and caller limits,
  including a five-level text/code/text/code/text chain.
- SVG placement, rotation, color and bold/italic spans.

The text-preservation regression fails against `ab3c152`: the previous scanner
returns zero elements for the synthetic text object. The nesting-limit
regression fails against `d52d2b8`: a five-level embedded chain is accepted with
a limit of four. Both pass with the current implementation. These comparisons
used isolated archive checkouts and separate Cargo target directories.

The external `01-basic-formatting.sdocx` conformance check still passes, as do
all three handwritten regressions (7,182 strokes and 924,442 points). The
workspace tests, Clippy, Rust 1.88 checks and WASM target checks pass.

A disposable synthetic archive was converted to SVG and PNG through the CLI.
Its text, rotation and stroke appeared, and the stored border generated the
expected warning. The SVG preserves Japanese/CJK text; this machine's installed
fonts produced missing-glyph boxes for those characters in the PNG. This is
not a Samsung reference export or evidence of complete visual equivalence.

## Remaining gaps

- Obtain a Samsung standalone-text fixture plus matching reference PDF to
  verify native placement, wrapping and style fidelity against real output.
- The standalone SVG renderer still approximates typography: several style
  values are selected for the whole box, and margins, paragraph layout,
  gravity, borders and embedded-object layout are not fully rendered.
- Diagnostics describe detected unsupported features; their absence does not
  certify a lossless parse or render. Inherited base properties and nested
  extension semantics remain incomplete.
- The subsequent [image migration](image-findings.md) replaces image scanning
  and encounter-order media assignment. Shapes and lines retain bounded
  best-effort interpretation and are the next structural migration.
