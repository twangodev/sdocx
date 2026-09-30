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
standalone cases and their callers before adding those semantics. The ordinary
baseline helper is shared with placed text as traced below; standalone visual
parity still needs a captured reference.

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

Rust now resolves glyph coverage before measurement and retains the selected face
through vector painting. Coverage preserves shaping normalization and default
ignorables. Static LTR Latin runs split only at whole graphemes when coverage
requires another face, then join adjacent selections of the same face. Other
scripts retain their contextual run while trying a covering face. Selection uses
only the caller's database, trying configured sans and deterministic family/style
candidates. Native Minikin fallback registration supports fallback as a layout
input; the concrete device family ordering is still unavailable.

Selected fallback weight/style are explicit typed SVG attributes. A candidate
must also be selectable by those exact properties in the original database;
hidden duplicate face IDs cannot be represented reliably through SVG family
selection. Unsupported positioning remains local, preserving neighboring Latin
glyphs and native advances. Bidirectional formatting controls keep their bounded
fallback group intact. Tabs retain their source while positioning following text
using the native four-space advance. Public regressions pin glyph IDs, selected
face IDs, positions, decorations and selectable vector PDF source.

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

The calculation and stores are at `0x6493c`–`0x64958`. The ordinary placed
view supplies `ceil(outer_height)`, not an inset or ceiled content height:
Widget `ScrollEditTextView::Layout` rounds `View::GetBounds().Height()` at
`0xbc9b8`, calls `TextLayout::Layout` at `0xbca04`, and Text forwards this
integer unchanged to gravity at `0x8ac94`–`0x8ac98`. Nonempty content uses
the raw measured float at Drawing member 308 (`0x648f4`).

The height producer is `RichTextLayout::DoLayout`. It initializes its cursor
from the top margin (`0x71628`–`0x71630`), then accumulates paragraph top
plus measured height (`0x71b14`–`0x71b28`). A paragraph's enabled before
spacing is included at `0x6a6c8`–`0x6a6e4` / `0x6a740`; enabled after spacing
is deferred until the following paragraph (`0x71a98`–`0x71abc`). Thus the
final paragraph's after spacing does not extend the final measured height.
The final line also advances the full cursor: `SetLayout` returns it at
`0x6badc`, and `DoLayTextOut` calls it at `0x6a9bc`, then subtracts the
paragraph's original top at `0x6a9c0`. Ordinary default text consequently
includes the final `1.35 * max_font_size` advance, rather than ending at the
last baseline or a font's ink descent.

The final height adds `max(bottom_margin, final_line_bottom_margin)` at
`0x72118`–`0x72148`. Ordinary text has zero line bottom margin: the measure
initializer zeroes members 72/76 (`0x791a4`–`0x791a8` -> `0x65944`), the
ordinary producer retains them, and their maxima become the line margin
at `0x6d7f0`–`0x6d7fc` / `0x6b55c`–`0x6b564`. For ordinary text without
objects, bullets or obstacles, height therefore consists of top margin,
enabled paragraph before/after spacing except final after, every full line
advance, and `max(bottom_margin, 0)`. Negative top margins remain inputs.

Entirely empty text takes a separate gravity branch: `RichText::GetTextSize(0)`
plus raw top/bottom margins (`0x6490c`–`0x64934`), without the 1.35 multiplier,
paragraph spacing or the nonempty bottom-margin clamp. Empty paragraphs
inside nonempty text still use layout. Native spacing-enable flags, list
state, object margins and obstacle adjustments are additional inputs; this
bounded height formula does not prove all their SDK behavior. Wrapping and
measurement must precede gravity positioning. The ordinary placed caller uses
the shared baseline path traced below.

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

Stored before spacing is not generally excluded from the first paragraph.
Drawing writes its scaled value to `EditTextParagraph` member 116 at
`0x92e60`–`0x92e6c`; Widget `BulletManager::updateRichTextParagraph` copies
before/after to `RichTextParagraph` members 80/84 at `0xa5b84`–`0xa5b88`.
Fresh Text `RichTextLayout::DoLayout` explicitly enables the first paragraph's
before-spacing byte at `0x71660`–`0x71670`. The ordinary nonbullet branch
retains or enables it at `0x71864`–`0x718c4`. The subsequent suppression at
`0x718e8`–`0x71958` requires a measured object entry with positive top and
bottom margins; it does not establish suppression for ordinary table-cell
text. Removing the captured cell's 12-unit before spacing merely to fit a
baseline would contradict the inspected ordinary producer.

After spacing is enabled by default in `RichTextLayout::DoLayout`. It is
suppressed when both the current and next native bullet enums are in 1–9
(`0x719c4`–`0x719ec`), or when the current paragraph's final measured entry
is an object with both vertical margins positive (`0x71a38`–`0x71a94`).
Before laying the next paragraph, enabled after spacing from the previous
paragraph is added to the vertical cursor (`0x71a98`–`0x71abc`). In the
captured body sequence, ordinary paragraph 59 has after spacing 4 logical
units, paragraph 60 is empty and paragraph 61 contains the table. Neither
suppression applies to paragraph 59: its 12 scaled units remain when entering
paragraph 60; the later table cannot suppress them across that empty paragraph.

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

The ordinary placed route uses this same helper: Drawing `0x80c80` calls
`ScrollEditTextView::Measure`; Widget `0xbc7f0` calls its layout, which forwards
to `TextLayout::Layout` at `0xbca04`. Text forwards through
`RichTextDrawing::layout` (`0x8ac88`), `RichTextLayout::DoLayout` (`0x64774`),
paragraph layout (`0x71aec`, `0x72e50`, `0x73ee0`) and `SetLayout` to
`GetBaseline` (`0x6b510`). Its ordinary branch has no placed/body switch.
Rust consequently uses line advance minus `0.35 * max_font_size` across
ordinary placed, shape, body, table and code text; object-margin lines retain
their separate native branch.

The body origin is the scaled component top margin, without adding the stored
flow-page padding: Widget `updateBound`, `0xd71b0`–`0xd7220`, supplies these
margins directly, and Text starts its cursor from the top margin at
`0x71628`–`0x71630`. The captured first heading has top margin 30, enabled
before spacing 12, font size 45 and multiplier 1.6. Its baseline is therefore
`30 + 12 + 72 - 15.75 = 98.25`. The reference PDF's actual 848-point viewport
gives that value; the retained logical 848.333333-point canvas convention in
`conformance/text-metrics.json` gives 98.85. Their 0.6-unit difference is the
page-height conversion, not a layout offset. The SDK no longer adds 24 units
of page padding or subtracts 4 logical units from continuation top margins.

`RichTextDrawing::getDrawnTextRun` reads `MeasureData` members 8/12 as
the run point (`0x66df0`) and adds the supplied export offset and stored
gravity offset (`0x66d0c`–`0x66d14`, `0x672b8`–`0x672c4`).
`appendTextBlock` stores that point unchanged at `0x680e8`.
`UpdateGravityOffsetY` stores zero for gravity 0 (`0x648c4`–`0x648d0`,
`0x6495c`). Composer measures the full body before indexing its pages
(`0x3a2654`, `0x3a2680`); a continued object paragraph must preserve its
source separator metric rather than become a source-first paragraph.

Capture has a separate producer. `BodyTextCapture::getStartMeasureIndex`
(`0xca9f0`–`0xcaadc`, Bodytext) starts from the requested page. For later
list-mode pages, a non-LF start inside a paragraph with any native type-5
record backs up to the saved section containing that paragraph's start.
`GetStartPageGroup` (`0xcef14`–`0xcef80`) then backs up while the preceding
section's end strictly exceeds the current start; touching ranges do not
join. `GetDrawnTextData` constructs that inclusive page range before measuring
a fresh body layout (`0xca17c`–`0xca2d0`). It selects the requested member's
viewport afterward (`0xca244`–`0xca478`). Full-body indexing therefore does
not establish unconditional full-body reflow as the native export contract.

`GetTextSectionByPageRange` (`0xcda18`–`0xcdb48`) ignores empty leading and
trailing sections and uses the last nonempty section's end, rather than the
maximum end across the group. Copying the resulting text separately removes
a leading LF at a positive source start (`TextViewUtil::CopyText`, Widget,
`0xdf48c`–`0xdf4bc`). Capture's reported source offset has a stricter guard:
it skips LF only when `start >= 1 && start + 1 < full UTF-16 length`
(`0xca198`–`0xca1d8`). Layout metadata retains the raw saved range, copied
measurement range and reported offset separately, validating all UTF-16
boundaries. A source-zero LF is retained when no earlier nonempty section
exists; otherwise the native copy flag removes it (`0xce29c`–`0xce2e4`).

The locked fixture's page-index-4 section `[1428,1670)` overlaps page index
3's `[1236,1430)`, so its native capture window covers pages 3–4. Measuring
starts at 1237 after copying removes the group's leading LF. Measuring only
the display slice loses the LF before the code paragraph and its inherited
45-unit font metric. Restoring that metric predicts 15.751 units of cursor
advance; it does not justify fitting the remaining approximately 0.75-unit
saved-versus-recomputed-height difference. Native code feedback still clears
and remeasures its float rectangle (`0xb0e14`–`0xb0e50`).

Fresh comparison of the locked five-page fixture places matched ordinary
body and heading baselines on the first four pages within 0.0001 SVG units
of the actual PDF viewport. Code origins and measured heights differ only
by float roundoff. Prepared numbered markers match the two locked marker
baselines within 0.000045 units. Remaining observed differences are table-cell
X +1 and baseline Y about +1.751, numbered-item text X about -0.04393, and
ordinary text after the continued code block Y about +0.75007, improved from
the former -15.001-unit slice error. Visible continued code matches within
0.0011 units. Four raw code lines are fully outside the native viewport,
independently verified from page/Form bounds, per-line clips and embedded
glyph outlines; their original coordinates remain recorded, while the test
requires no corresponding SVG text. These are
exclusions from the
passing reference subset, not evidence of complete flow or pagination parity.

## List marker geometry

Widget `ObjectTextLayout::initBulletPoint` reads constants 123/124 at
`0xd9620`–`0xd9650`: button width 20 and trailing margin 6, multiplied by
document density and local layout scale. Content records are `0x8370` /
`0x8388`, both unit kind 3 and rounding kind 3. Widget point types 4/5/6/7
receive the same values (`0xd96dc`–`0xd9780`), and `measurePointBullet`
adds the two widths at `0xd1c6c` / `0xd1c88`–`0xd1c8c`. The export local
scale is 1, so circles and squares reserve 26/52/78 units at densities
1/2/3, independently of the text font or marker artwork.

The Rust flow adapter now uses that shared point-marker reservation.
The captured paragraph 53, UTF-16 1317, starts its ordinary text at X126;
the previous solid-circle reservation placed it at X96. The nested circle
starts at X174 and retains that position. Independent preview/replay tests
cover both circles, both squares, three densities and odd/even indent levels.
Raw Arrow/Diamond values also map to native point type 4 at
`0xd92c4` and use the same solid-circle vector in Rust. Native default nesting
is SolidCircle, WhiteCircle, BlackSquare, WhiteSquare (`0x63830`,
`0xdc634`–`0xdc664`), while explicit type lists
override it (`0xd9020`–`0xd9124`). The SDK's odd/even circle substitution
is not a complete implementation of those lists; malformed signed indentation
remains unverified.

Point artwork is vector geometry. Native resource IDs 38–41 select the
4 × 4 circle/square XML paths (`0xd96fc`–`0xd9798`); Rust emits typed SVG
circles and rectangles with the same open-marker border proportions. Image
size uses constants 119–122 and descending font thresholds
(`0xd98fc`, `0xd9984`, `0xd2354`), then centers bounds using
`ceil(image_size / 2)` (`0xb310c`–`0xb3138`). The rendering display selects
mobile/tablet/UWP via `IsTablet` and `IsUWP` (`0xd9878`); this is device
context, not document page mode. Rust exposes `PointMarkerTarget`, defaulting
to mobile, and shares it across preview and export.

With zero explicit pixel spacing, point center Y is the post-line cursor
minus `1.35 * line_base_height / 2` (`0x6be9c`–`0x6becc`). Nonzero pixel
spacing instead uses baseline minus the global default face's cap-height
ratio times line base height / 2. `RichTextDrawing::layout` obtains that ratio
at `0x6474c`–`0x64770`; `TextUtil::GetCapHeightRatioFromBaseLine`, `0x8e198`,
selects the default typeface and divides cap height by text size. This is
independent of the paragraph's font family. The system-font flag is RichText
member 115 (`0x62934`); with it disabled, `FontListParser::GetDefaultFontFamily`
prefers `sec` on SDK 31+ and falls back to `sans-serif` (`0x7d5dc`–`0x7d720`).
With it enabled, the family is empty for system default selection (`0x84fec`).
Rust's caller font database supplies its sans default, so explicit-pixel
point placement is not claimed to match every
Samsung device font configuration.

Numbered markers use a separately measured nested text layout. The prepared
font size comes from `updateParagraphNumberSize` (`0xd852c`): normally the
paragraph's first content span, skipping its retained line-feed prefix;
empty and short paragraphs can use `GetFontSize(start + length)`. The size
is stored at paragraph offset 68 and supplied to `updateTextLayout`
(`0xd1998`–`0xd19a4`). Its fresh rich spans inherit no paragraph family,
bold or italic; the nested layout uses default-face selection and the
system-font flag (`0xd1dc4`–`0xd1e4c`). Initialization's font constant 116=15
is therefore not a universal marker size. Gap constants 117=9 / 118=6
scale with document density and local text scale (`0xd9810`–`0xd9864`).
The fresh-layout branch uses gap 9 for displayed values below 10 and 6
otherwise (`0xd19c8`); the retained-layout branch uses gap 9 (`0xd17d8`).
Reservation is fractional measured line width plus left/right margins and
the gap (`0xd19ec`–`0xd1aac`). The child starts at X=0 in LTR and X=gap in
RTL (`0xd1adc`–`0xd1b04`). Native supplies multiplier 1.3 to the nested
layout (`0xd1e98`–`0xd1eb0`), while ordinary zero-pixel-spacing centering
uses 1.35. This explains the captured -1.125 baseline difference at size
45 only when marker size and line base height agree; it is not a universal
offset. Rust now prepares an owned marker source and shared `TextLayout` from
the first content span's resolved size. It measures glyph width, constrains the
child to its upward-rounded width, and reserves its fractional advance plus
the scaled 9/6 gap using the fresh-layout policy. The marker uses the default
sans face without inheriting paragraph family, bold or italic. The former
fixed width 64 and +1.125 baseline error are corrected. Bundled Roboto's
`1.` advance is 37.1118164 versus 37.1250028 in the native PDF; ordered-item
body X still differs by about -0.04393. Native advance quantization and
device-default font selection remain unresolved.

Raw Digit and CircledDigit both map to native decimal type 3
(`0xd9318`–`0xd9328`); this route does not draw a circle. Native decimal,
upper/lower alphabetic and lowercase Roman strings all receive a trailing
period (`0xd18d8`–`0xd1900`). `ConvertNumberToBulletString` (`0xdbc08`)
uses spreadsheet-style alphabetic numbering and greedy Roman subtraction;
nonpositive values produce an empty string before the period is appended.
Default ordered types are decimal/uppercase/lowercase/Roman (`0xdc5f0`–
`0xdc628`); default point types are solid circle/white circle/black square/
white square (`0xdc634`–`0xdc664`). Explicit stored type lists override
these defaults (`0xd9020`–`0xd9118`). Selection uses indent level modulo
the type-list length (`0xd6fcc`–`0xd7004`), not the SDK's two-level point
alternation.

Rust applies wrapping 32-bit `number + initial_number - 1` before formatting,
including the native period-only result for nonpositive alphabetic/Roman
values. Roman strings have a 4096-byte ASCII resource limit with a measurement
diagnostic; this is an SDK allocation policy, not a native number clamp.

Checkboxes use resource IDs 42/43, `spen_ic_text_check_off/on`, with
24-unit VectorDrawable assets (`0xd95d4`–`0xd95dc`, `SpenResources.java`). The
rounded box runs from 4.5 to 19.5 with stroke width 1.5; the checked asset
adds a check subpath to the same compound path. `GetCheckboxImageSize` clamps
the f32 calculation `font_size * 68 / 100` between target-specific
5.5/5.5/4 and maximum 20, scaled by
document density (`0xcfa80`–`0xcfab0`). `setCheckboxView` expands this to
the asset viewport with `ceil(size * 24 / 15)` and even-rounds its bounds
(`0xb2c10`–`0xb2c88`), tints it with paragraph color and applies alpha 0.4
when checked, 1 otherwise (`0xb2c90`–`0xb2cb4`). Rust emits one typed compound
SVG path, applies checked opacity once, preserves that f32 operation order,
and reserves the 20-unit button plus 6-unit gap scaled by density. Its asset
center is one native physical unit above the shared marker container center.
It uses neither a checkbox font glyph nor an image. RTL number substitution
and device-specific default-face agreement remain unverified.

Point, numbered and checkbox markers share preparation and positioning in
`layout_text`: reservation precedes wrapping, the first line retains its owned
marker and absolute center, and gravity moves text and marker together. Scoped
renderers share fonts and diagnostic registries; painting registers retained
faces so prepared layouts can still embed their fonts. `FontSizeUnits`
distinguishes logical and resolved sizes, preventing repeated minimum, delta
or density conversion, including already-resolved sizes below 1. Body flow
uses the same layout loop, including marker preparation, spacing and object
feedback. Validated capture windows preserve source context across overlapping
saved sections and paint only their requested physical viewport. The legacy
saved-slice continuation policy remains for unavailable or edited capture
contexts. Complete point-type cycles and tiny-font serialization precision
remain unfinished.

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
`[0, -height, width, 0]` from converted object width/height, span members
48/52 (`0x77a80`–`0x77a90`). The margin-enabled branch expands the rectangle
and dimensions using `RichTextMeasure` object margins at members 40–52
(`0x77a40`–`0x77a78`). Stored object top/bottom margins, span members
56/60, are copied to entry members 72/76 (`0x77ac8`–`0x77ad0`).
`GetBlockInfo` scans text and object entries together and checks object
height/changes (`0x6ad34`–`0x6ada0`, `0x6ae34`–`0x6ae7c`); a text-bearing
paragraph is not replaced by an independent stack of embedded objects.
The exact replacement-character producer remains unverified in this trace.
Table text uses its content margins; absent margins retain zero constructor
defaults. The table's flexible fields 0/1 are global minimum column/row sizes,
not a cell-padding adapter (`0x3d4364`, `0x3d36bc`).

Widget `ObjectTextLayout::convertObjectSpan`, `0xd53e4`, iterates stored
spans and writes object geometry to the single entry at the supplied
UTF-16 anchor (`0xd54d8`–`0xd5540`). Repeated anchors overwrite that entry
in list order; duplicate painting behavior is not established. Only layout
option 1 becomes inline (`0xd5558`–`0xd55b4`). Options 2/3 receive symmetric
vertical margins from constants 344/345 multiplied by layout scale
(`0xd5570`–`0xd559c`); the current renderer uses these symmetric margins.
For a block entry, Text `measureObjectSpan` sets its advance to the entire
available layout width, subtracting left/right margins (`0x77a94`–`0x77aa8`).
An inline entry instead uses object width plus horizontal measurement margins.
Constants 344/345 are records at Content `0x9828` / `0x9840`: logical
10/20, unit kind 3, rounding kind 3, with zero variant overrides. Widget
initializes that constant provider with manager document pixel at
`0xd30d4`–`0xd3114`; conversion multiplies the resulting value by the
local layout scale. The Widget constructor initializes member 600 to 1.0
(`0xd3018`–`0xd301c`); `SetTextScale` writes a changed value at `0xd9528`.
The fresh BodyText capture layout is constructed at `0xca2ac`, receives its
document at `0xca2b8` and measures at `0xca2d0` without a scale setter.
That export route therefore uses local scale 1, giving margins 30/60 at
document density 3. Neither margin includes the font-size delta. Unrecognized
layout options follow the zero-margin block branch; only constraints 1/2
set `IsObjectOverPages` (`0xd55bc`–`0xd55d8`).

Object entries contribute both their height and their resolved source font
to the line metrics. `SpanRunFunctor` stores the font in member 60 at
`0x77674` before calling `measureObjectSpan` at `0x77844`; that callee preserves
the member. The earlier zero-font conclusion inspected only the callee and
missed this producer. Widget's default/font-span conversion and final span
copying do not clear the object's font.
`GetBlockInfo` takes maxima of entry height and text metric independently
(`0x6aeb4`–`0x6af44`). Both object types set block flag 41
(`0x6ad3c`–`0x6ad54`), so `GetBaseline` adds `0.001f` to both baseline and
vertical cursor (`0x6cc0c`–`0x6cc28`). With zero measurement margins,
`F=20`, `H=100`, default multiplier 1.35 and initial cursor 0 yield cursor
107.001 and baseline 100.001, including an object-only initial paragraph.
Rust now retains the font maximum separately from ordinary text height and
the object's current measured height. An object font 900 with height 100 and
neighboring text font 20 therefore yields base height 100, baseline 100.001,
and cursor 415.001 under default spacing; its font does not enlarge its
measured height. Object-only font 20/height 100 yields baseline/cursor
105.001/112.001 with multiplier 1.6, or 113.001/120.001 with pixel spacing 20.
`RichTextMeasure::measureParagraph`
gives a leading separator/type-4 entry the following span's font size in
both height and text metric (`0x78a64`–`0x78acc`). `GetBlockInfo` retains
that metric maximum (`0x6aef8`–`0x6af00`, `0x6b008`), including when its
second-entry special branch replaces the height/margins (`0x6aea0`–`0x6aeb0`).
That branch applies to any second content entry after a leading type-4
separator. Rust preserves its font input without retaining the replaced
separator height; separator-only empty paragraphs keep the ordinary height.
With a separator font 45 and object height 100, the zero-margin object
line advances 115.751 and has baseline 100.001. A paragraph adapter that
removes separators must preserve this first-line metric input explicitly.
The prefix condition is paragraph ordinal at least 1. Widget
`updateParagraphs` stores the ordinal in record member 0 (`0xd6fc8`–`0xd6fd0`);
`textToParagraphs` starts each subsequent record at the separator's own
index (`0xd67a0`–`0xd67ac`, `0xd684c`), before advancing one UTF-16 unit
(`0xd687c`). In a content range that excludes its preceding separator,
this applies only to the first line when its full-source preceding character
is CR/LF. A source-first object-only paragraph has no inherited prefix font;
a page slice must retain full-source context rather than infer it from its
local paragraph number.

A block with both stored vertical margins positive takes the separate
`IsLineOfObjectSpanWithMargin` branch (`0x6c7ec`, `0x6cb90`–`0x6cb9c`):
top margin is added before baseline, then base height is used without
percentage leading or the text baseline subtraction. `H=100`, `F=20`
and top/bottom margins 30 give baseline/cursor 130.001; final measured
height includes the deferred bottom margin and is 160.001. Without a
padding obstacle, adjoining vertical margins collapse to the maximum of
previous bottom and current top (`0x6c8e8`–`0x6c914`,
`0x6c9a0`–`0x6c9a8`). They are not added independently at each line edge.
Obstacle intersection uses `GetRectFromBlock`, `0x6c71c`: its candidate
contains the base height or full line advance plus adjusted top margin
(`0x6c754`–`0x6c7b8`), before the later object `0.001f` increment. Final
height adds the maximum of text-box bottom margin and deferred object
bottom margin (`0x72134`–`0x72144`), rather than their sum.

The object dimension producer calls virtual slot 160 at Widget
`0xd54c8`–`0xd54d4`. For images, Model's vptr `0x497b58` and relocation
`0x497bf8` identify this as `ObjectShape::GetDrawnRect`, `0x397d0c`,
rather than raw `GetRect` at slot 168. Its implementation, `0x3a63e8`,
can use path/stroke bounds and rotated bounds (`0x3a652c`, `0x3a6670`).
Raw saved image rectangle dimensions therefore do not establish measurement
parity for rotated images or drawing effects; crop-specific bounds still
require an image-path trace.

Tables also distinguish raw and drawn bounds. Model `ObjectTable::GetDrawnRect`,
`0x3d48c4`, delegates to `ObjectTableImpl::GetDrawnRect`, `0x3c6cac`.
The implementation expands raw left/top/right/bottom by the corresponding
drawn-border half-widths (`0x3c6cf4`–`0x3c6d30`). The edge getters
(`0x3c6d5c`, `0x3c6dd8`, `0x3c6e54`, `0x3c6ee0`) take the maximum of
the outer edge and matching boundary-cell edges. Cells are selected from
the stored row/grid vectors (`0x3c7510`, `0x3c75a8`); an explicit cell
border overrides the table default. Missing outer/default-cell records use
the native unit-width constructors (`0x3c5d20`, `0x3dc670`, `0x3dc280`).
These drawn-width getters do not gate on color or alpha. The captured
table's raw 984×216 rectangle and unit borders therefore yield a 985×217
measurement input. Rust now uses these drawn bounds when reserving an object.

Over-pages drawing regenerates cell frames from column widths and row heights
rather than reusing saved cell rectangles. `TableLayout::getHalfBorderWidth`,
`0xb325c`, finds the maximum positive drawable border-path width and halves it
at `0xb32d0`–`0xb32d8`. `init` stores that result at `0xaa714`–`0xaa71c`,
starts the first row/column at it, and accumulates prior frame endpoints
(`0xaaa6c`–`0xaaa84`, `0xaaac8`–`0xaaae4`). The captured 492-unit columns
and 108-unit rows consequently give first local frame
`[0.5,0.5,492.5,108.5]`. `updateMeasuredRect` unions frame bounds, expands
individual outer edges by their border half-widths and normalizes measured
bounds (`0xab23c`–`0xab2c0`), giving `[0,0,985,217]` here. Over-pages table
text selects these frames at `0xa6b04`–`0xa6b24`, then offsets by the supplied
draw rectangle minus measured origin at `0xa6b38`–`0xa6b50`. These rules
support prepared geometry, not an arbitrary -0.5 shift of table glyphs;
the composed draw-rectangle origin remains a separate input.

The ordinary, non-spannable drawing branch instead reads each model cell's
rectangle, subtracts the model table origin, and adds the supplied draw origin
(`0xa68bc`–`0xa6b50`). It must retain that distinct coordinate producer. Rust now
prepares complete unmerged grids for constraints 1/2 with retained split lists.
Each cell uses the shared Rust engine at the native integer-truncated cell
dimensions and retains that plan for painting. Cold measurement grows saved rows;
warm retries retain caches, shrink or grow rows, and preserve pending page gaps.
Normal tables, merged/sparse grids, nested objects and unsupported active size
constraints retain the saved-frame path. Partial horizontal obstacles report
`UnsupportedContent` and preserve saved-cell painting; the shared cell engine
currently represents full-width vertical bands. Invalid derived bounds retain
the separate replacement-marker recovery policy.

Native prepared table frames are stateful. Bodytext caches the child layout
(`0xb0cac`–`0xb0d64`); construction sets its dirty byte (`0xa9e78`), while
later table callbacks call `Layout` without clearing measurement
(`0xb0dec`–`0xb0e2c`). Cold measurement grows rows to their measured cell
heights; warm layout can grow or shrink them (`0xaecf4`). Later rows can
move past a split using actual first-line height plus top margin
(`0xac9f0`, `0xb2360`), and trailing empty space can compress a row
(`0xb294c`, `0xb28ec`). Fresh measurement at every candidate is therefore
not established as equivalent. Rust now retains these phases for unmerged grids;
merged-cell frame union and rowspan growth remain unverified.

Warm row movement also retains page displacement. Positive preceding-row growth
consumes that displacement before moving later frames; negative growth first
removes the retained gap (`0xade70`–`0xadfb4`). Split-list comparison checks count
and only the first rectangle at tolerance 0.001; distinct empty lists cause
relayout (`0xb2e18`–`0xb2ea4`). The first-rectangle comparison checks all four
coordinates with inclusive `f32` tolerance `abs(delta) <= 0.001`
(Base `0xb14b0`–`0xb150c`). Rust preserves this cache policy, including unchanged
later bands when count and first rectangle match. Warm processing updates each
row's list, then resizes all remaining rows from their current cached metrics,
before compressing the current row. Later rows can therefore still use older
split lists during the earlier resize callback. Numeric regressions distinguish
this ordering from fresh cold measurement and cover gap consumption/removal,
first-line relocation, and compression using last-line bottom without bottom
margin.

Cold `Measure` calls `init`, `extendRowBySplit(0)` and `updateMeasuredRect`
(`0xaa5bc`–`0xaa5d0`). Its growth test is strictly positive (`0xab0c8`–`0xab0cc`),
so even a positive delta below 0.001 grows the row. The separate editor insertion
route's `measureRow` threshold does not apply to this cold drawing phase. Warm
resizing retains its native 0.001 threshold.

Native geometry reaches measurement as `f32`: Model `ObjectBase::GetRect`
loads four endpoint registers (`0x2caa6c`–`0x2caa70`), Widget span conversion
stores the drawn endpoints (`0xd54e0`, `0xd54ec`) and width/height
(`0xd5528`, `0xd5540`), and Text `measureObjectSpan` reads those dimensions
at `0x779f0`. The SDK rejects nonpositive dimensions and bounds or dimensions
that are not finite when represented as `f32`, while retaining accepted
`f64` geometry. No explicit native finite-input rejection was established;
this is an SDK robustness guard, not malformed-input parity evidence.

Block advance fills available width, while its alignment uses the object's
actual visual rectangle width: `GetBlockInfo` replaces the logical block
width with entry rectangle member 32's width (`0x6b11c`–`0x6b1ac`) before
calling `GetBlockOffSetXByAlign` (`0x6b1d4`–`0x6b1e8`). For available width
300 and object width 100, center/right offsets are 100/200, despite the
block's 300-unit wrapping advance. Object visual bounds are translated by
entry X and shared line baseline (`0x6b774`–`0x6b7ac`).

The locked corpus has object-only U+FFFC paragraphs: the basic-formatting
table at UTF-16 1427 (option 3, constraint 2), code at 1429 (option 0,
constraint 1), and seven image-placement anchors (option 0, constraint 0).
The code anchor occurs on two saved pages because their text ranges overlap.
There is no captured mixed inline-text case. The SDK now validates UTF-16
U+FFFC anchors with half-open source ranges and measures text and objects in
one paragraph stream, preserving neighboring text. Synthetic mixed-layout
regressions do not establish captured inline parity. Prepared object dimensions
and full-source page-slice context remain incomplete.

## Body-flow pagination boundaries

Over-page objects still participate in parent obstacle checks.
`IsOverlappedWithObstacle` (`0x6c6bc`–`0x6c6e0`, Text) passes the object's
over-page flag and minimum first-page height to the obstacle tree. For padding
obstacles, the tree skips a band only when that minimum is effectively zero
or `band.top - candidate.top >= ceil(minimum_height)` (`0x6e7fc`–`0x6e820`).
Otherwise the parent can move past the obstacle (`0x6a830`–`0x6a894`).
Code's minimum includes title padding, measured title height, title/body gap
and first body-line height (`0x738f4`–`0x73948`, Drawing). The pinned code
minimum is `36 + 60.75 + 24 + 60.75 = 181.5`, although the copy-button frame
makes its actual body top 132. Rust now retains that minimum beside measured
height and settles parent movement before remeasuring child splits at the
final candidate. Ordinary obstacles always apply. An absent title contributes
zero to the SDK minimum; native construction always creates a title drawing,
so that synthetic absent-title policy is not established native parity.

Table `GetMinHeightInFirstPage` (`0xac9d4`, Drawing) instead measures its first
cached row (`0xac9f0`–`0xacb48`). Nonempty cells contribute first-line background
height plus text top margin; empty cells contribute measured height. A zero
maximum falls back to the first cached frame's height, then adds
`contentRect.top - measuredRect.top`. Cold unmerged frames begin at the global
drawing half-border offset. Bodytext lays out the child before reading its
minimum (`0xb0e28`, `0xb0f20`–`0xb0f30`), so a missing Rust plan cannot stand
in for native zero. Prepared paged grids now retain this minimum and their shared
cell layouts; parent retries update the retained native row state rather than
rebuilding a cold grid.

Bodytext `BodyTextDocument::convertPageList`, `0xa9384`, constructs
`IBodyTextDocument::Page` records with cumulative integer Y at member 0,
a local rectangle at member 4, and the original `WPage` pointer at member
24. It obtains the actual page width/height and stores `[0,0,width,height]`
at `0xa94bc`–`0xa94f4`, then adds that rectangle's height to the next Y
at `0xa9598`–`0xa95a4`. These are actual page sizes, not font density or
the default-page size used to resolve density.

The selected body's text frame and split rectangles use its maximum physical
page width. `convertPageList` computes the maximum immediately at
`0xa95b4`–`0xa95f0`; `GetPageMaxWidth`, `0xa98c8`, returns it. `SetBodyTextDocument`
(`0xb039c`–`0xb03b4`), `onLayoutText` (`0xb1a10`–`0xb1a2c`) and `measureText`
(`0xb2e64`–`0xb2e80`) pass it to `ObjectTextLayout::SetLayoutWidth`.
`updatePagePaddingRect` fetches it once at `0xb8ebc`–`0xb8ed0`, so every split
rectangle has X `[0,max_selected_width]`, including the terminal boundary.
Table/cell conversion subtracts only Y. The inspected right-padding producers
(`0xb9d24`, `0xb9d28`) are return stubs; this path does not add narrower-page
right-side obstacles. Rust uses this selected-range maximum for both body
measurement and retained table split rectangles, while projection preserves each
physical viewport's own dimensions.

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

## Captured embedded text and page-padding inputs

The `hf/01-basic-formatting.sdocx` / Samsung PDF pair independently exposes
table/code text as vector PDF spans. PDF Form 89 is page index 3, the
fourth physical PDF page; Form 111 is index 4, the fifth page. These match
the SDK visible/source page indices and CLI `_page3.svg` / `_page4.svg`
suffixes; CLI page selection uses one-based `--pages 4` / `--pages 5`.
coordinates below use SVG units, with `x = 1.8 * PDF_x` and
`baseline = 1.8 * (848.3333333333334 - PDF_y)`.
This preserves the source's 1527-unit native canvas reference. The PDF's
actual MediaBox and form BBox are 848 points high, or 1526.4 units; mapping
from that viewport instead subtracts 0.6 from every reported Y. Page content
applies no additional transform to these forms.

| Text | X | Baseline | PDF font advance |
| --- | ---: | ---: | ---: |
| Column A | 72.000000 | 1100.850000 | 195.390015 |
| Column B | 564.000018 | 1100.850000 | 194.085015 |
| Alpha | 72.000000 | 1208.850110 | 114.705009 |
| Beta | 564.000018 | 1208.850110 | 91.035007 |
| Code title `text` | 129.750005 | 1379.351990 | 75.600006 |
| `fn main() {` | 129.750005 | 1475.352063 | 208.665016 |
| `    println!("Markdown code fence");` | 129.750005 | 1573.600049 | 695.970053 |
| `}` | 129.750005 | 1634.350049 | 15.210001 |

All these spans select Roboto-Regular at approximately 45 SVG units.
Headers use PDF stroke-and-fill text mode (`Tr 2`) and 0.45-unit stroke
width; ordinary cells use fill-only (`Tr 0`). Bold headers consequently
retain Regular-font advances. Form 111 repeats code text shifted by exactly
one physical page height, 1527 units. Direct CMap decoding is necessary for
this PDF: its overlapping `bfchar`/`bfrange` entries make the inspected
generic `lopdf` font-encoding helper shift some ASCII characters.

The table border/background is a separate flattened image (Form 89 XObject
103, 987×220 pixels, alpha mask 97); the cell text above remains vector.
Its image matrix places the top at native-reference Y 1019.60007, with
one SVG unit per pixel. Straight horizontal border strips occupy rows
1, 109 and 217, so their centers are **1021.10007**, **1129.10007** and
**1237.10007**. Header/body baselines are approximately 79.74993 units below
the respective row-top border centers. This independently constrains the
painted table origin; deriving a cell origin solely from a desired text
baseline cannot prove that first-paragraph before spacing should be removed.
Pixel border centers do not themselves establish the cell draw-frame origin.

The code-panel clip starts at Y 1298.351994 under the native-canvas
convention above, whereas its saved object box starts at 1297.751953.
Using the PDF viewport gives clip Y 1297.751994: the 0.6 difference is a
coordinate convention, not evidence of a composition translation. Relative
to the painted panel, title and first-body baselines are **81** and **177**
units under either convention. The density-3 frame constants and ordinary
45-unit text baseline explain them without adding 0.6 to a shared baseline.

Code body source contains three `ParsingState` paragraph records and no
line-spacing, before-spacing or after-spacing record. Its first baseline
gap is 98.247986 units, followed by 60.75; the panel's 411.748128-unit height
exceeds the ordinary three-line frame height, 374.25, by 37.498128. These
captured differences must not become a fixed extra gap for every code block.
The 37.498-unit skip also does not establish the height of a padding band:
moving a partially overlapping line past a band can skip only part of it.

Bodytext `BodyTextPageObstacle::updatePagePaddingRect`, `0xb8dfc`, creates
each page-boundary band as `[0, boundary - Constant66, width,
boundary + Constant65]` (`0xb8f04`–`0xb8f1c`). Content records at `0x7e00`
and `0x7e18` establish both constants as logical 10, document-density units,
without rounding: each side is 30 at density 3. `GetSplitOffestForObject`,
`0xb3504`, selects these bands and offsets them by minus object top.
Constraint 1, `OverPagesOverlapPadding`, instead produces a one-unit-high
band starting at the original band's center (`0xb35d8`–`0xb35e8`);
constraint 2, `OverPages`, preserves the full band.
`onUpdateObjectSize`, `0xb0b9c`, passes this choice at `0xb0dd8` and supplies
the resulting vector to object-layout virtual slot 16 at `0xb0de4`.

The split-band origin is a live parent-layout candidate, not the object's
stored top. Text `GetBlockInfo` calls `m_CheckObjectChanged` only for entries
whose `IsObjectOverPages` flag is set (`0x6ae10`–`0x6ae7c`); Widget conversion
sets that flag only for constraints 1/2. `DoLayTextOut` forms the candidate
from the vertical cursor plus enabled paragraph-before spacing
(`0x6a6c0`–`0x6a770`, `0x6a7e8`–`0x6a808`); `m_CheckObjectChanged` adds the
adjusted object top margin at `0x6c344`–`0x6c374`. This path supplies no
stored object Y input.

Widget constructs the callback at `0xd3254`–`0xd326c` (vptr `0xf5578`);
dispatcher `0xd9d88` forwards through callback member 288 to Bodytext
`onUpdateObjectSize`, bound at `0xb01bc`–`0xb0220`. Bodytext updates child
split offsets at `0xb0dd8`–`0xb0dec`, measures the child, and reads measured
height through virtual slot 72 at `0xb0e30`–`0xb0e50`. The code child clears
measurement through slot 64 and measures through slot 56; the table child
uses layout slot 48 (`Drawing` vtable `0xc4b60` → `TableLayout::Layout`,
`0xaa3d4`). Text replaces entry height and visual top when the result differs
by more than `0.001f` (`0x6c42c`–`0x6c45c`). Child size therefore feeds back
into the same parent line calculation.

Constraint 1 produces `[band.center_y - candidate_top,
band.center_y - candidate_top + 1]` (`0xb35d8`–`0xb35e8`); constraint 2
offsets the full band by `-candidate_top` (`0xb35f0`–`0xb35fc`). Code then
subtracts its local body-frame top (`0x73480`–`0x7349c`). Although code
`GetMeasuredRect` retains its saved rectangle origin (`0x737dc`–`0x737e8`),
the height subtraction cancels that origin. This does not authorize using
saved Y to choose the bands.

`AdjustedBlockTopMargin`, `0x6c8a0`, normally returns
`max(current_top_margin, previous_bottom_margin)`. Previous bottom comes
from the last retained line's member 36, or caller-supplied
`ParagraphLayoutData` member 28 when no line is retained (`0x6c8c4`–`0x6c90c`).
Its f32 probe covers `[Q - 1, Q]`, where
`Q = candidate_top - enabled_paragraph_before` (`0x6c930`–`0x6c968`). Both
callers pass Y after paragraph-before spacing and before object top margin
(`0x6c79c`–`0x6c7a8`, `0x6c360`–`0x6c374`). Before spacing is added once
before the wrapping loop (`0x6a740`), but paragraph member 88 remains enabled
through that loop; the probe still subtracts it on subsequent wrapped lines.

The obstacle-tree Boolean **includes padding when true**. Its true branch
goes directly to intersection (`0x6e7fc` → `0x6e838`), bypassing the
minimum-first-page-height gate. Parent block checks pass the inverse of
their over-page flag (`0x6c6bc`–`0x6c6dc`), allowing that gate to apply to
over-page objects. The margin probe instead explicitly passes true and
minimum height zero (`0x6c944`, `0x6c958`); the paragraph wrapper preserves
the Boolean and forwards the minimum as s2 (`0x6c9ec`–`0x6c9f8`). Thus its
flagged-padding branch is reachable even with a zero minimum.

For an intersecting flagged band of nonzero height, the result becomes
`max(0, current_top_margin - band_height / 2)` (`0x6c970`–`0x6c99c`), ignoring
previous bottom. An unflagged obstacle keeps ordinary collapse. Intersection
requires more than `.0001f` overlap on both axes (`0x6e6b4`–`0x6e700`);
touching a band boundary does not count. These are literal consequences of
the native branches, not captured fixture measurements, for a flagged band
`[1497,1557]`, current top 60 and previous bottom 80:

| Candidate Y | Enabled before | Probe | Adjusted top |
|---:|---:|---|---:|
| 1557 | 0 | [1556,1557] | 30 |
| 1558 | 0 | [1557,1558] | 80 |
| 1497 | 0 | [1496,1497] | 80 |
| 1569 | 12 | [1556,1557] | 30 |

`SetLayout` adds the adjusted margin at `0x6b4f0`–`0x6b510`; the
margin-bearing object baseline uses base object height (`0x6cb90`–`0x6cb9c`)
before the object epsilon. Rust now retains the raw candidate separately from
its adjusted margin, applies this padding exception to every wrapped line,
and recomputes the margin after moving the raw candidate to an obstacle's
bottom. Independent regressions cover the literal cases above, native f32
probe rounding, and suppression of a preceding object's bottom margin on an
ordinary text line. Ordinary constraint-0 child feedback was not established
by this trace. Child callback origins now use their resolved object font:
inline entries receive raw Y, margin-bearing blocks receive raw Y plus the
adjusted margin, and marginless blocks receive raw Y plus percentage/pixel
leading minus `0.35f * object_font`. This is independent of the parent's
aggregate font metric (`0x6c380`–`0x6c3ac`). Images with either known over-page
constraint retain callback minimum height zero: the factory supports only
table/code layouts (`0x86810`–`0x868d0`), so the image's null-layout branch
returns without replacing the initialized minimum (`0x6c30c`, `0xb0d34`).

Drawing `CodeBlockLayout::Measure`, `0x732fc`, converts those bands to body
coordinates by subtracting body-frame top (`0x73484`–`0x7349c`). Its
`measuredObject`, `0x73694`, calls `ScrollEditTextView::SetPaddingRectList`
at `0x736d0` before measurement; returned text-layout height determines panel
height. The captured code object's constraint is **1**, with block layout
option 0. It therefore uses the collapsed band `[1527,1528]`, not the full
60-unit band. Its saved top 1297.751953125 plus the 132-unit body-frame
offset gives body top 1429.751953125. After the ordinary first-line advance
60.75, the second candidate starts at 1490.501953125. Moving its overlapping
line past the band's bottom adds `1528 - 1490.501953125 = 37.498046875`
units. This reproduces the independently extracted extra baseline gap,
37.497986, and panel enlargement, 37.498128, within PDF float precision.
The mechanism is a candidate-line move past a one-unit boundary marker;
37.498 is neither a constant paragraph gap nor the padding-band height.
The shared code frame's height consequently has a native-backed explanation.
The final drawing origin is distinct from the callback origin.
`BodyTextCapture::updateSplitOffset` reads the placed text bound and recomputes
bands (`0xcac30`–`0xcac7c`). Composer copies the object and assigns its final
drawing rectangle (`0x3763dc`–`0x376474`), then passes its drawing split vector
to a fresh `CodeBlockLayout` (`0x3764a8`–`0x3764d8`, `0x378804`–`0x37884c`).
That drawing measurement does not update parent height feedback.

Rust uses the same `prepare_code` function for these two native contexts.
The captured code measures at callback Y 1237.75 with height 411, while
previous-bottom-margin collapse places drawing at Y 1297.751 with height
411.75. Reusing the callback's line plan after translating it by 60 loses
the 37.498 page-boundary move. Repreparing at the final drawing origin fixes
that clipping and preserves the parent's 411-unit reservation. Fresh SVG
comparisons against the hash-checked native PDF differ by less than 0.00012
units for the title, all code lines, and following whitespace heading; the
earlier 0.75-unit following-text residual is resolved. The four wholly
clipped code lines remain absent.

The table PDF writer also creates a fresh layout at the final drawing
origin. Composer allocates `TableLayout`, supplies drawing split rectangles,
then invokes cold `Measure()` followed by warm `Layout()`
(`0x37e448`–`0x37e4a0`). The warm pass can shrink rows independently of the
parent's retained callback height. Cold initialization reads raw row heights
and column widths; `content_bbox`, minimum/maximum column-width arrays and
`max_width` do not constrain this selected cold/warm path.
The preceding clone placement uses `ObjectShape::SetRect` through vtable
slot 40 (`0x376464`–`0x376474`, relocation `0x495200`), whose detached-clone
branch stores the rectangle through `SetRectDataOnly`
(`0x397ad8`, `0x399954`, `0x3a6a60`, `0x37d104`). It does not invoke the
separate editing-time table fitting setter. Auto-fit metadata consequently
does not resize rows or columns in this export path. Normal body entries
keep the empty drawing split vector initialized at `0x8de40`; capture fills
it only for over-page constraints 1/2 (`0xcac1c`–`0xcac7c`).

The writer offsets normalized measured bounds by the source drawn origin
in native `f32` arithmetic, then floors left/top and ceils right/bottom
(`0x37e4dc`–`0x37e4f4`). Each ordinary text cell offsets its cached frame by
that rounded table origin and receives the same rounding
(`0x37ee14`–`0x37ee94`). `ExtendRect()` directly uses `FRINTM`/`FRINTP`
(`0xb16cc`); negative coordinates are not truncated toward zero.
The writer translates retained child drawing data into the rounded cell
frame without wrapping again (`0x37ef00`–`0x37ef80`), and
`TableLayout::UpdateTextDrawingPosition` is empty (`0xacc88`). The separate
expanded image clip does not change glyph origins.
Child text origins also use native `f32` addition (`0x37f670`–`0x37f684`):
at world X 16,777,216 a local text inset of 3 yields X 16,777,220. Retaining
the addition in `f64` would instead produce 16,777,219.

Rust regenerates complete, unmerged, unrotated grids with this same engine
and keeps the final world geometry in a distinct drawing plan. It preserves
the callback reservation and translates retained cell text once. The captured
table's former saved-frame fallback placed text 1 unit right and about
1.751 units low. Fresh hash-checked comparisons now include all four native
cell glyph origins, alongside the existing body and code references.
Merged/sparse grids, nested objects, active height limits and full table
border styling remain outside this supported drawing path.

Rust also applies the native paragraph indentation masks: saved RTL indent
adds only a right inset for right/center/default alignment; other indent
directions add only a left inset for left/center/default alignment. Both
alignment applies neither inset (`0x7397c`). Indent direction does not select
the bidi base or mirror markers. Drawing normalizes unknown alignment to
internal default 4 through the literal `[4,0,1,2,3]` conversion tables
(`0x93234`, `0xa5ac8`, `0xd1454`). Default alignment currently follows the
SDK's LTR display policy. Right/center markers share the glyph alignment
offset, matching `m_UpdateBullet`'s caller (`0x6b5dc`–`0x6b61c`).

Both alignment now distributes extra width through the shared positioned
line. Native ordinary constructors initialize layout options to zero
(`0x636e0`, `0x6fcfc`), so final lines and leading/trailing whitespace are
included. Only U+0020 and TAB expand, weighted one and four respectively
(`0x77688`, `0x6af4c`, `0x6b838`). The native f32 share is
`(available_width - measured_width) / weight_count`, without a positive
remainder clamp. Markers remain at their reserved block start, and source
ranges remain unchanged. Complex-script positioning still reports the
existing unsupported-positioning diagnostic rather than establishing RTL
justification parity.

The following table and discussion record an earlier debugging snapshot,
before the callback/drawing separation above; their SDK values are historical.
Table cells retain logical margins `[8,4,8,4]`, line multiplier 1.6, and
before/after spacing 4. Saved first-cell X is 49, while captured text X 72
and scaled left margin 24 imply a regenerated frame X 48. The current CLI
exports `/tmp/sdocx-child-feedback_page3.svg` and `_page4.svg` include candidate
code preparation, child-size feedback and table stroke-bound expansion. The
comparison retains the previous mixed-stream snapshot and accumulates all SVG
ancestor transforms. Index 3 is the fourth physical PDF page / Form 89; index 4
is the fifth page / Form 111. CLI `--pages 4-5` selects these two pages.

| Quantity | Previous mixed-stream SDK | Current SDK | Native canvas reference |
| --- | ---: | ---: | ---: |
| Table header baseline | 1113.501010 | 1114.001010 | 1100.850000 |
| Table body baseline | 1221.501010 | 1222.001010 | 1208.850110 |
| First-cell text X | 72.5 | 73 | 72 |
| Second-cell text X | 564.5 | 565 | 564.000018 |
| Code panel height | 411.75 | 399.75 | 411.748128 |
| Code title baseline | 1389.752010 | 1390.752010 | 1379.351990 |
| Code first body baseline | 1485.752010 | 1486.752010 | 1475.352063 |
| Code second body baseline | 1584.000060 | 1573.001000 | 1573.600049 |
| Code third body baseline | 1644.750060 | 1633.751000 | 1634.350049 |
| Code first baseline gap | 98.24805 | 86.24899 | 98.247986 |
| Code second baseline gap | 60.75 | 60.75 | 60.75 |

The current painted table starts at X 48.5 / Y 1033.25. Stroke-bound expansion
removed the previous -0.5 X translation and moved its painted origin by +0.5
in both axes. Cell text consequently differs from native by **+1 X** and
**+13.151 Y** under the native-canvas convention, or **+13.751 Y** under the
actual PDF viewport. This worsens the local X difference until the missing
regenerated table-frame producer supplies the correct frame. Retained first
paragraph before spacing is not evidence of an error: removing it to match
the target baseline would bypass the verified native policy.

The code panel now starts at Y 1309.751. Its title and first-body baselines
remain approximately **12.000 units below the actual PDF viewport reference**.
The second and third body baselines instead agree with that reference within
**0.001 units**. Candidate-based preparation moves the body-frame origin by
approximately 12 units relative to the captured copy, so the first boundary
skip shrinks by approximately 12 units: height 399.75 and first baseline gap
86.24899 are both about 11.998 below the captured local quantities. The later
lines land at the same boundary even though the first line starts too low.
This separates the remaining composition-origin error from the verified
boundary mechanism; forcing a constant panel height would hide that cause.

The saved-overlap code copy on page index 4 has current title/body baselines
-148.24805, -52.24805, 46.00000 and 106.75000. They are approximately 0.6
above the native-canvas reference and agree with the actual PDF viewport
convention; its height remains 411.75. Following ordinary text still diverges:
`Whitespace samples` now has baseline 311.5, versus previous 310.75098 and
native actual-viewport 337.750928. This **-26.250928** difference needs separate
continuation-flow verification. The ordinary lines preceding the table at
index 3 remain approximately +0.75 relative to the actual viewport, with no
accumulating vertical drift. Table frame regeneration and complete
body-composition origins therefore remain unresolved.

A fresh two-page CLI PDF at `--pdf-dpi 129.6` has MediaBox approximately
599.999939 × 848.333313 points. Independent Rust `lopdf` extraction of CTM,
text matrices, `ToUnicode` and font sizes reproduces all table/code SVG
baselines above within 0.001 units; font size 45 becomes approximately
44.999997 after float conversion. This checks export transport, not native
composition parity. The default pinned Roboto face returns missing glyph
ID 0 for `●`, `○` and `───`; the exported PDF has no direct `ToUnicode`
mapping for those missing glyphs. It carries `ActualText` for `○` and each
`─`, so absence from the glyph CMap alone does not prove that their selectable
source is lost. No `ActualText` for `●` was observed. These metadata paths do
not supply missing visible glyphs. Point-marker text currently bypasses coverage diagnostics.
The corrected marker advance does not establish complete marker rendering;
native vector marker geometry or suitable glyph fallback remains necessary.
The native point-marker producer already identifies a vector route:
Widget `initBulletPoint` assigns resource IDs 38–41 for solid circle,
outline circle, solid square and outline square (`0xd96fc`–`0xd9798`).
`SpenResources.ResourceID` maps these to the four `spen_ic_text_bullet`
vector assets. Their viewport is 4×4: solid circle radius 2; outline
circle radius 1.75 with 0.5 stroke; solid square 4×4; outline square
3.5×3.5 inset 0.25 with 0.5 stroke. Their final native size and placement
still need tracing before replacing the current glyph painter.

## Current Rust implementation and remaining gaps

The shared Rust body-flow and placed-text pipeline now measures shaped runs with actual
font faces and wraps using Unicode line-break opportunities. The
`render/text/{layout,measurement,breaks,wrapping,paint}.rs` modules replace estimated
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
Placed text now measures a complete plan before painting and applies
top/center/bottom gravity to ordinary text using the native height contract
above. Whitespace remains selectable, and empty boxes retain their highlight.
Final paragraph after-spacing is excluded from gravity height; between-paragraph
after-spacing remains. Glyph Y positions retain five decimal places, including
gravity midpoints; the parent text coordinate keeps its older display precision.
Table cells and code title/body now use the shared engine for every wrapped
line, styles, scaled margins and measured heights. Code frames include the
verified page-boundary exclusion rule above; cells retain native top gravity
and explicit-spacing baseline handling. Mixed object/text paragraphs now use
the same measured stream, including inline widths, block breaks, native
symmetric margins, object alignment and object baseline increments. Validated
anchors preserve neighboring text, and invalid or unsupported objects retain
typed diagnostics. This has synthetic coverage but no captured mixed-inline
reference. Code height feedback and table drawn bounds are implemented;
unmerged table frames and validated full-source page context are implemented;
merged grids and complex object composition remain incomplete. Paragraph-gap enable flags now
follow the native bullet conversion and source-edge object rules, with
synthetic regressions. Terminal-newline display paragraphs, clipping and
recomputed pagination still need captured cases and implementation. Ordinary
baselines share the native line-advance formula; object-margin lines retain
their separate branch.

Shared style resolution now checks native scaling before using its result.
A public-API probe with source `A\nB` and density 3 previously lost text
when finite `f32::MAX` font sizes, pixel spacing or margins overflowed the
f32 multiplication. Invalid summary sizes now use the configured default;
invalid local font overrides retain the preceding usable size. Invalid
scaled margins and paragraph gaps resolve to zero. Unusable explicit line
spacing uses the finite default line advance, with the finite font height
as a final fallback. Accepted scaling and line-height precision are unchanged,
and parsed values are preserved.

These SDK robustness fallbacks report typed `InvalidGeometry` diagnostics;
they do not claim Samsung rejects or recovers malformed inputs this way.
Only active font overrides are diagnosed; paragraph gaps are reported when
consumed, and explicit spacing is checked against actual wrapped line metrics.
Margin blocks skip the spacing they do not use. Regressions cover 336
malformed preview/replay cases and 18 selectable, vector PDF exports across
flow, placed, shape, table and both code text contexts. Arbitrary malformed
frame rectangles and oversized aggregate page geometry still need their own
validation policy.

Code preparation now retains owned title and body layouts, copy and panel
rectangles. Parent lines prepare these plans before placement; constraints
1/2 replace their reserved height when the measured height differs by more
than 0.001. Other constraints retain their saved reservation. Split bands
are selected using the live parent candidate, and painting translates the
retained plan without measuring it again, including under parent gravity.
Validated native capture windows now replace individual page-slice painting.
They retain native source separators, prepare object feedback in group-local
physical coordinates and project the selected viewport once. Typed glyph,
marker and prepared-panel bounds filter painting independently; code child
viewports receive both translations, while table cell frames already include
their vertical offset. Decorations use actual styled segments rather than the
first style in a shared shaping run. Fresh paint registries embed only painted
faces and report visible font resolution; planner issues retain source or
outer-object ownership. Structural and geometry diagnostics can still cover a
whole visible object. The negative-top adapter remains only for fallback
inspection slices. Full composition-origin and table preparation parity are
not established.

Rejected derived code geometry retains the original replacement marker and
neighboring text, reports `InvalidBounds`, and never falls back into measuring
and painting the rejected block again. The same preparation boundary checks
normal and split constraints. This is an SDK robustness policy, rather than
an established native malformed-input recovery rule.

The prepared-object milestone is committed as `0d17727`. Twenty focused
regressions cover native chrome constants, candidate-relative bands, retained
gravity, nested height feedback, rejected source markers and selectable PDF
text. Workspace tests, strict Clippy, render-only and no-default-feature
checks pass. The locked external corpus and independent first-page PDF text
comparison pass. A fresh WASM build of the same source passes all 23 Chromium
tests and all 66 web unit tests; typecheck reports no errors or warnings.
These checks establish the covered contracts, rather than closing the
composition and missing-glyph differences quantified above.

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
