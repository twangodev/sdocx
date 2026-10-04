# Structural standalone text boxes

## Native evidence

Confirmed from Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`:

- Outer object type 2 uses frames `0 + 6 + 7 + 2`. These are the object base,
  shared shape, shared shape text, and text-box-specific settings respectively.
  `ObjectTextBox::NewApplyBinary` delegates to the inherited shape reader and
  then applies the text-box remainder.
- Frame 7 field bit 0 contains a `u32` byte length followed by `TextCommon`.
  `ObjectShapeText::ApplyBinary_TextData` advances by that declared length.
  Its optional field bit 1 consumes a further byte containing the text-area
  mode, detailed below.
- `ComponentImage::TextboxGetOwnBinary` writes final frame type 2 with a
  one-byte property mask and a two-byte field mask. Its minimum header is 15
  bytes, with no fixed data. Flexible field bits 1, 2 and 3 store a four-byte
  border color, `f32` border width and `u16` border type. With no fields, the
  writer can use a zero flexible offset.
- Type 0 provides the format version, length-prefixed UTF-8 identity, raw
  modification timestamp and four `f64` bounds. Its first flexible field,
  selected by bit 0, is `f32` rotation.

See [`source-map.md`](source-map.md) for the disassembly addresses. These are
serializer findings; the available real-document corpus does not contain
a Samsung-exported standalone-text-box case.

## Text-area mode and separate visibility state

Frame 7 flexible bit 1 stores the text-area mode after the sized `TextCommon`
at bit 0. It can also appear without bit 0. Modern
`ObjectShapeBinaryHandler::GetOwnBinary` reads `ObjectShapeText` member 16 at
`0x3a8fc8`, omits zero at `0x3a8fcc`, and writes one byte at `0x3a8fd0`. The
field mask is set to 2 without text or 3 with text at `0x3a8fb4`–`0x3a8fdc`.

`ObjectShapeText::ApplyBinary_TextData` checks bit 1 at `0x3b2230`, validates
one available byte, and consumes it at `0x3b2244`–`0x3b2250`. Values below 3
are stored in member 16 at `0x3b2344`; absent or larger values set that member
to zero at `0x3b225c`. `ObjectShapeText::GetTextAreaType`, `0x3b30f8`, and
`ComponentText::GetTextAreaType`, `0x3a0bc8`, read the same member. The JNI
bridge `ObjectShape_getTextAreaType` reads it at `0x3bda24`–`0x3bda28`.

Decompiled `SpenObjectShape.java:64-66` names the values:

| Stored byte | Native name | SDK variant |
| --- | --- | --- |
| 0 | `TEXT_AREA_TYPE_MARGIN` | `TextAreaType::Margin` |
| 1 | `TEXT_AREA_TYPE_FREE` | `TextAreaType::Free` |
| 2 | `TEXT_AREA_TYPE_PATH` | `TextAreaType::Path` |
| 3–255 | Native reader normalizes to zero | `TextAreaType::Other(byte)` |

The SDK exposes `text_area_type: Option<TextAreaType>` on `RichTextBox` and
`NativeShape`, including a shape with no text payload. Embedded shape text
receives the same mode. `None` preserves absence, and `Some(Margin)` preserves
an explicit zero. `TextAreaType::raw` retains the byte, including unknown
values. Text slicing preserves the mode. Parsing consumes the byte within the
frame so later pen and fill fields stay aligned. Free, path and unknown modes
continue to report incomplete text-area layout support; naming the mode does
not implement its native wrapping or geometry.

Text visibility is a different state. `ComponentText::IsTextVisible` at
`0x3a0e1c` reads `ObjectShapeText` byte 20. `SetTextVisibility` at `0x3b1b0c`
updates that byte at `0x3b1b60` and forwards the value to
`TextCommon::SetTextVisibility`, whose store at `0x3e5330` updates its own
implementation byte 68. The `ObjectShapeText` constructor initializes byte 20
to true at `0x3af0b8` and the text-area member to zero at `0x3af0c4`.
`GetShapeBinary_PropertyFlag` at `0x3a7f84` reads byte 21 for property bit 2;
it does not serialize byte 20 through that flag. A text-area byte or the
editable flag does not establish text visibility. The drawing check
is documented in [object drawing findings](object-drawing-findings.md).

Synthetic tests exercise every byte value with and without text, absent versus
explicit zero, text slicing, the complete visible object path, and following
shape fills. A truncated field cannot borrow its byte from the next frame.

## Persisted auto-fit and ellipsis

Modern frame 7 also persists two one-byte options, independently of text-area
bit 1. `GetShapeBinary_TextBoxInfo`, `0x3b2120`–`0x3b2174`, writes ellipsis
at bit 12 unless it is 2, and auto-fit at bit 13 unless it is 3. Actual
constructor data at `0x163320`, copied by `0x3af0f4`, initializes those
members to TRIANGLE=2 and BOTH=3. `SpenObjectShape.java` names ellipsis
NONE=0/DOTS=1/TRIANGLE=2 and auto-fit
NONE=0/HORIZONTAL=1/VERTICAL=2/BOTH=3.

The modern reader bounds-checks each byte but stores raw values
(`0x3a97e4`–`0x3a9850`). Absent options retain the object's current members;
fresh-object defaults do not prove reused objects reset on each load. Public
setters instead validate auto-fit 0–3 and ellipsis 0–2 (`0x39f5bc`,
`0x39f86c`). Getters substitute NONE for invalid auto-fit and TRIANGLE for
invalid ellipsis (`0x39f808`, `0x39fac0`). This differs from text-area's
reader normalization of absent/invalid values to MARGIN.

The older standalone `ApplyTextBoxOptionBinary`, `0x39e204`, forwards to
`ApplyTextBoxBinary_Option`, `0x3b273c`, using ellipsis bit 11 and auto-fit
bit 12; its caller is `ObjectTextBoxBinaryHandler` at `0x3ab588`. These shifted
bits do not describe modern frame 7 or the older generic-shape option reader.

`RichTextBox` currently exposes text-area but no typed auto-fit/ellipsis.
`note.rs::parse_text_frames` reports their bits or remaining bytes as shape-text
extension fields. Persisted options and native consumers therefore remain
separate from implemented SDK behavior; see
[text options and derived overflow](text-layout-findings.md#text-options-and-derived-overflow).

## Native TextCommon object-span trailer

The pinned Model ELF SHA-256 is
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.
After margins, gravity and saved sections, `m_GetBinary_WDoc`, `0x3f61c8`,
writes an object-list presence byte followed by seven zero bytes
(`0x3f6314`–`0x3f6334`). Its size method, `0x3f5fbc`, accounts for this
header and the known list only. `m_ApplyBinary_WDoc`, `0x3f6568`, reads it
for format versions at least 2035 (`0x3f68dc`), tests only bit 0 of the
first word and skips the second word; upper flags and trailing bytes have
no separate consumer in this pair.

The list writer/reader (`0x3ff0d4`, `0x3ff234`) and ObjectSpan WDoc pair
(`0x417be4`, `0x417c98`) contain the embedded object's complete WDoc bytes,
UTF-16 index, layout option and layout constraint. Runtime identity/history
and measured object margins are separate from this enclosing wire header.
No additional vector/resource-owning trailer field is justified by these
complete source paths; embedded object payloads can retain their own fields.

[Rust decoding](../../crates/sdocx/src/note.rs) owns the known embedded bytes
and placement fields. Unknown upper flags or remaining common bytes only
record an extension diagnostic; they are not owned. The reserved u32 is
consumed and discarded without diagnosing a nonzero value. This source-only
result establishes neither runtime replay nor all-version extension coverage.

## SDK decoding

`StoredPage` traversal dispatches outer type 2 directly to a bounded frame
reader. It retains empty text, whitespace, a single
character, Unicode, and small or negative placement coordinates.

The note rich-text decoder reads `TextCommon` inside frame 7's
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

The historical `01-basic-formatting.sdocx` conformance check passed against
the shared decoder. The separate [fixture audit](fixture-validation.md)
recorded 7,182 strokes and 924,442 points across three retired audit inputs.

A disposable synthetic archive was converted to SVG and PNG through the CLI.
Its text, rotation and stroke appeared, and the stored border generated the
expected warning. The SVG preserves Japanese/CJK text; this machine's installed
fonts produced missing-glyph boxes for those characters in the PNG. This is
not a Samsung reference export or evidence of complete visual equivalence.

## Rendering behavior and evidence limits

Standalone, flowing and embedded text share UTF-16 span resolution for local
color, size, emphasis, decorations and hyperlinks. Only full-text spans become
box defaults; caret spans and partial formatting do not style the whole box.
The shared Rust engine measures font faces, wraps text and applies margins,
paragraph layout and ordinary vertical gravity. Its transport and composition
limits are recorded in [vector text support](../text-vector-support.md).
Standalone placement and typography have synthetic coverage; no Samsung
standalone-text reference pair establishes their visual parity.

Diagnostics describe detected unsupported features; their absence does not
certify a lossless parse or render. Inherited base properties and nested
extension semantics remain incomplete. The [image decoder](image-findings.md)
uses explicit media references, and the [shape/line decoder](shape-line-findings.md)
uses native fields and shared `TextCommon` for embedded shape text.
