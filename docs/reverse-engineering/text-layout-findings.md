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
(`0x3a1734`–`0x3a175c`). It takes the font-size delta from
`WNote::GetBodyTextFontSizeDelta` through `TextViewUtil::SetTextSizeDelta`
at `0x3a1774`–`0x3a1788`, and shares the resolved delta with the body editor
at `0x3a1794`–`0x3a17a8`. Widget `CalculateTextSizeDelta`, `0xe0ea8`,
passes an explicit delta through unchanged; `INT_MIN` selects a display
default. The exported tablet/phone default constants at `0x6526c` /
`0x65270` are -5 / 0, respectively.

Standard PDF's X delegate constructs a `NoteTextManager` and calls this
same `SetDocument` at Composer `0x3575ec`–`0x3575fc`. Widget's body layout
constructor reads its manager's resolved delta at `0xd3124`–`0xd3138`;
Drawing's constructor does so at `0x8c520`–`0x8c53c`. Bodytext
`BodyTextLayout::SetTextScale`, `0xb3a2c`, forwards a changed local scale
to the shared `ObjectTextLayout` at `0xb3a64`–`0xb3a6c`. A document density
of 3 can explain scale 3 in a captured fixture; it cannot justify a universal
factor of 3. Export context, native density, stored delta and local scale must remain
explicit inputs to one Rust layout pipeline.

The current parser exposes the stored delta as
`StoredNote::metadata(note_bytes)?.body_font_size_delta` (`NoteMetadata`,
flexible field 11). It is not promoted to `DocumentMetadata`. Neither public
structure currently exposes `document_density`; `StoredNoteHeader` provides
flow dimensions and `StoredNote::default_page_dimensions` provides the
separate default dimensions. Resolving the native density getter remains
necessary before choosing which dimensions define text scale.

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

`SetLayout` adds the line's aggregated top margin (member 32) before this
baseline call (`0x6b4f0`–`0x6b510`). Completed intermediate lines call it
at `0x6b388`; the final line calls it at `0x6a9bc`. Thus the inspected
ordinary baseline calculation is shared by those lines. Lines containing
objects with margins take a separate baseline branch at `0x6cb90` and can
add an extra offset at `0x6cc0c`; do not apply the ordinary text formula to
embedded objects. Paragraph-edge, bullet, empty-line and page-limit effects
still need captured cases before claiming complete first/last-line parity.

## Measurement, body flow and embedded objects

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
