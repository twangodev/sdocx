# Native text layout inputs and defaults

Inspected Samsung Notes 4.4.45.37 ARM64 libraries in
`scratch/apk-analysis-native/arm64-v8a` and matching decompiled SDK sources.
Addresses below belong to the named library. These are native implementation
findings, not a claim that the SDK matches Samsung typography visually. The
[standalone text findings](text-box-findings.md) describe the shared `TextCommon`
serialization; [PDF export findings](pdf-export-findings.md) describe the captured
body-text reference.

## Text defaults and range application

`libSPenWidget` exports `ObjectTextLayout::DEFAULT_FONT_COLOR` at `0x66410`;
`libSPenDrawing` exports `ObjectTextDrawing::DEFAULT_FONT_COLOR` at `0x52b24`.
Both contain bytes `26 26 26 ff`, or ARGB `0xFF262626`.

This value is used, not merely an unused exported constant. Widget
`ObjectTextLayout::updateSpan`, `0xd4fc4`–`0xd4fd0`, and Drawing
`ObjectTextDrawing::UpdateSpan`, `0x8d08c`–`0x8d098`, pass it through context
virtual slot 80 with usage value 3. They store the returned color into each
text slot at `0xd4ffc` and `0x8d0c8`, before applying stored styles. The context
may transform the color for a theme; the constant is the untransformed default.
The decompiled `composer_def_font_color` resource is `#252525` in
`resources/res/values/colors.xml`. A UI or zero-length caret style using that
resource does not replace the native rendered-text default, or define stroke
ink color.

The same initialization uses `(17 + font_size_delta) * resolved_scale` as its
default font size: Widget `0xd4fdc`–`0xd4ff8`, Drawing `0x8d0a8`–`0x8d0c4`.
Widget also exports `ObjectTextLayout::DEFAULT_FONT_SIZE` at `0x6640c` with
the integer bytes `11 00 00 00` (17).
The resolved scale comes from context and text-scale state; these functions do
not establish that every document uses a universal factor of 3.

Stored styles then apply to the intersection of their half-open UTF-16 range
and the requested text interval. Widget `convertTextSpan`, `0xd56b8`, clamps
the endpoints at `0xd5794`–`0xd57a0`, skips an empty intersection at
`0xd57a4`–`0xd57a8`, and applies the style to each selected slot at
`0xd57c8`–`0xd57ec`. Drawing repeats this at `0x8d6c4`–`0x8d71c`.
The source list is traversed in order; subsequent assignments can replace
earlier values for the same field. A `start == end` style has no rendered
characters in this path. Preserve it as editing metadata without promoting
it to a whole-box style.

Widget `TextLayoutUtil::ConvertFontSizeSpan`, `0xdb794`, preserves start/end
and interval flags and computes `max(1, stored_size + font_size_delta) *
text_scale` at `0xdb800`–`0xdb808`. No upper-size clamp appears in that
conversion. `TextLayoutUtil::SetFromObject`, `0xdae94`, transfers the text,
all ordinary spans, separate font-size spans, paragraphs, gravity and ellipsis
settings to a `TextLayout`. A shared Rust input adapter should preserve these
inputs instead of choosing the first style of each kind for the entire box.

## Placed text adapter and rectangle inputs

Drawing `ObjectDrawing::drawTextContent`, `0x80818`, constructs an
`ObjectTextDrawing` at `0x80ac8`, positions its `ScrollEditTextView` at
`0x80bd8`, then calls `Update`, `Measure` and `DrawObject` at
`0x80c78`–`0x80c8c`. This is the ordinary placed-object route. The direct
Drawing caller of `TextLayoutUtil::SetFromObject` is instead
`ObjectImageDrawing::drawHintText`, `0x86120`, with scale 1 and delta 0;
its helper's paragraph defaults do not establish ordinary placed defaults.

The ordinary route reads the object's virtual slot 168 at `0x80af8`.
Model's shape/text-box vtables identify this as `ObjectShapeBase::GetRect`,
`0x37aa94`, forwarding to `ObjectBase::GetRect`, `0x2caa60`. The latter
returns four saved rectangle floats directly at `0x2caa68`–`0x2caa70`;
it does not compute the rotated `GetBounds` rectangle. For object type 7,
the Drawing caller separately adds template left/top margins and subtracts
template right/bottom margins (`0x80b28`–`0x80b68`). It preserves the original
rectangle's center as the rotation pivot relative to that inset rectangle
(`0x80b7c`–`0x80bc0`). Ordinary type-2 text boxes do not take that template
inset branch. Saved object rotation is supplied separately at `0x80c00`–
`0x80c44`; do not wrap against the rotated bounding box.

`ScrollEditTextView::SetPosition`, `0xbc218`, rounds `View::GetWidth`
upward to an integer at `0xbc234` and stores it as measurement width
(member 1272). `MeasureText` passes that width directly to
`TextLayout::Measure` at `0xbc890`. The view's layout then supplies that
width and an upward-rounded `View::GetBounds().Height()` at
`0xbc998`–`0xbca04`. These are outer view dimensions, with margins supplied
separately; avoid rounding an already inset width in place of this adapter
input. `Layout` keeps the fixed measured view width unless it is nonpositive
or word wrapping is disabled (`0xbca40`–`0xbca60`). This does not prove an
object-resizing or auto-fit policy.

Inside Text layout, `sm_ApplyIndentLevel`, `0x7395c`, starts paragraph
left/right margins from RichText members 128/136 (`0x73964`–`0x73978`,
`0x739b0`–`0x739c0`). `ParagraphLayout::m_CopyLayoutData`, `0x6aa00`, reads
those resolved paragraph margins at members 120/124 and sets its wrap bounds
to `left_margin` and `outer_width - right_margin` (`0x6aa80`–`0x6aaa4`).
The outer integer width is converted to float in `CalculateParagraphLayout`
at `0x73cf0` and transferred into this layout data at `0x73d44`.

Drawing `ObjectTextDrawing::UpdateTextBound`, `0x8cdc8`, multiplies all four
component text margins by its manager's document pixel value at
`0x8ce6c`–`0x8ce78` and calls `ScrollEditTextView::SetMargin` at `0x8ce84`.
It does not multiply margins by local text scale here. Widget
`ObjectTextLayout::updateBound`, `0xd713c`, does multiply by both document
pixel and local text scale (`0xd71f8`–`0xd7220`). Drawing forwards gravity
at `0x8ce90`–`0x8ce9c` and ellipsis at `0x8cea8`–`0x8ceb8`; Widget forwards
them at `0xd7230`–`0xd7258`. Keep these context adapters distinct while
sharing the paragraph measurement engine.

Fresh `RichText::Construct` stores `0x00010000` at member 112
(`0x61db0`–`0x61db8`): single-line member 113 is false and word-wrap member
114 is true, confirmed by their setters `0x628c0` and `0x628f4`. The inspected
ordinary Drawing construction/update route does not override those values.
`ObjectTextDrawing::DrawTextContent` forwards the rectangle and point to
`TextLayout::DrawRect` with its boolean argument true (`0x92870`–`0x92884`).
This boolean alone is not evidence for shape-path clipping: the low-level
drawing also selects visible lines from the rectangle (`0x64c48`–`0x64c68`).

No `GetTextAreaType` or `GetTextAutoFitOption` read was found in the inspected
ordinary Drawing/Widget adapter route. The saved Margin/Free/Path values are
verified in [text-box-findings.md](text-box-findings.md), but mapping Free to
no-wrap, Path to a shape clip, or auto-fit to dynamic font resizing remains
unverified. Upstream editing/resizing and export clipping need captured
standalone cases and their callers before adding those semantics. The body
baseline caveat below also remains unresolved for placed exports; a shared
low-level helper is insufficient to establish their layout origin.

## Font-name payload and measured fallback

For the modern WDoc record, `FontNameSpan` (kind 4) has this payload **after**
the 16-byte kind/start/end/expansion header, excluding the outer record length:
`[8 skipped bytes][u16 little-endian UTF-8 byte count][counted UTF-8 bytes]`.
The count includes one trailing NUL. Model `GetBinary`, `0x40974c`, calls
`TextSpanBase::GetBinarySize` at `0x409780`; `0x409794` adds that result to
the original buffer, `0x40979c` writes the count at payload offset 8, and
`0x4097a0` supplies payload offset 10 to `String::GetUTF8`. The WDoc base
size is 16 (`0x40ccbc`, reader `0x40cfb4`–`0x40d050`). Base
`String::GetUTF8Size` adds the NUL byte at `0xc4980`; `GetUTF8` writes it at
`0xc48b0`. An empty native name therefore has count 1 and a zero byte.
`FontNameSpan::ApplyBinary`, `0x4097fc`, skips 8 payload bytes for format
version >= 8 (`0x40986c`–`0x409898`) and 4 for older versions (`0x409900`).
Keep the prefix and original payload; its contents are not explained here.

The Java `SpenFontNameSpan` setter stores the supplied nonnull string.
Model `FontNameSpan::SetName`, `0x409404`, forwards to `String::Set`,
`0xc3380`, which clears and appends the source. These setters do not trim,
case-fold or replace unknown names. A bounded SDK getter can validate the
count and UTF-8, remove the terminal NUL and preserve the original bytes.
Native reading uses `String::Set(char const*)` at `0x40999c`, so an embedded
NUL terminates the native name; malformed payload handling must remain
distinct from a valid but unavailable font name.

Widget `convertTextSpanImpl`, `0xd7ba8`, copies `FontNameSpan::GetName`
into `RichTextSpan` member 24 at `0xd7d60`–`0xd7d98`. Text
`SpanRunFunctor::operator()`, `0x7710c`, selects that name when nonnull,
otherwise the supplied default name (`0x7715c`–`0x77164`), and passes it to
`TypefaceFactoryImplMinikin::CreateFromFontName`, `0x8a614`. The factory
resolves name to family through `FontManager`, calls `resolveFontStyle`,
then creates a typeface for that family. The selected family enters the
Minikin measurement setup at `0x76bbc`–`0x76bec`, before run measurement
at `0x77304`. A font name is a measurement input, not just an SVG attribute.

`FontListParser::GetFontFamilyNameByFontName`, `0x8076c`, obtains the
configured default family first (`0x8079c`) and returns it when its name
lookup finds no entry (`0x807b4`, `0x807d8`–`0x807ec`). Preserve an unknown
stored name while resolving a fallback for measurement. This proves the
fallback rule, not the concrete default family or fallback order on a device.

## Document scale and font-size delta

The rendered scale is `document_pixel * local_text_scale`. Widget
`ObjectTextLayout::updateSpan` reads text-manager virtual slot 144 and
multiplies by its local scale at `0xd4f44`–`0xd4f5c`; Drawing does the same
at `0x8cfbc`–`0x8cfd8` / `0x8d09c`. The TextManager vtable identifies slot
144 as `GetDocumentPixel` (vptr `0xf1ec0`, relocation `0xf1f50`), whereas
`GetTextScale` is slot 440 (`0xf2078`). Local scales initialize to 1 in
Widget `0xd3018`–`0xd301c` and Drawing `0x8c49c`–`0x8c4a0`.

Composer `NoteTextManager::SetDocument`, `0x3a16f0`, assigns document pixel
from `WNote::GetDocumentDensity` when positive, otherwise 1
(`0x3a1734`–`0x3a175c`). The getter itself is now directly verified in
WDoc at `0x9ec70`: it reads default dimensions from implementation members
176/180 and orientation from member 188 (`0x9ec84`–`0x9ecb0`). Orientation
0 selects width; **every nonzero orientation** selects height. It converts
that dimension as signed i32 to f32 and divides by `360.0f`
(`0x9ecb4`–`0x9eccc`). These are the optional default dimensions loaded
after the body object (`WNoteLoadHandler`, `0xa9290`–`0xa92f0`), not flow
canvas dimensions or individual page bounds. The getter does not clamp zero
or negative dimensions; a missing implementation returns 0 at `0x9ed10`.
The fallback to 1 belongs to `NoteTextManager::SetDocument`, not this getter.

`NoteTextManager::SetDocument` takes the font-size delta from
`WNote::GetBodyTextFontSizeDelta` through `TextViewUtil::SetTextSizeDelta`
at `0x3a1774`–`0x3a1788`, and shares the resolved delta with the body editor
at `0x3a1794`–`0x3a17a8`. Widget `CalculateTextSizeDelta`, `0xe0ea8`,
passes an explicit delta through unchanged; `INT_MIN` selects a display
default. The exported tablet/phone default constants at `0x6526c` /
`0x65270` are -5 / 0, respectively.

Standard PDF's X delegate constructs a `NoteTextManager` and calls this
same `SetDocument` at Composer `0x3575ec`–`0x3575fc`. Its page-section
preparation then creates a separate manager through `BodyTextUtil::
CreateTextManagerWithFontSizeDeltaBy` (`0x3a2624`). That helper, Bodytext
`0xcee14`, uses the same `WNote::GetDocumentDensity` at `0xcee60` and
forwards the density unchanged to `SetDocumentPixel` at `0xcee70`, without
the nonpositive clamp. It resolves the stored delta at `0xcee74`–`0xcee88`.
Widget `updateSpan` replaces an exactly zero combined scale with 1 at
`0xd4f60`–`0xd4f94`; this is not a general negative-scale clamp.

Widget's body layout
constructor reads its manager's resolved delta at `0xd3124`–`0xd3138`;
Drawing's constructor does so at `0x8c520`–`0x8c53c`. Bodytext
`BodyTextLayout::SetTextScale`, `0xb3a2c`, forwards a changed local scale
to the shared `ObjectTextLayout` at `0xb3a64`–`0xb3a6c`.
`BodyTextLayout` initializes its local scale to 1 (`0xaf964`, `0xaf9b0`),
and constructs the shared Widget layout at `0xafed0`. Standard PDF's
section preparation constructs this layout, assigns the body document and
measures at Composer `0x3a2640`–`0x3a2654`, without an intervening scale
setter. Both the ordinary placed Drawing layout and this fresh body layout
therefore start with local scale 1.

Interactive body views can change that state. With text scaling enabled,
`BodyTextView::updateTextScale`, `0xdacc0`, multiplies its view delta
(member 1192) by manager `GetTextScale` (slot 440), then forwards the product
to `BodyTextLayout::SetTextScale` (`0xdacf0`–`0xdad4c`). Disabling it resets
the layout scale to 1 (`0xd1e98`–`0xd1ea0`). This proves a view-dependent
local scale; it does not establish that editor zoom belongs in saved exports.

A document density
of 3 can explain scale 3 in a captured fixture; it cannot justify a universal
factor of 3. Export context, native density, stored delta and local scale must remain
explicit inputs to one Rust layout pipeline.

The current parser exposes the stored delta as
`StoredNote::metadata(note_bytes)?.body_font_size_delta` (`NoteMetadata`,
flexible field 11). It is not promoted to `DocumentMetadata`. Neither public
structure currently exposes `document_density`; `StoredNoteHeader` provides
flow dimensions and `StoredNote::default_page_dimensions` provides the
separate default dimensions. Those default dimensions and raw orientation
are sufficient to resolve the verified native density formula, with fallback
policy explicit at the selected layout-context adapter.

## Margins and vertical gravity

`SetFromObject` multiplies left/top/right/bottom margins by its supplied
`text_scale` at `0xdaf04`, `0xdaf10`, `0xdaf1c`, `0xdaf24`, then calls
`TextLayout::SetMargin` at `0xdaf38`. Keep coordinate conversion explicit;
margin values and output SVG coordinates are not interchangeable.

`SpenObjectShape.java:41-43` defines gravity as `TOP=0`, `CENTER=1`,
`BOTTOM=2`. These are a small enum, not Android gravity bit flags.
Text `RichTextDrawing::UpdateGravityOffsetY`, `0x6489c`, reads that enum at
`0x648c0` and assigns:

| Value | Vertical offset |
| --- | --- |
| 0 or another value | 0 |
| 1 | `max(0, (available_height - content_height) / 2)` |
| 2 | `max(0, available_height - content_height)` |

The calculation and stores are at `0x6493c`–`0x64958`. Nonempty content uses
the drawing's measured height. Empty content uses `RichText::GetTextSize(0)`
plus top/bottom margins, `0x6490c`–`0x64930`; it is not treated as zero-height
content. Wrapping and measurement must precede gravity positioning.

## Paragraphs and line spacing

Decompiled `SpenAlignmentParagraph.java:8-11` defines left/right/center/both
as 0/1/2/3. `SpenIndentLevelParagraph.java:7-10` defines none/LTR/RTL as
0/1/2; these differ from deprecated text-box direction constants. Paragraph
records select paragraph ordinals, whereas style spans select UTF-16 offsets.

Indent direction is a separate input from text layout direction. Widget
`convertTextParagraphImpl` maps stored RTL value 2 to internal boolean 0,
and none/LTR to 1 (`0xd8fc0`–`0xd8fcc`); Drawing repeats this at
`0x92f4c`–`0x92f58`. By contrast, the fresh Widget layout reads display
virtual slot 112 and forwards that value to `TextLayout::SetLayoutDirection`
(`0xd31d8`–`0xd3200`); the placed `ScrollEditTextView` constructor does the
same at `0xbb6e8`–`0xbb710`. The Text setter stores it as RichText member 184
(`0x62c70`–`0x62c78`), used by the ICU paragraph-direction fallback described
in the measurement section below.
An indent record is not evidence for forcing its paragraph's bidi base level.
The deprecated text-box direction JNI setter `0x41df58` validates the handle
and returns success without storing the supplied direction; its getter
`0x41dfec` returns 0. Do not use that getter as saved bidi metadata.

The indent increment is also document-specific. Content's constant table
entry 137 at `0x84c0` contains scale kind 3, rounding kind 3, default float
16 and zero alternate values. `Constant::GetPixels`, `0x13504`, supplies
the constructor's density to `CalculatePixels`, `0x1331c`. Kind 3 multiplies
16 by nonzero density, otherwise by document width / 360
(`0x13380`–`0x13398`); rounding kind 3 leaves the float unchanged
(`0x1339c`–`0x133cc`). Drawing supplies manager document pixel to the
constant constructor (`0x8c4e8`–`0x8c510`), separately from local text scale.
Its converter treats the stored level as signed 32-bit, multiplies it by
that increment, then truncates to a signed integer (`scvtf`/`fcvtzs`,
`0x92f34`–`0x92f40`); Widget does the same at
`0xd8fa8`–`0xd8fb8`. Thus an increment of 48 is the density-3 case, not a
universal physical width or `16 * density * local_text_scale` rule.

Indent application depends on alignment in the inspected Text helper.
`sm_ApplyIndentLevel` adds indent width (paragraph member 68) to the left
margin for internal direction 1 and alignment 0/2/4 (mask `0x15`,
`0x7397c`–`0x739a8`), or to the right margin for direction 2 and alignment
1/2/4 (mask `0x16`, `0x739bc`–`0x739ec`). Alignment 3 does not add it in
this helper. These internal paragraph direction values and margins remain
separate from RichText's global layout direction member 184; an adapter
that always applies indent on one side cannot claim this alignment parity.

Widget `TextLayoutUtil::GetParagraphCount`, `0xda9c0`, walks `String::GetChar`
and recognizes CR and LF at `0xdaaec`–`0xdaaf8`. Each is processed separately;
this routine does not coalesce a CRLF pair. Empty text returns one paragraph
at `0xdac70`. A newline at index zero replaces its initial boundary with -1
at `0xdab04`–`0xdab18`, retaining the leading empty paragraph. Base
`String::GetChar`, `0xc470c`, reads a 16-bit unit at `buffer + index * 2`
(`0xc4734`), so these indices include surrogate code units. Do not normalize
line endings or use Rust scalar offsets before resolving stored ranges.

This is also the model and body/drawing ordinal behavior, not only a helper's
convention. Model `TextCommonParaHandler::GetParagraphCount`, `0x402cfc`,
increments for each CR/LF at `0x402d50`–`0x402d60`;
`ToParagraphStartIndex`, `0x400800`, repeats it at `0x400864`–`0x400874`.
Widget `TextViewUtil::GetParagraphIndex`, `0xdc998`, repeats it at
`0xdca24`–`0xdca34`. The body path's `ObjectTextLayout::textToParagraphs`,
`0xd6638`, checks both units at `0xd67a0`–`0xd67ac`, appends a paragraph
for each, and advances the UTF-16 cursor by one. Drawing
`ObjectTextDrawing::textToParagraphs`, `0x91278`, does the same at
`0x913e8`–`0x914d8`. The corresponding paragraph converters apply stored
ordinal ranges to these records. In raw `a\r\nb`, `b` belongs to ordinal 2,
not 1. Any display policy that coalesces CRLF must retain the native ordinal
mapping separately. Reference fixtures still need to establish whether the
Samsung editor permits/saves raw CRLF or normalizes it during text entry.

There are two inspected paragraph conversion paths:

| Native path | Default and conversion behavior |
| --- | --- |
| Widget `TextLayoutUtil::convertParagraphs`, `0xdb5b8` | Initializes pixel spacing 0 and multiplier `1.3f` at `0xdb610`–`0xdb628`; the multiplier's bytes at `0x637c8` are `66 66 a6 3f`. Its `convertParagraphImpl`, `0xdba5c`, stores pixel spacing, before and after spacing directly, with no scale multiplication. |
| Widget `ObjectTextLayout::convertTextParagraphImpl`, `0xd8e38`, and Drawing `ObjectTextDrawing::convertTextParagraphImpl`, `0x92dc0` | Their `textToParagraphs` initializes pixel spacing 0 and multiplier `1.35f` (`0x3faccccd`) at Widget `0xd6680` / `0xd6800` and Drawing `0x912b8` / `0x9144c`. Conversion multiplies pixel line spacing and paragraph before/after spacing by its first float argument; leaves percentage multiplier unscaled. It also resolves indent width using the second float argument. Callers obtain that second argument from `Constant::GetPixels(137)`, `0xd6b2c` / `0x91788`. |

For the second path, Widget's scaling stores are `0xd8ee0`–`0xd8ee4`
(before), `0xd9014`–`0xd9018` (after), `0xd9178`–`0xd917c` (pixels).
Drawing's equivalents are `0x92e68`, `0x92fa0`, `0x93104`. Percentage
stores are `0xd9004` / `0x92f90`. The first path's 1.3 multiplier must not
be asserted as the default of every editor/body/export route without tracing
the selected caller. Neither path proves a native fallback of `font_size * 1.6`.

Text `TextUtil::GetLineSpacing`, `0x8e180`, computes `pixels` when nonzero,
otherwise `(multiplier - 1) * metric`. `GetLineHeightWithSpacing`, `0x8e100`,
adds that amount to a base height. Thus a pixel value is extra spacing in
this helper, not an absolute line height. If the result exceeds the supplied
limit, it marks the overflow flag and returns the original base height
(`0x8e12c`–`0x8e170`). The five float arguments are base height, extra
pixels, multiplier, percentage metric and height limit, respectively.

The ordinary text producer establishes that percentage metric: Text
`SpanRunFunctor` copies the resolved `RichTextSpan` font size to
`MeasureData` members 4 and 60 at `0x7766c`–`0x77674`. `GetBlockInfo`
aggregates the maximum size into `BlockInfo` member 52, with base height in
member 56 (`0x6aef8`–`0x6af44`, `0x6b008`). `LineLayoutInfoManager::
AddNewBlock` takes maxima across blocks into line members 24 and 28 at
`0x6d7b4`–`0x6d7dc`. Ordinary mixed-size text consequently uses the
largest resolved size in the line for both base height and percentage metric,
rather than a raw font ascent, descent or `getFontSpacing` result.

`ParagraphLayout::GetBaseline`, `0x6cb0c`, passes line member 28 as base
height, paragraph members 32/36 as pixel/multiplier spacing, line member 24
as percentage metric and `ParagraphLayoutData` member 36 as the limit
(`0x6cba4`–`0x6cbbc`). It advances the vertical cursor by the resulting
height, then sets the ordinary baseline to `cursor - 0.35f * max_font_size`
at `0x6cbd0`–`0x6cbdc`; the constant bytes at `0x2663c` are `33 33 b3 be`.
For uniform ordinary text with default 1.35 spacing and no limit overflow,
the advance is `1.35 * resolved_size` and the baseline offset from the line
start is `resolved_size`. This does not define glyph ink bounds.

In this inspected helper, explicit spacing changes the baseline too, not just
the next line's cursor.
Writing `F` for the maximum resolved font size of an ordinary text line,
its base height is `F`. With no limit overflow, pixel spacing `P != 0`
therefore yields advance `F + P` and baseline offset `0.65 * F + P`;
percentage multiplier `M` with zero pixel spacing yields advance `M * F`
and offset `(M - 0.35) * F`. For example, `F=20, P=6` places the baseline
at 19 and advances 26; `F=20, M=1.5` places it at 23 and advances 30.
The helper's overflow fallback returns the original base height before
both calculations (`0x8e128`–`0x8e170`), producing offset `0.65 * F`.

`SetLayout` adds the line's aggregated top margin (member 32) before this
baseline call (`0x6b4f0`–`0x6b510`). Completed intermediate lines call it
at `0x6b388`; the final line calls it at `0x6a9bc`. Thus the inspected
ordinary baseline calculation is shared by those lines. Lines containing
objects with margins take a separate baseline branch at `0x6cb90` and can
add an extra offset at `0x6cc0c`; do not apply the ordinary text formula to
embedded objects. Paragraph-edge, bullet, empty-line and page-limit effects
still need captured cases before claiming complete first/last-line parity.

The captured body-text export currently contradicts applying this helper
formula directly to the SDK's body composition cursor. In
`hf/01-basic-formatting.sdocx`, the first heading has stored size 15,
document density 3 and paragraph multiplier 1.6. The body top margin 10
and page top padding 24 place the SDK cursor at 54. The helper formula
would place its baseline at `54 + 1.6 * 45 - 0.35 * 45 = 110.25`, while
the hash-locked Samsung PDF baseline in `conformance/text-metrics.json`
is 98.85. The current body renderer's `cursor + max_font_size` gives 99,
within the captured 0.25 tolerance; its line advance remains 72. The
helper's limit-overflow fallback gives 83.25, so that branch alone does
not explain the capture. Keep the captured baseline contract unchanged
until the export caller, layout-top adjustments and selected context are
resolved. The helper arithmetic and its producer remain verified; their
application to this captured body route is unverified.

The inspected Text drawing path does not establish that missing adjustment.
`RichTextDrawing::getDrawnTextRun` reads `MeasureData` members 8/12 as
the run point (`0x66df0`) and adds the supplied export offset and stored
gravity offset (`0x66d0c`–`0x66d14`, `0x672b8`–`0x672c4`).
`appendTextBlock` stores that point unchanged at `0x680e8`.
`UpdateGravityOffsetY` stores zero for gravity 0 (`0x648c4`–`0x648d0`,
`0x6495c`). This proves offset forwarding in this path, not which layout
context and cursor origin produced the captured export.

## Measurement, body flow and embedded objects

### Run shaping and feature policy

Ordinary text is measured as runs, not by summing independent character
`SkPaint::measureText` calls. Text `ParagraphMeasureImplMinikin::Measure`,
`0x76e28`, joins adjacent spans with `RichTextSpan::JoinableForMeasureTo`
at `0x76fac`, then invokes `SpanRunFunctor` at `0x76fe0` / `0x77030`.
The joining predicate, `0x8dc28`, compares resolved font size, foreground
color, style bits under mask `0xc3`, and font-name string equality; either
object span prevents joining. A color-only transition can therefore change
the native shaping boundary.

`SpanRunFunctor` creates the Minikin paint through `0x76ad4` and obtains
a shaped layout through `0x97b34` at `0x77304`. The style-run virtual
method resolves to `0x96cc8` (vtable entry `0xf57b8`), which reaches the
layout-piece producer `0x9b150` through `0x970fc`. That producer calls
the bundled HarfBuzz shape entry `0xecdcc` at `0x9c5d4` / `0x9c754`.
It retains glyph positions, UTF-16 cluster indexes and character advances;
`SpanRunFunctor` copies the shaped per-UTF-16 advances divided by 100
at `0x77640`–`0x77668`. Paint size was multiplied by 100 at
`0x76b30`–`0x76b44`. Keep source indexes separate from glyph indexes
and shaped clusters in the Rust layout result.

SPen initializes the Minikin feature string empty at `0x76c34`. For a
Latin script run with ordinary zero letter spacing, the layout-piece producer
explicitly supplies `liga=0` and `clig=0` before shaping
(`0x9c548`–`0x9c754`). The 16-byte feature records at `0x267a0` and
`0x26750` contain the tags, value 0, start 0 and end `0xffffffff`.
Its nonzero-letter-spacing branch also adds these features
(`0x9b34c`–`0x9b3d8`). This proves disabling those two optional ligature
features for the inspected Latin path; it does not disable required script
shaping or establish the policy for every other script.

No `kern=0` override was established in this producer/caller trace. The
captured heading matching an unkerned bundled-font width is useful fixture
evidence, but does not prove a universal native kerning setting. Font choice,
style-run boundaries and native advance quantization still need comparison.
The native horizontal-advance callback, `0x9d728`, converts a font advance
to a 256-scaled integer with `trunc(advance * 256 + 0.5)`; its vector
counterpart at `0x9d858` truncates each 256-scaled advance. Those inputs
use the 100-scaled paint above. Do not infer pixel-grid rounding from this.
The argument 3 at `0x76b28` selects glyph text encoding through
`TextPaintImplSkia::setTextEncoding`, not a kerning/paint flag.

### Direction, break boundaries and tabs

`RichTextMeasure::measureParagraph`, `0x78a0c`, calls ICU
`ubidi_getBaseDirection` at `0x78b48`. It supplies base level `0xff`
to `ubidi_setPara` for RTL text, or neutral text with stored layout direction
1; otherwise it supplies `0xfe` (`0x78b4c`–`0x78c24`). These are ICU's
default RTL/LTR paragraph levels, rather than forcing every character into
one direction. Function-table binding is explicit: `0xee160` stores
`ubidi_getBaseDirection` at member 72; `0xee0f4` stores `ubidi_setPara`
at member 48, using the symbol strings at `0x2436a` and `0x2592d`.

The measure caller groups consecutive characters by resolved ICU bidi-level
parity and sends that parity to `AddStyleRun` (`0x78cc4`–`0x78d48`;
`ubidi_getLevelAt` binds to member 104 at `0xee1f0`). The Minikin style
run then selects direction value 4/5 at `0x96e28`–`0x96e74`; its bidi
iterator bypasses further paragraph analysis when that bit is present
(`0x9abdc`, `0x9ad8c`). Paragraph base direction and shaped-run direction
are separate inputs. Mixed RTL/LTR output still needs a captured reference.

Text `WordBreakerImplMinikin::Next`, `0x7b2f8`, delegates to `0x9e9b4`.
The ordinary branch filters ICU break candidates in `0x9ea00`–`0x9ec08`,
including surrogate decoding, soft hyphen, Myanmar virama, ZWJ and emoji
property checks. The URI/email branch recognizes an ASCII token containing
`@` or `://` (`0x9ec64`–`0x9ed14`) and uses different punctuation breaks
through `0x9ed2c`, rather than ordinary whitespace splitting.
`RichTextMeasure::adjustMeasureParams` uses this breaker to expand an
incremental measurement range (`0x79868`–`0x799ac`). That caller proves
the boundary policy for remeasurement; it does not by itself prove the same
URI policy controls every final line-wrap decision. The final layout uses
the separate ICU line-break path described below. Unsafe-cluster avoidance
in its emergency overflow branch remains unverified; Unicode scalar,
grapheme and shaped-cluster boundaries must not be treated as interchangeable.

`SpanRunFunctor` labels U+0020 as space and U+0009 as tab at
`0x77688`–`0x776c0`. For a tab, it can measure a single U+0020 with
SkPaint (`0x77774`), divide by 100 (`0x77794`), and initialize the tab
advance to four times that width (`0x7735c`, `0x777a0` / `0x777b4`).
This is a tab-specific probe within the shaped pipeline. The trace does
not establish position-dependent tab stops in the later line layout.

The Drawing converter proves the style bits used by measurement. In
`ObjectTextDrawing::convertTextSpanImpl`, `BoldSpan::IsBoldStyleEnabled`
(`0x90f88`) sets bit 1 (`0x90f94`); the italic getter (`0x90f08`) sets bit 2
(`0x90f14`). Underline sets bit 4 (`0x90f58`, `0x91088`) and strikethrough
sets bit 8 (`0x90f70`, `0x90f7c`). Consequently the native joinability mask
`0xc3` includes bold/italic and excludes underline/strikethrough. Decoration
changes alone need not split a measurement run; drawing still retains them.

## Ordinary underline and strikethrough

Text `RichTextDrawing::drawTextDecorations`, `0x666d4`, draws filled
rectangles after the glyph batch (`0x65f94`–`0x65fb4`,
`0x66518`–`0x66534`). It reads retained entry X/Y and the ending entry's
advance (`0x66758`, `0x66788`–`0x6678c`), forming horizontal endpoints from
the supplied draw offset plus those positions (`0x667c0`–`0x667f4`). Its
ordinary thickness is resolved span size / 18; underline top is baseline
+ size / 9 (`0x66814`–`0x66824`), and strikethrough top is baseline
- 2 * size / 7 (`0x66944`–`0x66958`). The native f32 constants are at
`0x2666c`, `0x26670` and `0x2667c`. These use resolved span size, not a
face's underline metric. Canvas rectangle dispatch reaches SkCanvas's
filled-rectangle call at `0x6a028`–`0x6a07c`.

Suggestion/spell-correction flags take another thickness branch
(`0x668ac`–`0x668e4`), and hypertext has separately gated forced underline
and color in `setTextPaint` (`0x63c88`–`0x63ccc`). The ordinary constants
do not establish those cases or RTL endpoint ordering. Drawing also joins
only fully equal spans with matching font IDs, direction and adjacency
(`inSameDraw`, `0x65998`); measurement's narrower join mask does not imply
that decoration changes can be discarded during paint.

## Final wrapping, alignment and object runs

Text `RichTextLayout::DoParagraphLayout`, `0x7278c`, opens an ICU iterator
with type 2 (`UBRK_LINE`) over the paragraph's UTF-16 slice at
`0x7297c`–`0x729a0`. The call's locale pointer is the native string at
`0x23e7a`; no explicit language-specific locale override is established.
The dynamic ICU table binds `ubrk_open` at `0xee49c` (name `0x2508f`) and
`ubrk_following` at `0xee52c` (name `0x23681`). The paragraph loop calls
`following` when it reaches the preceding break end, then stores the new
end in each 80-byte `MeasureData` entry at member 56
(`0x72d44`–`0x72d90`). Type 3 entries bypass that call. There is no traced
Minikin URI/email breaker call in this final paragraph loop.

The inspected Text layout path wraps already shaped paragraph data.
`RichTextDrawing::Measure` calls `RichTextMeasure::Measure` at `0x6460c`
before its `layout` call at `0x64650`; that layout calls
`RichTextLayout::DoLayout` at `0x64774`. The measurement loop dispatches
`measureParagraph` at `0x78804`, and the Minikin producer above shapes
the paragraph's joined spans. `RichTextLayout::SetMeasureData` retains the
existing vector pointer at `0x707ec`.

`DoParagraphLayout` reads each stored float advance from member 0 of an
80-byte `MeasureData` entry (`0x72cf4`–`0x72d00`), annotates bidi/break
fields, and passes the same paragraph entries into
`CalculateParagraphLayout` (`0x72e14`–`0x72e50`). The latter calls
`ParagraphLayout::DoLayTextOut` at `0x73ee0`. `GetBlockInfo` adds stored
advances when testing/committing candidates (`0x6ada4`, `0x6ae04`–
`0x6ae18`, `0x6af08`–`0x6af0c`); `SetLayout` places ordinary entries by
advancing X with member 0 (`0x6b6e8`, `0x6b73c`). These inspected line
selection and placement functions do not call the Minikin measurement
producer again when a line boundary is chosen.

The corresponding Rust contract is to shape the paragraph's joined runs
once, retain source/cluster mappings and advances, then select and position
lines from those advances. A prefix-sum implementation can avoid repeatedly
shaping progressively shorter portions of a long unbroken word. Native
width comparisons here use float additions and a strict `>` test; no
additional per-line width rounding or special kerning correction is proved.
Native emergency splitting is still not proven grapheme/cluster-safe;
keeping graphemes intact is an SDK policy until captured boundary cases
establish native behavior. The retained advances also do not prove that
independently reshaping each exported SVG line reproduces the same glyph
positions; measurement and paint must eventually consume the same layout.
The native ordinary drawing path obtains cached `GlyphInfo` at `0x65e04`
and builds each glyph point by adding its retained X/Y offset to the
`MeasureData` run point and supplied draw offset (`0x660c4`–`0x660e8`).
`drawGlyphs` forwards those positions to the canvas at `0x66a94`–`0x66ac8`.
That is stronger than matching line advance alone: faithful Rust placement
must eventually retain and paint selected glyphs, face choices and cluster
positions from the same measurement result, including clusters spanning
several UTF-16 entries.

`ParagraphLayout::GetBlockInfo`, `0x6ab9c`, accumulates measured advances
and tests candidate width against the current available rectangle with a
strict `>` comparison (`0x6ae80`–`0x6ae84`): an exactly fitting candidate
fits. A committed break is tracked when the current paragraph-relative
index equals the stored break end minus one (`0x6af18`–`0x6af38`). Spaces
and tabs also commit a break; their space counts increase by 1 and 4
respectively (`0x6af4c`–`0x6af98`). On overflow it uses the last committed
break when its index is at least 1 (`0x6b014`–`0x6b028`), otherwise the
preceding index (`0x6b03c`–`0x6b06c`). This is measured greedy wrapping,
not a character-count estimate.

An oversized first ordinary entry can still be included. The helper
`isCharacterOverflowWidth`, `0x6c5b0`, requires the candidate to be the
block start, or start plus one when the start is type 4, and compares its
advance plus the partial width with the whole layout interval
(`0x6c5fc`–`0x6c610`). Object types 1/2 are excluded from that helper and
have separate handling. The caller includes the overflowing entry at
`0x6b0dc`–`0x6b0ec`. This establishes progress for oversized entries; it
does not prove a grapheme-safe emergency break rule.

Horizontal alignment uses the available **block** width, which can differ
from the complete text-box width when obstacles or indents are present.
`GetBlockOffSetXByAlign`, `0x6c61c`, computes available width minus measured
block width and returns 0 when the remainder is nonpositive. Align 1 uses
the remainder; align 2 uses half; other values use 0. `SetLayout` applies
that offset at `0x6b5d8`–`0x6b5ec`. `GetLineAlign`, `0x6aac0`, resolves
internal align 4 through layout direction member 212; `m_CopyLayoutData`
stores the supplied direction boolean there at `0x6aa8c`. Its caller
compares stored `RichTextImpl` direction with 1 at `0x73eb0`–`0x73ec4`.
Internal align 4 is not another public saved alignment enum.

Justification (align 3) calls `m_GetExtraSpaceWidth`, `0x6cc64`, from
`SetLayout` at `0x6b628`–`0x6b650`. Its numerator is available block width
minus measured block width, divided by the counted spaces. Layout-option
bit 0 can suppress the paragraph's final line; bit 1 trims eligible leading
and trailing entries from the distribution range (`0x6cc84`–`0x6cd84`).
The option producers/defaults are not verified here. Distribution adds one
share to a space and four shares to a tab (`0x6b838`–`0x6b89c`). A generic
SVG `text-anchor` or spacing every glyph does not express this algorithm.

Objects participate in the same measured paragraph stream. Text
`SpanRunFunctor::measureObjectSpan`, `0x779d0`, writes an entry at the
supplied UTF-16 index, sets entry type 5 (`0x77ac4`–`0x77acc`) and object
type 1/2 (`0x77a68`, `0x77aa4`, `0x77ac0`). The ordinary branch builds
`[0, -height, width, 0]` from stored object width/height, span members
48/52 (`0x77a80`–`0x77a90`). The margin-enabled branch expands the rectangle
and dimensions using `RichTextMeasure` object margins at members 40–52
(`0x77a40`–`0x77a78`). Stored object top/bottom margins, span members
56/60, are copied to entry members 72/76 (`0x77ac8`–`0x77ad0`).
`GetBlockInfo` scans text and object entries together and checks object
height/changes (`0x6ad34`–`0x6ada0`, `0x6ae34`–`0x6ae7c`); a text-bearing
paragraph is not replaced by an independent stack of embedded objects.
The exact replacement-character producer and table-cell padding adapter
remain unverified in this trace.

## Body-flow pagination boundaries

Bodytext `BodyTextDocument::convertPageList`, `0xa9384`, constructs
`IBodyTextDocument::Page` records with cumulative integer Y at member 0,
a local rectangle at member 4, and the original `WPage` pointer at member
24. It obtains the actual page width/height and stores `[0,0,width,height]`
at `0xa94bc`–`0xa94f4`, then adds that rectangle's height to the next Y
at `0xa9598`–`0xa95a4`. These are actual page sizes, not font density or
the default-page size used to resolve density.

`isDownLine`, `0xb7d90`, uses `TextLayout::GetLineBgBound` and returns true
when `line_top >= page_y`, or when `line_top < page_y` and
`line_bottom > page_y` (`0xb7e54`–`0xb7e74`). A line ending exactly at the
page start is excluded. `isUpLine`, `0xb7e88`, uses `GetLineTop` (vtable
slot 248) and returns `line_top < page_y + page_height`
(`0xb7f60`–`0xb7f78`); equality with the page end is excluded.
These tests admit a line crossing a page boundary to both page ranges.
For line zero with nonpositive top, both helpers use
`GetFirstLineRectInEmpty(GetLayoutWidth())` instead; empty text uses
`TextCursorUtil::GetDefaultCursorRect` (`0xb7e14`–`0xb7e48`,
`0xb7f1c`–`0xb7f58`). This special case must remain separate from ordinary
line geometry.

`UpdateTextRangeOnEachPage`, `0xb77a4`, scans until these boundary predicates
fail and stores `[first_line, line_count]` at `0xb7978`. Its text range starts
at `GetLineStartIndex(first_line)` (`0xb7980`–`0xb7990`) and counts through
`GetLineEndIndex(last_line)` inclusively (`0xb79b8`–`0xb79dc`). It can seed
the next page scan with the preceding page's last line
(`0xb7868`–`0xb7878`, `0xb79e8`–`0xb7a34`), preserving overlap instead of
forcing disjoint string chunks. This proves page indexing of an already
measured layout; moving/resizing objects or repaginating the document is
a separate operation and still needs captured reference cases.

Text `TextPaintImplSkia::getFontMetrics`, `0x7c16c`, calls
`SkPaint::getFontMetrics` at `0x7c1ac`; `getFontSpacing`, `0x7c290`, delegates
to the same backend. The library includes Minikin font-family/collection
implementations, fallback registration (`SystemFonts::RegisterFallback`,
`0x88d98`) and language fallback registration (`FontListParser::
RegisterSystemFallbackForLanguage`, `0x804ac`). `RichTextLayout::
SetLineBreakIndex`, `0x73c3c`, uses an ICU break handle. This supports measured
font advances, shaping/fallback and language-aware breaking; it does not
establish a particular fallback font order on the Samsung reference device.

Drawing `ObjectDrawing::drawTextContent`, `0x80818`, constructs an
`ObjectTextDrawing` at `0x80ac8` for positioned object text. Bodytext
`BodyTextLayout::onLayoutText`, `0xb1928`, sets the shared `ObjectTextLayout`
width at `0xb1a2c`, updates obstacles, updates text and invokes measurement
at `0xb1a40`–`0xb1a68`. Its `createObjectLayout`, `0xb09d0`, obtains the
embedded object from an `ObjectSpan` at `0xb0a10`, and creates/reuses a
separate object layout. `newObjectLayout`, `0xb30a0`, calls
`ObjectLayoutFactory::CreateObjectLayout` at `0xb3130`.

Bodytext `BodyTextPageIndexer::UpdateTextRangeOnEachPage`, `0xb77a4`,
walks measured lines through `isDownLine` / `isUpLine`, writes page line
ranges at `0xb7978`, and derives page text ranges at `0xb7980`–`0xb79f4`.
Pagination depends on measured line positions and document geometry, not
just splitting the string into equal chunks. The SDK can retain saved page
text sections while developing one Rust layout engine, but recomputed page
breaks and obstacle/inline-object behavior still require reference cases.

## Current Rust implementation and remaining gaps

The shared Rust body-flow and placed-text pipeline now measures shaped runs with actual
font faces and wraps using Unicode line-break opportunities. The
`render/text/{measurement,breaks,wrapping}.rs` modules replace estimated
ASCII/script widths and the URL-punctuation heuristic. Measurement accounts
for bidi levels and script/extensions, uses native span-joining inputs
(size, source color, family, bold and italic), and disables `liga`/`clig`
for the inspected Latin policy. Paragraph runs are measured once and line
widths use their retained advances; graphemes, spaces and source ranges
are retained. Missing usable metrics report `MeasurementFailure`
instead of estimating widths.

For covered static-font LTR Latin clusters, explicit typed SVG positions
now carry the retained glyph geometry through Chromium and usvg/PDF. This
also reproduces the native Latin ligature policy without relying on usvg
support for SVG font-feature properties. Multi-scalar combining clusters
stay together, and independently reproduced glyph IDs/offsets are checked
before positioning. Variable-font optical size, bidi L1 resets,
paired script punctuation, native emergency breaking and the reference
device's ICU version/locale remain unverified. Placed and shape text now use
the shared measured wrapper, scaled margins, paragraph spacing, integer
outer-width ceiling, native indent conversion and measured alignment.
Placed vertical gravity, clipping, table/code layout, embedded-object
composition and recomputed pagination still need
the shared measured layout engine and captured cases. The current body
baseline follows the capture above; it is not a universal native baseline
formula.

The measured paint path now supplies retained X positions and Y offsets
for clusters that can be expressed by the current typed SVG text adapter,
and uses filled rectangles for ordinary underline/strikethrough. Unsupported
glyph positioning, clusters crossing a paint-style boundary, or nonfinite
positions produce `UnsupportedGlyphPositioning` diagnostics and preserve
the text through the existing SVG text fallback. Those fallback bounds and
complex cluster placement are not native glyph-geometry parity; the diagnostic
must remain visible until measurement and paint share a complete glyph path.
Missing glyph coverage already reports `MissingGlyphs`; it does not also
emit the positioning diagnostic. The default placed baseline is preserved
pending a captured standalone reference; explicit spacing currently changes
line advance, not that baseline adapter.

The public-API release probe used a 640-unit page, 17-unit text, density 1
and 544 units of usable width. Repeated Latin, long URLs and spaced prose
at 5k, 20k and 50k characters retained every line range from the preceding
wrapper. At 50k characters, paragraph measurement reuse reduced the Latin
case from 14.9 seconds to 81 ms and the URL case from 15.0 seconds to
94 ms on the same machine. These are observed timings, not portable limits.
Retaining paragraph kerning intentionally changes a separate `AV` boundary
case; its independent pinned-font regression records that behavior.
With retained glyph geometry and typed paint positions enabled, the same
release probe measured 50k-character Latin at 62.5 ms, URLs at 61.8 ms and
prose at 43.3 ms. Source preservation and empty diagnostics passed in all
nine cases; these timings are observations from this machine.

The retained scaling probe checks complete source text, contiguous emitted
lines and diagnostics without asserting wall-clock thresholds:

```sh
cargo test -p sdocx --all-features --test text_layout_scaling --release --offline -- --ignored --nocapture
```

## Evidence required before claiming visual parity

- A Samsung-exported standalone text box and reference PDF covering margins,
  vertical gravity, rotation, wrapping and all text-area modes. Existing
  standalone cases are synthetic parser/renderer tests.
- Mixed font sizes/families and overlapping nonempty styles, including emoji,
  combining marks, RTL text and CJK fallback; verify actual selected fonts,
  advances, baseline metrics and break positions.
- Pixel/percentage line spacing, before/after spacing, indent and justified
  paragraphs in both positioned text and body flow. Resolve the context scale
  and default spacing of each captured export route.
- Inline images/tables/code, obstacle wrapping, explicit page breaks and
  multipage text. Preserve raw UTF-16 and paragraph ranges through measurement,
  pagination and export; ranges are not interchangeable with glyph clusters.

These rules can become Rust contract tests backed by the addresses above.
Captured Samsung output remains reference evidence; it need not become a
second authored layout implementation.
