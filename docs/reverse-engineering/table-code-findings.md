# Native table and code-block records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ARM64 virtual addresses in `libSPenModel.so` unless stated otherwise. This pass
uses native serializers/readers and synthetic records without new SDOCX files.

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

Bits 0/1 were previously mislabeled as cell padding. The reader stores them
at members 12/8 (`0x3cfb38`–`0x3cfbd4`); the public getters establish their
size-limit meanings independently. Rust fields and serialized inspection keys
are now `min_column_width` and `min_row_height`; deserialization also accepts
the old JSON keys. Rust field access and emitted inspection keys change, while
the raw optional values and rendering behavior are unchanged. Cell text margins
are separate: absent
margins default to zero in `TextCommonImpl` (`0x3e747c`–`0x3e7484`), and the
table-cell constructor does not replace them.

Cold Drawing measurement reads saved row heights and column widths directly
(`0xaaa60`, `0xaaab8`) and does not consult auto-fit mode. Model auto-fit acts
through `fitTableRect` and `SetAutoFitOption` (`0x3c6b94`, `0x3cbaa0`), outside
that cold measurement path. An absent mode still means native default 3;
accepting its existing geometry does not interpret absence as mode 0.

`RichTextTable.style` now exposes these properties alongside existing actual
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

A row's fixed data is `f32` height, `u32` row index, `u32` cell count, then a
`u32` payload size and cell record per cell. A cell's fixed data contains
`u32` column index, row span, column span and ARGB background; four `f64`
coordinates; `u8` editability; and a sized rich-text object.
Cell property bit 0 identifies an owned background color.

The editability byte was previously mislabeled as vertical alignment. The
writer loads member 80 at `0x3c30a0` and writes it at `0x3c30ac`.
`TableCell::SetEditable`, `0x3c24a0`, stores the same member at `0x3c24b0`;
`IsEditable`, `0x3c24c4`, reads it directly. The reader loads the byte at
`0x3c344c`, compares it with zero at `0x3c3458`, uses `cset ne` at
`0x3c345c`, then stores the boolean at `0x3c3460`. Every nonzero byte means
editable, including 2 and 255. Rust exposes `editable: bool` with this
normalization; the retained embedded object record preserves the original bytes.

The SDK now bounds each fixed reader at the declared flexible offset. Previously
it only rejected offsets beyond the record after parsing the fields, allowing
a malformed fixed-field length to consume flexible bytes. Invalid offsets now
fail before those reads. Offset zero remains valid with an empty field mask.
The shared mask reader supports wider future masks without truncating the
check for set bits. Unknown embedded bytes remain in the original object data.

## Row height and cell-border findings

Row flexible fields are **not** in ascending bit order. Both writer and reader
place `f32` maximum height under bit 9 before `f32` minimum height under bit 1.
`GetMaxHeight` at `0x3c4244` reads member offset 124; `GetMinHeight` at
`0x3c4300` reads offset 128, matching the serialization accesses. Default maximum
height is `f32::MAX`, and default minimum height is zero. `RichTextTableRow`
now exposes both values when present. If any unknown row field bit is set,
neither constraint is decoded, and the complete flexible payload is retained.
The nonnumerical native order does not establish where an unknown field would
appear, so decoding known fields past that uncertainty would be unsafe.

Cell flexible bit 0 contains a four-byte size followed by a table-border
payload. The analyzed writer writes size 73 and then calls
`TableBorder::NewGetBinary` (`0x3c31a0`–`0x3c31bc`). `RichTextTableCell.border`
now decodes that record within the declared payload size. Its bytes cannot be
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

## Export row sizing and merged-frame ownership

The ARM64 Drawing library used here has SHA-256
`788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd`;
Model has SHA-256
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.

Drawing's cold `TableLayout::init`, `0xaa6b4`, reads actual row heights at
`0xaaa60` and column widths at `0xaaab8`. `extendRowBySplit`, `0xaaf2c`,
measures cell height differences and grows the row for a strictly positive
maximum difference (`0xab0ac`–`0xab0f0`). The warm
`updatePositionFromRow`, `0xaecf4`, selects the largest measured visible-cell
height. Only a result below the small-height threshold is replaced by
`ObjectTable::GetMinRowHeight` (`0xaedfc`–`0xaee10`); the stored minimum is
not an unconditional lower clamp on every nonempty row.

These selected cold/warm export routines do not read the saved row maximum.
Model exposes that value through `ObjectTable::GetMaxRowHeight`, `0x3d3844`,
and `TableRow::GetMaxHeight`, `0x3c4244`, but its presence must not reject the
export preparation path or introduce a maximum-height clamp. Rust now accepts
saved row maxima while retaining their parsed values. The table-wide
maximum-height flag is separate and still outside the supported prepared path.

Model `ObjectTableImpl::GetCell`, `0x3c7570`, indexes the stored row and column
vectors directly (`0x3c75a8`–`0x3c75b0`).
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
visible owners and cached frame geometry. Sparse records, frame union, rowspan
growth and malformed overlap handling are not yet established by these traces.

`TableLayout::getHalfBorderWidth`, `0xb325c`, iterates
`ObjectTable::GetBorderPath` (`0xb3288`) and selects positive widths with
nonzero ARGB values (`0xb32ac`–`0xb32c4`) before halving the maximum.
Model's `GetBorderPath`, `0x3cb464`, reads the outer border pointer at member
152 and constructs its four paths. This initializer does not inspect every
cell border. The later per-edge drawn-bound widths include boundary cells and
do not gate on color; those two measurements must remain distinct.

Rust regressions exercise saved row maxima below measured content height through
callback preparation, final drawing, light/dark SVG preview, replay and retained
PDF. They require unchanged source ranges, baselines and vector text, with no
PDF image resources. These establish the metadata-invariance and transport
contracts; the current locked Samsung corpus has no dedicated row-maximum or
merged/sparse table reference capture.

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
above and below the body. Split-page padding rectangles can shift these frames;
their full behavior remains unverified.

The adapter supplies frames and padding, not fixed text baselines or line
advances. Paragraph styling, wrapping, margins and text metrics belong in the
same text engine used for other rich-text objects.

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
the public names independently of decompiled class field order. The exact
corner geometry used by the native renderer remains to be investigated.

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
Table border styling remains approximate, and standalone table decoding is not
established. Inherited shape data and native layout behavior are separate from
the bounded embedded-record decoding described here. Current layout support
and transport limits are documented in
[vector text support](../text-vector-support.md).
