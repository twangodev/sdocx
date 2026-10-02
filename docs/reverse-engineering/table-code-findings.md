# Native table and code-block records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ARM64 virtual addresses in `libSPenModel.so` unless stated otherwise.
Evidence includes native serializers/readers and synthetic records, without new
SDOCX source/reference pairs.

| Symbol | Address | Confirmed behavior |
| --- | --- | --- |
| `ObjectCodeBlock::NewGetBinary` | `0x474b7c` | Serializes `ObjectBase`, then code-block data |
| `ObjectCodeBlockImpl::GetOwnBinary` | `0x475178` | Typed frame 23 |
| `ObjectCodeBlockImpl::GetBinary_FlexibleData` | `0x475230` | Bits 0 and 1 contain separately sized title and body text objects |
| `ObjectTable::NewGetBinary` | `0x3d9f0c` | Serializes `ObjectShape`, then table data |
| `ObjectShape::NewGetBinary` | `0x399b40` | Shape-base chain, followed by the type-7 shape frame |
| `ObjectTableImpl::GetOwnBinary` | `0x3cf1ec` | Typed frame 22 |
| `ObjectTableImpl::GetBinary_FlexibleData` | `0x3cf310` | Size limits, column widths, rows and additional table styles |
| `TableRow::NewGetBinary` | `0x3c515c` | Untyped row record with a relative flexible offset and two masks |
| `TableRow::GetBinary_FixedData` | `0x3c5230` | Height, index and individually sized cells |
| `TableRow::GetBinary_FlexibleData` | `0x3c53cc` | Maximum height under bit 9, then minimum height under bit 1 |
| `TableRow::ApplyBinary_FlexibleData` | `0x3c5840` | Reads those two fields in the same nonnumerical bit order |
| `TableCell::NewGetBinary` | `0x3c2e4c` | Untyped cell record with a relative flexible offset and two masks |
| `TableCell::GetBinary_FixedData` | `0x3c2f50` | Cell coordinates, spans, color, editability and sized text |
| `TableCell::GetBinary_FlexibleData` | `0x3c3150` | Field bit 0 contains a sized `TableBorder` record |
| `TableBorder::NewGetBinarySize` | `0x3dc9a4` | Current border payload is 73 bytes |
| `TableBorder::NewGetBinary` | `0x3dc9ac` | Untyped header followed by four 16-byte edge styles |
| `JNI_TableCellBorder::ConvertToCBorderStyle` | `0x320254` | Maps Java color, width, start radius and end radius to the native edge fields |

## Object frame chains

The code-block chain is `0 + 23`. Its type-23 frame has no currently written
fixed fields. Flexible bit 0 contains a `u32` text-object payload size followed
by the title object; bit 1 uses the same layout for the body. Sizes exclude
their own four-byte prefixes. Missing title or body is representable.

The table chain is `0 + 6 + 7 + 22`. The call at `0x3d9f4c` uses
`ObjectShape::NewGetBinary`, which calls `ObjectShapeBase::NewGetBinary` at
`0x399c08` and writes its own shape frame at `0x399c20`. The existing embedded
decoder finds the type-22 frame after the base. The native table record also
contains inherited shape geometry and styles.

## Table properties and flexible fields

`ObjectTableImpl::GetBinary_Property` at `0x3cf2cc` writes heading-column
enabled under bit 0, heading-row enabled under bit 1, and maximum-height
**disabled** under bit 2. These map to implementation bytes 185, 184 and 186,
respectively. The setters at `0x3cef0c`, `0x3ceed8` and `0x3d17dc` confirm
the names and distinguish heading rows from heading columns.

The type-22 frame has no currently written fixed fields. Its flexible fields
are serialized in ascending bit order:

| Bit | Encoding | Meaning | Getter or implementation member |
| ---: | --- | --- | --- |
| 0 | `f32` | Global minimum column width | `ObjectTable::GetMinColumnWidth`, `0x3d4364`; member 12; default 10 |
| 1 | `f32` | Global minimum row height | `ObjectTable::GetMinRowHeight`, `0x3d36bc`; member 8; default 10 |
| 2 | `u32` count, then `f32[]` | Actual column widths | Vector at offset 32 |
| 3 | `u32` count, then sized row records | Rows | Each size excludes its own prefix |
| 4 | Four `f64` coordinates | Content bounds | `GetContentRect`, `0x3c6f6c`; rectangle at offset 16 |
| 5 | `u32` size, then border record | Outer table border | `GetBorderStyles`, `0x3cb7f8`; pointer at offset 152 |
| 6 | `u8` | Auto-fit mode | `GetAutoFitOption`, `0x3da474`; byte at offset 168 |
| 7 | `u32` count, then `f32[]` | Minimum column widths | `GetMinColumnWidth`, `0x3ca290`; vector at offset 56 |
| 8 | `u32` count, then `f32[]` | Maximum column widths | `GetMaxColumnWidth`, `0x3ca2b8`; vector at offset 80 |
| 9 | `f32` | Maximum table height | `GetMaxHeight`, `0x3d11a4`; offset 176 |
| 10 | `f32` | Maximum table width | `GetMaxWidth`, `0x3d11ac`; offset 180 |
| 11 | `u32` size, then border record | Default cell border | `GetDefaultCellBorderStyles`, `0x3cb920`; pointer at offset 160 |
| 12 | `u32` ARGB | Heading background color | `GetHeadingBackgroundColor`, `0x3cc03c`; offset 188 |
| 13 | `u32` ARGB | Default cell background color | `ObjectTable::GetDefaultCellBackgroundColor`, `0x3d7ec0`; offset 192 |

The constants in `SpenObjectTable.java` identify auto-fit values 0 as none,
1 as horizontal, 2 as vertical and 3 as both. The writer omits the default
value 3. The SDK represents absent optional fields as `None` and preserves
unrecognized mode bytes as `TableAutoFit::Other`; it does not materialize
native defaults into fields that were absent from the file.

The reader stores bits 0/1 at members 12/8 (`0x3cfb38`–`0x3cfbd4`);
the public getters establish their size-limit meanings independently. Rust
fields and inspection keys are `min_column_width` and `min_row_height`;
deserialization also accepts the legacy `vertical_cell_padding` and
`horizontal_cell_padding` keys, respectively.
Cell text margins are separate: absent
margins default to zero in `TextCommonImpl` (`0x3e747c`–`0x3e7484`), and the
table-cell constructor does not replace them.

Cold Drawing measurement reads saved row heights and column widths directly
(`0xaaa60`, `0xaaab8`) and does not consult auto-fit mode. Model auto-fit acts
through `fitTableRect` and `SetAutoFitOption` (`0x3c6b94`, `0x3cbaa0`), outside
that cold measurement path. An absent mode still means native default 3;
accepting its existing geometry does not interpret absence as mode 0.

`RichTextTable.style` exposes these properties alongside existing actual
column widths, rows and cells. `TableRecordMetadata` retains complete masks
and separate fixed/flexible trailing bytes for table, row, cell and border
records. The complete embedded object also remains available through
`RichTextObjectSpan.object_data`, including inherited shape frames.

## Row and cell boundaries

Rows and cells are sized by their parent and do not have typed-frame headers.
Their own records begin with:

```text
u32 flexible_offset_relative_to_record_start
u8 property_mask_length
property_mask_bytes
u8 field_mask_length
field_mask_bytes
fixed_data
flexible_data
```

The current native writer uses one property-mask byte and two field-mask bytes,
giving a nine-byte header. It writes offset zero when no flexible fields are
present; otherwise it records the end of the fixed data. Parent size prefixes
are excluded from this relative offset.

A nonempty row's fixed data is `f32` height, `u32` row index, `u32` cell count, then a
`u32` payload size and cell record per cell. A cell's fixed data contains
`u32` column index, row span, column span and ARGB background; four `f64`
coordinates; `u8` editability; and a sized rich-text object.
Cell property bit 0 identifies an owned background color.

The byte following the cell rectangle is editability, not vertical alignment.
The writer loads member 80 at `0x3c30a0` and writes it at `0x3c30ac`.
`TableCell::SetEditable`, `0x3c24a0`, stores the same member at `0x3c24b0`;
`IsEditable`, `0x3c24c4`, reads it directly. The reader loads the byte at
`0x3c344c`, compares it with zero at `0x3c3458`, uses `cset ne` at
`0x3c345c`, then stores the boolean at `0x3c3460`. Every nonzero byte means
editable, including 2 and 255. Rust exposes `editable: bool` with this
normalization; the retained embedded object record preserves the original bytes.

The SDK bounds each fixed reader at the declared flexible offset, preventing
fixed-field lengths from consuming flexible bytes. Invalid offsets fail before
those reads. Offset zero remains valid with an empty field mask.
The shared mask reader supports wider future masks without truncating the
check for set bits. Unknown embedded bytes remain in the original object data.

### Dense editor construction and sparse transport

Native editor construction produces dense rows. `ObjectTableImpl::Construct`,
`0x3c5f60`, clamps nonpositive row/column counts to one, and
`TableRow::TableRow`, `0x3c3abc`, creates a nonnull cell for each physical
column with its matching column index.

Binary transport stores column widths independently from each row's cell
count. The row parser at `0x3c5598` preserves stored row index and cell count;
the cell parser at `0x3c3360` preserves column index. These parsers do not
validate the cross-count/index relationship. The enclosing
`ObjectTable::NewApplyBinary`, `0x3da00c`, ends with `ClearChanged`,
`0x3d96b8` through vtable relocation `0x495298`, which clears flags without
normalizing topology.

Zero-cell rows have a separate source-level writer/parser asymmetry. The
fixed-data writer, `0x3c5230`, omits the cell-count word for an empty vector;
with default bounds, the size getter reports a 17-byte record. The row parser
still expects a cell-count word. Native writer output and an explicitly encoded
zero-count input are therefore different transport cases; neither empty-row
round trip is established by the supplied topology probes below.

That transport flexibility does not establish a supported sparse renderable
document. `ObjectTableImpl::GetCell`, `0x3c7570`, checks the global column
count, then indexes the selected row's cell vector without checking its
length. Null cell holes have no binary representation: serialization
dereferences each stored cell pointer. Existing sparse algorithm probes
therefore establish behavior for supplied kernel inputs, independently of
native editor construction or valid document admission.

[`table-grid-admission.json`](../../conformance/table-grid-admission.json),
SHA-256 `826c7ec115166289fe3be59c2bae8fb754b07a54a35b2a7fbbd0d6705147c45f`,
captures 25 synthetic cases and 117 queries through native `GetCell`,
`0x3c7570`, public `GetCell`, `0x3d2be0`, and `GetFrameCell`, `0x3c75c0`.
The [capture module](../../conformance/native_table/table_grid_admission.rs)
executes 351 native calls per fill, repeated at `0x00`, `0xa5` and `0xff`.
Memory hooks record the actual instruction address and physical cell slot;
the fixture contains 76 pointer reads beyond a row's declared vector length.
Twenty-six frame-owner queries stop before the null candidate's span load at
`0x3c7678`; the null dereference itself does not execute.

Inputs supply one or two three-column rows, declared row lengths 0–3,
nonnull backing capacity beyond short declared lengths, null-slot masks and
two merged-owner-hole cases. This deliberately keeps out-of-length pointer
reads inside initialized host-supplied backing memory. Native constructors,
binary loading/writing and device execution do not occur. Those reads and
stopped null candidates explain the dense Rust admission boundary; they are
not evidence that native documents admit or safely render these topologies.

Rust's prepared grid requires nonempty dense rows, matching stored row/column
origins and positive in-bounds spans. Sparse or invalid topology retains usable
saved-frame painting with `UnsupportedContent`; it does not enter dense native
measurement preparation.

## Row height and cell-border findings

Row flexible fields are **not** in ascending bit order. Both writer and reader
place `f32` maximum height under bit 9 before `f32` minimum height under bit 1.
`GetMaxHeight` at `0x3c4244` reads member offset 124; `GetMinHeight` at
`0x3c4300` reads offset 128, matching the serialization accesses. Default maximum
height is `f32::MAX`, and default minimum height is zero. `RichTextTableRow`
exposes both values when present. If any unknown row field bit is set,
neither constraint is decoded, and the complete flexible payload is retained.
The nonnumerical native order does not establish where an unknown field would
appear, so decoding known fields past that uncertainty would be unsafe.

Cell flexible bit 0 contains a four-byte size followed by a table-border
payload. The analyzed writer writes size 73 and then calls
`TableBorder::NewGetBinary` (`0x3c31a0`–`0x3c31bc`). `RichTextTableCell.border`
decodes that record within the declared payload size. Its bytes cannot be
treated as additional cell text.

## Shared cell text layout

Addresses in this section refer to the named libraries rather than Model.
Drawing `ObjectTableCellLayout` derives from ordinary `ObjectTextLayout`:
its constructor calls the base constructor at `0x8c008`. Its `updateBound`,
`0x8c328`, calls the base implementation at `0x8c338`, then unconditionally
sets the underlying `TextLayout` gravity to 0 at `0x8c33c`–`0x8c34c`.
This is top gravity and overrides the cell content's stored gravity. The
editability byte must not select top, center or bottom gravity.

Widget `ObjectTextLayout::updateBound`, `0xd713c`, reads the content object's
four margins and multiplies each by document density and local text scale
before `TextLayout::SetMargin` at `0xd7220`. The cell adapter retains this
margin setup before overriding gravity. Model `TableCell::SetMargin`,
`0x3c2554`, delegates to the content object's `ObjectShapeText::SetMargin`
at `0x3c2560`; it does not maintain a separate cell alignment field.

Drawing `TableLayout::updateCell`, `0xae914`, supplies the measured cell-frame
width and height through `ObjectTextLayout::SetLayoutWidth` at `0xae9f4` and
`SetLayoutHeight` at `0xaea04`. `TableLayout::layoutCell`, `0xb06d4`, supplies
the same dimensions at `0xb07bc` and `0xb07cc`, then lays out the complete
text range at `0xb07dc`–`0xb07f0`. All cell lines consequently pass through
the shared text engine; native drawing does not extract the first physical line.

These findings establish adapter inputs and editability semantics. They do
not prove complete row sizing, merged-cell layout, page splitting or visual
parity for arbitrary cells.

### Captured cell text measurement

[`table-text-cell-measurement.json`](../../conformance/table-text-cell-measurement.json),
SHA-256 `890c2f236d6cb6711f0821520a491079fa3304d2b64e108a2d3dfcc787549416`,
records 14 cases, 68 UTF16 slots, 61 glyph-cache records and 24 emitted runs.
The [capture module](../../conformance/native_table/text_cell_measurement.rs)
executes Model `TableCellContentObject` construction (`0x3c17f0`),
`ComponentText::SetText` (`0x39c9d4`) and optional font-size/margin setters.
Actual Drawing cell/Widget/Content/Text constructors, `SetObject` (`0xd3974`)
and Widget `Update` (`0xd3e3c`) produce native spans and paragraphs. Widget
measurement (`0xd73b0`), native NAME selection, bundled
Minikin/HarfBuzz/Skia/FreeType, Drawing cell layout (`0x8c05c`) and native
paragraph calculation/placement execute. Real caches feed complete
`getDrawnTextRun`, Text `0x66c98`, and actual `TextLayout::GetTextBound`,
`0x8afd4`; entries, classifications, glyphs, widths and font getters are not
supplied. Strict independent replay matches the frozen bytes.

Native owned memory repeats under fills `0x00`, `0xa5`, `0xff` and zero again.
The fixture retains source text, native text, constructor slots, measured and
placed entries, glyph/Fakery/font state, getter bounds and emitted-run fields.
Cases cover narrow/wide/fractional and auto widths, combining/supplementary
text, tabs/newlines, multiple paragraphs, margins, a raw direction-1 caller
control and zero-advance text across lines. Constructor-selected font size 50
and explicitly set size 17 are distinct controls; these do not establish a
parsed document's absent-font default. The scoped
[parsed-text capture](#captured-parsed-cell-text) records common-field and
serialized-span admission separately.

The actual float-to-integer width route preserves zero. Width `0.75` reaches
measurement/layout as integer zero and selects effective width 22 for `AV` at
size 17. Width `80.9` reaches integer width 80. `AV abc` at constructor size
50 and width zero selects 157; multiple auto-width paragraphs at size 17 select
22 and 39 independently, and zero-advance paragraphs select 1. Traces record
`DoParagraphLayout`, Text `0x7278c`, with width in W2 and the separate raw W3
control; Model shape type 4 is independently observed and is not that argument.

Production table preparation now enters `layout_table_cell_text` with a
`NativeCellMeasurementWidth`: positive integer dimensions are fixed and zero
selects automatic measurement. A positive subpixel frame can therefore select
automatic width after native integer conversion. For admitted native paragraphs,
the automatic fold adds horizontal margins in f32, folds each dense entry's
advance in source order, adds `0.001f32` and rounds upward to an integer before
wrapping. It resolves each paragraph independently; margins are removed from
the resulting outer width in the captured f32 order. Focused regressions match
29 dense slots across ten admitted paragraphs, including independent widths
22/39 and zero-advance widths 1/1. Parsed-body regressions cover five explicit-17
width/multiple-paragraph/margin controls through normal/replay SVG and selectable
vector PDF without image resources.

Automatic measurement returns `UnsupportedContent` when native entries are
unavailable, including unadmitted font/tab profiles, actual bullets, nonzero
indent or empty paragraphs inside a nonempty source. An explicit bullet clear
and entirely empty cell remain supported. Generic constrained zero-width
wrapping retains its existing emergency behavior. These width checks cover the
admitted subset rather than all 14 captured inputs, native placement or complete
TableLayout/runtime parity.

Combining and supplementary continuation slots retain kind 3, height zero,
no drawable glyph cache and no font wrapper, while shaped owners retain their
actual metrics and glyph records. Native newline entries use kind 4 and flush
cached runs; the zero-advance cross-line case emits two runs. Actual Font
wrapper getters (`0x85d7c`, `0x85d98`, `0x85db0`) expose a native registry source
token, bitmap flag and C++ language string. The captured Roboto language is the
empty string, distinct from an absent font wrapper; a source token does not
identify a fontdb entry or a global ID across fresh native registries.

Caller services supply density/direction, a null text manager, identity ARGB
conversion and deterministic opaque UUIDs. Pinned host ICU 76.1/Unicode 16
replaces Android bidi/break/property/locale services; bundled HarfBuzz Unicode
tables remain native. Allocation/file/libc/single-thread services retain the
NAME capture's boundaries. The fixture records library/font/XML/host hashes,
initializers and instruction limits; TLS storage is relocated away from the
deep native stack. This establishes the single-cell producer chain for these
inputs. Complete TableLayout allocation, merged preparation, table rectangle
producers, document pagination, Model callbacks, source parsing/cloning,
origin transport, device ICU, SVG/PDF writer consumption, pixels and arbitrary
system font configuration remain outside it.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-cell-measurement scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-text-cell-measurement.json
cmp /tmp/table-text-cell-measurement.json conformance/table-text-cell-measurement.json
```

### Captured parsed cell text

[`table-parsed-cell-text.json`](../../conformance/table-parsed-cell-text.json),
SHA-256 `5801413a28a251d1313aa5574bd5882c3b70c05aee123bd463e2df35a47f55a7`,
records seven synthetic common-field/span inputs. The
[capture module](../../conformance/native_table/parsed_cell_text.rs) executes
native type-4 Model content construction (`0x3c17f0`),
`ApplyBinary_TextData` (`0x3b21d4`), the TextCommon WDoc parser, native span
factory/readers and source getters. Actual Widget `SetObject`, `Update` and
bounded native font measurement consume that parsed Model state. The explicit
parser caller version is 5500; native source `GetFormatVersion`, `0x3e6410`,
reports required format 4000 for retained common data. These are separate
caller and source properties.

When serialized common data is present with raw span count zero, native parsing
retains constructor font-size 50 and foreground ARGB `0xff252525` spans with
`ClosedClosed` range `[0, UTF16 length]`. A nonzero serialized span count clears
both constructor defaults before record admission. Thus a foreground-only
input retains no font span and measures at fallback 17; a font-only input has
black foreground. An empty-text `[0,0]` font-50 span is admitted, while that
same caret record on `AV` is rejected: its positive raw count still clears the
defaults and the text measures at 17. Absent serialized common data removes
the common object and exposes fallback font size 17 and black foreground.
Source getters, wrapper default/caret values and measured entry font sizes are
recorded separately; the wrapper's default 17 does not override retained
font-50 source spans on nonempty text.

Serialized payloads and parser ABI controls are caller inputs. Font-size and
foreground source spans are observed through native Model getters, not supplied
as measurement state. Fresh native heap fills `0x00`, `0x55`, `0xa5`, `0xff`
and a repeated zero run agree; strict independent replay matches the fixture.
The shared pinned NAME/HarfBuzz/Skia/FreeType and host ICU services retain their
existing boundaries, with supplied default device profile and identity color
conversion. The parent TableCell binary caller, complete document parsing and
layout, callback timing, clone drawing and final export do not execute.

The production loader uses `TextLoadContext::TableCell` to retain the two
constructor spans only when common data is present and its serialized raw span
count is zero. Standard text objects retain their existing loading semantics.
Serialized spans, unknown payloads and interval values remain available as raw
metadata, including the caret record rejected by native admission. Five focused
parser regressions verify scoped defaults, source preservation, effective span
limits 0/1/2, normal/replay SVG and selectable vector PDF. The synthetic retained
spans count toward the same limit as serialized spans. These establish the
implemented loading and export contracts without complete native document
parsing or rendering parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --parsed-cell-text scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-parsed-cell-text.json
cmp /tmp/table-parsed-cell-text.json conformance/table-parsed-cell-text.json
```

### Captured cell text emission

[`table-text-cell-emission.json`](../../conformance/table-text-cell-emission.json),
SHA-256 `0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce`,
records 17 cases, 78 UTF16 slots, 67 glyph-cache records and 27 native runs.
The [measurement module](../../conformance/native_table/text_cell_measurement.rs)
and [emission input observer](../../conformance/native_table/text_cell_emission.rs)
extend the actual cell producer chain above with native pre-emission state.
Complete `getDrawnTextRun`, `0x66c98`, consumes real placed entries and caches.
Native `GetSpan`, `0x61f3c`, returns the 72-byte effective span for each slot;
`GetParagraphIndex`, `0x62344`, supplies its paragraph identity. Captured inputs
also include paragraph flag at member 65, RichText gravity offset at member 212,
inclusive source range, paint offset and native inverse logical maps. Raw span
fields, source indexes and registry font tokens retain their separate meanings.

The original 14 cases retain their exact frozen fixture. Three added controls
exercise leading, consecutive and newline-only text. Kind-4 newline entries
flush accumulated glyphs without creating an empty append: `\nAV` emits range
`[1,2]`, `AV\n\nTo` emits `[0,1]` and `[4,5]`, and newline-only text emits no
runs. Real kind-3 continuation slots, font-wrapper language and source-token
identity remain part of the captured native input. Native measured/placed
entries, span results and logical maps are observed rather than synthesized.

Fresh owned-memory fills `0x00`, `0xa5`, `0xff` and repeated zero runs agree;
strict independent replay is byte identical. The actual constructor, Widget
update, font/shaping, paragraph placement and cached-emission chain shares the
14-case capture's supplied device, identity color, UUID, host ICU/file/libc and
TLS boundaries. Complete TableLayout, merged preparation, source parsing,
document pagination, Model callbacks, origin transport and final SVG/PDF writer
consumption remain excluded. This records authoritative single-cell emission
inputs and outputs. The bounded
[whole-source Rust paint-plan adapter](text-draw-identity-findings.md#certified-whole-source-paint-plans)
separately matches 12 admitted cases, 20 runs and 50 glyphs, plus newline-only
source/maps, before shared SVG/PDF transport. Other captured profiles retain
their explicit admission limits.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-cell-emission scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-text-cell-emission.json
cmp /tmp/table-text-cell-emission.json conformance/table-text-cell-emission.json
```

### Captured cell source inputs

[`table-text-cell-source-inputs.json`](../../conformance/table-text-cell-source-inputs.json),
SHA-256 `8529637cdcc67b8c5e4747f0dd6105d060ee58390809b9f252cccab35de7b9de`,
records 13 cases, 76 dense UTF16 slots, 71 glyph-cache records, 34 emitted runs
and 15 glyph records with nonzero offsets. The
[source observer](../../conformance/native_table/cell_source_inputs.rs) extends
the [actual cell producer](../../conformance/native_table/text_cell_measurement.rs):
native Model construction, text/font/margin setters, Widget conversion,
measurement, paragraph placement, bounds getters and complete cached emission
execute. Source font/foreground spans, margins, gravity and the complete
paragraph list are observed alongside the resulting dense entries and runs;
these inputs are not reconstructed from expected outputs.

Source getters run after all captured producer outputs. Model
`GetParagraphList`, `0x3e380c`, calls `GetParaList`, `0x400f08`, which normalizes
owned paragraph ranges through `0x400f90` and `0x400f9c`. Native Widget's
original conversion route also normalizes those ranges at `0x40198c` and
`0x401998`. The observer therefore executes a mutating getter, with temporary
native vector allocation/deletion, but cannot change the already captured
measurement, placement, bounds or emitted runs. These ranges index paragraphs,
not UTF16 slots. The constructor's Centre alignment value 2 covers only
paragraph range `[0,1)`; later kind-6 ParsingState records carry no alignment.
The explicit size-17 and retained constructor-size-50 controls keep their
actual source spans and foreground ARGB `0xff252525`.

Cases include stacked and uncomposed marks, leading/negative ink, supplementary
continuations, wrapped mixed Hebrew/Arabic, tabs/newlines, zero advances and
fractional margins. Independent arithmetic checks reproduce all 76 placed
slots' ink translation and layout endpoints with the native two-step f32
translation order; combining the translations changes rounding. Positive and
negative glyph-cache Y offsets remain recorded, while the cached emitter uses
entry X plus glyph-cache X and does not apply those glyph Y offsets. The Arabic
control records actual glyph 0 under the pinned font configuration; it does
not establish general Arabic shaping or system fallback coverage.

Fresh owned-memory fills `0x00`, `0x55`, `0xa5`, `0xff` and repeated zero agree;
strict independent replay is byte identical. The original 14-case measurement
and 17-case emission fixtures retain their frozen hashes. Library/font/XML and
host ICU 76.1/Unicode 16 provenance, instruction limits, initializers and the
shared supplied device, identity-color, UUID, allocation/file/libc and relocated
TLS boundaries are recorded. Complete TableLayout, source parsing/cloning,
Model callbacks, document pagination, origin transport, SVG/PDF writer
consumption and pixels remain excluded. This capture and its arithmetic checks
do not establish production adapter or export parity across all 13 cases.
The [certified Rust adapter](text-draw-identity-findings.md#certified-whole-source-paint-plans)
separately matches eight admitted LTR profiles, 14 runs, 33 glyphs and 33
UTF-16 slots; five RTL, `.notdef` or mixed profiles retain typed fallback.
Stacked-mark PDF transport preserves cached geometry, while SVG separately
validates its rigid shaping projection and diagnoses unsupported positioning.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-cell-source-inputs scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-text-cell-source-inputs.json
cmp /tmp/table-text-cell-source-inputs.json conformance/table-text-cell-source-inputs.json
```

### Captured live cell images

[`table-live-cell-images.json`](../../conformance/table-live-cell-images.json),
SHA-256 `88351ebe30102d23f31ec96a12a757d632068d00e5fba5d8b90d700e3d5bc00f`,
records eight cases, 16 cold/warm states, 64 physical-child observations and
188 dense entries, plus one actual warm image-size notification. The
[capture module](../../conformance/native_table/live_cell_images.rs) executes
genuine Model Image constructor/Construct (`0x420328`, `0x420450`), source
`SetRect` (`0x397708`), ObjectSpan setters and public cell `AppendObjectSpan`
(`0x3c26e8`). Accepted Image type 3 inserts U+FFFC into `AV`; genuinely
constructed Table type 22 and Code type 23 are rejected by this public insertion
API without changing source text/count. This does not establish saved-parser
admission rules.

Actual TableLayout cold Measure/warm Layout and Widget conversion, shaping and
placement consume the object source. Inline width/height derive from native
image bounds; block controls retain visible image width 30.5 while their dense
advance is 80, with object margins 10/10 or 20/20. Source review of
`measureObjectSpan`, Text `0x779d0`, distinguishes inline byte 64 from the block
route: `0x77a94`–`0x77ab0` subtracts MeasureParams left and right from available
width in two f32 steps and stores that advance. The visible width used for ink
is separate. `GetBlockInfo`, `0x6ad54`, sets its object-encounter flag before
fit selection, so an excluded object can retain that flag; this is not a claim
that every encountered object belongs to the selected block. Selected block
width and alignment later use the visible ink width (`0x6b15c`–`0x6b1ac`,
`0x6c61c`–`0x6c678`).

The tall-image control changes source size from 30×150.25 to 55.5×205.75 and
executes native `onObjectSpanChanged`, Drawing `0xb0d6c`, and Widget's actual
size-change/conversion route while retaining the warm cache. Image bounds,
options/constraints and that resize are caller controls; measured dimensions,
dense entries and callback outputs are native. Nil media/bitmap data excludes
loading, decoded pixels and image painting. Strict independent replay matches
the frozen 221,717 bytes across four fills `0x00`, `0x55`, `0xa5`, `0xff` and
repeat zero. Recorded library/font/ICU/runtime provenance and supplied default
width 1000/density 1 retain the shared live-table boundaries. Bodytext placement,
document pagination, raster rendering and final writers do not execute.

Production preparation admits typed Image content at a valid source anchor,
with absent/zero rotation and finite f32-projected positive dimensions. The
[shared-engine regression](../../crates/sdocx/src/render/table/native_cell_image_tests.rs)
matches six genuine image profiles through cold/warm preparation and one fresh
resize: 13 states and 52 cells, including ordinary `AV` X positions and image
X/baseline/height/ink coordinates by exact bits. Block available-width advance,
encountered-object metrics and the native two-stage f32 alignment feed the same
engine; entry layout top retains background-top semantics rather than raw line
cursor semantics. Seven malformed-input guards preserve the admission boundary.
Public SVG/replay/PDF controls check image geometry and selectable surrounding
text. Source models remain unchanged. The resized SDK plan is freshly prepared
and compares with the final native manual-notification state; retained cache
lifetime and automatic observer dispatch are not reproduced. Native image
pixels/media behavior and general saved-parser admission remain outside this
proof.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --live-cell-images scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-live-cell-images.json
cmp /tmp/table-live-cell-images.json conformance/table-live-cell-images.json
```

## Cell layout frames and integer text dimensions

Drawing `TableLayout::layoutCell`, `0xb06d4`, looks up the text wrapper and
frame cache by the exact supplied cell pointer. A null pointer or missing
wrapper/frame entry returns zero without calling the text engine. This routine
does not resolve a frame owner or combine saved row/column spans.

The split-list lookup uses the same raw cell key. Widget
`ObjectTextLayout::SetPaddingRectList`, `0xd73a0`, forwards that list to the
underlying text layout. Drawing then offsets the cached frame to local origin
and passes its `f32` width and height to Widget's setters at `0xd398c` and
`0xd399c`. `ClearLayout`, `0xd3b80`, resets the completed-layout flag before
`ObjectTextLayout::layout`, `0xd3b88`, processes the complete text range.

The setters retain fractional dimensions. Inside the wrapper's layout routine,
`0xd3cc0`–`0xd3cd0` converts each dimension to a signed integer by truncating
toward zero before calling `TextLayout::Layout`. Thus a width immediately below
40 becomes 39, while a width immediately above 40 becomes 40. Drawing's returned
height difference still subtracts the fractional local frame height from the
cached measured height at `0xb07f4`–`0xb0808`; it does not subtract the integer
text-layout height.

[`table-cell-inputs.json`](../../conformance/table-cell-inputs.json), SHA-256
`0e936c9d767d9db6556d62e4298a2aa63435be41e64d8d31d093f93182249020`,
captures 138 inputs and 850 cell-layout calls through native Drawing and Widget
instructions. Cases include covered cells and span chains, per-cell cache
frames, subpixel and zero-height frames, integer boundaries, large translated
coordinates, empty split lists and unsorted bands. Every input also verifies
that null and uncached cell pointers return zero without text calls. Allocation
fills `0x00`, `0xa5` and `0xff` produce identical results.

Frames and measured heights are supplied. Text wrappers start in measured
state with their completed-layout flag set; native `ClearLayout` removes that
flag before layout. The `TextLayout::Layout` and `SetPaddingRectList` boundaries
record inputs without shaping or assigning obstacles. Text initialization,
critical-section calls, bullet positioning, wrapper status and diagnostics are
isolated. The capture establishes cell input selection and conversion, not
native line breaking, complete merged preparation or device appearance.

The Rust regression uses the production `cell_text_bounds` conversion and
`BandList::for_row` filter. It matches floating dimensions, integer bounds,
band order/coordinates and height-difference bits for all 850 calls.

Reproduce the capture with the hash-pinned libraries:

```sh
rustc --edition 2024 -D warnings -C panic=abort conformance/native_table.rs \
  -L native=scratch/apk-analysis-runtime/python/unicorn/lib \
  -C link-arg=-Wl,-rpath,"$PWD/scratch/apk-analysis-runtime/python/unicorn/lib" \
  -o /tmp/sdocx-native-table
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-inputs scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-cell-inputs.json
cmp /tmp/table-cell-inputs.json conformance/table-cell-inputs.json
```

## Cold row sizing from raw cell measurements

Drawing `TableLayout::extendRowBySplit`, `0xaaf2c`, walks physical rows from the
supplied starting index and visits every stored column. It reads each raw cell's
cached frame directly, without resolving merged-frame ownership. A missing
frame cache skips that cell.

For each present frame it replaces the cell's split list with all supplied
bands whose top is at least the cached frame top, subtracting that frame top
from their vertical coordinates. List order is retained. It then calls native
`updateCell`, `0xae914`, and `measureCell`, `0xaea60`. The former supplies the
raw frame's local dimensions and split list to the text wrapper; the latter
returns measured-width and measured-height differences against that frame.

The driver retains the largest positive height difference across the row.
It ignores the width difference. Any positive height difference, including
values below `0.001`, extends the row and offsets subsequent rows through the
native pending-gap rules. Zero or negative differences leave the row height
unchanged. This driver does not read saved row minima or maxima. Its selection
differs from warm sizing, which reads frame-owner measurements.

[`table-cold-rows.json`](../../conformance/table-cold-rows.json), SHA-256
`c807d89806b9696aa3708a873f9c8811873693f063b5c931c2d729dd17fa5f62`,
captures 160 inputs, 163 runs, 704 measurement calls and 1,356 resulting slot
frames. It covers dense and merged grids, covered-span chains, positive-growth
boundaries, zero/subpixel heights, row starts including the end of the table,
pending gaps, unsorted/empty bands and repeated cold measurement. Minima and
maxima vary independently of measured heights. Allocation fills `0x00`, `0xa5`
and `0xff` produce identical captures.

The Rust regression runs the production cold scheduler with supplied heights.
It matches call order, fractional dimensions, local bands, frame coordinates,
pending gaps and absent/present split-cache states. Each cell's growth uses its
own fractional frame height.

The native capture executes frame initialization, the cold driver, cell update
and measurement-difference routines, frame setters/getters and row updates.
Text initialization, update/measurement, padding assignment, font selection,
single-thread synchronization and diagnostics are isolated. Measured heights
are supplied caches; native shaping, complete merged preparation and device
pagination remain unverified.

Reproduce the capture with the same compiled harness and hash-pinned libraries:

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cold-rows scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so > /tmp/table-cold-rows.json
cmp /tmp/table-cold-rows.json conformance/table-cold-rows.json
```

## Public table measurement and layout lifecycle

Drawing `TableLayout::Measure`, `0xaa530`, runs when the table is present and
its dirty byte at offset 649 is set. It clears that byte, checks the native
visible-cell list, then initializes frames, sizes cold rows and updates measured
bounds. A repeated clean `Measure` leaves the captured state unchanged.

`TableLayout::Layout`, `0xaa3d4`, dispatches dirty state to `Measure` through
virtual slot 56. For clean state it calls `layoutFromRow(0)`, `0xaa448`.
Each row receives movement/relayout decisions, owner-based sizing of that and
subsequent rows, then bottom compression. Measured bounds are updated afterward.
Cold measurement visits raw slots; warm text relayout selects self-owning cells.

[`table-lifecycle.json`](../../conformance/table-lifecycle.json), SHA-256
`a71265b72c29a2acab94934d865c8a23587befa593051075d4b53546e6eceeb5`,
captures 69 dense unmerged/merged inputs, including covered-owner chains,
fractional dimensions and ordered, reversed or empty split bands. Thirty-two
inputs enter cold measurement through dirty `Layout`. All inputs check clean
`Measure` and two warm layouts with changed bands. Rust matches 839 cell
selections and 1,665 frame snapshots, pending gaps, split caches, content/measured
bounds and warm first-page minima bit for bit. Allocation fills `0x00`, `0xa5`
and `0xff` produce identical output.

The public drivers, native ownership, frame arithmetic, text metric getters,
row movement and measured bounds execute together. Heights and line metrics
are supplied; text initialization/update/measurement,
padding/font selection, diagnostics and final observers are isolated. This
capture establishes phase composition with fixed text caches, not native
shaping, complete merged preparation or device pagination.

Reproduce with the compiled harness and the same hash-pinned libraries:

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --lifecycle scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-lifecycle.json
cmp /tmp/table-lifecycle.json conformance/table-lifecycle.json
```

### Captured live table layout

[`table-live-layout.json`](../../conformance/table-live-layout.json), SHA-256
`cedaea603a3aa3c6e07ea8bdd09af7ec7c1186f35cba0e8c4282dd5fe8309b87`,
records 18 cases, 38 states, 152 physical-child observations and 592 dense
entries. The [capture module](../../conformance/native_table/live_table_layout.rs)
executes actual Model table construction (`0x3d2690`) and 2×2
`Construct` (`0x3d27d8`), including row/cell/content constructors and native
merge where specified. Text is assigned through native
setters to each physical content object **after** merging; covered-slot text is
an explicit source control rather than a claim about ordinary editor merge
transfer. Complete Drawing `TableLayout` construction (`0xa9d58`), its native
TextManager, cold `Measure` (`0xaa530`) and `init` (`0xaa6b4`) execute with
genuine Widget constructors, source binding/update, NAME font selection and
bundled Minikin/HarfBuzz/Skia/FreeType measurement. Native cache frames,
geometry, widths, entries and font getters are not supplied.

Cold measurement constructs and measures all four physical child wrappers in
every captured case, including whole-grid merges and an empty owner with
nonempty covered text. Clean warm `Layout`, `0xaa3d4`, enters `layoutCell`,
`0xb06d4`, then Widget `layout`, `0xd3b88`, and Text `Layout`, `0x8ac10`.
Without split bands it visits four ordinary cells, three cells for the partial
merges and one owner for the whole-grid merge. The supplied-band ordinary case
visits two cells, while the whole-grid supplied-band case visits none. Those
counts establish the captured phase selection rather than a universal merged
text visibility rule.

Cases independently vary source bounds, font size 17, asymmetric margins,
text scale 0.75/1.5, document density 2, a raw direction-1 profile, local split
bands and warm column growth/shrink. Native Model `SetColumnWidth`, `0x3d3b64`,
and Drawing `resizeColumn`, `0xb0088`, execute in the two resize cases. Translated
and zero-origin cases produce identical local frames. Each physical content's
saved Model rectangle remains unchanged through warm layout and width resize
under the captured callback state. Source and wrapper text are observed
independently; the empty owner's Model empty string becomes a null Widget text
pointer without replacing its covered-slot strings.

Native `init` installs the editing callback at the child's `std::function`
storage member 64. Its active callback is present in all 152 observations;
update-size callback target at member 288 and table forwarding callbacks remain
constructor-null. This does not establish Bodytext Model-rectangle notification
timing. Snapshot observation itself executes native `getDrawnTextRun`, Text
`0x66c98`, after each stage, including between cold measurement and warm layout.
It allocates fresh DrawnText/glyph/output records retained until case reset, so
the guest heap advances. Emoji slices are null; the reviewed nil route does not
write dense entries, frames or source rectangles. This is a bounded observer
route rather than a general claim that the emitter is a pure getter.

Owned native allocations repeat under fresh fills `0x00`, `0x55`, `0xa5`,
`0xff` and zero again; strict independent full-driver replay is byte identical.
Caller context/display services supply document width 1000, density/direction,
identity ARGB conversion and local split bands through the actual setter;
native table TextManager construction executes. Host ICU 76.1/Unicode 16,
allocation/files/libc/single-thread services, UUIDs, `pow`/`acosf` primitives and
disabled ATrace remain explicit boundaries. Saved parsing/cloning, complete
Bodytext callbacks and document-bound production, pagination/split-band
production, device ICU, final writer consumption and pixels do not execute.
This establishes live native table phases for the supplied cases without full
parent-producer or SDK runtime parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --live-table-layout scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-live-layout.json
cmp /tmp/table-live-layout.json conformance/table-live-layout.json
```

### Captured live table padding

[`table-live-padding.json`](../../conformance/table-live-padding.json), SHA-256
`d9c2d969448ea5f2c517f9bdf47c91c68a9dae0866dca3b84897a9ccdd9907d5`,
records ten cases, 20 states, 80 physical-child snapshots, 320 dense entries and
40 after-warm native capacity-getter checks. The
[capture module](../../conformance/native_table/live_table_padding.rs) reuses
complete native Model/TableLayout/Widget/Text construction and cold/warm
measurement from the 18-case fixture. It observes each child's sorted native
padding vector at layout member 184 and checks `m_GetMaxCharHeight`, Text
`0x70f64`, against the exact stored f32 bits at member 276. Four fresh owned
allocation fills (`0x00`, `0x55`, `0xa5`, `0xff`) and a repeated zero run agree;
strict independent full-driver replay is byte identical.

Native `SetPaddingRectangles`, `0x70d24`, discards empty rectangles and sorts
remaining rectangles by bottom. The capacity store at `0x70ed4` uses the first
sorted pair's `second.top - first.bottom`; it is not a minimum across all gaps.
Fewer than two nonempty rectangles retain `f32::MAX`. The three-band control
returns 80 despite a later gap of 5; sorted/unsorted input controls produce
identical placement. Other controls retain capacity 20, overlap −20, touching
zero, or font-17 capacity 12. An empty-width second band is discarded.
Per-child filtering remains visible: second-row children often retain no padding
after warm layout, so their capacity stays `f32::MAX` rather than inheriting the
caller's first gap.

When requested line height with spacing exceeds capacity, native line-height
selection (`0x8e100`) falls back to base font height before collision placement
and paragraph baseline calculation (`0x6cb0c`). It does not clamp the font to
the available gap. Captured controls include the first-line collision and
asymmetric margins, retaining actual measured/placed entries, Text bounds and
cached emission. The allocating cached-emitter observer runs between phases
with null emoji slices under the same bounded route as the live-layout capture.

Rust regressions match all 18 live-layout cases and ten padding cases for exact
table/content/cell geometry, ownership, and each line's top, baseline and bottom.
`BandList::spacing_capacity` matches all 40 stored/native-getter capacities.
The resize checks compare fresh SDK reflow with the captured native resized
states; they do not establish retained native warm-cache lifecycle parity.
Horizontal Bidi/per-character placement, ink bounds and `GetTextBound` parity
are outside these assertions.

Source bounds/text and local split bands are caller inputs, with default device
services, width 1000, density 1, identity color and null caller manager; the
actual table TextManager executes. Host ICU/math/file/allocation/UUID boundaries
and the observer's retained allocations remain explicit. Saved parsing/cloning,
document pagination and split-band production, Bodytext source-rectangle
callbacks, final SVG/PDF writer consumption and pixels do not execute here.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --live-table-padding scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-live-padding.json
cmp /tmp/table-live-padding.json conformance/table-live-padding.json
```

## Export row sizing and merged-frame ownership

The ARM64 Drawing library used here has SHA-256
`788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd`;
Model has SHA-256
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.

Drawing's cold `TableLayout::init`, `0xaa6b4`, reads actual row heights at
`0xaaa60` and column widths at `0xaaab8`. `extendRowBySplit`, `0xaaf2c`,
measures cell height differences and grows the row for a strictly positive
maximum difference (`0xab0ac`–`0xab0f0`). The warm
`updatePositionFromRow`, `0xaecf4`, selects the largest measured frame-owner
height across every column position. Only a result below the small-height threshold is replaced by
`ObjectTable::GetMinRowHeight` (`0xaedfc`–`0xaee10`); the stored minimum is
not an unconditional lower clamp on every nonempty row.

These selected cold/warm export routines do not read the saved row maximum.
Model exposes that value through `ObjectTable::GetMaxRowHeight`, `0x3d3844`,
and `TableRow::GetMaxHeight`, `0x3c4244`. Rust retains the parsed maxima without
capping export row measurement.

### Table-wide height limit

`ObjectTable::IsMaxHeightEnabled`, `0x3db38c`, reads implementation byte 186;
`GetMaxHeight`, `0x3dabcc`, forwards to the float at offset 176. Model's
`SetMaxHeightEnabled`, `0x3d17dc`, writes that flag and calls `fitTableRect`
(`0x3d183c`) when it changes. That editing-time resize is separate from Drawing's
layout of saved rows and columns.

The cold-row, warm-row and measured-geometry captures each include sixteen
height-limit cases: flag off/on, maxima 0/1/10/1000, and unmerged/merged grids.
The native getters verify the supplied metadata before cold frame initialization.
For each case the harness also runs an otherwise identical grid without the
limit. Every captured output agrees: cold/warm frames, measurement selections,
split lists, pending gaps, measured bounds and first-page minima. Repeats under
all three allocation fills agree as well. Text measurements remain supplied
caches; the editing setter itself is not executed by these captures.

Rust accepts the flag for supported dense preparation and retains saved
height metadata. It does not replay the editing-time resize or cap the measured
rows.

### Warm row sizing from cached owner measurements

For each row from the supplied start index onward, Drawing
`updatePositionFromRow`, `0xaecf4`, calls `ObjectTable::GetFrameCell` for each
column (`0xaed90`). It looks up the owner's cached text layout and skips null
layouts. Widget `ObjectTextLayout::GetMeasuredHeight`, `0xd3b78`, reads the
cached `f32` at offset 404; no text shaping occurs in this routine. An owner
from an earlier row contributes its full measured height again. Covered raw
cells' cached heights do not contribute, even when those cells appear in the
separate paint-visible list.

The maximum starts at zero. A result strictly below `0.001f` is replaced by
the indexed saved row minimum. Heights at or above that threshold are not
clamped to the minimum. The saved row maximum is ignored. Only an absolute
`f32` difference strictly greater than `0.001f` changes geometry: `extendRow`
changes the physical row height, then `offsetFromRow` moves subsequent rows.
Rows before the start index retain their frames.

[The warm-row capture](../../conformance/table-warm-rows.json), SHA-256
`fc01a44d106fb82a4810c0ea78dff1411a28a7a41370387163055cdda9193c92`,
contains 158 inputs and 1,285 slot frames. It covers merged owners, covered-span
chains, supplied/null layout measurements, threshold-adjacent heights, saved
minima/maxima, start indices and pending gaps. Each input produces identical
outputs with allocation fills `0x00`, `0xa5` and `0xff`. The
[Rust capture module](../../conformance/native_table/rows.rs) checks Model,
Drawing and Base hashes recorded below, plus Widget SHA-256
`cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9`.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_row_tests.rs)
matches every coordinate and pending-gap bit using production row updates.
Preparation caches the bounded `TableGrid` ownership map once and warm sizing
reads owner measurements through that map. The capture supplies measured
heights directly and uses the isolated text initialization described below;
it does not establish native
text measurement, complete merged pagination or device appearance.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --warm-rows scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so
```

### Cold frame cache and row updates

`TableLayout::init`, `0xaa6b4`, starts grid coordinates at the half-width of
the widest drawable outer border. Missing serialized border styles retain
Model's constructed one-unit black border, giving a half-width of 0.5. Each
stored slot receives a rectangle with its saved column width and row height.
The next row starts at the preceding row's first-slot bottom
(`0xaaa6c`–`0xaaa80`); the next column starts at the preceding column's right
in the first grid row (`0xaaac8`–`0xaaad8`). Native Base `RectF::OffSet`,
`0xb11a4`, adds those origins using `f32` arithmetic. The initializer also
stores the rectangle in a cache keyed by the raw cell pointer
(`0xaac90`–`0xaae08`). It does not expand rectangles by row/column spans in
this path. This establishes initial cache geometry, not final merged layout.

Drawing `extendRow`, `0xaff74`, adds its supplied amount to the bottom of
every stored slot and corresponding cached frame in that row. It does not
move later rows. `offsetFromRow`, `0xade0c`, moves stored slots and cached
frames from a supplied row onward. Before movement, a pending gap of at least
`f32::EPSILON` changes the amount: positive movement consumes the gap and
stops if more than epsilon remains. Negative movement clears the gap and
recursively removes its height before applying the original movement. These
two routines use stored cells directly, independently of frame ownership.

[The frame capture](../../conformance/table-cold-frames.json), SHA-256
`498bfb15e663cfeb648ed51a8c0ed2a2490b09fa47b578d1419e0d7eac0cdb84`,
records 93 inputs, 1,082 initial/final slot frames and 532 explicit row changes.
Cases include unit/merged grids, covered-span chains, fractional/large values,
zero saved row heights, border-width selection, gap consumption/removal and
deterministic sequences of row updates. All outputs are identical with fresh
allocations filled by `0x00`, `0xa5` or `0xff`. The capture hash checks Model
and Drawing as above, plus Base SHA-256
`e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb`.

The [Rust harness module](../../conformance/native_table/frames.rs) runs native
frame initialization, row updates, rectangle arithmetic, containers and callback
copies. Local ELF relocations and explicit PLT bindings resolve those native
calls. Text initialization is isolated: the harness replaces the base
text-layout constructor with zeroed storage and omits `SetObject` and
`SetTextScale`. Update amounts
and pending gaps are supplied inputs. Consequently this capture does not prove
native text measurement, the decision to grow a row, final merged-frame sizing,
pagination selection or device appearance.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_frame_tests.rs)
matches initial/final coordinate bits and final pending-gap bits for every input.
It uses the same frame initializer and row updates as export preparation, before
text measurement. Matching these frame primitives does not establish complete
merged-table parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cold-frames scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-cold-frames.json
cmp /tmp/table-cold-frames.json conformance/table-cold-frames.json
```

### Measured bounds and first-page minima

Drawing `updateMeasuredRect`, `0xab168`, resolves `(0,0)` and the last grid
position through `GetFrameCell`. It reads their cached rectangles and unions
those two frames with Base `RectF::ExtendRect`, `0xb1724`; it does not union
every stored cell. The union is the content rectangle at layout offset 652.
Model's four `GetDrawnBorder*Width` getters supply independent edge widths.
Each width is the maximum of the outer edge and borders on the corresponding
physical grid edge, including inherited default cell borders. The getters read
stored widths without filtering transparent colors. Drawing expands each side
by half its width, then offsets the measured rectangle to origin `(0,0)` using
Base `RectF::OffSet`. The expanded rectangle is at layout offset 668.

Drawing `getMinRowHeightInFirstPage`, `0xac9f0`, resolves every column in the
requested row through its frame owner. An owner's nonempty text layout
contributes its first-line height plus top margin. An absent text layout or
empty text contributes the cached measured height. Widget `GetTextLayout`,
`0xd39ac`, reads offset 368; `GetMeasuredHeight`, `0xd3b78`, reads offset 404.
The virtual getters called at `0xaca98`, `0xacab4` and `0xacacc` are Text
`GetTextLength`, `0x8b104`, `GetLineHeight`, `0x8b3ac`, and `GetTopMargin`,
`0x8b90c`. The branch tests text length, not line count.

The maximum starts at zero. If it remains zero, the routine uses the physical
first row's height, even for a request concerning a later row. It does not
substitute the requested row's saved minimum. When the origin flag is enabled,
the result includes `content.top - measured.top`; after measured-origin
normalization this is the content's top coordinate. `GetMinHeightInFirstPage`,
`0xac9d4`, requests row zero with this flag enabled.

[The geometry capture](../../conformance/table-measured-geometry.json), SHA-256
`6b210cfa59fa00971addb39da7d7576eeb3238214fa560d31a9dcdf7f4039ed4`,
records 154 table states, 1,281 supplied/cold slot frames and 914 minimum-height
queries with both origin-flag values. Cases include endpoint owners from earlier
rows/columns, covered-span chains, endpoint-cache union, asymmetric edge widths,
nonempty/empty text, absent text layouts and the first-row fallback. Every output
is identical with fresh allocation fills `0x00`, `0xa5` and `0xff`.

The [Rust capture module](../../conformance/native_table/geometry.rs) checks the
Model, Drawing, Base and Widget hashes recorded above, plus Text SHA-256
`5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b`.
Bounds, border getters, rectangle arithmetic and text getters execute native
code. Text initialization remains isolated; cached frames and text metrics are
supplied inputs, with synthetic storage for the native getters. This establishes
selection and arithmetic, not text shaping, final merged frames, pagination
selection or device appearance.

The [Rust regressions](../../crates/sdocx/src/render/table/native_geometry_tests.rs)
match content/measured bounds, edge widths and every minimum-height query by
`f32` bits. Production bounds and first-page sizing use the same owner lookup
as warm row sizing.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --measured-geometry scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-measured-geometry.json
cmp /tmp/table-measured-geometry.json conformance/table-measured-geometry.json
```

### Row split caches and warm text selection

Drawing `updateSplitOfARow`, `0xb24cc`, resolves the first column's frame
owner (`0xb2524`). It selects supplied table split rectangles whose top is at
or below that owner's cached frame top, then offsets them by the negative owner
top using Base `RectF::OffSet`. Rectangle order is retained; it is not sorted.
The resulting native list is compared with the first owner's cached list.

`isSameRectList`, `0xb2e18`, first checks pointer identity and null pointers.
Distinct nonnull lists must have the same count and a first rectangle whose
four coordinates differ by at most `0.001f`, through Base `RectF::Equal`,
`0xb14b0`. Later rectangles are not compared. Distinct empty lists therefore
compare unequal: their first-rectangle lookups return null. A missing list and
a stored empty list are separate cache states.

If the comparison succeeds, the new list is discarded and every cell cache
remains unchanged. Otherwise the first owner's list is replaced. For columns
one onward, replacement happens only when `GetCell(row,column)` equals
`GetFrameCell(row,column)` (`0xb2710`–`0xb2734`). Those cells receive separate
copies of the first owner's local list, even when their own frame tops differ.
Covered raw cells retain their previous list or absent-cache state. A first
owner from an earlier row can make the comparison succeed before an independent
cell in the requested row has received any list.

Drawing `layoutRow`, `0xb0420`, also compares raw cells with frame owners before
`layoutCell` (`0xb0484`–`0xb04b4`, `0xb0644`–`0xb0674`). It lays out only cells
that own their frames. Its current/previous first-list lookups use first-column
frame owners (`0xb04f4`, `0xb050c`), independently of the physical row frames
that row offsets move. These text-selection branches are disassembly findings;
the split capture below does not execute text layout.

[The split capture](../../conformance/table-row-splits.json), SHA-256
`0e77850a6312268018e9e27ceeab0dc7f41654690de13e0171982b9fa6f2ff2e`,
records 139 inputs, 1,068 split updates and 10,828 cell-cache snapshots. Cases
include column/row merges, owners from earlier rows, covered-span chains,
first-rectangle threshold boundaries, changes to later rectangles, empty lists
and unsorted supplied bands. Every output is identical with allocation fills
`0x00`, `0xa5` and `0xff`.

The [Rust capture module](../../conformance/native_table/splits.rs) checks Model,
Drawing and Base hashes recorded above. Frame lookup, list construction/copying,
cache comparison, rectangle arithmetic and deletion paths execute native code.
Host allocation/deletion interfaces, text initialization, single-thread mutex
operations and diagnostic interfaces are isolated. In particular, the native
list's empty first-element lookup takes its diagnostic path and returns null;
the capture does not validate its logging or thread-local error storage.
The results establish split-cache selection, not text shaping, first-line
relocation decisions, row compression, complete pagination or device appearance.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_split_tests.rs)
matches every changed flag, cache-presence state and rectangle coordinate bit.
Cell split caches are typed `Option<BandList>` values. Warm splitting uses the
first owner's top, copies only to frame-owning cells and preserves stale lists
when the native comparison succeeds.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --row-splits scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-row-splits.json
cmp /tmp/table-row-splits.json conformance/table-row-splits.json
```

### Warm-row movement and relayout decisions

Drawing `layoutRow`, `0xb0420`, first updates the requested row's split caches.
When that update changes a cache, it runs `updatePositionFromRowForFirstLineCut`
and lays out only cells that own their frames. It returns true even when the
requested row contains no self-owning cells. Distinct empty caches compare
unequal, so repeated empty-list updates retain this relayout branch.

When the split cache is unchanged, row zero returns false. Later rows use the
current and previous first-column frame owners. With a pending gap greater than
`0.001f`, an empty previous split list prevents movement. Otherwise the gap is
removed only when the current first-line minimum is at most
`pending_gap - previous_first_band_height`. Gap removal returns true without
calling `layoutCell` or updating the cached split list.

Without that pending-gap branch, movement requires
`first_line_minimum - current_first_band.top > f32::EPSILON`. The displacement
is the first band's bottom. Physical rows move through `offsetFromRow`, the
current pending gap receives that displacement, and split caches are updated
before self-owning cells are laid out. Equality at either comparison boundary
does not select the greater-than branch.

`updatePositionFromRowForFirstLineCut`, `0xb2360`, returns immediately for row
zero or a missing/empty first-owner split cache. A pending gap above `0.001f`
is removed, split caches are updated, and the routine retries against the new
cache. Otherwise it uses the same first-line/first-band comparison and
displacement as above. Earlier-row owners supply bands independently of the
physical rows that move.

Drawing `layoutFromRow`, `0xaa448`, calls `layoutRow`,
`updatePositionFromRow` and `updatePositionForRowBottom` in that order for every
remaining row (`0xaa480`–`0xaa4ac`). It does not use `layoutRow`'s return value
to suppress sizing or compression.

[The warm-control capture](../../conformance/table-warm-control.json), SHA-256
`b78403ccecffe594cecf18eea48ba60c4c56808905049230589a9c9ab42c30ab`,
records 148 inputs and 1,207 initial slots. Its 1,089 actions contain 354 direct
first-line calls, 415 row-layout calls and 320 ordered layout/sizing/compression
sequences, yielding 10,892 slot snapshots and 418 `layoutCell` calls. Inputs
cover merged owners, rows without self owners, empty/missing caches, both
comparison thresholds, gap removal, retries and supplied minima. Every output
is identical with allocation fills `0x00`, `0xa5` and `0xff`.

The [native Rust harness](../../conformance/native_table/control.rs) checks the
same five library hashes as the compression capture. Native row control,
owner lookup, list operations, metric getters, sizing and compression execute
unchanged. The `layoutCell` boundary records cell selection without shaping;
the supplied two-line measurements remain fixed. Initialization, host memory
interfaces, single-thread mutex operations and diagnostics are isolated. The
ordered sequences call the three native routines directly; final observer
notifications from `layoutFromRow` are outside the capture.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_control_tests.rs)
uses the production scheduler with a measurement callback and matches every
changed flag, ordered cell selection, frame coordinate, pending gap and split
cache bit. This establishes control flow with fixed caches, not native shaping,
merged-frame construction, nested content or complete device pagination.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --warm-control scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-warm-control.json
cmp /tmp/table-warm-control.json conformance/table-warm-control.json
```

### Row-bottom compression from owner caches

Drawing `getLastIntersectSplitRect`, `0xb304c`, reads the first column's frame
owner and that owner's cached split list. It traverses the list in stored order,
retaining the last rectangle whose top is at most the physical row height. It
stops at the first rectangle beyond that height; unsorted lists are not searched
for later matches. The owner supplies local bands, while raw row frames supply
the height. A covered row therefore does not reinterpret the bands relative to
its own physical top. Missing or empty lists return an empty rectangle.

`getRowOffsetForLastEmptyInPage`, `0xb294c`, requires a nonempty selected
rectangle, then resolves frame owners across the requested row. For each owner
with a cached `TextLayout`, it reads `GetLineCount` and `GetLineBottom` for the
last line; missing text layouts are skipped. The maximum starts at zero, so
negative line bottoms cannot lower it. Cached measured height and cell bottom
margin do not participate in this comparison.

If that maximum is strictly below the selected band's top, the returned offset
is `(band.top - physical_row_height) - half_border_width`, with native `f32`
rounding after each subtraction. Equality does not compress. Drawing
`updatePositionForRowBottom`, `0xb28ec`, applies only negative offsets: it
extends the current raw row and moves subsequent rows through `offsetFromRow`.
That movement retains the existing pending-gap consumption rules. Split caches
are unchanged by this operation, even when row movement makes them stale.

[The compression capture](../../conformance/table-row-bottom.json), SHA-256
`c2ea6ef80738b101c32d54e910275c3a7308916da286684f89ad4af1af9e3d50`,
records 148 inputs, 1,216 initial slots, 425 updates and 4,368 updated slot
snapshots. Inputs include first/other-column merges, covered-span chains,
missing caches, missing text layouts, empty/unsorted bands, later pending gaps,
and values immediately around line-bottom and physical-height boundaries.
All outputs are identical with allocation fills `0x00`, `0xa5` and `0xff`.

The [native Rust harness](../../conformance/native_table/bottom.rs) checks the
Model, Drawing, Base, Widget and Text hashes recorded above. Compression,
owner lookup, list traversal, native text getters and frame updates execute
unchanged. Text initialization, host allocation/deletion, single-thread mutex
operations and diagnostic interfaces are isolated. Present layouts receive two
supplied line records: first-line height 10.25, top margin 12.5, measured height
300, and the case's independent last-line bottom. The capture establishes
control flow with these cached measurements, not native text shaping or full
merged preparation.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_bottom_tests.rs)
matches selected-band, displacement, frame-coordinate and pending-gap bits.
Production compression reads owner caches through the bounded `TableGrid`.
Full native merged shaping, nested objects, complete pagination and device
appearance remain outside this evidence.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --row-bottom scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-row-bottom.json
cmp /tmp/table-row-bottom.json conformance/table-row-bottom.json
```

### Stored cells and frame owners

Model `ObjectTableImpl::GetCell`, `0x3c7570`, indexes the stored row and column
vectors directly (`0x3c75a8`–`0x3c75b0`). The public
`ObjectTable::GetCell`, `0x3d2be0`, validates the requested indices and reads
the same stored slot (`0x3d2c30`–`0x3d2c60`); it does not substitute a frame owner.
`GetFrameCell`, `0x3c75c0`, is a different producer: it searches earlier
positions backwards, tests whether their row/column spans cover the requested
position (`0x3c7630`–`0x3c7698`), and follows a covering cell's own origin
(`0x3c76c0`–`0x3c76cc`). Without a covering earlier cell it returns the
stored cell at the requested position (`0x3c7754`–`0x3c7764`). This is not
equivalent to expanding every serialized cell into an independent visible box.

Drawing `layoutRow`, `0xb0420`, compares `GetCell` with `GetFrameCell` and
lays out text only when they return the same cell
(`0xb0478`–`0xb04c8`). `updateMeasuredRect`, `0xab168`, obtains the first
and last grid positions through `GetFrameCell` before reading cached frames
(`0xab198`–`0xab238`). Merged layout therefore needs distinct stored slots,
frame owners, paint-visible cells and cached frame geometry. Sparse records,
final merged-frame construction and rowspan growth are not established by these traces.

### Native frame ownership and paint visibility

[The ownership capture](../../conformance/table-ownership.json) records native
`GetFrameCell` and whole-grid `GetVisibleCells` outputs for 279 synthetic dense
grids: seven named cases, all sixteen positive in-bounds span combinations for
a 2×2 grid, and 256 deterministic mixed-span cases up to 5×5. Stored origins
match their physical row and column positions. The Model library SHA-256 is
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.

`GetFrameCell`, `0x3c75c0`, searches earlier candidates in reverse row/column
order. It includes spans on cells already covered by another cell, then follows
the selected candidate's stored origin until no earlier span covers it.
An origin outside the originally covering rectangle can consequently resolve
to that rectangle's owner through a covered cell's retained span.

Whole-grid `GetVisibleCells`, `0x3c7784`, calls the range implementation at
`0x3c77b0`. Its forward row-major scan tests a visited bit at
`0x3c7b48`–`0x3c7b60`, appends only an unvisited stored cell at
`0x3c7bb4`–`0x3c7c5c`, then marks that cell's span at
`0x3c7c60`–`0x3c7cc0`. Covered cells do not contribute their retained spans to
this mask. Drawing's ordinary and text-canvas paths request this list at
`0xa6174` and `0xa6894`, respectively.

For a one-row, four-column grid with spans `(1,2)` at column 0 and `(1,3)`
at covered column 1, native frame owners are `[0,0,0,0]`, but paint-visible
cells are `[0,2,3]`. The equivalent four-row chain produces the same distinction.
Thus selecting only cells whose frame owner is themselves does not reproduce
the native paint list. These synthetic cases establish the two local APIs;
they do not establish which overlapping retained spans Samsung's editor emits.

The [Rust capture harness](../../conformance/native_table.rs) executes the
unmodified native routines and their vector helpers in ARM64 Unicorn emulation.
Intra-library PLT entries resolve to their native implementations; only
allocation, deletion and memory fill use host interfaces. Fresh allocations
filled with `0x00`, `0xa5` and `0xff` give identical outputs across all cases:
7,197 frame-owner calls and 837 visible-list calls. The inputs do not exercise
empty/sparse rows, invalid origins, zero/overflowing spans, clipping subranges,
native table layout, or a device-rendered reference.

```sh
rustc --edition 2024 -D warnings -C panic=abort conformance/native_table.rs \
  -L native=scratch/apk-analysis-runtime/python/unicorn/lib \
  -C link-arg=-Wl,-rpath,"$PWD/scratch/apk-analysis-runtime/python/unicorn/lib" \
  -o /tmp/sdocx-native-table
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  > /tmp/table-ownership.json
cmp /tmp/table-ownership.json conformance/table-ownership.json
```

### Merge construction without an attached document

Model `ObjectTable::MergeCells`, `0x3d6038`, first calls
`ObjectTableImpl::IsValidMergeRange`, `0x3ca320`. This checks range bounds and
requires the resolved frame owner at every selected position to fit entirely
inside the range. A range that cuts through an existing merged owner is rejected.
Repeating an already matching merge returns success without changing spans.

With no document attached at object offset 16, a successful changed merge writes
the top-left raw cell's row/column spans and sets its dirty bytes at offsets
81 and 82. It retains the other raw cells and their spans. The document/history
branch also contains text-transfer and undo operations; those are not exercised
by this capture.

[`table-merge-cells.json`](../../conformance/table-merge-cells.json), SHA-256
`96eda7bc2741c68e0eb18469193217678aad7922e78c2b4acda4d1879214ac33`,
records 134 inputs and 1,046 merge attempts through the native public method.
Cases include row/column/rectangle merges, expansion, repeated merges,
combining disjoint owners, rejected bounds/inverted ranges and an initial
covered-span chain. All raw cell pointers remain identical. Each attempt
records the resulting spans, dirty bytes, frame owners and paint-visible list.
Allocation fills `0x00`, `0xa5` and `0xff` produce identical captures.

Native cold frame initialization then runs against the resulting model,
including the merge-set dirty bytes. It retains per-slot frames from saved
column widths and row heights; it does not combine them into a spanning owner
rectangle. Rust matches all 9,240 captured owner positions, visible lists and
the final initialized frames. This verifies merge-generated structural states
and cold frame primitives. It does not establish document-attached text
transfer, native shaping, complete merged preparation or device appearance.
Diagnostics and cold text initialization are isolated in the harness.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --merge-cells scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-merge-cells.json
cmp /tmp/table-merge-cells.json conformance/table-merge-cells.json
```

### Rust preparation and painting

The [Rust grid model](../../crates/sdocx/src/render/table/grid.rs) preserves stored
slots, resolves frame owners and selects paint-visible cells separately. Its
regression matches all 2,399 owner positions and 279 visible lists in the pinned
native capture. Input validation requires a dense grid, matching stored origins,
positive in-bounds spans, at most 65,536 slots and at most 1,048,576 accumulated
span positions. These bounds are Rust work limits, not recovered Samsung limits.

Dense merged grids use the shared Rust preparation engine. Cold sizing measures
every raw slot; warm sizing reads frame owners, retaining covered-cell caches.
Callback preparation supports constraints 1/2; fresh drawing supports 0/1/2.
Content bounds retain the first/last owner-frame union through drawing rounding
and painting. The outline does not substitute the last raw slot's frame.
Saved and prepared painting share native visible-cell selection.

Sparse records, invalid spans, rotated layouts and nested cell objects remain
outside this prepared subset. Sparse/invalid grids retain usable saved frames
and report `UnsupportedContent` with the original source anchor.
[Vector-output regressions](../../crates/sdocx/tests/embedded_text_layout.rs)
cover horizontal, vertical, rectangular and covered-span-chain cases in both
themes and all three layout constraints. They check identical SVG preview/replay,
selectable/tagged PDF source, matching baselines and zero PDF image resources.
[Paged Rust regressions](../../crates/sdocx/tests/prepared_tables.rs) compare
merged owners with an unmerged projection while holding saved bounds fixed.
Native cache captures establish row sizing and bounds independently; native
merged shaping, parent placement and complete device pagination remain unverified.

### Column minimum cache selection

Drawing `TableLayout::GetMinColumnLayoutWidth`, `0xab7a0`, starts with Model's
indexed `ObjectTable::GetMinColumnWidth`, `0x3da870`. That getter reads the
per-column minimum vector at implementation offset 56, independently of the
global minimum at offset 12. An absent table returns zero.

For each stored row, Drawing uses `GetCell` directly and looks up that cell's
cached layout (`0xab80c`–`0xab848`). It does not resolve frame owners or skip
covered cells. A missing layout causes an immediate return of the saved
per-column minimum, discarding values from preceding rows. An existing layout
with no optional width contributes nothing. When every layout exists and at
least one optional width is present, the result is the larger of the saved
minimum and the **smallest** present cached width (`0xab850`–`0xab898`). For
example, cached widths 100 and 25 with a saved minimum of 10 return 25; replacing
either layout with a missing entry returns 10. With no rows or no present
optional widths, the getter returns the saved minimum.

The optional payload is a float at cell-layout offset 620, followed by its
presence byte at 624. `ObjectTableCellLayout::GetMinLayoutWidth`, `0x8c078`,
returns those fields. The traced producer `getCellLayoutWidth`, `0x8c13c`,
takes the maximum of each line's width plus its paragraph rectangle width and
indent, then adds the text layout's left and right margins (`0x8c19c`–`0x8c238`). It
sets the optional width when that result is at least the configured layout
width; otherwise it clears the presence byte and returns the configured
width (`0x8c23c`–`0x8c260`). This producer trace is separate from the capture
of supplied cache values below; the
[live-width capture](#captured-live-column-widths) executes it on actual native
measured text and source-change callbacks.

[The column-minimum capture](../../conformance/table-column-minima.json),
SHA-256 `571f54fbc638314d34080bedee692d7010d42574844d21af03869cf56aacc68f`,
contains 179 synthetic cache inputs and 182 column queries. It covers all 81
four-row combinations of missing layouts, unset optional widths and present
widths, both with unit spans and with a four-row owner span. Other cases check
column independence, reversed value order, fractional float bits, unset payloads
containing stale values or NaNs, global-minimum independence and an absent
table. Allocation fills `0x00`, `0xa5` and `0xff` produce identical captures
across 546 native getter calls.

The [Rust capture module](../../conformance/native_table/columns.rs) constructs
the cache with native hash-map insertion and verifies entries with native
lookup. Both ELF libraries are hash checked and all load segments are mapped;
PLT calls resolve to native implementations except allocation/deletion. Cache
widths are supplied inputs: this capture does not execute native text shaping,
column resizing, cold merged-frame construction or Samsung device rendering.
Rust export preparation retains saved column widths and does not implement
this column-minimum cache getter.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --column-minima scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  > /tmp/table-column-minima.json
cmp /tmp/table-column-minima.json conformance/table-column-minima.json
```

### Captured live column widths

[`table-live-column-widths.json`](../../conformance/table-live-column-widths.json),
SHA-256 `b2ddd2a31d79c65426c0a8fd64bcc9bcdb521d69a07bc78dd1e9645bd48cc223`,
records ten controls and 101 canonical native width-producer observations.
The [capture module](../../conformance/native_table/live_column_widths.rs)
executes genuine cold/warm cell measurement through the shared live TableLayout,
Widget, NAME/font and paragraph pipeline. Passive instruction hooks record
`getCellLayoutWidth`, `0x8c13c`, configured width, actual per-line
width/paragraph-rectangle/indent operands, horizontal margins, returned width
and optional minimum before/after. These hooks neither write guest state nor
invoke nested native calls. The measured width (`0x8c070`) and optional minimum
(`0x8c078`, payload 620/presence 624) remain separate fields.

Controls cover empty text, fractional saved width, margins, constructor font 50,
covered merged cells and actual warm column growth/shrink. Ordinary cases return
their configured width with no optional minimum. The source-edit controls call
native `ComponentText::SetText`, `0x39c9d4`, and `SetSpannableOverPages`,
`0x2d21f4`, then invoke complete `TableLayout::OnCellChanged`, `0xb1848`,
with an explicitly supplied full-replacement text event. Native Widget source
update and Measure execute. Replacing empty text with `W` in a width-10 column
produces width `15.079999923706055`, stores the optional minimum and reaches
the native column setter observer at `0xb1984`, Model `SetColumnWidth`,
`0x3d3b64`, and Drawing `resizeColumn`, `0xb0088`. Replacing `WWWW` with `i`
in a width-80 column leaves the column width 80 and emits no column-setter call.
These establish a captured source-change growth route, not automatic cold-export
column sizing. The actual indexed editor minimum getter described above reads
the optional cache; it is not a dependency of cold table export measurement.
Source audit identifies its external consumer in `libSPenObjectControl.so`
(SHA-256 `3211b70ad105e285b57aaa085e2bcd543f197238d702f9233aedea1c7caa1aea`):
`ControlObject::ResizeColumn`, `0xf7168`, calls it at `0xf7224`, clamps the
converted drag width against positive minimum/maximum values and calls Model
`SetColumnWidth` at `0xf7300`. Drawing cold/warm/resize paths do not import
that getter. The separate `OnCellChanged` route compares measured width minus
old frame width against `0.001f32` at `0xb193c`–`0xb1940`, then reaches the
setter and resize at `0xb1984`/`0xb1990`. Model's setter applies
`GetValidColumnWidth`, `0x3c8420`, using global/per-column minima followed by
the per-column maximum. The ObjectControl drag route is source evidence and
does not execute in this fixture.

Each control repeats under owned-memory fills `0x00`, `0x55`, `0xa5`, `0xff`
and zero again. Strict independent replay matches the frozen 291,685 bytes.
Library/font/XML and host ICU 76.1/Unicode 16 provenance and shared device,
allocation/file/libc/single-thread, UUID, math and disabled-trace boundaries are
retained. The [ICU adapter](../../conformance/native_table/text_paragraph_layout.rs)
executes real host `utext_openUChars`, `ubrk_setUText` and `utext_close` with
owned UTF16 backing storage. Guest UText addresses are opaque keys for real
host UText objects; guarded native reads/writes of their guest structure abort
rather than assuming Android/host structure compatibility. This remains a host
ICU substitution, not device ICU equivalence.

Caller inputs are source bounds/text/font/margins, requested resize widths and
source-edit event type/range. Measured widths, optional cache values, dense
entries, frames and callback dimensions are native outputs. Native event
construction, history/listener dispatch, editor drag control, saved parsing,
clone/Bodytext placement, pagination-band production, final vector transport
and pixels remain excluded. SDK export retains saved column widths and does
not implement this source-edit callback; this capture introduces no production
width change or export autosizing contract.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --live-column-widths scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-live-column-widths.json
cmp /tmp/table-live-column-widths.json conformance/table-live-column-widths.json
```

### Outer-border initialization

`TableLayout::getHalfBorderWidth`, `0xb325c`, iterates
`ObjectTable::GetBorderPath` (`0xb3288`) and selects positive widths with
nonzero ARGB values (`0xb32ac`–`0xb32c4`) before halving the maximum.
Model's `GetBorderPath`, `0x3cb464`, reads the outer border pointer at member
152 and constructs its four paths. This initializer does not inspect every
cell border. The later per-edge drawn-bound widths include boundary cells and
do not gate on color; those two measurements must remain distinct.

### Native border paths

[The border-path capture](../../conformance/table-border-paths.json), SHA-256
`4cae7223084cff60765bddb5e0b9926e8a706b741bf7cb9949cebfe4e2f8dc0d`,
records native `GetCellBorderPath` (`0x3cad9c`) and `GetBorderPath`
(`0x3cb464`) outputs for twelve synthetic grids. The 69 requested cell paths
contain 364 segments; the twelve outer queries contain eight segments. Captures
under allocation fills `0x00`, `0xa5` and `0xff` agree byte for byte. These are
Model outputs; they do not capture Drawing, pagination or device appearance.

`getCellBorder`, `0x3c7510`, reads the stored cell's border pointer at member 96
and falls back to the table's default-cell pointer at member 160
(`0x3c7554`–`0x3c755c`). A present cell border replaces the whole default;
a zero-width edge does not select that edge from the default. Null pointers in
the synthetic capture are distinct from omitted serialized fields.

`GetCellBorderPath` first resolves `GetFrameCell` at `0x3caddc`. It constructs
the owner's left, top, right and bottom perimeter from the corresponding stored
boundary cells. A merged rectangle can therefore have different styles along
one side, including styles on covered cells. It does not expand just the visible
origin's border around the merged rectangle. Path construction retains zero
colors and nonpositive widths; later Drawing filters are separate.

The outer path is independent of those cell styles. Its ordered endpoints run
bottom-to-top on the left, left-to-right on top, top-to-bottom on the right and
right-to-left on the bottom. Cell-path segments run top-to-bottom on both
vertical sides and left-to-right on both horizontal sides. Start/end radius
values must follow the captured endpoint direction.

Model's `TableBorder` constructor produces black (`0xff000000`), one-unit-wide
edges with zero radii. The capture executes that constructor for the default
case. `ObjectTableImpl` constructs both the outer and default-cell borders at
`0x3c5d44` and `0x3c5d68`. Missing serialized styles consequently do not imply
native null pointers.

Segment coordinates use `f32` prefix sums, followed by subtraction of the
current row height or column width to recover its start. The large-origin,
fractional-size case preserves the resulting cancellation and rounding. The
captured line equations also preserve native fused multiply-add results;
neither follows a sum performed once in `f64` and narrowed at the end.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --border-paths > /tmp/table-border-paths.json
cmp /tmp/table-border-paths.json conformance/table-border-paths.json
```

The Rust border model matches every captured segment, style and endpoint,
including the native `f32` staging. Its topology is bounded to 65,536 cells;
paint work is independently bounded to 1,048,576 perimeter segments after
frame-owner propagation. Invalid numeric inputs report `InvalidBounds`;
unsupported topology or excessive paint work report `UnsupportedContent`.

### Border painting

Drawing's `drawTableCellWithoutText`, `0xa748c`, scans paint-visible cells and
calls Model's perimeter builder at `0xa7c34`. It rejects ARGB zero and exact
width zero (`0xa7c6c`–`0xa7c7c`), then raises widths below one to one. Negative
widths survive Model and reach this minimum-width branch. A nonzero ARGB with
zero alpha still participates in selection, although its paint is transparent.

Shared boundaries generally select the later cell's left/top edge. With an
active outer border, Drawing suppresses the first-column left edge, first-row
top edge and all right edges (`0xa7c98`–`0xa7cd0`). A bottom edge is retained
when the next row's pending-gap magnitude exceeds `0.001f`. Without an active outer
border, the right edge is retained only for a cell originating in the last
column, and bottom edges are retained on the last row or before a pending gap
(`0xa7cdc`–`0xa7d0c`). This uses the layout's pending-gap cache at member 856,
not a difference inferred from rounded cell frames.

`getTableBorderStyle`, `0xa6fb4`, aggregates the outer paths: the last nonzero
ARGB with positive width supplies the color, and the maximum drawable width
supplies thickness after a one-unit minimum. Horizontal-edge radius maxima
supply `rx`; vertical-edge maxima supply `ry` (`0xa70e8`–`0xa711c`). Radii
are read even from inactive edges. When both radii are positive,
`drawTableBorder`, `0xa8218`, paints one rounded rectangle with this aggregate
style. Otherwise it retains each outer edge's style and endpoint direction.
The rounded cell backgrounds are painted separately; non-perimeter corners
are squared by additional rectangles (`0xa7b10`–`0xa7c0c`).

[The Drawing outline capture](../../conformance/table-border-drawing.json),
SHA-256 `63b57494de2caa428cd110021ab5ec36d5a93316ecf373bdd87bb1cbcb288c87`,
executes native `getTableBorderStyle` against Model's native outer paths.
It covers 25 style inputs at seven positive canvas X scales (`0.125`, `0.3`,
`0.5`, `0.75`, `1`, `2`, `4`): 175 outputs, each repeated under allocation
fills `0x00`, `0xa5` and `0xff`. Both libraries are hash-checked. The host
supplies the canvas matrix getter, allocation, deletion and memory fill;
style aggregation and Model constructors/path helpers execute natively.

The outputs confirm all sixteen active-edge masks, absent outer pointers,
constructor defaults, nonpositive widths, nonzero colors with zero alpha,
signed radius maxima and radii from inactive edges. All-negative radii remain
negative; the aggregation does not clamp them to zero. After selecting the
drawable width, native Drawing raises it to `1.0f / canvas_scale` when
`width * canvas_scale < 1.0f`. The Rust border model matches all four output
fields, including exact `f32` width bits, for every captured scale. Document
vectors use the unit-scale result.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --drawing-borders scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  > /tmp/table-border-drawing.json
cmp /tmp/table-border-drawing.json conformance/table-border-drawing.json
```

Spannable paths use prepared content and cell frames; saved paths retain Model
coordinates with drawing offsets. Outer content bounds are distinct from the
border-expanded measured rectangle (`TableLayout::updateMeasuredRect`,
`0xab168`; `GetContentRect`, `0xab4a8`). Cell backgrounds and borders are
painted in cell order, then the outer border; text uses a separate pass.

The Rust vector painter applies these rules with typed lines and rectangles,
stored edge colors/alpha, shared theme conversion and metadata-derived radii.
Its width policy is the native unit-canvas policy. Native `drawLinePath`,
`0xa8008`, also enforces a one-pixel minimum using the canvas X scale;
zoom-dependent screen coverage is not reproduced by the document-space vectors.
The outline aggregation and the cell pass below have captured Drawing execution.
Canvas line/rectangle calls are recorded at their interfaces; canvas painting
and clipping do not execute.
Public SVG/replay and PDF regressions check edge precedence, widths, opacity,
axis radii, selectable source and zero image resources. Native device appearance,
merged shaping and complete border clipping across split pages remain
outside this evidence.

### Captured table drawn bounds

[`table-drawn-bounds.json`](../../conformance/table-drawn-bounds.json), SHA-256
`f610b86467e0be9b1ddcd18d2265c08ac4e7810a600115061948c3fc38efebae`,
records 13 cases across four native allocation fills and a repeated zero fill.
The [capture module](../../conformance/native_table/drawn_bounds.rs) executes
actual Model table/row/cell/content and default-border construction, border
setters, merging, complete `GetDrawnRect` (`0x3d48c4` → `0x3c6cac`) and the
four drawn-width getters (`0x3dbba4`–`0x3dbd48`). Raw table bounds and explicit
border color/width controls are supplied; bounded input storage starts zeroed
while native owned allocations receive the selected fill.

The default constructor creates nonnull black one-unit outer and default-cell
borders. Missing saved style fields therefore cannot be represented by the
capture's explicitly injected null pointers. With an outer border present,
each drawn edge width includes the corresponding physical perimeter cells,
whose own border replaces their default border; a zero own width also replaces
it. Merged frame owners do not replace physical edge slots, and interior slots
do not enlarge the perimeter. Zero or partial alpha does not suppress drawn
widths. Native `f32` half-width subtraction/addition expands the raw rectangle;
fractional and large-origin controls preserve those rounding steps.

An injected nil outer pointer makes `GetDrawnRect` return raw bounds even
with wide cell borders; direct width getter calls are excluded for that state.
The capture uses actual constructors and getters, not the earlier artwork
fixture's supplied virtual-slot origin. Saved parsing, allocation failure,
Drawing/text layout, Composer cloning and final export do not execute. This
establishes the bounded Model drawn-origin producer, without complete document
or export parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --table-drawn-bounds scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-drawn-bounds.json
cmp /tmp/table-drawn-bounds.json conformance/table-drawn-bounds.json
```

### Complete cell artwork pass

[`table-cell-drawing.json`](../../conformance/table-cell-drawing.json), SHA-256
`cee7c9dd9d06c5ce623c474183e058ada95af6de0d5a2c2834ea32cdc73a0b3d`,
records 60 synthetic 2×2 inputs, 501 selected Model border paths and 738
canvas commands. The [capture module](../../conformance/native_table/cell_drawing.rs)
executes complete Drawing `drawTableCellWithoutText`, `0xa748c`, and
`drawLinePath`, `0xa8008`, with native Model path/background getters and Base
rectangle operations. Heap allocation fills `0x00`, `0xa5` and `0xff` agree.

Inputs distinguish saved cell rectangles, source table bounds and prepared
cache frames under constraints 0, 1 and 2. Native cold initialization and
`offsetRows`, `0xade0c`, produce the ordinary cache frames. Named
`supplied-*-merged-frame` cases replace only the owner's cached rectangle and
saved rectangle with separate literal inputs; they do not execute merged text
measurement or pagination feedback. Cases cover retained covered spans,
translated coordinates, large origins, display-row filtering, rounded corners,
pending gaps around `0.001f`, canvas scale and inactive edge styles.

Constraint 0 takes background bounds from the saved cell rectangle, subtracts
the virtual slot-160 rectangle's origin, then adds the supplied horizontal
scroll and Y-offset inputs. In this
60-case fixture a supplied table vtable routes slot 160 to `GetRect`,
`0x3d48c0`, making source table bounds an explicit origin input. The real
`GetDrawnRect` route is captured separately below. Saved border endpoints
subtract the separately computed source-origin-minus-drawing-offset. These
f32 operation orders differ at large origins. Constraints 1 and 2 take cached
background frames and remap border endpoints to those frames. Recorded cell
positions come from raw Drawing lookup and can differ from Model frame owners.
The selected paths and commands also establish native edge filtering, corner
squaring and background/border command order under these supplied inputs.

Allocation, deletion and memory operations are host services. Paint setters
record host state; global alpha is disabled, theme conversion is identity and
the outer outline style is supplied independently of `getTableBorderStyle`.
Canvas line, rectangle and rounded-rectangle interfaces record commands without
rasterization or clipping. The capture does not establish device pixels,
application theme conversion or complete merged preparation.

The production artwork geometry and command builders match all 60 cases,
including exact f32 coordinates, selected paths, corner patches, visibility,
command order and fixture canvas-width scaling. Normal artwork uses the saved
coordinate route even when text preparation has cached frames; constraints 1
and 2 retain prepared background frames and border remapping. Document vectors
use unit canvas scale. Native Drawing's display rectangle tests the background
frame and can skip both background and all borders (`0xa79d8`–`0xa79f0` →
`0xa7e5c`). SDK page-viewport visibility is a separate producer: saved artwork
bounds conservatively union the background frame with translated paintable
perimeter extents, including stroke half-width and paths later suppressed by
edge selection, before later SVG/PDF page clipping. This preserves
visible emitted borders when the native display rectangle is empty; it does
not establish native display-filter or complete producer parity. Clone placement
and the actual virtual source-origin getter are captured separately below.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-drawing scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-cell-drawing.json
cmp /tmp/table-cell-drawing.json conformance/table-cell-drawing.json
```

### Captured table clone placement

[`table-clone-origin.json`](../../conformance/table-clone-origin.json), SHA-256
`78022f27f297e5b9accf5f58000f0da6c60b4e09fc809a2ffc5cf5cd885b8cc9`,
records 11 cases, 44 stages, 176 cell snapshots and 154 paint commands. The
[capture module](../../conformance/native_table/table_clone_origin.rs) executes
native Model construction, factory `0x36d6cc`, and table copy through virtual
slot 184, `ObjectTable::Copy`, `0x3d9af4`. Native Widget
`GetRectByDrawnRect`, `0xe1f88`, computes the affine input; Composer windows
`[0x376418, 0x376478)` and `[0x376494, 0x3764a8)` execute placement and the
spannable flag update. Placement dispatches virtual slot 40 to
`ObjectShape::SetRect`, `0x397708`, rather than the table-specific setter at
slot 320. The detached route changes the clone's table rectangle while copied
saved cell rectangles and content Model rectangles remain unchanged.

The real table virtual slot 160 returns `GetDrawnRect`, including half-border
inflation. Native Normal artwork subtracts that clone drawn origin from both
saved background rectangles and Model border endpoints, retaining their distinct
f32 translation stages under the supplied scroll control. Neither the original
source origin nor the clone's raw
Model rectangle is interchangeable with this result. The capture distinguishes
raw bounds, BaseData drawn bounds and the virtual drawn result through copy,
placement and cold Drawing preparation. Cases include Normal/over-pages,
merged cells, translated/negative/large origins, retained-run offsets and
half/double scaling.

All three native heap allocation fills (`0x00`, `0xa5`, `0xff`) agree. Retained
DrawnText rectangle/position and writer origin are supplied, and only the named
Composer instruction windows execute. Cold layout storage and a text-wrapper
adapter are supplied; `SetObject`, `SetTextScale` and shaping are omitted.
Paint/Canvas calls are recorded with identity theme and an empty display clip.
The fixture's `drawing_x` is the supplied Drawing member-112 horizontal scroll,
seeded from the clone drawn left. `SetScrollX`, `0xa5824`, writes that field;
the actual constructor at `0xa5634` initializes it to zero (`0xa5684`). It is
not a world origin. Commands remain canvas-local with native literal zero Y
offset. Canvas world translation,
parent measurement, the complete Composer writer, Bodytext callbacks and final
PDF transport are outside this capture. Composer's canvas translation at
`0x380018`–`0x38003c` uses rounded measured origin minus crop origin, with crop placement
as a separate world-transport step; the supplied scroll does not prove that
writer producer. The separate [background transport capture](#captured-table-background-transport)
executes the fresh constructor and selected writer geometry windows with scroll
zero. Production direct virtual-origin subtraction and its f32
arithmetic are tested separately. Inline Normal clone placement retains the
existing SDK compatibility behavior without a full writer/Model-callback
activation claim.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --table-clone-origin scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-clone-origin.json
cmp /tmp/table-clone-origin.json conformance/table-clone-origin.json
```

### Captured table background transport

[`table-background-transport.json`](../../conformance/table-background-transport.json),
SHA-256 `7c0ec937bcbc7c5e2c7475146ae5d4430a22d8eddb16606c09921b4a7e7a917c`,
records 10 cases, 113 local paint commands, 76 selected paths and 50 Canvas
events; 40 commands are marked as outer-border artwork. The
[capture module](../../conformance/native_table/table_clone_origin.rs)
`capture_transport` extends native source construction, clone/copy, Widget
affine production, Composer placement and cold Drawing frames. Heap fills
`0x00`, `0xa5` and `0xff` agree, strict independent replay is byte identical,
and the separate 11-case clone and 60-case artwork fixtures remain unchanged.

Composer `[0x37e4ac, 0x37e4f8)` reads the supplied local measured rectangle
through Drawing `GetMeasuredRect`, `0xab494`, offsets it by the actual clone
drawn origin and rounds outward. `[0x37e534, 0x37e56c)` computes the background
crop from page size and scaled margins. `[0x380004, 0x380040)` records native
Canvas save/translation, with translation equal to rounded measured origin
minus crop origin. `[0x380098, 0x3800d8)` executes actual fresh
`TableDrawing` construction, `0xa5634`, `SetTextSizeDelta(0)` and complete
`DrawObjectWithoutText`, `0xa6738`, including cell and outer-border artwork.
The constructor initializes horizontal scroll at member 112 to zero; no
`SetScrollX` call occurs in this writer window. `[0x37e7c0, 0x37e7d8)` produces
the density-scaled image target. Cases cover Normal/over-pages, merged cells,
zero/negative origins, fractional measured bounds, page cropping and half/double
density. Scroll and text-size delta remain zero in every captured case.

The local commands and translation are observed separately. Inferring image
placement algebra adds local coordinate, Canvas translation and crop placement,
then applies density; the graphics backend matrix and final image placement do
not execute. For `zero-source-inline-ten`, the saved source left is zero,
the clone drawn left is 10, the first local background left is −10 and the
rounded measured left is 10. Their algebraic sum predicts world left zero
under that supplied immutable source state. It does not establish a universal
world origin or the source state after upstream Model callbacks.

Local measured/content layout rectangles, page dimensions, scaled margins,
density and retained-run inputs are supplied. Warm parent text measurement
does not produce those rectangles here. Host services cover allocation/free,
bytes, mutex/C++ guards, UUIDs and constructor logging; the text-wrapper adapter
omits native text binding/shaping. Paint/Canvas interfaces record local commands
and events with identity theme; command positions retain the last raw cell
lookup, while `outer_border` marks table-wide paths. Pixel factory, bitmap data,
graphics backend matrices, upstream Bodytext callbacks, warm shaping and final
PDF backend execution are excluded. The capture establishes these writer
geometry windows without full world-transport or production activation parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --table-background-transport scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-background-transport.json
cmp /tmp/table-background-transport.json conformance/table-background-transport.json
```

### Visible rectangles and canvas clipping

Drawing `TableDrawing::SetDisplayRect`, `0xa84f4`, stores the four supplied
coordinates unchanged. `applyClippingForSpannableTable`, `0xa72d4`, emits no
canvas clip for constraint 0 or an empty display rectangle. Otherwise it scans
the split bands in supplied order. A display top strictly inside a band moves
to its bottom; a display bottom strictly inside moves to its top. Equal edges
remain unchanged, and band X coordinates do not participate.

`TableLayout::GetVisibleMeasuredRect`, `0xaba50`, returns measured bounds when
the split list or query rectangle is empty. Otherwise `getLastVisibleRow`,
`0xabba4`, selects the prefix of physical rows whose first raw cell top is at
most the query bottom. When a row is selected, the visible rectangle is the
union of the first raw cell and the last raw cell of that row, followed by a
vertical clamp only when the query overlaps strictly in Y. It ignores query X;
disjoint Y retains the rectangle. With no selected row, measured bounds remain
the starting rectangle. The private `getVisibleMeasuredRect`, `0xab8e8`, with
its intersection flag instead intersects both axes and returns zero on a miss.
An empty split list/query bypasses even that intersection.

These raw endpoints differ from the frame-owner endpoints used for measured
bounds. A captured whole-grid merge has measured bounds `[0,0,41,21]`; with a
nonempty split list and a query covering all rows, its visible rectangle is
`[0.5,0.5,100.5,90.5]`. Visibility does not enlarge the owner frame or remeasure
the table. Cell backgrounds and border paths still use full raw cached frames.

The canvas clip expands this visible rectangle by
`f32::mul_add(outline_width, 0.5, 1.0)`. Base `RectF::ExtendRect(float)`,
`0xb16e0`, then floors left/top and ceils right/bottom. Drawing computes f32
width/height and truncates X, Y, width and height to signed integers, passing
operation 0 to canvas virtual slot 64. The border guard and outward rounding
belong to the clip; they do not change glyph origins or layout bounds.

[`table-clipping.json`](../../conformance/table-clipping.json), SHA-256
`5d21686615f74c43a054e850dd19ec57c2170e860920846965f71d7b8084e476`,
records 55 native states, 1,690 rectangle queries and 1,240 canvas clip calls.
It covers merged/covered owners, empty/inverted queries, adjacent-float row
boundaries, disjoint axes, empty/unsorted/overlapping bands, signed row offsets,
constraints 0/1/2 and fractional outline widths. Public and private vertical-only
visibility agree bit for bit. Allocation fills `0x00`, `0xa5` and `0xff`
produce identical output.

Native cold frame initialization, owner-based measured bounds, display-rectangle
assignment, rectangle helpers and clip selection execute unchanged. Empty text
caches isolate shaping and row sizing. The host supplies allocation, deletion,
memory fill/move and the canvas command recorder. This capture establishes clip
arguments, not canvas pixels, the caller's display-rectangle selection, text-pass
clipping or complete device pagination. The PDF writer uses the separate
[export artwork crop](#export-artwork-crop) below; it does not supply a display
rectangle to this Drawing clip.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --clipping scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-clipping.json
cmp /tmp/table-clipping.json conformance/table-clipping.json
```

### Export artwork crop

Composer `ObjectTablePDFWriter::WriteObject`, `0x37e37c`, offsets measured
bounds by the source drawn origin and rounds them outward before computing
its artwork crop. At `0x37e534`–`0x37e568`, it constructs
`[0, scaled_body_top_margin, page_width, page_height - scaled_body_bottom_margin]`,
intersects it with the rounded measured bounds, then calls
`RectF::ExtendRect(1)`. Page mode 0 uses the note body's margins multiplied
by document density (`0x37e50c`–`0x37e530`); other modes use zero margins.
The margin getters read ComponentText's stored top/bottom fields at
`0x39e6c4` / `0x39e74c` in Model.

Base intersection uses strict edge comparisons and leaves its receiver
unchanged on a miss. The writer ignores that return value, so disjoint or
exactly touching tables retain the page/body rectangle before expansion.
Expansion subtracts/adds one in f32, then floors left/top and ceils right/bottom.
It does not substitute an empty rectangle on a miss.

`createBackgroundImage`, `0x37fed8`, translates the canvas by rounded table
origin minus crop origin and constructs a fresh `TableDrawing` at `0x380098`.
The constructor, `0xa5634`, leaves the display rectangle zero. No
`SetDisplayRect` call intervenes before `DrawObjectWithoutText`, `0x3800c0`;
the separate spannable Drawing clip therefore emits no canvas clip even for
constraints 1/2. Native export renders table artwork into this cropped bitmap;
the SDK represents the same crop with vector SVG elements.

[`table-export-clipping.json`](../../conformance/table-export-clipping.json),
SHA-256 `872b937dedb795d3b995249c4ab26ae421d3f6f0eaec909ccf396038cf7d38b8`,
records 78 crop cases and the constructor's zero display rectangle/no-clip
results for constraints 0/1/2. It executes the unmodified Composer crop
instruction window, Base helpers and Drawing constructor with allocation fills
`0x00`, `0xa5` and `0xff`. Bounds, integer page dimensions and density-scaled
margins are supplied; fractional bounds probe the helper independently of the
writer's preceding rounding. Logging and canvas recording are host interfaces;
bitmap factories and pixels are not executed by this capture.

Rust matches every crop coordinate bit. Prepared table fills, cell borders and
outer outlines share this artwork clip; page and parent paint translations
retain its coordinate space. SVG/replay regressions check page mode and density,
and PDF checks retain selectable text without image resources. Saved-frame
fallback painting remains outside this prepared export contract.
Text uses a separate pass. SDK tables without certified native source/placement
context retain the conservative measured-table clip.
The native per-run clip decision and cached-entry run-rectangle production are
captured below. The [live writer capture](#captured-live-table-text-clipping)
executes their combined native path with actual cloned content sources;
complete split-page/device appearance remains unverified.

### Export text clips

Composer `writeTextContent`, `0x37ec88`, gets the cell's content object at
`0x37ecd0` and its cached frame at `0x37ee14`. It offsets the frame by the
rounded measured-table origin, then rounds it outward for ordinary type-2
text (`0x37ee90`–`0x37ee94`). The layout supplies `GetDrawnTextData` with zero
local offset (`0x37eee4`–`0x37ef00`); the writer passes the frame origin separately.
Drawing initialization assigns the cell content object to its text wrapper
(`0xaad20`–`0xaad30`); Widget `SetObject`, `0xd3974`, retains that pointer at
offset 416. Layout width/height setters write wrapper fields 524/528, independently
of the Model rectangle read by `GetRect`, `0x2caa60`.

`writeTextBlock`, `0x37f508`, starts with an empty explicit clip. It compares
`DrawnText` rectangle bottom at offset 116 with the content object's rectangle
height in f32 (`0x37f6ac`–`0x37f6e4`). Equality takes the no-clip branch; one
representable step above the height takes the clipping branch. The test uses
height, not the object's world-space bottom. On overflow it offsets the run
rectangle by the cell origin and intersects it with the content object rectangle.
The intersection result is ignored: disjoint and touching rectangles retain
the translated run bounds, rather than becoming empty. PDF scaling and an
offset relative to the scaled object center follow; the paint receives the
matching center translation (`0x37f724`–`0x37f7ac`).

Pdfium `DrawText`, `0xa2230` in `libSPenPdf.so`, adds a clip path only when the
explicit rectangle pointer is nonnull and `RectF::IsEmpty` is false
(`0xa23d4`–`0xa23e0`). Zero/inverted clip dimensions therefore skip clipping.
The separately supplied run rectangle provides character width/height at
`0xa2474`–`0xa24a0`; it is not used as an unconditional clip by this handler.

[`table-text-clipping.json`](../../conformance/table-text-clipping.json), SHA-256
`6e72c668c96255d87561db715cbec349371ae02be499ead7d675440165343850`,
records 132 cases across exact/adjacent height boundaries, translated/fractional
origins, PDF scales, disjoint/touching bounds and degenerate dimensions. The Rust
harness executes the unchanged Composer decision/transform window, Model getter,
Base rectangle/point helpers and Pdfium empty-clip gate. Supplied object and
stack memory prefilled with `0x00`, `0xa5` and `0xff` yields identical outputs.
Page bounds and paint translation recording are host interfaces. Shaping, the
content object's Model rectangle lifecycle, final PDF paths and device pixels
are outside this capture. Cached-entry run-rectangle production is captured
separately below.

Text `getDrawnTextRun` offsets and unions retained entry rectangles
(`0x67274`–`0x672ec`); `appendTextBlock` stores the second rectangle argument
at `DrawnText` offsets 104–116 (`0x680e0`–`0x6810c`). The
[text-bound capture](#retained-text-entry-and-run-bounds) executes these producer
windows, and the [complete emitter capture](#complete-retained-text-run-emission)
executes the ordinary cached-glyph run path separately from the Composer clip
capture. The SDK's compatibility table-wide clip does not reproduce the
conditional native contract; the
[bounded one-page adapter](#captured-ordinary-one-page-padding) now supplies
the required source and placement context for its admitted profile.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-clipping scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenPdf.so > /tmp/table-text-clipping.json
cmp /tmp/table-text-clipping.json conformance/table-text-clipping.json
```

### Captured live table text clipping

[`table-live-text-clipping.json`](../../conformance/table-live-text-clipping.json),
SHA-256 `3106348e8271cc6ca15437c0daa792c8eca86097f5505ef3c5cf55d17d054503`,
records 11 cases, 44 selected physical cells and 47 actual foreground runs.
Eight runs select clips: three intersections succeed and five fail. The
[capture module](../../conformance/native_table/live_table_text_clipping.rs)
records 156 scaled position floats, 91 balanced Paint creations/releases and
91 skipped sibling export calls. Two independently compiled native outputs
agree on the captured results; the final 403,945-byte fixture additionally
records metadata and callback observations. Four fresh fills `0x00`, `0x55`,
`0xa5`, `0xff` and a repeated zero run agree, totaling 55 case samples.

The actual Model table/content/text constructors and complete native
TableLayout/Widget/Text preparation execute. Factory `0x36d6cc` and Copy
virtual slot 184 create a real clone; a second native TableLayout constructor
(`0xa9d58`), Measure (`0xaa530`), Layout (`0xaa3d4`) and
`UpdateTextDrawingPosition` (`0xacc88`) prepare it. The writer's measured-world
rounding window `[0x37e4ac,0x37e4f8)` executes, followed by complete
`writeTextContent`, `0x37ec88`, and `writeTextBlock`, `0x37f508`. Actual
`GetDrawnTextData`, `0x8c084`, and `GetDrawnText`, `0x68418`, consume native
cached entries; real `GetTextBound`, `0x8afd4` / `0x7a4ac`, supplies observed
bounds. The content pointer passed to each block is verified to be that selected
clone cell's actual Model content object, whose `GetRect`, `0x2caa60`, supplies
the clip-height and intersection rectangle.

Default source rectangles derive from native construction/copy. The first
default content retains `[0,0,80,100]` while the clone's rounded measured world
rectangle is `[-1,-1,161,47]`. With genuinely short source cells of height 4,
warm layout expands the rows while retaining saved source bounds; six runs
select clips, with intersection results true/false/false/true/false/false.
Named source-control cases explicitly call the actual source setter at virtual
slot 40 after layout. They bracket the bottom/height comparison by adjacent f32
values: below and equal skip clipping, above selects it. Horizontal overflow
alone skips clipping. A disjoint source still selects a clip and retains the
translated run bounds when native intersection returns false. Selection at
`0x37f6e8`, intersection return at `0x37f724`, native scaling/paint translation
and the final `DrawText` callsite at `0x37f8cc` are observed. These finite ordinary
inputs complement the separate 132 supplied controls for touching, inverted and
signed-zero dimensions.

The caller selects all four physical slots, including merged covered cells;
aggregate writer visibility selection does not execute. Clone child update-size
callbacks are absent and editing callbacks present in all 44 observations.
Source-control setters are supplied geometry controls, not callback-produced
bounds. Parent Composer clone placement is excluded; the separate
[clone-placement capture](#captured-table-clone-placement) executes that route.
PDF factory/Paint methods and final `DrawText` are recording interfaces;
bitmap-font, background and decoration exports are skipped. Native font getters,
geometry, guards, intersections and scaling execute under the shared supplied
context/device/color and pinned host ICU/math/allocation/UUID/libc/file
boundaries, with APK/library/font provenance retained. Parent document bounds,
Bodytext callback source production, source parsing, device ICU, final backend
clipping and pixels remain excluded. The [isolated Rust clip kernel](../../crates/sdocx/src/render/text/native_cell_clip.rs)
replays all 132 supplied controls and all 47 actual produced runs, including
eight selected clips and three successful intersections. It rejects nonfinite
or overflowing geometry without normalizing native dimensions. Production
table export uses this decision only when retained source/placement context is
certified, including the [ordinary one-page profile](#captured-ordinary-one-page-padding)
below. Other contexts retain the compatibility table-wide clip. This establishes
the native writer path for these controls and its bounded decision kernel,
without complete document export parity.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --live-table-text-clipping scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-live-text-clipping.json
cmp /tmp/table-live-text-clipping.json conformance/table-live-text-clipping.json
```

### Cell content model rectangles

The cell rectangle and its content object's Model rectangle are independent
stored values. `TableCell::SetRect`, `0x3c2008` / `0x3c2024`, and
`SetRectDataOnly`, `0x3c20cc`, compare the requested rectangle with the cell's
four f32 coordinates at member 64. Equality skips the content setter, even
when the content rectangle differs; signed zero compares equal. A changed
rectangle is stored in the cell, dispatched to the content object at member
88, and marks cell bytes 81/82 as 1.

`TableCellContentObject` has vtable address point `0x494bf0`. Its ordinary
rectangle setter at slot 40 resolves to `ObjectShape::SetRect`, `0x397708`;
slot 480 resolves to `SetRectDataOnly`, `0x399954`. The native shape and base
setters execute through `ObjectShapeImpl::SetRect`, `0x3a6a60`,
`ObjectBase::t_SetRectOnlyData`, `0x2d2b18`, and
`ObjectBaseImpl::SetRect`, `0x2d7324`. The Model rectangle at BaseData member
8 normalizes inverted axes; the cell's own rectangle retains the requested
orientation.

Drawn bounds at BaseData member 24 have a separate update gate.
`ObjectBaseImpl::setDrawnRect`, `0x2d740c`, compares integer truncations of
old/new width and height plus f32 epsilon bits `0x36a7c5ac`
(`0x2d7468`, `0x2d7494`). Matching dimensions offset the existing drawn
rectangle; differing dimensions replace it. A one-ULP width increase in the
capture updates the Model rectangle but leaves the drawn width unchanged.

[`table-cell-model-bounds.json`](../../conformance/table-cell-model-bounds.json),
SHA-256 `898739ecada7c4a2b66f71a4893e712fbd484eb715d96dfdd51cbc882a2210fd`,
records 60 cases: 15 initial/requested geometries through all four public
setter variants. It covers equality, divergent cell/content bounds, translation,
resizing, adjacent float coordinates, signed zero, degenerate and inverted
dimensions. All methods return 1 and agree on geometry under the supplied
detached-object conditions. Native type-0 template, fill and list constructors
execute; geometry getters/setters are not replaced. Native allocation fills
`0x00`, `0xa5`, and `0xff` produce identical results. Manually supplied objects
are zero-initialized, with independent cell/content rectangles and initially
matching Model/drawn content rectangles. Context and observer are nil, followers
are disabled, and images are absent. Allocation, memory operations, mutex
construction, Android logging and error reporting are host interfaces.
Document history, observer/follower callbacks and Drawing layout are outside
this capture.

Embedded cloning uses a separate rectangle route. Composer `0x376464` dispatches
slot 40 on the table to the inherited shape setter, through relocation
`0x495200`. Its detached-context branch calls slot 480; it does not enter
`ObjectTableImpl::SetRect` or its grid fitting loop. Cell copying at `0x3c29f4`
preserves the cell rectangle and dispatches content copying to
`ObjectShape::Copy`, `0x39841c`, without a target affine. Fresh Drawing frames
derive from saved row heights and column widths; `updateCell`, `0xae914`,
supplies their local dimensions to Widget layout width/height setters.
`UpdateTextDrawingPosition`, `0xacc88`, is a single return instruction.
The [clone capture](#captured-table-clone-placement) executes this detached
placement route and preserves copied cell/content Model rectangles. These
isolated setter routes omit the Bodytext bridge and notification captured below.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-model-bounds scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  > /tmp/table-cell-model-bounds.json
cmp /tmp/table-cell-model-bounds.json conformance/table-cell-model-bounds.json
```

### Cell model bridge after frame preparation

[`table-cell-model-lifecycle.json`](../../conformance/table-cell-model-lifecycle.json),
SHA-256 `e2807875de0d6462b601318a352cbda482f692689b72fafafc916336e8f16aa7`,
records 17 cases, 88 stage snapshots and 327 cell snapshots. The
[capture module](../../conformance/native_table/cell_model_lifecycle.rs)
executes native Model table construction (`0x3d2690`, `0x3d27d8`), cell and
content constructors (`0x3c1cac`, `0x3c17f0`), and registry initializers
`0x2a2ed0`, `0x2863a0`, `0x3c35ec` and `0x3c5918`. Contents have final shape
type 4. Native Bodytext size gate `0xd76e0` and complete cell-model bridge
`0xd78ec` execute through native runtime-handle lookup and bundled C++
`dynamic_cast`. Library hashes are retained in the fixture.

Cold Drawing frame preparation (`0xaa6b4`, `0xab168`) leaves saved cell and
content Model rectangles unchanged. The explicit bridge offsets each cached
frame by the supplied caller rectangle's origin and invokes native cell/content
setters. Warm native row extension and offset (`0xaff74`, `0xade0c`) change
Drawing frames; the Model rectangles in these captured operations remain
unchanged until the explicit bridge runs. This does not establish the behavior
of other document rectangle-update routes. The
separately recorded size gate is false for zero and `0.0005` growth and true for
`0.002` growth. The harness explicitly calls the bridge even when that gate is
false; those calls do not establish application dispatch in that state.

Equal cell rectangles retain divergent content rectangles through the setter's
equality skip; a changed warm frame updates both. Clean variants retain clean
unsaved flags. Native whole-grid merge cases retain per-slot frames and source
rectangles, with owners recorded separately. Source clip extents cannot be
inferred from owner spans. Raw table `t_SetRectOnlyData`, `0x2d2b18`, changes
the table Model/drawn rectangles while preserving implementation content bounds
and all cell/content rectangles; a translated bridge origin remains explicit
input rather than a measured document origin.

Native heap allocations throughout Model construction and container execution
are repeated with fills `0x00`, `0xa5` and `0xff`. Supplied table storage,
Drawing layout/vtable and one-element Bodytext owner/view association start
zeroed. Host boundaries include allocation/free, byte operations, deterministic
UUIDs, mutex/C++ guard services, destructor registration, logging/errors and a
zeroed text-wrapper adapter omitting `SetObject` and `SetTextScale`. Native text
shaping, first measurement, complete Drawing/Bodytext construction, listeners,
document ObjectSpan rectangle production, parsing/cloning and final PDF run
clipping do not execute. These are bounded construction/bridge captures, without
a full application lifecycle parity claim.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-model-lifecycle scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so > /tmp/table-cell-model-lifecycle.json
cmp /tmp/table-cell-model-lifecycle.json conformance/table-cell-model-lifecycle.json
```

### Captured cell text constructors

[`table-widget-text-constructor.json`](../../conformance/table-widget-text-constructor.json),
SHA-256 `284bb1d14fc3b8be45462cdb3ba6309e2b071a8f0f69f0db48f8e7da58d8c37d`,
records three device profiles and 24 semantic states. Native owned allocations
repeat under fills `0x00`, `0xa5`, `0xff` and zero again; strict master-driver
replay agrees byte for byte. The
[capture module](../../conformance/native_table/widget_text_constructor.rs)
uses the shared [cell host](../../conformance/native_table/text_cell_host.rs)
and the fixture's pinned NAME/XML/four-font, libc++, host ICU/file and library
dependencies.

Actual Drawing cell layout construction (`0x8bff8`) runs Widget ObjectTextLayout
(`0xd2fb4`), Content constants (`0x133e0`), Text wrapper construction
(`0x8a820`, `0x8a8e8`), RichText construction (`0x61d7c`) and bullet setup.
Native Model content construction and actual getters establish ObjectTextBox
shape type 4 and table membership. This Model type is not a MeasureData entry
classification; no copied type-4 field in the Text wrapper is established.
Native class identity and virtual update-bound targets are checked.

Single-line remains false and word-wrap true. RichText bytes 112–115 are
`[0,0,1,1]`, including Widget's enabled system-font flag. Complete `SetObject`
(`0xd3974`) binds the Model pointer and updates bounds, changing default font
size from 17 to the absent-text branch's 50 while retaining cell gravity zero.
The actual Model default text getter returns null; `SetObject` does not copy
Model text into the wrapper. Its constructor-default text length remains zero
because `updateText` does not execute.

Caller context/display/manager inputs supply density, raw direction/profile,
document width/pixel and font delta. Margins are set through the actual
ComponentText setter and retain native `f32` grouping
`(document_pixel * source_margin) * text_scale`. Complete `SetTextScale`
(`0xd948c`) applies positive controls 0.75 and 1.5; zero, negative and repeated
1.5 controls preserve the prior state. Direction 1 is a raw caller control,
without an inferred RTL contract. Native constants, defaults and C++ algorithms
execute; allocation, bytes/files, deterministic UUIDs, single-thread services
and host ICU remain interfaces. Text copying/classification, font production,
measurement/width conversion, paragraph placement and cached emission do not
execute in this constructor fixture. The separate
[cell measurement capture](#captured-cell-text-measurement) executes the native
single-cell producer chain under its recorded text and font controls.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --widget-text-constructor scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so > /tmp/table-widget-text-constructor.json
cmp /tmp/table-widget-text-constructor.json conformance/table-widget-text-constructor.json
```

### Captured cell model callbacks

[`table-cell-model-callbacks.json`](../../conformance/table-cell-model-callbacks.json),
SHA-256 `d57a73b6845f293acf318b6cfcc1b23eb1fdd1529578fb1fdaac4e17c63bae05`,
records 18 cases with 37 state snapshots and 148 cell snapshots. Each runs
in a fresh native machine under heap fills `0x00`, `0x55`, `0xa5` and `0xff`,
then repeats fill zero. The
[capture module](../../conformance/native_table/cell_model_callbacks.rs)
executes complete Drawing notification `0xb1bf0`, native `std::function`
clone/invoke/destroy, Bodytext forwarding `0xb59d0`, view callback `0xdce8c`,
runtime owner lookup, span lookup `0xd6d78`, size gate `0xd76e0` and bridge
`0xd78ec`. Native registration `[0xb3228, 0xb3314)` and view installation
`[0xd080c, 0xd085c)` instruction windows allocate/balance the listener node
and install both closures; the rest of those setup functions is excluded.
Model ObjectSpan construction/setters (`0x417440`, `0x417fc4`, `0x417598`),
native span-list copy/traversal and Widget `GetTextLayout`, `0xd39ac`, execute.

The callback clears the supplied nil page cache, resolves the span/index and
calls the document rectangle getter before the size gate. A false gate leaves
Model rectangles unchanged on this size-notification route, including
origin-only changes. This is not a general lifecycle rule for document-driven
rectangle updates. Native assembly
compares absolute f32 width/height differences with `>= 0.001f32`; captured
`0.0005` and `0.002` cases bracket that threshold without an exact-equality
case. A true gate offsets each per-slot Drawing frame by the document bound's
origin and invokes the Model setters; supplied bound extent does not alter
that offset. Equal cell rectangles retain divergent content rectangles, changed
cell rectangles restore content bounds, and clean unsaved flags remain clean.
Missing owner/layout and view-callback guards prevent their corresponding route.
Repeated warm notification calls the document getter twice but enters the
bridge once after size convergence.

A separate document update route compares the complete rectangle.
`BodyTextView::updateObjectSpanRect`, `0xd6fac`, and
`updateObjectSpanRectBySpanList`, `0xd9618`, obtain native
`PageViewGroup::GetObjectUpdateInfo`, `0xf80e8`. It reads the document text
layout's virtual-slot-112 `GetTextBound` (`0xf8258`–`0xf8268`), removes the
page origin and calls Widget `GetRectByDrawnRect` at `0xf82a4`.
`updateObjectSpanRectList`, `0xd9320`, compares the object's current raw
`GetRect` (virtual slot 168)
with that complete update rectangle using the `0.001` tolerance
(`0xd9410`–`0xd9434`). A changed rectangle calls `setOriginalRect`, object
virtual slot 48 and, for type 22 tables, `setCellRectForTable` directly at
`0xd949c`. This bypasses the size-notification width/height gate above.
These instructions establish a distinct update path; that producer and its
document-to-cell callback timing do not execute in the 18-case fixture. The
[Bodytext placement capture](#captured-bodytext-table-placement) separately
executes that route with an actual document object-entry bound.

Document virtual slot 112 is `GetTextBound(index)`, not `GetTextRect`.
The actual Text producer is `TextLayout::GetTextBound`, `0x8afd4`, forwarding
to `RichTextMeasure::GetTextBound`, `0x7a4ac`. It consumes placed entry bounds
and the RichTextImpl member-212 vertical gravity offset (`0x7a558`), computed
by `UpdateGravityOffsetY`, `0x639dc`. That producer does not execute here.
The fixture supplies its RectF result and checks the actual native ObjectSpan index, including signed
`-1`; the negative-index case therefore does not establish the real getter's
zero-return behavior. The [cell measurement capture](#captured-cell-text-measurement)
executes that getter on real placed single-cell entries; document object-entry
bound production and callback integration remain outside those two captures
and are executed in the bounded Bodytext capture below.

Drawing/Bodytext storage, owner association, ComponentText/TextCommon chain,
one-element span vector and the document getter result are supplied. Native
heap allocations vary while supplied storage starts zeroed. Shared allocator,
UUID, byte, mutex/guard, destructor-registration and logging services retain the
previous capture's host boundaries. The text-wrapper adapter omits `SetObject`
and `SetTextScale`. Document parsing, shaping/placement that produces the
document bound, complete listener setup outside the named windows, first cell
measurement, Composer cloning and final export clipping do not execute.
This establishes conditional native notification/model mutation without a
production callback activation or complete document lifecycle claim.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cell-model-callbacks scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so > /tmp/table-cell-model-callbacks.json
cmp /tmp/table-cell-model-callbacks.json conformance/table-cell-model-callbacks.json
```

### Captured Bodytext table placement

[`table-bodytext-placement.json`](../../conformance/table-bodytext-placement.json),
SHA-256 `7a66e259030edb4e019258fc49f07a717d6386bba6db2d11945f1307c0fa594b`,
records three document-placement controls and nine cloned-writer stages, with
54 foreground runs and 30 selected clips. Two independent strict replays match
the frozen 218,445 bytes. Each case repeats under fresh owned-memory fills
`0x00`, `0x55`, `0xa5`, `0xff` and zero again; supplied caller storage starts
zeroed in every sample. The
[capture module](../../conformance/native_table/bodytext_table_placement.rs)
extends the actual live table and writer captures with document production and
Bodytext placement.

Actual Model document construction (`0x3e1378`, `0x3e1408`), ObjectSpan
construction/setters and `AppendObjectSpan`, `0x39d510`, insert one U+FFFC
entry for the table. Native Widget conversion, NAME/font measurement and
document layout produce a real kind-5 entry and `GetTextBound`, Text `0x8afd4`.
The bounded registration window `[0xb01bc,0xb02dc)` installs the genuine
Bodytext `onUpdateObjectSize`, `0xb0b9c`: native cold table Measure precedes
document feedback into actual TableLayout Layout. The callback's dimensions,
minimum height and the resulting placed document bound are observed rather
than supplied.

`PageViewGroup::GetObjectUpdateInfo`, `0xf80e8`, reads that document bound,
subtracts the supplied page origin and applies native Widget
`GetRectByDrawnRect`, `0xe1f88`; the target and converted rectangle are observed
at `0xf82a4` and `0xf82a8`. Complete `updateObjectSpanRectBySpanList`,
`0xd9618`, and `updateObjectSpanRectList`, `0xd9320`, compare the complete raw
Model rectangle and invoke its native setter. The type-22 direct cell bridge,
`0xd78ec`, derives all four saved cell and content Model rectangles from actual
Drawing frames through `SetRectDataOnly`, Model `0x3c20cc`. There are no supplied
table/cell source setters, object dimensions or dense entries on this route.
The fixture retains raw/drawn table bounds and every saved cell/content/frame
rectangle before and after placement.

Actual listener `[0xb3228,0xb3314)` and view callback `[0xd080c,0xd085c)`
installation precede complete Drawing notifications, `0xb1bf0`, with the real
document getter. Changing only the supplied page origin leaves saved cells
unchanged on size notification. Subsequent complete placement changes them,
demonstrating the separate complete-rectangle route rather than a general
origin-insensitive lifecycle. Native factory Copy, second TableLayout
measure/layout and complete cached per-cell writer execute before placement,
after placement and after the origin-change placement. The three initial writer
stages select 18 clips: six intersections succeed and 12 fail. The six later
stages select 12 clips, all with successful intersections. The writer reads the
actual copied content Model rectangles produced by placement; this records the
source-bound effect on native clips without final backend clipping or pixels.

Caller inputs include explicit document shape/Widget dimensions, source table
bounds/text/font, span constraints 1/2, context/device and owner/view
associations with one genuine table layout in the owner map. A finite one-page
infinite-scroll interface and initial/moved page origins are supplied;
obstacle/history/cache services are null. Shared pinned host ICU 76.1,
math/allocation/UUID/libc/file boundaries and APK/library/font provenance remain
recorded. Full `SetBodyTextDocument`, `newObjectLayout`, `initBodyTextView`,
saved parsing and actual page-bound production do not execute. Each writer
explicitly selects four physical slots; aggregate visibility selection and
parent Composer placement remain excluded. The writer measured-rounding window
and complete foreground blocks retain the live-clipping capture's recorded
PDF/Paint interfaces and skipped bitmap/background/decoration exports. These
controls establish native document-bound/placement/clip composition without
complete document lifecycle/export parity.

The [typed object-entry adapter](../../crates/sdocx/src/render/text/layout/native_object_bound.rs)
separately matches the three caller profiles in Flow and Capture: six exact
comparisons of advance, height/font size, entry XY, layout/ink rectangles and
`GetTextBound`, plus native callback size/minimum. Its admitted source is one
U+FFFC block table with over-page constraint 1/2, font 17, unit scale, local
zero origin, no style/paragraph changes or exclusions, finite exact-f32 frame
dimensions, width at least the object width and sufficient height.
The table Model origin must also be zero. Every prepared cell must retain a
native local frame, nonempty lines/source and measured clusters with registered
entry facts; this excludes empty cells and native-measured sizes outside the
17/50 fact profile without approximating their bounds.
Page bands, shifted/inexact frames, short heights and unsupported source
metadata withdraw this optional geometry certificate. It does not certify the
native kind-5 cached glyph 1855, object run/font/style emission or parent
SVG/PDF writer behavior, and does not extend `NativePaintPlan` to kind 5.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --bodytext-table-placement scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-bodytext-placement.json
cmp /tmp/table-bodytext-placement.json conformance/table-bodytext-placement.json
```

### Captured ordinary one-page measurement

[`table-bodytext-one-page-placement.json`](../../conformance/table-bodytext-one-page-placement.json),
SHA-256 `cc0810421dceddffaa3454e8010ece57d7afddcce1f67dba1b662eeea4acd15a`,
records three controls: one table object and two ordinary text sources. The
[Bodytext capture](../../conformance/native_table/bodytext_table_placement.rs)
and [ordinary text observer](../../conformance/native_table/bodytext_one_page_text.rs)
execute genuine `BodyTextDocument(false)`, `0xa8c3c`, its actual infinite-scroll
getter (`0xa98a8`) and complete `BodyTextLayout::measureText`, `0xb2e54`.
Native page max-width (`0xa98c8`) and total-height (`0xa98e0`) getters consume
the supplied one-page record. `GetPageTotalHeight(false)` returns last-page
offset 0 and supplies Widget layout height zero; including the last page returns
600. The physical page record and Model document rectangle retain positive
height 600. Thus the measured Widget height is not the supplied physical height.

Actual Model document/Widget conversion, NAME/font measurement, paragraph
placement, text bounds and cached emission execute. The object U+FFFC and
ordinary `AV abc` source retain common data with empty raw font-size and
foreground span lists; effective font size is 17. The separate explicit-font-50
control has a real size-50 span and wraps `AV abc AV` at native width 40.
Source styles are observed after producer outputs. The table-object control
executes ordinary `GetObjectUpdateInfo` page-range lookup, `0xf2480`, Bodytext
placement/notifications and three real cloned-writer stages: 18 foreground
runs, ten selected clips, six successful and four failed intersections.
The text-only controls retain six/nine dense entries and one/eight emitted runs;
they do not execute table-object callbacks or Composer writers.

Six passive Model state observations follow first `SetRect` entry/completion,
`SetContentSize` entry/completion and moved-origin `SetRect` entry/completion.
The observed implementation windows are `0x3c6f78` / `0x3c7058` and
`0x3c6ae4` / `0x3c6b50`. They retain raw/content rectangles, column widths,
row heights and maxima. The first `SetRect` retains saved row heights while
moving the raw/content bounds; `SetContentSize` then expands those bounds.
Both columns remain 80, both saved rows remain 4 and row maxima remain
`f32::MAX` throughout these controls. These transitions distinguish saved
dimensions from later content-size feedback.

Strict independent replay matches all 167,494 bytes across fills `0x00`,
`0x55`, `0xa5`, `0xff` and repeat zero. Supplied one-page integer records,
selected maximum widths, owner/document/Widget associations, page-view origins
and group range from actual native text length bound the experiment.
`IsInfiniteScrollDocument=false` is native; it is not an interface override.
Full `SetDocument`/page-list conversion, page-indexer/group-range production,
saved parsing, complete view setup, device ICU, aggregate cell visibility and
PDF backend/pixels remain excluded. Shared pinned libraries/fonts/host services
and recorded PDF/Paint foreground interfaces retain the previous capture's
limits. The [bounded one-page SDK composition](#captured-ordinary-one-page-padding)
below separately verifies public per-run clipping for its admitted source.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --bodytext-table-placement-one-page-zero-height scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-bodytext-one-page-placement.json
cmp /tmp/table-bodytext-one-page-placement.json conformance/table-bodytext-one-page-placement.json
```

### Captured ordinary one-page padding

[`table-bodytext-one-page-obstacles.json`](../../conformance/table-bodytext-one-page-obstacles.json),
SHA-256 `0a919c37edba851e952c112a8d0ac4f1c4a58f6856b5d118fd26ad75098d5da5`,
records one ordinary table-object control with actual page-padding production.
Genuine Widget TextManager (`0xaeea0`, configured width 360/density 1),
BodyTextPageObstacle (`0xb869c`) and complete `updateObstacle`, `0xb2cfc`,
execute `MakeObstacle`, `0xb8964`, and `updatePagePaddingRect`, `0xb8dfc`.
Native Constants 63/64/65/66, document page-list/max-width/height getters and
actual list traversal produce padding `[0,-10,240,10]` and `[0,590,240,610]`;
these rectangles are not seeded. Two genuine object-size callbacks pass the
first pair and then relative `[0,580,240,600]` into Drawing split handling at
`0xad040`. The actual document bound is
`[0,10.000999450683594,161,102.8010025024414]`.

Native document measurement, ordinary page-range lookup, source cell feedback,
placement/notifications, six passive Model transitions and three cloned-writer
stages retain the ordinary capture's execution. The writer records 18 runs,
ten selected clips, six successful intersections, 48 position floats and
30 balanced Paint creations/releases. Strict independent replay matches all
81,555 bytes under four fills and repeat zero.

The supplied page record has a null WPage pointer, so native nontext obstacle
selection returns an empty list. Source review of a genuine empty page-object
list supports the same empty-selection result, but no nonnull WPage runtime
control executes. The native padding producer is established for this explicit
null-pointer boundary. Supplied document dimensions, selected page width,
owner associations/page origins and text-derived group range remain inputs;
full document setup/page conversion, page indexing, nonnull WPage selection,
saved parsing, device ICU, aggregate visibility and final PDF backend/pixels
remain excluded from native execution.

### Certified ordinary one-page cell clipping

The bounded Rust composition now carries the actual source Model/placement
context into table text export. The parsed regression
[`parsed_document_exports_preserve_the_captured_parent_clips`](../../crates/sdocx/src/render/native_table_clip_tests.rs)
matches this fixture's parent entry and callback bounds, four cell frames,
six exported run origins and glyph X positions, and two conditional clip
rectangles. Cached and fresh normal SVG agree; direct PDF retains selectable
text, and parsed source models remain immutable. The serialized source must
explicitly retain `TableContentBBox [0,0,160,8]`; absent content bounds are not
normalized into that value. This parsed Rust control is separate from the
native fixture's explicit constructor route, which bypasses saved parsing.

Admission requires the retained ordinary U+FFFC source profile and supported
cell/parent geometry, one page in mode 0 at density 1 with zero flow padding,
and an empty original `Page.objects` list. The original Model source bounding
box is authoritative through `BodyTextSlice`; positive physical page bounds
and measured height zero remain separate. Page-padding and callback bands must
match the native producer before the parent-entry certificate can supply
per-cell Model/clip provenance. Changes from empty to nonempty to empty original
page objects withdraw and restore that certificate and invalidate cached `Rc`
plans; unchanged repeated renders reuse the plan. The
[cache regression](../../crates/sdocx/src/render/native_table_cache_tests.rs)
compares fresh/cached output at each transition. With nonempty page objects,
the source lies outside this certificate and keeps compatibility clipping
around its six source carriers. These checks establish the admitted public
composition, not general pagination, native cache lifecycle, browser newline
behavior or device pixels.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --bodytext-table-placement-one-page-native-obstacles scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenSkia.so \
  crates/sdocx/assets/fonts/Roboto-Regular.ttf \
  scratch/apk-analysis-native/arm64-v8a/libSPenLibxml2.so \
  scratch/apk-analysis-native/arm64-v8a/libc++_shared.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenContent.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBodytext.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so > /tmp/table-bodytext-one-page-obstacles.json
cmp /tmp/table-bodytext-one-page-obstacles.json conformance/table-bodytext-one-page-obstacles.json
```

### Final PDF text clip paths

The Pdfium text handler consumes the explicit clip in
`0xa23e4`–`0xa2460` of `libSPenPdf.so`. It builds the rectangle with
`CPDF_Path::AppendRect` after converting Y from the supplied page height:
`[left, page_height - bottom, right, page_height - top]`. Each subtraction
widens the rectangle coordinate to f64 and narrows the result back to f32
(`0xa23f4`–`0xa2408`). The path is transformed with the paint's rotation and
translation, then supplied to `CPDF_ClipPath::AppendPath` with fill type 1.

The linked `libSPenPdfiumB.so` supplies the actual path constructor at
`0x47957c`, rectangle append at `0x479768`, matrix conversion at `0x56efd8`,
path transform at `0x4796e4`, and copy constructor at `0x479584`.
`CFX_Path::Transform`, `0x5247dc`, uses f32 multiplication, fused multiply-add
and a separate translation addition for each point. Copy construction retains
the same backing path and increases its reference count to two; it does not
copy the point array.

[`table-text-clip-paths.json`](../../conformance/table-text-clip-paths.json),
SHA-256 `e5f6d442e39aedf995d807a276d86ab7ff877c31d044a5597b620adb4eaa6bdc`,
extends all 132 clip cases through native path construction and transformation.
Of 105 Composer-selected clips, 69 pass the nonempty gate and produce 345
retained path points. Each rectangle has an initial move, four line points,
and a close flag on the repeated first point. Allocation and supplied memory
fills `0x00`, `0xa5`, and `0xff` produce identical serialized results.

The capture supplies page height 800 and zero rotation. Composer's center-relative
clip and paint translation reconstruct the world clip in PDF coordinates,
within intermediate f32 rounding; direct world scaling is not bit-identical.
The Rust retained-PDF transport regression consumes the supplied world clips,
compares decoded PDF clipping bounds with the native path points, and verifies
selectable `ActualText`, embedded fonts, no image resources, closed winding
clips and restored clipping for neighboring text. Its 132 pages include all
69 nonempty paths; bounding rectangles compare within the test's tolerance of
two f32 steps at the 800-point page height. This checks transport of supplied
clips, independently of production run grouping and conditional clip selection.
Allocation/deletion, memory copy, page bounds, paint translation recording,
clip-holder initialization and the final append sink are host interfaces.
The sink records the real retained path before installing it; installed clipping,
destruction, shaping, the complete `DrawText` call, PDF serialization and device
pixels are outside this capture. The Pdfium library digest was checked against
the APK entry as well as the extracted ELF.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-clip-paths scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenComposer.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenPdf.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenPdfiumB.so > /tmp/table-text-clip-paths.json
cmp /tmp/table-text-clip-paths.json conformance/table-text-clip-paths.json
```

### Retained text entry and run bounds

Text `ParagraphLayout::SetLayout`, `0x6b4a4`, adds the line's adjusted top
margin to the supplied cursor before calling `GetBaseline`, `0x6cb0c`.
For ordinary entries it writes rectangle member 16 as
`[x, cursor_before_baseline, x + advance, cursor_after_baseline]`
(`0x6b6e8`–`0x6b708`). That vertical extent includes line spacing and the
object-metric epsilon. Member 32 instead retains the glyph ink rectangle,
translated from its old entry position to the new X/baseline
(`0x6b70c`–`0x6b734`). These are separate rectangles.

`getDrawnTextRun` offsets both rectangles and maintains separate unions:
member 16 supplies the union at stack offset 560; member 32 supplies the union
at 576. `appendTextBlock` stores them at `DrawnText` members 104 and 88,
respectively. Thus the table writer's overflow check uses layout height,
independently of glyph ink. With font/base height 20, multiplier `1.35f` and
cursor 0, the captured entry spans Y=0–27 with baseline 20; its supplied ink
rectangle relocates to Y=5–23.

`inSameDraw`, `0x65998`, requires nonnull Font wrappers, matching direction,
exact f32 horizontal adjacency, selected span fields and native font IDs.
Kind 5 prevents merging. LTR compares previous X plus advance with current X;
RTL compares current X plus advance with previous X. One representable step
to either side of adjacency fails. A changed baseline alone passes this
predicate. Distinct wrappers with null FontImpl pointers both yield native
ID -1 and pass; this does not establish equality for real resolved faces.
Captured span mutations at members 0, 4, 8, 12, 16, 24, 32, 36, 40 and 66
prevent merging; mutations at 44, 52 and 60 do not affect this predicate.

[`table-text-bounds.json`](../../conformance/table-text-bounds.json), SHA-256
`a3fd5eba3513c55bfbe3e3e14fe069592a384989700079ae0537ddbd1fbe1d8c`,
contains 162 placement cases, 810 entries, 378 run unions and 22 isolated
grouping probes. Native `SetLayout`/`GetBaseline`, alignment, spacing,
`inSameDraw`/span comparison and rectangle union/storage instructions execute
unchanged. Every output repeats with memory fills `0x00`, `0xa5` and `0xff`.
The Rust line-placement regression matches local tops, bottoms, baselines
and post-cursors as exact f32 bits. Frame-origin composition remains separate
from these local comparisons; see [local line bands](text-layout-findings.md#rust-f32-local-line-bands).

Entry advances/ink bounds, logical maps, block metric flags, line metrics,
spacing, offsets and ordinary span inputs are supplied. Memory copy is a host
interface. Wrap selection, shaping, actual embedded objects, bullets,
justification, emoji, real-font gates and the complete `getDrawnTextRun` loop
are outside this capture. The complete cached-glyph emitter is captured separately
below. Rust uses a table-wide text clip; conditional per-run clipping and device
appearance parity are not established.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-bounds scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-text-bounds.json
cmp /tmp/table-text-bounds.json conformance/table-text-bounds.json
```

### Complete retained text run emission

[`table-text-runs.json`](../../conformance/table-text-runs.json), SHA-256
`dae6b6929ce1349eefdcb9202bffd4ac9393c3b731511ccf1f493d38cdba0792`,
records 230 cases, 1,116 supplied entries, 528 emitted runs and 1,180 output
glyph codewords. The Rust harness executes complete native
`RichTextDrawing::getDrawnTextRun`, `0x66c98`, and `appendTextBlock`, `0x67ebc`.
Cached `RichTextMeasure::GetGlyphInfo`, `0x78274`, copies supplied glyph vectors;
`RichText::GetSpan`, `0x61f3c`, reads supplied spans. Run grouping, rectangle
unions, temporary-vector growth, RTL reversal and output allocation execute
unchanged. Every serialized field repeats across memory fills `0x00`, `0xa5`
and `0xff`, including initially empty and preallocated output vectors.

Ordinary adjacent entries form one run per line. Changing the middle entry's
font size or resolved font ID splits the surrounding entries. Distinct Font
wrappers with equal supplied IDs remain grouped. A middle baseline change alone
does not split the run; its origin retains the first entry's baseline. Moving
the middle X by one representable f32 step splits the first run, even though the
following adjacency can still pass through native addition rounding.

`inSameDraw` is only the first grouping gate. At `0x66f18`–`0x66f44`,
`getDrawnTextRun` queries the previous font's implementation slots 56 and 80.
The public wrappers `Font::IsBitmapFont`, `0x85d98`, and `Font::GetLanguage`,
`0x85db0`, identify those slots. Bitmap fonts force a split. Language length 8
and exact bytes `und-Deva` also force a split (`0x670cc`–`0x6710c`);
`und-Deve` and `und-DevaX` do not. Both short and allocated NDK string
representations exercise this check. The fixture supplies these font interfaces;
it does not establish which fonts native selection resolves for actual text.

The run origin and both rectangle unions include caller X/Y offsets. Vertical
gravity at RichTextImpl member 212 is added to caller Y before placement
(`0x66d0c`–`0x66d14`). Stored glyph positions are entry X plus cached glyph X
offsets. RTL reverses the accumulated codeword and position vectors, including
the glyph order within a multi-glyph entry. Each output retains one baseline;
the supplied third glyph field of -0.75 does not produce per-glyph Y positions
in these records. This capture does not execute the PDF glyph consumer.

`appendTextBlock` copies foreground, background, font size and style into the
record. Span member 40 bit 1 is stored separately at DrawnText member 120.
Bit 0 selects foreground `0xff0054ff` and adds style bit 2
(`0x68164`–`0x68178`). A selected paragraph's nonzero byte 65 then sets
foreground alpha to `0x66` and adds style bit 3 (`0x681bc`–`0x681dc`), retaining
the link override's RGB when both apply. These are storage contracts; the
capture does not establish final decoration or theme appearance.

The Rust line-placement regression checks all 230 cases and the emitted local
run tops, bottoms and baselines as exact f32 bits. The expanded
[cached-entry snapshots](text-draw-identity-findings.md#captured-cached-entry-snapshots)
also verify the typed emitter's grouping and geometry for supplied cache inputs.
Those comparisons do not establish production shaping, horizontal adjacency,
complete world-frame composition or visual parity.
Inputs include supplied metrics, logical maps, cached codewords/offsets, spans,
font getters and empty or single-record paragraph vectors. Allocation, deletion
and memory copy/move are host interfaces. Emoji slices are null. Native shaping,
font selection, wrap selection, actual embedded objects, bullets, justification,
emoji images, Composer clipping and final PDF paths/pixels remain outside this
capture. Rust retains the conservative table-wide text clip.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --text-runs scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-text-runs.json
cmp /tmp/table-text-runs.json conformance/table-text-runs.json
```

### Cell background selection

[The background capture](../../conformance/table-backgrounds.json), SHA-256
`46fe91b4a788c5276534b641b4a7224604a2410154724cd8ce56296809b78f44`,
records `TableCell::GetBackgroundColor(bool)` (`0x3c2384`) and
`ObjectTableImpl::GetCellBackgroundColor(int,int,bool)` (`0x3cbf7c`). The
hash-checked Rust harness executes both getters, native row forwarding and
the bound callback operators. Their outputs agree for 40 synthetic 3×3 grids:
720 selected colors across both values of the heading-override argument.
The inputs cover all heading flag combinations, owned/unowned cells, zero and
partial-alpha colors, and unit/merged spans.

`ObjectTableImpl::getCellBgColor`, `0x3c8cfc`, selects heading color when the
stored row index is zero with heading-row enabled, or the stored column index
is zero with heading-column enabled. Otherwise it selects the default cell
color. The constructor initializes both colors to ARGB zero at `0x3c5d2c`.
`TableCell::GetBackgroundColor` returns this inherited value when the cell
has no owned background, or when heading override is enabled for a heading
cell; otherwise it returns the owned color. The table getter selects the raw
stored cell, independently of merged-frame ownership.

Drawing requests heading override at `0xa79f8`, then calls the context's color
conversion with theme mode 3 at `0xa7ad0`. `Context::GetColor`, `0x6c39c` in
`libSPenView.so` (SHA-256
`c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8`),
selects the active default color theme for that mode. The light and dark
theme primitives preserve the input alpha. Drawing passes the resulting ARGB
to `SPPaint::SetColor` at `0xa7ae0`; no opaque beige substitution appears in
this path.

The Rust fill model matches every captured selected color. SVG/replay and PDF
preserve the alpha of inherited and owned fills, including zero. Text contrast
uses the fill composited over the resolved paper, in both measurement and final
drawing. This retains the renderer's documented contrast policy; the captures
do not establish device appearance or application-level theme selection.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --backgrounds > /tmp/table-backgrounds.json
cmp /tmp/table-backgrounds.json conformance/table-backgrounds.json
```

Rust regressions exercise saved row maxima and enabled table-wide height limits
below measured content height through callback preparation, final drawing,
light/dark SVG preview, replay and retained
PDF. They require unchanged source ranges, baselines and vector text, with no
PDF image resources. These establish the metadata-invariance and transport
contracts; the current locked Samsung corpus has no dedicated height-limit or
merged/sparse table source/reference pair.

## Shared code-block text layout

Drawing `CodeBlockLayout::initConstants`, `0x73084`, resolves these IDs through
`Constant::GetPixels`. The Content library stores the values in 24-byte records
at `0x77e8 + 24 * id`. Each record below has unit kind 3, document density,
and rounding mode 3, no rounding. These values come from the native table,
not measurements of the captured PDF.

| Constant ID | Value before density scaling | Layout use |
| ---: | ---: | --- |
| 332 | 16 | Left padding |
| 333 | 12 | Top padding |
| 334 | 16 | Right padding |
| 335 | 12 | Bottom padding |
| 338 | 12 | Horizontal gap between title and copy button |
| 339 | 8 | Vertical gap around body |
| 342 | 24 | Copy-button width and height |
| 343 | 12 | Rounded radius |

Drawing `CodeBlockLayout::Measure`, `0x732fc`, positions the copy button at
`width - rightPadding - copySize`, `topPadding`. The title frame extends from
left padding to `copy.left - horizontalGap`, using the copy button's top and
bottom. The title measurement return is discarded at `0x73410`; the original
frame is retained at `0x73428`.

The body starts at `copy.bottom + verticalGap`, with width
`width - leftPadding - rightPadding`. Both objects use ordinary
`ObjectTextDrawing` through `measuredObject`, `0x73694`, which returns
`bottom = input.top + TextLayout::GetHeight(false)` at `0x73764`–`0x73770`.
The final object bottom is `original.top + measuredBody.bottom + verticalGap
+ bottomPadding` at `0x73584`–`0x735a0`: the vertical gap is reserved both
above and below the body. The split-input behavior is captured below.

The adapter supplies frames and padding, not fixed text baselines or line
advances. Paragraph styling, wrapping, margins and text metrics belong in the
same text engine used for other rich-text objects.

### Code-block chrome and split inputs

[`table-code-layout.json`](../../conformance/table-code-layout.json), SHA-256
`cf98c08c0a67850b3db1bf044b5056161742c7853456cdcc9f7b4c2c044f6f0f`,
records 18 cases with cold, warm and cleared measurements, repeated under
heap allocation fills `0x00`, `0xa5` and `0xff`. The
[capture module](../../conformance/native_table/code_layout.rs) executes complete
Drawing `Measure`, `0x732fc`, `measuredObject`, `0x73694`, `ClearMeasure`,
`0x737d0`, and `GetMinHeightInFirstPage`, `0x738f4`. Native Base RectF, Matrix
and List operations execute, as does Model `ObjectCodeBlock::GetBody`,
`0x474570`, on supplied implementation fields.

Only the first split rectangle's top affects the header. If it is strictly
less than the original copy-button bottom, that top is added to the header's
vertical coordinates; equality does not shift it. Later rectangles and
horizontal intersection do not enter this decision. Split lists retain input
order and are translated to body-local vertical coordinates. Cases cover
touching and adjacent-float boundaries, negative/zero tops, unsorted lists,
fractional/double density, translated/narrow bounds, differing title heights
and empty/absent bodies. Title measurement does not resize the title frame.
The first-page minimum adds top padding, retained title rectangle height
(the copy-button height), vertical gap and first body-line height. It does
not use the measured title text height; changing a header origin does not
change this retained height.
Warm `Measure` ignores mutated bounds and child heights until `ClearMeasure`;
the cleared call consumes those inputs.

Source bounds, child frame/update/measurement interfaces, child text heights
and density-scaled constants are supplied. Native instructions compute output
geometry; child interfaces record frames and padding without shaping, wrapping
or margin layout. Native object/layout construction and constant resolution,
upstream constraint-specific frame selection, compositor split production,
complete nested code/table pagination and final drawing do not execute.

The production `NativeCodeGeometry` kernel matches all 18 cold and 18 cleared
chrome outputs at exact f32 precision. `prepare_code_frame` uses it for callback
preparation and fresh drawing; translated code-block SVG/replay/PDF regressions
pass. This establishes the captured chrome geometry, without warm-cache parity,
native child-placement parity, large-origin or nested-callback claims.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --code-layout scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-code-layout.json
cmp /tmp/table-code-layout.json conformance/table-code-layout.json
```

## Border records

Borders use the same untyped offset/mask header as rows and cells. The current
writer emits offset zero, a one-byte zero property mask and a two-byte zero
field mask, followed by four edge records. The resulting size is
`9 + 4 * 16 = 73` bytes, excluding the parent's four-byte size prefix.

Each edge stores `u32` ARGB color, `f32` width, `f32` start radius and `f32` end
radius. The wire order is **left, top, right, bottom**: `GetBorderStyleLeft`
at `0x3dc830` returns the first in-memory style, and the top/right/bottom
getters at `0x3dc834`, `0x3dc83c` and `0x3dc844` add 20, 40 and 60 bytes.
The serializer copies only the first 16 bytes of each 20-byte in-memory style;
the two runtime flags at offsets 16 and 17 are not serialized.

The JNI conversion writes color at offset 0 and the three floats at offsets
4, 8 and 12. Its field-name strings at `0x14c26b`, `0x1512ec`, `0x137f52`
and `0x156998` are `color`, `width`, `startRadius` and `endRadius`, confirming
the public names independently of decompiled class field order. Captured axis
radii and the limits of corner-painting evidence are described in
[border painting](#border-painting).

## Validation and limits

The note parser's synthetic unit tests cover variable masks, valid zero offsets,
cell identity/geometry, all offsets that truncate fixed cell or row data, offsets
beyond the record, wider field masks and nested cells. Additional tests cover
all 14 table fields individually and together, every truncated field prefix,
all combinations of the three table flags, known and unknown auto-fit modes,
independent edge colors and radii, sized-border boundaries, bounded column
allocations, and the nonnumerical row-height field order. Unknown bytes stay
available for inspection. The existing embedded table/code-block tests continue
to pass. These tests establish bounds and structure, not Samsung rendering
fidelity.

The shared Rust engine measures and paints supported embedded table/code text.
Table border painting follows the captured Model geometry and traced Drawing
rules within the evidence limits above; standalone table decoding is not
established. Inherited shape data and native layout behavior are separate from
the bounded embedded-record decoding described here. Current layout support
and transport limits are documented in
[vector text support](../text-vector-support.md).
