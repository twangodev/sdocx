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
settings to a `TextLayout`. These are separate interval/paragraph inputs;
selecting the first style of each kind for the whole box does not reproduce
the native transfer contract.

The fourth header word is an interval enum, not a boolean expansion flag.
Model's WDoc reader preserves it unchanged at `0x40d040`–`0x40d050`;
`GetIntervalType`, `0x40c8cc`, and Widget's converter at `0xdb7e4`–`0xdb7ec`
forward that value. Rust retains it as `SpanIntervalType`, including unknown
values. Ordinary glyph styles still use the half-open slot intersection above;
caret font lookup is a separate operation.

Text `RichText::m_FindLatestFontSize`, `0x62104`, scans font-size records in
reverse order and returns the first matching resolved size (`0x621f4`–`0x621fc`).
For position `i`, its exact endpoint predicates are:

| Native interval | Caret predicate |
| --- | --- |
| 0 / ClosedOpen | `i == start \|\| (start < i && i < end)` |
| 1 / ClosedClosed | `start <= i && i <= end` |
| 2 / OpenOpen | `start < i && i < end` |
| 3 / OpenClosed, or another value | `i == end \|\| (start < i && i < end)` |

Zero-length font spans therefore affect caret metrics for values 0/1/3,
without styling a glyph. The producer retains them in source order
(`0x8d820`–`0x8d968`); the latest matching record wins. `GetFontSize`,
`0x62044`, does not read paragraph predefined styles: it returns that already
scaled override, or global default size times selected density plus resolved
delta (`0x6208c`–`0x620d8`). It accepts end-of-text positions and has no upper
text-length check; negative positions skip overrides. Native comparisons use
signed 32-bit offsets. Rust separately requires ordered, in-bounds UTF-16
endpoints at scalar boundaries, rejecting surrogate interiors and malformed
ranges. That safety policy is not native malformed-input recovery parity.

Caret defaults retain the separate f32 fused operation: `base.mul_add(document_density,
logical_delta * document_density)`. Glyph defaults and explicit spans keep their
existing addition-then-multiplication path. The current export contexts use local
scale 1; nonunit local scaling is not established by these adapters.
Native caret defaults are unclamped; Rust diagnoses nonpositive or
nonfinite results and uses its finite recovery value.

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
unverified. Standalone editing/resizing, export clipping and visual parity
have no captured reference in this evidence set. The ordinary baseline helper
is shared with placed text as traced below.

### Shape template text frames

Composer's actual shape writer calls `ObjectTextPDFWriter::WriteTextContent`
for type 7 (`0x37c8b8`). That writer applies template insets with separate
`f32` additions/subtractions (`0x3805bc`–`0x380608`) and preserves the original
geometry center for rotation. Shape text bypasses the outward `ExtendRect`
rounding used by type-2 PDF text boxes (`0x380684`). Drawing follows the same
inset policy at `0x80b28`–`0x80b68`.

Model's saved-shape reader converts rectangle endpoints from `f64` to `f32`
(`0x3a93f4`–`0x3a9400`). It restores the saved path at `0x3a95a0`, through
the common path loader `0x20bad0` and inverse rotation `0x20c278`, before
replaying saved control points at `0x3a9614`. Rotation uses SPenBase's mixed
precision helper `0xb0f10`: narrow stored coordinates, subtract the original
center in `f32`, use `f64` angles/trigonometry, then narrow the result before
the `f32` center addition. Path and control replay order matters.

Rust shares a typed `NativePathCommand` decoder between shape inspection,
SVG path building and template text frames. Actual horizontal/vertical flips
are retained from type-7 property bits 0/1; type-0 `metadata.flip_enabled`
is a capability flag, not the current orientation. Model's property writer
`0x3a7f84` reads horizontal getter `0x20d8c4` and ORs bit 0 at `0x3a7fb0`;
vertical getter `0x20d928` produces bit 1 at `0x3a7fc8`. Loader
`0x3a95ac`/`0x3a95b8` passes that decoded state to common loader `0x20bad0`,
which stores bytes 16/17. The placed adapter's
`native`, `triangle`, `rounded` and `polygon` modules restore world paths
about the original geometry center and replay the applicable controls before
deriving insets. Canonical template paths use a bounded ten-command stack
buffer; path-dependent templates do not invent geometry for an empty path.
Prepared template margins remain separate from density-scaled component
text margins. The covered producers are:

| Template | Retained inset contract | Native producer |
| --- | --- | --- |
| Ellipse, ID 1 | `(width * 3) / 20` horizontally; corresponding height formula vertically | `0x213b5c` |
| Triangle, ID 2 | Saved apex ratio gives asymmetric half-width margins; vertical flip selects the half-height side | Margin `0x2148e8`, control `0x2144ec` |
| Right triangle, ID 3 | Retained vertex direction selects horizontal weights 35/165 and vertical weights 235/35, multiplied before division by 400 | Margin `0x215084`, direction `0x211670`, branches `0x214d3c`/`0x214dec` |
| Rectangle, ID 4 | Zero template insets | `0x2152e8` |
| Rounded rectangle, ID 5 | Saved/replayed radius `R` gives `R + ((R * f32::from_bits(0xbfb504f3)) * 0.5)` on all sides | Margin `0x2165f4`, control `0x2157f0` |
| Hexagon, ID 6 | Retained shoulder distance determines symmetric horizontal/vertical margins; controls reconstruct flipped vertices | Margin `0x2174f0`, control `0x2169b8` |
| Diamond, ID 8 | `width * f32::from_bits(0x3e851eb8)` horizontally; corresponding height product vertically | `0x218e14`, load path `0x2190f0` |
| Pentagon, ID 11 | Saved vertex ordering selects the horizontal inset and the quarter-height side | `0x21b6d0` |

For hexagon shoulder distance `D`, native computes
`(((K * 80) / 200) * D) / halfW`, then adds `(K * 35) / 200`, where `K`
is `halfW` horizontally and `halfH` vertically. Rust retains the separate
`f32` operations without algebraic simplification. Triangle, rounded-rectangle
and hexagon control setters ignore the control index and replay saved points
in order; right-triangle and pentagon setters retain the base no-op behavior.

Rust's typed placed-text frame adapter preserves this operation order and
supplies the inset frame to the shared layout engine without cloning or
modifying source text. Measurement dimensions remain independent of frame
coordinates; rebuilding endpoints from origin plus width can erase the width
at very large origins. The engine also derives wrapping width from local
insets rather than subtracting absolute positions. Native measurement
subtracts inset endpoints in `f32`
and ceilings width/height to signed 32-bit integers
(`ScrollEditTextView::SetPosition`, `0xbc234`; layout height, `0xbc9b8`).
The conversion rounds toward positive infinity and saturates to the integer
range, as defined by Arm's [FCVTPS documentation](https://developer.arm.com/documentation/dui0801/g/A64-SIMD-Vector-Instructions/FCVTPS--vector-)
and [FPToFixed pseudocode](https://www.scs.stanford.edu/~zyedidia/arm64/shared_pseudocode.html#func_FPToFixed_6).
Rotation uses the original endpoint sums multiplied by `0.5` in `f32`
(`RectF::CenterX/Y`, `0xb1820`, `0xb1838`), independent of the inset frame.

Right-triangle replay retains the direction/flip-dependent extra inverse
rotation branch, rather than replacing it with a default margin table.
The direction helper (`0x211670`) multiplies `atan2f` radians by 180 in `f32`,
divides by pi in `f64`, then narrows before the exact 0/180 comparisons.
Rust supplies signed zero or binary32 pi for exactly horizontal vectors:
host `atan2` rounding otherwise made the flipped 180-degree branch fail in
macOS ARM64 CI. Nonhorizontal directions retain the native arithmetic chain.
Arbitrary noncanonical paths and unsupported templates remain diagnosed.
Horizontal control projection preserves native comparison behavior (`0x210df8`):
positive/negative infinity selects an endpoint, while unordered projection
retains the input X. These intermediates do not make a finite result invalid.
Triangle controls separately skip a zero length, including squared-length
underflow (`0x2145bc`); common rounded/hexagon projection uses divisor 1.
Polygon margin getters retain finite signed insets (`0x21b740`–`0x21b774`,
`0x217564`–`0x2175e0`); Rust validates the resulting frame instead of clamping
those margins to zero.
Model's subsequent saved-bounds `Refresh` has a separate load-state contract,
described below. Changed bounds and nonunit load magnification are outside
the verified per-template replay contracts.
The covered contracts do not establish unrestricted template parity.
Unsupported nonempty shape text retains its saved frame with `UnsupportedTextFrame`;
invalid known geometry reports `InvalidGeometry` and stays in placed context.
Native background spans use retained layout ranges as described below;
author-supplied legacy highlight summaries retain their whole-box behavior.
Free/Path editing and autofit semantics remain unverified. The hash-checked
HF02 corpus has five empty shape text boxes and no native shape glyph
operators, so these regressions establish source
contracts and synthetic vector behavior, not captured shape typography parity.

[`shape_template_paths.rs`](../../crates/sdocx/tests/shape_template_paths.rs)
checks literal native origins, ordered controls, flips, rotation, ceiled
wrapping/alignment/gravity, original background clips, replay and selectable
Unicode PDF output. Non-cardinal source-instruction literals in
[`placed/native.rs`](../../crates/sdocx/src/render/placed/native.rs) pin the
center arithmetic and narrowing stages; these are not Android runtime captures.

### Saved-bounds refresh

`ObjectShape::NewGetBinary`, `0x399b40`, snapshots geometry `G`, drawn bounds
`D` and rotation `R` (`0x399b84`–`0x399bbc`). For type 7 it temporarily puts
`D` and zero rotation into the base frame (`0x399bd0`–`0x399be8`), writes both
frames, then restores `G/R` (`0x399cb4`–`0x399ccc`). Normal unscaled loading
therefore starts with current owner bounds `C == D`, even when `G != D`.

After path/control replay, `Refresh`, `0x3a9af0`, compares narrowed `D` with
the current owner base rectangle `C` (`0x3a9b80`, getter `0x37aa94`). Equal
bounds cache `G` without calling template slot 24 (`0x3a9bd4`–`0x3a9bdc`).
All five HF02 shapes have identical base/drawn `f32` endpoint bits, distinct
geometry bounds, zero shape rotation and absent base rotation. The locked
corpus regression preserves those predicates. Using saved `G/R` for their
text frames is consistent with this ordinary load branch.

Changed bounds inverse-rotate `C/D`, affine-map `G` between those rectangles
with separate `f32` operations and FMAs, then call template slot 24
(`0x3a9bf0`–`0x3a9c40`). Load magnification comes from the current supplied
axis divided by the stored orientation-selected axis (`0x2db4b8`–`0x2db4cc`).
Nonunit loading separately resets template bounds to scaled original `G`
after Refresh (`0x3a96c8`–`0x3a96f0`), while the owner receives Refresh's
cached rectangle/rotation (`0x399edc`–`0x399ef8`). These are separate states.
Common slot 24 (`0x20dba0`) transforms retained paths through `0x20dd10`;
templates can override it. Scaling the saved rectangle alone does not replay
those path/control-dependent margins. Changed-bounds, nonunit and additional
owner-rotation cases remain outside the proven current adapter scope, and
HF02's empty text does not establish typography parity for them.

## Native text backgrounds and current vector limits

`BackgroundColorSpan`, kind 17 (Model constructor `0x414670`), applies to a
half-open UTF-16 source range. `SetColor`, `0x41479c`, retains ARGB; Drawing
dispatch `0x90e18` calls converter `0x91058`, which requests theme color kind 3
and enables background painting at byte 135. The raw BGRA payload preserves
that ARGB color, including alpha. Composer skips alpha-zero backgrounds at
`0x380ba0`–`0x380bbc` and applies object opacity at `0x380cc4`–`0x380cd8`.
The RGB-only `highlight_color` summary is retained for compatibility.
Parsed spans, including a whole-text span,
do not cause that summary to paint the object's bounding box. Legacy
author-supplied summaries without kind-17 spans retain their existing behavior.

Ordinary background geometry follows line placement rather than glyph ink.
At Text `SetLayout`, `0x6b508`, the upper edge is the candidate cursor after
the adjusted object top margin. `GetBaseline`, `0x6b510`, advances that cursor;
`0x6b598` reads the lower edge after the complete line advance. Ordinary
entries receive `[x, top, x + retained_advance, bottom]`; characters sharing
an object line include its full advance and the object epsilon
(`0x6cc18`–`0x6cc28`), rather than a font-specific rectangle height.
Space/tab backgrounds retain their advances. The background right edge adds
the justification extra at `0x6b858`–`0x6b860`; tabs add four extras through
the FMA at `0x6b888`–`0x6b890`. Measurement join `0x8dc28` does not compare
the background fields at offsets 8/12; full paint-style equality
`0x8db7c`–`0x8db98` does. Background changes therefore do not split shaping.

Native advances are associated with UTF-16 measurement entries and shaped
cluster indexes (`0x77390`, `0x77640`–`0x77668`). Ordinary export skips entries
without glyph data. Dividing a cluster's width equally among source characters
has no native proof. Rust's shared `render_line_backgrounds` instead uses
retained cluster positions and advances, including justification, with
`background_top` preserved independently from the pre-margin line top.
The SDK separates text-cluster rectangles from supported SVG object bands,
applies frame/gravity/table translations, and paints typed vector rectangles
before glyphs with span alpha divided by 255.
Background and glyph visibility are evaluated independently. A background
change inside a retained cluster or unavailable measured positions reports
`UnsupportedBackgroundPositioning`; highlighted bidi, complex-script and
variable-font lines also retain that diagnostic. Their visual mapping is not
verified. Selectable text remains available. Widget's native converter skips
ordinary backgrounds on object spans (`0xd7e20`–`0xd7e50`); Drawing's converter
does not share that exclusion. Rust distinguishes those producers:
Widget body/capture/table conversion guards ordinary/composing backgrounds on
validated object slots, while placed/code Drawing conversion permits them.
Composing tags remain unguarded. Geometry flow mode alone does not select the
span producer.

Composing background, kind 15, occupies a separate raw member from ordinary
background. The native table preview selects its complete theme-mapped ARGB
word when nonzero, while Composer retained text copies only ordinary background.
Widget applies object identity first and excludes kind-15 and kind-17
background assignments on those slots; composing-tag kind 18 has no such
guard. [Draw identity findings](text-draw-identity-findings.md#preview-and-composer-backgrounds)
record the exact producer/consumer routes and the separate behavior of
composing underline and composing-tag style flags. This native selection
trace does not establish complete composition-span preview/PDF appearance.

Composer clips backgrounds at `0x380bf0`–`0x380c0c` through object virtual slot
168, resolved to `GetRect`, `0x37aa94`: the original geometry. The inset text
frame instead lives at stack offset 48 (`0x3805bc`–`0x380608`). Rust clips
placed backgrounds independently of glyphs. Shape clipping uses its original
geometry, rather than its inset text frame or a stale saved child frame.
`PlacedTextFrame.background_bounds`
retains this boundary through rotation and translation; glyph clipping is
unchanged.

Native theme context `0x6c39c` calls virtual slot 80 on its current theme
object at offset 8. Light (`0xe54f8`) and high-contrast (`0xe54b4`) themes
retain source ARGB. Dark (`0xe51a4`) preserves alpha, converts RGB to HSL at
`0xe51d0`, and reverses lightness only outside the inclusive `[0.4,0.6]`
interval (constants at `0x41734`/`0x41724`). Rust uses that primitive and selects
its dark-background flag once from the resolved page, preserving it through
local table/code/shape surfaces. The exact native caller policy for selecting
the theme remains unproven.

Composing-background fallback tests the mapped ARGB after that conversion.
Native dark conversion maps transparent black `0x00000000` to `0x00ffffff`,
and transparent white `0x00ffffff` to zero. Checking raw zero or alpha before
mapping therefore does not reproduce the native fallback decision. Enabled
composing tags can also background-paint native object entries: their converter
and the native entry background painter have no object guard. Rust's text
background geometry includes supported SVG/replay inline/block object bands,
with ordinary backgrounds also retained in document PDF Frame contexts
(standalone, table and code). Body document PDF excludes object backgrounds,
matching its native outer caller. Missing or ambiguous reordered placements
report object-owned `UnsupportedBackgroundPositioning`; valid composing tags
on supported objects have no unsupported-style diagnostic. The
[object background capture](text-draw-identity-findings.md#captured-embedded-object-background-geometry)
executes native measurement, placement and preview rectangle painting for 40
supplied cases. It establishes margin-inclusive inline bands and visible-width
block bands, separately from Widget conversion and full line-metric production.
The [retained object-run capture](text-draw-identity-findings.md#captured-retained-object-runs)
connects native object measurement and placement to ordinary-background/layout
fields with supplied upstream shaped glyphs. It does not execute native font
selection, shaping, complete Drawing object conversion or PDF painting.
The [export caller capture](text-draw-identity-findings.md#captured-object-export-caller-policy)
separately verifies Body object exclusion, Table/Code/Placed background calls
and foreground-versus-background alpha gates with supplied retained records.

The hash-checked HF corpus contains one kind-17 span: `04-marker4-highlighter`
body range `[2,6)`, selecting `Text`, with an eight-byte all-zero payload.
Independent Rust lopdf inspection of the paired native PDF Form 5 confirms
selectable `Text` at actual-viewport `(47.999998855590825,218.70000000000002)`,
with no background rectangle operators. The PDF's colored stroke images are
separate pen content. This is evidence for transparent-span suppression,
not opaque background geometry or color parity. No captured nonzero-alpha
background span exists in these four fixtures.

Terminal LF/control behavior, complex-script character-to-cluster background
mapping, complete cross-context object-span painting, and captured opaque/semitransparent
backgrounds remain unverified. Native object opacity is also separate from
the span alpha currently represented by the SDK. Proportional per-character
subdivisions and fitted rectangles have no native proof.

The [native PDF alpha capture](text-draw-identity-findings.md#captured-pdf-alpha-transport)
separately executes paint color/alpha transport. Legacy Table foreground uses
a signed source-alpha branch before writer opacity; Code foreground uses
writer opacity alone; background alpha uses an unsigned source-alpha product.
An alpha setter replaces the paint's existing alpha byte. Rust's RGB glyph
paint does not implement these distinct source-alpha rules. The public
Standard exporter and newer native exporters also select different writer
routes; bounded helper transport is not complete page-appearance evidence.

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

Rust resolves glyph coverage before measurement and retains the selected face
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
(`0x3a1734`–`0x3a175c`). The getter is directly verified in
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

Entirely empty text separates measured height from gravity height. Fresh
`RichTextMeasure::Measure` clears entries and succeeds (`0x78530`–`0x78624`);
`DoLayout` returns before paragraph layout or height storage
(`0x715e8`–`0x715f4`, `0x722e4`–`0x72300`). Fresh measured height and line
count remain zero; Drawing initializes height member 308 to zero at `0x636b8`
and `GetHeight`, `0x63978`, reads it.

Gravity instead uses `RichText::GetTextSize(0)` plus raw top/bottom margins
(`0x6490c`–`0x64934`). `GetTextSize`, `0x62280`, resolves the caret font and
paragraph spacing: nonzero pixels give `F + pixels`; otherwise it computes
`fma(F, multiplier - 1, F)` (`0x622dc`–`0x6231c`). Without paragraph records
it uses `F * 1.35f` (`0x62324`–`0x6232c`, constant `0x26660`). Thus F20,
default multiplier 1.35 and a height-100 frame give empty center offset 36.5,
while measured height stays zero. A matching zero-length font span can change
that gravity offset. Paragraph before/after gaps do not enter this empty
gravity branch. Empty paragraphs inside nonempty text still use layout.
Native spacing-enable flags, list state, object margins and obstacles remain
additional inputs. The ordinary placed caller uses the shared baseline path
traced below.

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
ordinal ranges to these records. Subsequent paragraphs start at the
separator's own UTF-16 index (`0xd684c`, `0x914a0`), and terminal separators
still append a paragraph. For `A\n`, the two native records are start/count
`0/1` and `1/1`; `\n` retains initial `0/0` plus separator-only `0/1`.
In raw `a\r\nb`, `b` belongs to ordinal 2, not 1, and the intervening CR-only
paragraph has layout height. Neither inspected producer coalesces CRLF.
Whether the Samsung editor saves raw CRLF or normalizes it during text entry
is not established by the reference fixtures.

The separator-only paragraph resolves its font at `start + 1` using
`GetFontSize` (`0x78ab4`), including the terminal caret; a separator followed
by content uses that content's resolved `GetSpan` font (`0x78aa4`). The
initial zero-length paragraph inside nonempty text takes the first-empty-line
helper (`0x727e0`–`0x729ec`, `0x6bb3c`) and inserts a line with native source
sentinel -1/-1 (`0x731a0`–`0x731bc`, `0x745e4`). Ordinary separator lines
retain zero advance and real height. Drawing flushes preceding glyphs and
skips the separator itself (`0x66f48`–`0x670bc`). With F20, multiplier 1.35,
zero margins and no bullets, objects or obstacles, `A\n`, `\nA` and `\n`
each have two lines, height 54 and baselines 20/47; `\n\n` has three lines,
height 81 and baselines 20/47/74. These are source-backed metric cases,
not captured clipping or selectable-empty-line references. Positive-start
capture-window LF removal remains the separate `CopyText` rule below.

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
embedded objects. These branches do not establish complete first/last-line
parity for paragraph edges, bullets, empty lines and page limits.

The ordinary placed route uses this same helper: Drawing `0x80c80` calls
`ScrollEditTextView::Measure`; Widget `0xbc7f0` calls its layout, which forwards
to `TextLayout::Layout` at `0xbca04`. Text forwards through
`RichTextDrawing::layout` (`0x8ac88`), `RichTextLayout::DoLayout` (`0x64774`),
paragraph layout (`0x71aec`, `0x72e50`, `0x73ee0`) and `SetLayout` to
`GetBaseline` (`0x6b510`). Its ordinary branch has no placed/body switch.
Rust consequently uses line advance minus `0.35 * max_font_size` across
ordinary placed, shape, body, table and code text; object-margin lines retain
their separate native branch.

### Rust f32 local line bands

[`NativeLineMetrics`](../../crates/sdocx/src/render/text/native_line.rs) retains
f32 maximum font size, base height, pixel/percentage spacing and object-line
flags. Percentage spacing uses the native FMA order; pixel spacing uses its
separate addition. Placement adds top margin, computes the ordinary baseline
with FMA by `-0.35`, and adds the object `0.001f` epsilon afterward as separate
operations. Object-margin lines preserve their separate baseline branch.

Rust comparisons against all 162
[entry/run-bound cases](table-code-findings.md#retained-text-entry-and-run-bounds)
and 230 [cached-run cases](table-code-findings.md#complete-retained-text-run-emission)
check local top, bottom, baseline and post-cursor as exact f32 values. The
independent audit found 63 post-cursor differences between native execution and
the former formula that folded additions before narrowing; that count is an
audit observation, not a fixture JSON field. Separate native operations match
all compared local values. A focused object-leading second-cursor check
distinguishes folded `214.0019989` from native `214.0020142`.

The production cursor keeps line placement local to the text frame, then adds
the Model/frame Y origin in f64. This preserves successive local line advances
even when an extreme frame origin makes world f64 endpoints coincide. Gravity,
exclusions, paragraph/flow policy and frame-origin production remain separate.
Nonfinite native inputs/results retain bounded finite recovery and geometry
diagnostics. Production supplies an unbounded height limit; the finite-limit
kernel test is source-derived synthetic evidence, not a captured runtime limit
context. Native horizontal shaping, wrapping/ownership and complete world-frame
composition are not established by these local vertical-band comparisons.

The body origin is the scaled component top margin, without adding the stored
flow-page padding: Widget `updateBound`, `0xd71b0`–`0xd7220`, supplies these
margins directly, and Text starts its cursor from the top margin at
`0x71628`–`0x71630`. The captured first heading has top margin 30, enabled
before spacing 12, font size 45 and multiplier 1.6. Its baseline is therefore
`30 + 12 + 72 - 15.75 = 98.25`. The reference PDF's actual 848-point viewport
gives that value; the retained logical 848.333333-point canvas convention in
`conformance/text-metrics.json` gives 98.85. Their 0.6-unit difference is the
page-height conversion, not a layout offset. The SDK does not add flow-page
padding to this origin or adjust continuation top margins by a fixed offset.

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
advance. The final drawing origin has a separate producer, described below.
Native code feedback clears and remeasures its float rectangle
(`0xb0e14`–`0xb0e50`).

Comparisons of the locked five-page fixture place matched ordinary body and
heading baselines on the first four pages within 0.0001 SVG units of the actual
PDF viewport. Prepared numbered markers match the two locked marker baselines
within 0.000045 units. Numbered-item text X differs by about -0.04393;
[marker measurement](#list-marker-geometry) describes that limitation.
Four raw code lines are fully outside the native viewport,
independently verified from page/Form bounds, per-line clips and embedded
glyph outlines; their original coordinates remain recorded, while the test
requires no corresponding SVG text. These are exclusions from that reference
subset. The code/table drawing contracts below describe their final origins;
these references do not establish arbitrary pagination parity.

## List marker geometry

Widget `ObjectTextLayout::initBulletPoint` reads constants 123/124 at
`0xd9620`–`0xd9650`: button width 20 and trailing margin 6, multiplied by
document density and local layout scale. Content records are `0x8370` /
`0x8388`, both unit kind 3 and rounding kind 3. Widget point types 4/5/6/7
receive the same values (`0xd96dc`–`0xd9780`), and `measurePointBullet`
adds the two widths at `0xd1c6c` / `0xd1c88`–`0xd1c8c`. The export local
scale is 1, so circles and squares reserve 26/52/78 units at densities
1/2/3, independently of the text font or marker artwork.

The Rust flow adapter uses that shared point-marker reservation.
The captured paragraph 53, UTF-16 1317, starts its ordinary text at X126.
The nested circle starts at X174. Independent preview/replay tests
cover both circles, both squares, three densities and odd/even indent levels.
Raw Arrow/Diamond values also map to native point type 4 at
`0xd92c4` and use the same solid-circle vector in Rust. Native default nesting
is SolidCircle, WhiteCircle, BlackSquare, WhiteSquare (`0x63830`,
`0xdc634`–`0xdc664`), while explicit type lists
override it (`0xd9020`–`0xd9124`). The SDK's odd/even circle substitution
is not a complete implementation of those lists; malformed signed indentation
remains unverified.

Point artwork is vector geometry. Native resource IDs 38–41 select the
4 × 4 circle/square XML paths (`0xd96fc`–`0xd9798`): solid circle radius 2;
outline circle radius 1.75 with 0.5 stroke; solid square 4 × 4; outline square
3.5 × 3.5 inset 0.25 with 0.5 stroke. Rust emits typed SVG circles and rectangles
with the same open-marker border proportions. Image
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
offset. Rust prepares an owned marker source and shared `TextLayout` from
the first content span's resolved size. It measures glyph width, constrains the
child to its upward-rounded width, and reserves its fractional advance plus
the scaled 9/6 gap using the fresh-layout policy. The marker uses the default
sans face without inheriting paragraph family, bold or italic. Bundled Roboto's
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
are not covered by the current adapter.

## Measurement, body flow and embedded objects

### Run shaping and feature policy

Ordinary text is measured as runs, not by summing independent character
`SkPaint::measureText` calls. Text `ParagraphMeasureImplMinikin::Measure`,
`0x76e28`, joins adjacent spans with `RichTextSpan::JoinableForMeasureTo`
at `0x76fac`, then invokes `SpanRunFunctor` at `0x76fe0` / `0x77030`.
The joining predicate, `0x8dc28`, compares resolved font size, foreground
ARGB including alpha, style bits under mask `0xc3`, and nullable font-name
string equality; either
object span prevents joining. A color-only transition can therefore change
the native shaping boundary. Null and a nonnull empty name do not join, even
when a font resolver might choose the same face. Background/decorations and
hyperlink identity do not split this predicate. The
[measurement-identity capture](text-draw-identity-findings.md#measurement-identity)
executes the complete native kernel for 291 supplied cases, separately from
the full measurement producer and actual shaping.

Rust's ordinary measurement key now retains f32 resolved size, native-theme
mapped ARGB, nullable font name and supported bold/italic bits under that mask.
It is projected separately from glyph paint: hyperlink blue, display contrast,
backgrounds and decorations do not become accidental measurement identity.
Typed key regressions match 89 finite supported native predicate cases;
independent `AV` advances check the resulting shaping boundaries. Native font
selection and complete run shaping remain separate evidence limits.

Paragraph-heading style phases are also separate. Model
`CreateSpanList`, `0x419f3c`, and paragraph application at
`0x3eff64`–`0x3eff84` can create stored FontSize/Bold spans before runtime
measurement. The traced Widget predefined-style type 10 affects spacing.
The native factory capture below establishes those ordinary size/bold values;
the measurement-key regression itself does not execute Model editing.

### Predefined-style span factory

[`table-text-predefined-style.json`](../../conformance/table-text-predefined-style.json),
SHA-256 `3cde41d9e6077133f7382310fa2229aafe168bbaf32d0b5a34d93212376ca030`,
contains ten factory cases and five paragraph-application cases, repeated with
memory fills `0x00`, `0xa5` and `0xff`. Native `CreateSpanList`, `0x419f3c`,
produces an ordinary FontSize span followed by Bold, with interval type 3:

| Predefined value | Style | Font size | Bold |
| ---: | --- | ---: | --- |
| 0 | Heading 1 | 21 | true |
| 1 | Heading 2 | 19 | true |
| 2 | Heading 3 | 15 | true |
| 3 | Body | 15 | false |

Factory value 4 fails without appending spans. Seeded vectors preserve their
existing prefix and append new records; this tests construction order rather
than final overlapping style precedence. These are explicit predefined-style
source spans, not replacements for native ordinary default font size 17.

The [capture module](../../conformance/native_table/text_predefined_style.rs)
executes the actual global assignment window `0x41a54c`–`0x41a5ac`, reached
from native initialization at `0x41a4e0`, then complete factory construction and
range/interval/property getters. Size and bold values are read from the pinned
ELF's globals, rather than supplied by the harness.

Complete `m_ApplyPredefinedStyle`, `0x3efe7c`, uses actual paragraph construction
and getters with supplied paragraph-to-text index mapping and string length.
The first source paragraph retains the mapped start; later source paragraphs
advance that start by one. The end is the smaller of mapped end plus one and
text length. `AppendSpan` is intercepted at the call boundary to record these
ordinary span inputs before destruction. Paragraph-list generation is also
intercepted with an empty successful result. Model editing/history/storage,
native paragraph-list generation, Widget conversion, measurement and rendering
do not execute.

Rust preserves parsed size/bold spans, permits later explicit overrides, and
uses Heading 3 size fallback 15. It does not force heading bold off or create
Model editing spans while rendering paragraph metadata. Tests exercise native
factory span outputs at scales 1 and 3 and later explicit size 12/bold false.
Complete native editing or rendered heading appearance is not established.

### Shaped measurement output

`SpanRunFunctor` creates the Minikin paint through `0x76ad4` and obtains
a shaped layout through `0x97b34` at `0x77304`. The style-run virtual
method resolves to `0x96cc8` (vtable entry `0xf57b8`), which reaches the
layout-piece producer `0x9b150` through `0x970fc`. That producer calls
the bundled HarfBuzz shape entry `0xecdcc` at `0x9c5d4` / `0x9c754`.
It retains glyph positions, UTF-16 cluster indexes and character advances;
`SpanRunFunctor` copies the shaped per-UTF-16 advances divided by 100
at `0x77640`–`0x77668`. Paint size was multiplied by 100 at
`0x76b30`–`0x76b44`. Source indexes, glyph indexes and shaped clusters
are separate native inputs/results.

SPen initializes the Minikin feature string empty at `0x76c34`. For a
Latin script run with absolute letter spacing at most the double constant
0.03, the layout-piece producer
explicitly supplies `liga=0` and `clig=0` before shaping
(`0x9c548`–`0x9c754`). The 16-byte feature records at `0x267a0` and
`0x26750` contain the tags, value 0, start 0 and end `0xffffffff`.
Its larger-letter-spacing branch also adds these features
(`0x9b34c`–`0x9b3d8`). This proves disabling those two optional ligature
features for the inspected Latin path. Larger absolute spacing also supplies
those features for non-Latin chunks; at most 0.03, non-Latin chunks retain the
supplied feature string without that override. This does not disable required
script shaping or establish complete device shaping behavior.

No `kern=0` override was established in this producer/caller trace. The
captured heading matching an unkerned bundled-font width is useful fixture
evidence, but does not prove a universal native kerning setting. Font choice
and complete native style-run/shaping profiles remain separate evidence limits.

### Native shaping numeric domains

The backend font advance is already a measured value. Text's scalar/vector
font callbacks, `0x8868c`/`0x8882c`, configure Skia paint through `0x88760`
and call `SkPaint::getTextWidths`, `0x1cdaac`. Skia converts its cached glyph
advance from fixed 16.16 to f32 at `0x1cdc18`–`0x1cdc28`, then applies any
measurement normalization scale. Nonlinear FreeType metrics store the 26.6
slot advance shifted left by ten at `0x27e360`–`0x27e37c`; linear/matrix
metric paths use separate fixed-16.16 calculations. This backend quantization
precedes the HarfBuzz callback conversion below. Backend ink bounds round
outward with floor/ceil at `0x27e2c8`–`0x27e2f4`.

Skia's measurement helper, `0x1ccdb0`, normalizes to paint size 64 when the
LinearText flag is set or transform squared length exceeds 4,194,304.
For ordinary scale-X one/skew zero, the latter means paint size above 2,048.
That branch changes hint/subpixel settings and returns normalization scale
paint-size/64 at `0x1ccefc`–`0x1ccf88`. It does not describe every ordinary
size or hinting profile.

HarfBuzz also receives paint-scaled integer coordinates before shaping.
For paint size `P` and horizontal scale `s`, the layout-piece producer
rounds `f64(P) * f64(s)` back to f32 at `0x9b62c`. It sets scale-Y to
`trunc_i32(scalbnf(P, 8))` and scale-X to the corresponding conversion of
that rounded product (`0x9bf18`–`0x9bf44`), then refreshes the font at
`0x9bf4c`. The separate ppem values truncate `P` and its f64 horizontal
product to unsigned integers. With horizontal scale one, shaping therefore
uses a 24.8 paint-unit integer domain. GPOS operates in that domain;
rescaling or rounding a final UPEM-domain advance cannot generally recover
its intermediate quantization.

The source-traced scalar horizontal-advance callback at `0x9d754` multiplies
the f32 font advance by 256, promotes that result to f64, adds 0.5 and
truncates to an integer. Its vector counterpart at `0x9d858` uses
`scalbnf(advance, 8)` and integer truncation without the addition. Both
callbacks are installed at `0x9d618`; their distinct conversion paths are not
one interchangeable rounding rule.

After HarfBuzz shaping, native positions/advances convert signed integers to
f32 and apply `scalbnf(value, -8)` at `0x9cd48`–`0x9cd50` and
`0x9c918`–`0x9c938`. Owner accumulation at `0x9cd6c` uses f32 additions;
`SpanRunFunctor` divides by 100 at `0x77658`. These are separate arithmetic
stages in the 100-scaled paint domain, not pixel-grid rounding.

Native TextPaint construction at `0x7bb14` reaches Skia's paint constructor,
`0x1c9c14`, which initializes bitfield member 104 to `0x08000000`. The
Text getter at `0x7c1f4` supplies packed Minikin paint value `0x20000` at
`0x76ca0`: low flags are zero and the high hint value is two. The value three
at `0x76b28` selects glyph-ID encoding through
`TextPaintImplSkia::setTextEncoding`, not a kerning/paint flag.

These instruction findings establish the conversion sequence and defaults.
They do not capture actual native glyph advances or establish a universal
width correction. Rust's compatibility paragraph producer shapes in font
units, accumulates its pen as integers and projects geometry with f64
font-size/units-per-em scaling. The bounded native-paint paragraph adapter
below uses separate captured paint-domain stages. Both retain logical owner offsets
and transport advances in `MeasuredGlyph`, shared by PDF, SVG, viewport ink
and marker consumers; browser reproduction independently compares raw
font-unit shaping. Local f32 line-band parity alone does not establish the
native horizontal numeric stages in document layout.

### Captured native shaping

The [Rust capture](../../conformance/native_table/text_shaping.rs) and
[`table-text-shaping.json`](../../conformance/table-text-shaping.json)
execute the native file-font constructor, `FontFamily` (`0x8609c`),
`FontCollection` (`0x8f7a8`), `LayoutPiece` (`0x9b150`), bundled HarfBuzz
shaping (`0xecdcc`), Skia and bundled FreeType. The fixture SHA-256 is
`a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b`.
Its 16 cases produce 16 HarfBuzz calls, 48 glyphs, 49 UTF-16 character
advances and 48 raw Skia ink bounds. Three allocation fills (`0x00`, `0xa5`,
`0xff`), a repeated zero-fill run and an independent replay agree byte for
byte.

Inputs select one supplied Roboto Regular face, index zero, with SHA-256
`56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d`.
Native getters verify source ID 1, no CBDT bitmap flag, empty raw font
language, weight 400 and italic false. Family locale `en-Latn`, Minikin paint,
UTF-16 source/range, direction and zero hyphen edits are supplied caller
inputs. The cases cover `AV`, `To`, `office`, `fi`, combining marks,
supplementary source units, paint sizes at/above the normalization threshold,
hint/linear flags, RTL, partial source context, skew, requested ligatures and
letter spacing. They do not select Samsung's device Roboto or exercise
system fallback.

Each HarfBuzz call records scale/ppem, source codepoints and clusters,
direction/script/language, context, feature records, resulting glyph IDs and
integer positions. Layout-piece output records full and owner positions,
UTF-16 owners, character advances, ink bounds, extents and semantic font
fakery bits; the latter are zero in this suite. All 16 calls use the vector
advance callback, producing 48 raw/quantized metric samples. No scalar
advance callback executes, so its distinct rounding remains a source finding.
For the default-profile `AV` case, supplied paint size 1700 produces
HarfBuzz scale 435200 and ppem 1700. Native Skia advances are 1109 and 1082;
post-shaping advance integers are 265416 and 276992, giving layout-piece
advance 2118.78125 in paint units. The no-hinting and LinearText profiles
both advance 2118.35546875. The profile therefore affects measured output even
with the same font and source.

The native ICU loader resolves suffix 76 through its fallback search, but
`dlopen` supplies hash-pinned host ICU 76.1 code/data with Unicode 16.0.
The four exercised ICU APIs are character type, locale canonicalization,
likely-subtag expansion and language-tag conversion. Bundled HarfBuzz Unicode
tables and shaping execute unchanged; they are separate from these host
Minikin property/locale services. Host `qsort` makes 48 calls and executes
2,278 allowlisted native leaf-comparator invocations in a separate emulator.
Allocation/file/memory services, single-thread synchronization, numeric feature
parsing, formatting and libm are host boundaries. The fixture pins their
library hashes and versions and records the nine native initializers used.

The [Rust shaping comparisons](../../crates/sdocx/src/render/fonts/native_shaping_tests.rs)
pass all 16 cases through public `ResolvedFace::shape` with the recorded
pre-shaping source infos, properties, features and context. All 48 glyph IDs
and UTF-16 owner starts match; native layout-piece owners equal the recorded
clusters minus requested start. Controls detect optional-ligature changes and
incorrect UTF-8 cluster offsets. The partial-context case observes one preceding
and one following scalar. These comparisons establish identity for those
inputs, rather than native metric equality: default-profile `AV` has Rust
font-unit advance 2552, whose scaling by `1700 / 2048` gives 2118.359375,
distinct from the captured native 2118.78125.

This establishes the supplied single-face layout-piece boundary. It excludes
device ICU/font configuration, font-manager reuse, bitmap/fallback fonts,
complete library initialization, the whole `SpanRunFunctor`, wrapping,
document composition, pixels and vector output. Rust's production horizontal
metrics remain unchanged; this capture does not establish their native parity.

### Captured post-shaping numeric geometry

[`table-text-shaping-numeric.json`](../../conformance/table-text-shaping-numeric.json)
extends the same native producer without changing the original 16-case fixture.
Its SHA-256 is
`1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455`.
The 17 cases produce 19 HarfBuzz calls, 182 glyphs, 182 UTF-16 advances,
182 raw Skia bounds and 182 vector advance samples. It uses the same supplied
Roboto face, native library hashes, host services, three allocation fills and
repeated zero-fill check described above.
The public raw-font-unit shaping comparison also matches all 182 captured
glyph IDs and normalized UTF-16 owners across the 19 calls, including the
three separate script chunks. This identity check does not compare advances.

Six cases exercise large paint scales; `long_pen` captures repeated `AV` pairs
at paint size 10000. `mixed_scripts` executes separate Latin, Greek and Cyrillic
chunks rather than treating the source as one Latin call. Eight combining-mark
cases isolate LTR/RTL ordering, skew `-0.1234567`, positive/negative spacing
`0.04` and the smaller `0.025` spacing profile. `supplementary_partial` starts
the UTF-16 request at two, after an emoji, and records context outside the
requested range. The emoji is context, not a captured fallback glyph.

The callback still uses only the vector metric path. Host `qsort` makes
57 calls and executes 2,662 allowlisted native comparator invocations. This
extension captures numerical sensitivity and additional producer inputs;
it does not establish device font selection, the complete paragraph producer,
wrapping or production Rust metric equality.

The [typed post-shaping geometry](../../crates/sdocx/src/render/fonts/paint_layout.rs)
accepts native HarfBuzz integer output and raw Skia bounds as supplied inputs.
Its [fixture comparisons](../../crates/sdocx/src/render/fonts/paint_layout/fixture_tests.rs)
check exact f32 bits for full/owner positions, ink bounds, character advances
and total advance: all 16 original cases and all 17 numeric cases,
covering 230 glyph placements and 231 UTF-16 advance slots. The three-chunk
`mixed_scripts` case retains the separate Latin, Greek and Cyrillic calls.
The isolated vector advance conversion also matches all 230 raw-to-quantized
callback samples across both fixtures; scalar callback rounding remains
unexecuted.

The model preserves integer-to-f32 narrowing before division by 256,
step-by-step f32 pen and owner accumulation, f64 spacing additions narrowed
at each native store, and fused skew offset calculation. In `large_to_2000`,
advance integer 28076755 narrows to 28076756 before scaling; computing the
division in f64 gives a different result. In `long_pen`, accumulating scaled
advances in f64 and narrowing only the final sum changes the captured total.
The corresponding native stores occur at `0x9c780`–`0x9c7cc` (initial half
spacing), `0x9c8bc`–`0x9c8fc` (cluster transition spacing), `0x9cab0` (fused
skew), `0x9cb60`–`0x9cb70` (owner-relative subtraction then offset addition),
`0x9cc5c`–`0x9cc88` (ink offsets), `0x9cd48`–`0x9cd70` (converted owner
advance), `0x9cda0` (cursor accumulation) and `0x9cdb0`–`0x9cdf4` (final
half spacing).
Other controls expose incorrect skew and UTF-8 ownership. Unsupported
scripts, incompatible or noncontiguous chunks, nonzero word spacing, vertical
advance, extra font slots/fakery, malformed owners/ranges and nonfinite geometry
return typed unavailability. These supplied-input regressions isolate the
bounded arithmetic layer from metric production and paragraph integration.

### Captured horizontal GPOS scaling and fused skew

[`table-text-shaping-gpos.json`](../../conformance/table-text-shaping-gpos.json),
SHA-256
`9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2`,
records four cases through the same supplied-font producer and host boundaries.
It contains four HarfBuzz calls, 12 glyphs, 12 UTF-16 advance slots, 12 raw
metric/bounds samples, four horizontal GPOS adjustment events and 12 local
fused-skew traces. The original and numeric fixtures remain separate.

The capture observes the actual `ValueFormat=4` horizontal `x_advance` path:
`0xd013c` multiplies the signed 16-bit table value by the font's signed 16.16
multiplier; `0xd014c` logically shifts the 64-bit product register right 16;
`0xd0150` exposes the low 32-bit delta; `0xd015c` follows the destination update.
Source byte offsets
identify values inside the supplied font's GPOS table, which starts at
`0x71bf0`. The `AV` value at `0x79464` is `-87`; `To` at `0x79760` is `-99`.
For default `AV`, multiplier 13926400 gives product `-1211596800`, adjustment
`-18488` and first-glyph advance `283904 -> 265416`. Interpreting the low 32
bits as a signed delta equals flooring the negative fractional adjustment for
these captured products. Adding 32768 before shifting changes every one of
the four captured deltas.

The [GPOS regressions](../../crates/sdocx/src/render/fonts/native_gpos_tests.rs)
verify the fixture/font hashes, source bytes and GPOS table bounds, glyph/owner
identity, scale-derived multiplier, executed product, shifted delta and stored
position. The pre-adjustment advance also equals the actual recorded vector
callback result. These tests establish this horizontal value path for the
captured pairs, not every GPOS value format, anchor, device/variation correction
or every positioning operation accepted by a font library.

The skew trace reads operands at `0x9cab0` and the actual fused result at
`0x9cab4`, before adding the horizontal pen. In `pair_combining_fma`, offsets
47.67578125 and -8.36328125 with skew bits `0xbc0000f5` produce native result
bits `0x423e7117`. Separately rounding the multiplication and subtraction
produces `0x423e7118`. The [typed geometry controls](../../crates/sdocx/src/render/fonts/paint_layout/fixture_tests.rs)
compare the local operation directly, since a later pen addition can erase
that one-step difference. Across the three fixtures, the post-shaping model
matches 37 supported cases, 242 glyph placements and 243 UTF-16 advance slots;
the vector conversion matches 242 callback samples. Native glyph output and
raw bounds remain supplied inputs to these isolated arithmetic regressions.

### Bounded Rust paint metrics

[`ResolvedFace::paint_metrics`](../../crates/sdocx/src/render/fonts.rs) exposes a
reusable [Skrifa-based metric provider](../../crates/sdocx/src/render/fonts/paint_metrics.rs)
for raw glyph advances and ink bounds in paint units, before HarfBuzz
positioning. `PaintMetricInput` selects `Normal`, `Unhinted` or `Linear`,
paint size, horizontal scale and skew. The provider accepts finite paint sizes
in `[1, 8388608)`, horizontal scale one, finite skew and static `glyf` faces
without `fvar`, `CBDT` or `VARC` tables. Nonzero skew uses original simple-glyph
points; skewed composite glyphs return `UnsupportedComposite`. Zero-skew
composites retain the Skrifa scaler path. Unsupported transforms/fonts,
invalid glyphs, scaler failures and out-of-range ink return typed errors.

For zero-skew `Normal` below the normalization threshold, advances come from
the hinted outline's adjusted metrics. The captured FreeType load flags
`0x120208` select `FT_LOAD_TARGET_MONO`; the public paint mode's `Normal`
name does not select FreeType's normal hint target. Rust uses Skrifa's
`Target::Mono` for this captured profile. Nonzero skew disables hinting.
Unhinted advances retain the linear 16.16 metric through the signed residual
matrix product before converting to f32. `Linear` or either transform-column
squared length above 4,194,304 measures at backend size 64, rounds ink outward
there and applies the f32 normalization factor `P / 64` afterward. Skewed
simple-glyph points preserve separate fixed-point product rounding before
bounds are rounded outward. Reusing `PaintMetrics` retains the font's outline
collection, linear metrics and hinting instance across glyphs.

The [metric regressions](../../crates/sdocx/src/render/fonts/paint_metrics/tests.rs)
match exact f32 bits for 401 native raw advances and 1604 ink coordinates
across 74 cases in the original, numeric, GPOS, Skia matrix, logical-entry and
fractional hinting fixtures. Controls distinguish the unhinted `V` advance
1081.591796875 from its unhinted outline advance 1081.59375 and the default
hinted advance 1082; they also check the normalization threshold, inkless
space, zero-advance marks and invalid/unsupported inputs.

This validates the supplied Roboto backend metric boundary for those profiles.
The provider does not shape glyphs, reproduce GPOS, select device fallback or
replace `ParagraphMeasurer`. Its acceptance of static `glyf` fonts is an
API domain, not arbitrary-font equality; the named-face capture below supplies
separate proof for three additional pinned Roboto styles. Production
paragraph admission and wrapping have separate contracts below.

### Captured Skia residual matrices and outline metrics

[`table-text-shaping-skia-metrics.json`](../../conformance/table-text-shaping-skia-metrics.json),
SHA-256
`c8aa3d08839e9f7e7ddff6d9200616faa11adb67f8a7124f7fd91d5088c9eec0`,
records 18 supplied-Roboto cases, 108 raw callback advances and 432 ink
coordinates. It executes the same font/layout-piece producer and host
boundaries as the original fixture, with three allocation fills and a repeated
zero-fill check. Cases cover both skew signs, fractional paint size, size one,
near-zero skew, LinearText and the skew-sensitive normalization threshold.
Additional hooks retain scaler configuration, fixed cached advances and actual
FreeType outline points. The outer-transform pre-hook executes for 90
nonidentity loads. For identity-matrix loads, no transform executes and the
before-outline is the same post-load snapshot as the after-outline.

For backend size `P`, Skia's residual float matrix at
`0x2049dc`–`0x204a2c` preserves successive f32 products: diagonal
`P * (1 / P)` and off-diagonal
`(P * skew) * (1 / P)`. Fixed conversion at `0x27d2e8`–`0x27d310` multiplies
by 65536 and truncates, with the off-diagonal sign flipped. At paint size 1700
and skew -0.25, the diagonal is therefore 65535 and off-diagonal 16383,
rather than 65536 and 16384. The second-column normalization test at
`0x1ccea4`–`0x1cced4` includes the squared skew product. Hint filtering
at `0x27cde4` / `0x27cf44` clears hints for nonzero skew.

Cached linear advance multiplication at `0x27e318`–`0x27e320` and
`0x27db54`–`0x27db68` retains the signed 16.16 source integer through the
truncating matrix operation. For glyph `x` at size 1700/skew -0.25,
source advance 55216000 becomes cached 55215157. Reconstructing that source
integer from an already rounded f32 advance can lose bits. FreeType's vector
transform at `0xfed38`–`0xfed90` separately rounds each fixed-point product
before addition; transforming already synthesized curve midpoints can change
the outward ink bound.

First-use cached fields are recorded as null until an executed native store;
subsequent before-values are checked against that observed store. Raw
allocation bytes are not interpreted as initialized glyph metrics. This
capture excludes pixels, other fonts, bitmap/variable fonts and complete
document composition.

The Rust metric regressions independently reproduce 102 fast
`FT_Get_Advance` stores, 102 linear `FT_Load_Glyph` stores and 1105 original
plus 1105 transformed points, with 4420 exact coordinate comparisons.
Original-point reconstruction excludes the 12 normal hinted load records.
Independent capture inspection additionally checks all 1235 captured
before/after point pairs against fixed-matrix arithmetic, separately from the
Rust unhinted design-point reconstruction.
Controls distinguish the signed half-product rounding, loss of low fixed
advance bits in f32, and tiny nonzero skew that disables hinting even when
the fixed off-diagonal is zero.

### Captured fractional paint hinting

[`table-text-shaping-entry-skia-metrics.json`](../../conformance/table-text-shaping-entry-skia-metrics.json),
SHA-256
`c7216fb3148f524a8e7cde5c0bb83a616c60ae81c3cedb7c7f2d266f1bd3800a`,
records 11 cases, 11 shape calls, 26 glyphs/UTF-16 advances, 11 scaler
configurations, 50 FreeType glyph loads and two fast advances. It reuses the
Skia metric hooks and supplied-Roboto producer above. Three allocation fills,
repeated zero-fill and independent replay agree.
The loads retain 800 before/after point pairs; their identity matrices leave
those points unchanged. These snapshots are separate from Rust design-point
reconstruction and its comparisons in the matrix suite.

Supplied raw f32 paint sizes include 1712, 1712.25, 1712.49, 1712.5,
1712.51, 1712.75 and 1713, with combining-mark, RTL and unhinted controls.
The captured Normal profile uses FreeType flags `0x120208`, selecting the
Mono hint target. At size 1712.5, glyph 84 (`o`) has maximum outline X
57602 / 64 = 900.03125, so its outward right bound is 901. Skrifa's smooth normal
hint target produces 900 instead. The Rust provider uses the captured Mono target
without altering the supplied font bytes.

The configured Rust hinter also matches all 192 fixed-coordinate extrema from
48 actual hinted FreeType loads, before outward bbox rounding. This compares
the four bounds coordinates for each identity-transformed load, separately
from complete ordered point-stream equality. Unhinted loads remain a separate
profile control.

Paint size is a supplied raw scaler input in this suite; the source-size field
is informational. Source-size multiplication by 100 is established separately
by the logical-entry capture. The metric suite does not extend that caller
proof or establish device font selection, raster output or document composition.

### Captured mixed-script chunk geometry

[`table-text-shaping-mixed-scripts.json`](../../conformance/table-text-shaping-mixed-scripts.json),
SHA-256
`00d1634d29634fac146b42baa1e1a349bd66a2a74724ca7e7d5c539f116703ad`,
records 33 cases, 380 actual HarfBuzz calls, 800 glyphs and 800 UTF-16
advance entries. The same supplied-Roboto layout-piece producer and host
boundaries capture Latin/Greek/Cyrillic chunk discovery, spacing, RTL,
supplementary partial ranges, long accumulation, combining marks and script
reentry. Three allocation fills, repeated runs and independent processes
produce identical bytes; the four earlier fixtures replay unchanged.

For these supplied RTL pieces, script calls remain in ascending source order;
glyph owners reverse within each call. The native f32 pen continues across
calls, while owner origin and leading/trailing half-spacing follow each call's
boundary. The [Rust chunk regressions](../../crates/sdocx/src/render/fonts/paint_layout/fixture_tests.rs)
match all 800 glyph placements and advance entries from captured shaping
inputs. Public producer/layout comparisons derive the font metrics and also
match those outputs. Negative controls detect flattening chunk origins,
summing independent chunk totals and reversing the RTL call order.

This establishes the captured single-face chunk arithmetic, separately from
whole-paragraph bidi resolution, font fallback, full `SpanRunFunctor`, entry
conversion, wrapping and document composition.

### Captured script itemization

[`table-text-shaping-itemization.json`](../../conformance/table-text-shaping-itemization.json),
SHA-256
`eddd4290eb75ab124b82c07814d4354debe5af8a167cb0535a83541238dff710`,
records 44 cases, 314 native script queries, 102 shape calls, 252 glyphs and
264 UTF-16 advance entries. The
[capture](../../conformance/native_table/text_shaping/itemization.rs) observes
the actual layout-piece inline script loop: first query/result at
`0x9bfd0` / `0x9bfd4`, subsequent query/result at `0x9c048` / `0x9c04c`,
and finalized chunk range/tag at `0x9c0c4`.

Native uses the bundled HarfBuzz plain Script property. Common/Inherited
characters join a neighboring strong-script chunk; a Common/Inherited prefix
adopts the following strong script, and an entirely Inherited chunk becomes
Common. This is separate from Script_Extensions, bracket-pair resolution or
paragraph bidi analysis. Captures cover punctuation/digits/spaces, combining
extension marks, emoji/variation/joiner, script reentry and scalar-aligned
partial ranges in both supplied directions. Each HarfBuzz buffer receives up
to five preceding/following scalars from the complete source, including
outside the requested range.

The bounded [Rust `PaintItemization`](../../crates/sdocx/src/render/fonts/paint_itemization.rs)
uses the private HarfRust script table and the captured chunk loop, with checked
UTF-16 boundaries through `TextIndex`. Its immutable chunks retain absolute
byte/scalar/UTF-16 ranges, scalar infos and full-source context. The
[itemization regressions](../../crates/sdocx/src/render/fonts/paint_itemization/tests.rs)
compare native script-query results and exact chunk/context inputs. The API
can identify other script tags; paint shaping supports only the bounded tags
below. Version-independent Unicode properties, malformed UTF-16 behavior,
device fallback and general font coverage remain outside this evidence.
Observed all-neutral missing glyphs remain captured outputs.

### Captured cache-word context windows

The [Rust capture](../../conformance/native_table/text_context_windows.rs) and
[`table-text-context-windows.json`](../../conformance/table-text-context-windows.json),
SHA-256
`07d5f3c44ae387975f8070fe24dbe90049395a5ad8f1ff4e80f4780c36417d5d`,
execute the native UTF-16 separator predicate (`0x9b050`), previous/next
scans (`0x9af90` / `0x9b0b0`), directional iterator (`0x98168`) and bounded
caller TextView window (`0x96f94`–`0x96fd8`). All 65,536 raw UTF-16 units,
including surrogate units, are queried through the predicate and `A-unit-B`
scan stencils. The fixture also records 32 valid source cases, 215 offset
queries, 1390 selected LTR/RTL subranges and 2564 actual iterator windows
and emitted source views. Three memory fills, repeated zero-fill and an
independent process agree byte for byte.

Native separators are U+0020, U+2000–U+200A, U+200E–U+200F,
U+202A–U+202E, U+2066–U+2069 and U+3000. The scans separately isolate
U+3400–U+9FFF. These are cache-word boundaries, separate from Unicode line
or grapheme breaking. Direction reverses visitation in the captured cases;
the source-ordered window partition is the same. The emitted view retains
the entire cache word around the selected range, so context can cross a
style or bidi selection inside that word. It does not establish whole-paragraph
HarfBuzz source/context or the narrower partial-measure fast path.

Separately inspected paragraph caller instructions form the paragraph pointer
as document base plus twice its UTF-16 start (`0x78b3c`), retain its length
(`0x78ad0`, `0x76670`–`0x76674`) and skip the preceding separator for later
paragraphs (`0x78a80`). Style selection subtracts that paragraph origin
(`0x771cc`–`0x771d0`); final owners add the global style start (`0x773a4`).
The partial fast path builds a requested-subrange view instead
(`0x78f14`–`0x78f48`) and is excluded from normal paragraph activation.

[`PaintContextWindows`](../../crates/sdocx/src/render/fonts/paint_context.rs)
retains immutable paragraph-relative source/selection ranges and each local
TextView selection. Its bounds are 262144 source bytes, 65536 UTF-16 units,
16384 selected scalars and 128 windows; source and selection endpoints must
be scalar-aligned. Its [regressions](../../crates/sdocx/src/render/fonts/paint_context/tests.rs)
match the exhaustive predicate/scans and all directional windows/caller views,
with checked-range and budget controls. The capture supplies paragraph views and selection/direction;
full `measureParagraph`, cache lookup, `LayoutPiece` and HarfBuzz do not execute.

### Bounded Rust paint shaping

[`ResolvedFace::paint_shaper`](../../crates/sdocx/src/render/fonts.rs) constructs
a reusable [paint-sized shaper](../../crates/sdocx/src/render/fonts/paint_shaping.rs)
with the metric provider above. It shapes caller-supplied Latin, Greek,
Cyrillic or Common chunks at the native signed 24.8 paint scale. Requests
retain the full source, contiguous UTF-16 scalar starts, direction, script,
language, buffer flags, cluster level, features and bounded preceding/following
context.
Results retain glyph IDs, UTF-16 owners, all four integer positioning fields,
unsafe-break flags, raw backend metrics, source range and paint/scale metadata.
Glyph metrics are cached for the reusable font/paint instance.

The SDK packages a private copy of HarfRust 0.13.3 with its upstream license
and [source provenance](../../crates/sdocx/src/render/harfrust/PROVENANCE.md).
Its default integer scaling retains upstream rounding. The explicit `Floor`
policy used by paint shaping removes the 32768 bias before the fixed-point
shift, matching the captured native horizontal GPOS adjustment. Public
`ResolvedFace::shape` remains the raw font-unit Rustybuzz API.
The [scaling regressions](../../crates/sdocx/src/render/fonts/native_scaling_tests.rs)
match 242 native integer glyph-position records across the three shaping
fixtures with supplied native callback advances. A default-rounding control
produces `AV` first advance 265417, while the explicit floor policy reproduces
native 265416. These supplied-advance comparisons isolate font-table scaling;
they do not prove the metric producer.

The [producer regressions](../../crates/sdocx/src/render/fonts/paint_shaping/tests.rs)
instead derive metrics from the Rust provider and match all 558 HarfBuzz calls
and 1453 glyphs across eight fixtures: seven shaping suites and the logical-entry
capture below. Glyph IDs/UTF-16 owners, integer positions, raw advances and raw
ink coordinates match exactly.

`PaintShapedRun::layout` and `PaintLayout::from_runs` apply the shared
[post-shaping geometry](../../crates/sdocx/src/render/fonts/paint_layout.rs)
to immutable shaped results and supplied letter/word spacing. The result
retains full and owner-relative f32 glyph positions, shifted ink, request-relative
UTF-16 owners, per-UTF-16 advances and total advance in paint units, with
read-only access to source/range and geometry. Producer-derived comparisons
match all 1453 glyph placements across 151 supplied-Roboto cases, including
mixed Latin/Greek/Cyrillic and ten Common chunks. Supplied-integer comparisons
remain independent arithmetic checks over the same runtime implementation.
Chunks require the same source contents, font SHA-256/face index, paint,
scale and direction, with adjacent ranges in ascending source order.
The shared f32 pen continues across calls; the owner origin resets per call,
and trailing/leading half-spacing stores remain separate. Unsupported scripts,
font slots/fakery, nonzero word spacing and vertical advances return errors.
F64 spacing additions narrow at each store, and skew remains fused.
The logical-entry conversion below is a separate operation on this paint layout.

The paint shaper rejects malformed source ownership and invalid properties,
and bounds input to 262144 source bytes, 65536 UTF-16 entries, 16384 scalar
infos, 256 features and five context scalars on each side. Engine limits cap
glyph expansion at twice the input scalar count, with a minimum of one and
maximum of 32767, and tracked shaping operations at 2000000. Exhaustion
returns `ShapingBudget`. These counters are engine resource limits, not a
wall-clock deadline. Fonts containing GPOS Device/VariationIndex
corrections or hinted contour-point anchors return an unsupported-positioning
error; native ppem metadata is retained separately from the shaping scale.
Unverified parent glyph-extents or vertical callbacks also return explicit
errors instead of borrowing the library's default metrics.

A [numeric positioning guard](../../crates/sdocx/src/render/fonts/paint_shaping/positioning_domain.rs)
checks actual expanded glyph IDs and measured integer advances after GSUB,
before GPOS executes. It bounds font positioning values, active mark-coverage
anchors and cumulative coordinates in the selected paint scale. Unsafe
arithmetic, other scripts, contextual/chained/cursive GPOS and fonts containing
legacy `kern`, `kerx` or `trak` tables return `UnsupportedPositioningDomain`.
Font-table inspection is bounded to 100000 records. The guard preserves all
1453 original producer glyphs; acceptance of another font remains distinct from
native metric or positioning parity for that font.

This script-level API does not itself select device fonts, resolve paragraph
bidi/font runs, combine fallback-font chunks or wrap lines. The bounded
production adapter below consumes its results; Chromium text reproduction
and document SVG/PDF transport remain separate validation boundaries.

### Whole-piece Rust measurement

[`PaintShaper::shape_text`](../../crates/sdocx/src/render/fonts/paint_shaping/pieces.rs)
accepts a checked `PaintItemization`, supplied horizontal direction, finite
letter spacing, zero word spacing and typed caller features. It creates each
chunk's native-shaped input, then applies the shared pen/layout and logical
entry conversion once. The immutable `PaintMeasuredPiece` retains source/range,
paint/scale/direction, shaped runs with font identity, paint layout and entry
geometry. Empty ranges and unsupported scripts/paint/font inputs return typed
errors.

`PaintFeatureProfile` preserves the actual feature recipe. Latin chunks always
append `liga=0` / `clig=0`; other supported chunks append them only when the
absolute f32 letter spacing, promoted to f64, exceeds 0.03. The native double
constant at `0x26858` has bits `0x3f9eb851eb851eb8`. Caller feature records
precede these forced disables. The factory supplies no language, zero buffer
flags and monotone-grapheme cluster level, with each chunk's recorded infos
and full-source context.

The [whole-piece regressions](../../crates/sdocx/src/render/fonts/paint_shaping/pieces/tests.rs)
match all 151 cases, 558 generated shape requests and 1453 glyphs across eight
fixtures. They compare generated source infos, script/direction/language,
buffer properties, contexts and ordered features before checking Rust-derived
metrics, integer shaping and shared layout. The eight entry-capture cases
also compare native logical-entry geometry. Ten Common chunks use the bounded
default shaping path.

Source limits remain 262144 bytes and 65536 UTF-16 slots, with at most 16384
selected scalars. A whole piece allows at most 128 chunks, 254 caller features
so the final feature list stays within 256, and 65536 aggregate output glyphs.
Per-chunk engine and numeric positioning limits remain in effect. This factory
does not itself perform font/style selection, paragraph bidi resolution or
wrapping; the production adapter below supplies its paragraph-derived windows.

### Production paragraph measurement boundary

[`ParagraphMeasurer`](../../crates/sdocx/src/render/text/measurement.rs)
attempts the paint-sized producer before compatibility measurement. It keeps
the first complete native drawing-span result when spans merge under the
native measurement key, then builds the literal source f32 size/style paint
profile and resolves the configured native NAME/default face. Admission is
bounded to pinned Roboto Regular bytes, face index zero, Typeface metadata
400/nonitalic, covered source and supported paint/script/positioning/resource
domains. NAME fallback cannot replace an available but unregistered database
family such as Roboto Mono with default Regular. Fake-bold, unavailable
native span identity, other physical faces, unsupported tabs and exhausted
budgets report typed measurement diagnostics before compatibility geometry.
Supported neighboring coverage ranges can retain native measurement.
Prepared body-plan reuse compares the immutable native NAME configuration,
font database identity, source and layout inputs. A configuration change
invalidates cached plans even with the same database.

The adapter derives paragraph-local [cache-word windows](#captured-cache-word-context-windows),
itemizes and shapes each selected view, and consumes layout/entry conversion
once. It retains all requested UTF-16 entry slots, including continuation
slots, and maps owners back to global source coordinates. Cached logical
owner positions use native f32 division by 100 before widening to f64.
The shared retained glyph geometry serves PDF, SVG, viewport ink and markers.
Per-glyph PDF advances are transport metadata derived from paint positioning;
they are not a captured Samsung PDF field.

Production regressions compare six immutable native entry cases with both
zero origin and a supplementary-character/paragraph prefix, including exact
entry advances, retained owners and owner-relative positions. Failure controls
pin scoped diagnostics and compatibility geometry for unsupported styles,
tabs, composite skew, budgets and unavailable physical style identity.

Compatibility runs retain their native measurement rejection reason. Export
emits style/font/shaping/budget/tab warnings for the source clusters that reach
the painted-text visibility gate, rather than replaying aggregate preparation
warnings. Failed-font cases without a measured run retain reasons in an
isolated local-text collector and attach intersecting source spans to fallback
lines. `UnsupportedNativeWrapping` is retained on the affected line and emitted
for its visible source. All six native-support aggregate kinds are excluded
from export replay; complete producer diagnostics remain available separately.
This preserves child-local visibility for prepared table cells and code
title/body text despite their shared outer object attribution.

Browser reproduction still independently shapes natural font-unit text and
checks whether one rigid translation reproduces the retained positions within
strict roundoff. Unsupported relative differences across multiple glyphs retain
the source-preserving SVG fallback. Font selection outside the pinned face,
device fallback, the partial-measure fast path, native paragraph bidi/break
production and complete paragraph composition remain separate limits.

### Captured logical-entry conversion and paint profiles

[`table-text-entry-geometry.json`](../../conformance/table-text-entry-geometry.json),
SHA-256
`4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4`,
records eight supplied-Roboto cases, eight shape calls, 25 glyphs, 30 source
UTF-16 slots and 64 actual paint profiles. The
[Rust capture](../../conformance/native_table/text_entry_geometry.rs)
executes initialized file-font/family/collection and layout-piece production,
then actual Text layout initialization `0x97c98` and complete append `0x9dbf0`.
The appended 64-byte glyph record retains ID at +16, full XY at +20,
owner-relative XY at +28, source UTF-16 owner at +40 and ink at +48.
Three allocation fills, repeated zero-fill and independent replay agree.

Bounded `SpanRunFunctor` instruction windows preserve the conversion into
logical entry caches. Glyph owner positions divide by `100.0f` at
`0x773e0`–`0x7741c`; glyph-cache insertion advances at
`0x774e4`–`0x774e8`. Per-owner width divides separately by `100.0f` in
`0x77640`–`0x77678`. Ink translation at `0x775fc`–`0x77610` adds the
owner position to layout ink that already contains shaped offsets, then
`0x77610`–`0x7761c` calls actual Base `RectF::Scale`, `0xb1510`, with
`0.01f` (bits `0x3c23d70a`). The addition therefore applies an offset again;
replacing it with one translation or replacing multiplication by division
changes captured bits. Entry bounds use actual `RectF::SetEmpty`, `0xb10ec`,
and `RectF::Union`, `0xb1538`, through `0x7761c`–`0x77640`, preserving
empty/degenerate rectangle handling.

The actual paint construction/setter window `0x76b14`–`0x76bb8` and native
getters capture source styles 0–7 at each of eight sizes. In this window,
mask 1 sets underline, mask 2 fake bold and mask 4 skew -0.25. Paint size is
the source f32 size multiplied by 100 at `0x76b44`; encoding is glyph-ID 3,
and packed Minikin flags are `0x20000` / fake-bold `0x20020`.
These profile observations
are separate from the supplied shaping paint and do not establish full face
selection or Rust fake-bold metric parity.

The capture supplies layout owner origin, zero extra advance, source/entry
storage and preallocated glyph capacity. It does not execute the whole
`SpanRunFunctor`, typeface/font-manager initialization, editing, entry
classification, wrapping, fallback fonts or document composition. The
production adapter's source mapping is separately tested; it does not make
this capture a complete native paragraph execution.

`PaintLayout::entry_geometry` exposes the shared
[Rust logical-entry conversion](../../crates/sdocx/src/render/fonts/paint_layout/entry_geometry.rs).
The immutable result retains the source/range, one advance/ink/glyph interval
per requested UTF-16 slot and glyph IDs with absolute UTF-16 owners,
owner-relative logical XY and ink. It validates scalar-boundary ranges and
owners through `TextIndex`, retaining continuation slots without assigning
glyph ownership inside a surrogate. Noncontiguous owner groups, inconsistent
advance counts, nonfinite geometry and resource-limit violations return typed
errors.

All eight native entry cases match public Rust metric production, shaping,
layout and entry conversion for 25 glyphs. Independent replay from captured
layout output also matches those 25 glyphs. Controls distinguish width division
from reciprocal multiplication and detect missing or repeated owner translation
in ink. This proves the captured geometry conversion. The production adapter
retains the same entry geometry under its admitted face/style boundary; native
classification, fake-bold metrics, general font selection and wrapping remain
separate contracts.

### Captured complete span paint helper

[`table-text-span-paint.json`](../../conformance/table-text-span-paint.json),
SHA-256
`1d0479915ad0d602da2245b5a3767c7cd308229e0232b9fc544949989c790fdb`,
records 93 profiles through actual file-font/family construction, complete
`RegisterFallback` (`0x89410`) / `RegisterDefault` (`0x89978`),
`CreateFromFamilyName` (`0x8a278`), family/style getters (`0x89ef8` /
`0x89f40`) and complete span paint helper `0x76ad4`. The
[Rust capture](../../conformance/native_table/text_span_paint.rs) supplies one
regular Roboto family, locale, requested typeface weight/italic, source f32
size/foreground/flags and a null feature string. Allocation fills, repeated
zero-fill and independent drivers produce identical bytes.

The helper initially creates Minikin style weight 400 / italic false at
`0x76c18`–`0x76c2c`. Its later instructions at `0x76d18`–`0x76d24` copy
the Typeface style into the final paint: low 16 bits become weight and bit 16
becomes italic. Inferred requests `(-1,-1)` resolve the supplied physical
font's 400/false metadata; explicit `(700,true)` controls retain 700/true in
the final paint. The getter's consumed low 17 bits are compared; its fourth
storage byte is unwritten allocation padding.

Source styles 0–15 plus 64/128/192 confirm mask 1 underline, mask 2 fake bold
and mask 4 skew -0.25. Underline changes transient TextPaint state while
leaving final MinikinPaint fields unchanged. Source f32 size multiplies by 100
at `0x76b44`; threshold controls straddle paint size 2048, with tiny positive
profiles at paint sizes below one. Those tiny profiles establish constructor
fields, separately from the supported metric domain.

[`PaintSpanProfile`](../../crates/sdocx/src/render/fonts/paint_span.rs) retains
the scalar size/scale/skew, underline/fake-bold bits and packed paint flags.
Its [regressions](../../crates/sdocx/src/render/fonts/paint_span/tests.rs)
match the captured scalar values and check final Typeface-style transport.
`metric_input` returns the captured Normal profile for supported sizes, with
typed rejection of fake-bold metrics and out-of-domain paint sizes. It does
not resolve a Typeface or claim that explicit bold/italic metadata selects a
different physical font.

The supplied regular font remains the physical source even in 700/italic
metadata controls. Native font-name/XML/system-default resolution, physical
style-face selection, synthesis metrics, whole `SpanRunFunctor`, shaping,
fallback, wrapping and document composition remain outside this capture.

### Captured span font-name and default selection

The [Rust capture](../../conformance/native_table/text_span_font_name.rs) and
[`table-text-span-font-name.json`](../../conformance/table-text-span-font-name.json),
SHA-256
`20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754`,
record 130 profiles through actual `FontManager` initialization and
`FontListParser`, bundled libxml2/libc++, native file-font construction and
the `SpanRunFunctor` name/default-selection window (`0x7710c`–`0x7719c`).
The virtual factory call at `0x77184` returns a Typeface consumed by the
complete paint helper (`0x76ad4`) and actual `TextPaint::setTypeface`
(`0x7b8bc`). Three allocation fills, repeated zero-fill and an independent
driver agree byte for byte.

The supplied XML defines one `sans-serif` family with pinned Roboto Regular,
Bold, Italic and BoldItalic files. A null span NAME uses the supplied default;
a nonnull empty NAME suppresses it. `Roboto-Regular`, `Roboto-Bold`,
`Roboto-Italic` and `Roboto-BoldItalic` resolve through the native style-name
parser (`0x8a42c`) to matching physical files and style metadata. `Roboto`,
`Roboto Bold`, empty and missing names select the supplied regular face.
The factory receives the caller direction unchanged. With both names null,
direction false yields `sans-serif` family metadata and true yields empty
family metadata, while both select the same physical regular file.

The capture observes physical best-match (`0x913d4`), source getters
(`0x88ab4`–`0x88b68`), face index, file length/hash and retained typeface
pointer. It also captures final Typeface weight/italic copied into paint,
independently of fake-bold/skew source flags. Style comparisons exclude
unwritten getter padding. XML, files, source size/style and direction are
caller inputs; source IDs are process-local with a fixed seed/file order.
This is a fixture font configuration, not Samsung device font configuration.
Alternate XML/schema/error paths, fallback glyph selection, shaping/metrics,
fake-bold metric support, later whole-span execution, wrapping, composition
and SVG output remain outside the capture.

[`NativeFontNameRequest` and configuration](../../crates/sdocx/src/render/fonts/paint_font_name.rs)
preserve nullable span/default names and direction. `FontBook::resolve_native_name`
retains native family metadata separately from the physical database family,
requested/Typeface styles and selected face. The
[typed resolution regressions](../../crates/sdocx/src/render/fonts/paint_font_name/tests.rs)
match all 130 captured requests, paint styles and physical file hashes,
indices, lengths and metadata. The default book registers the four pinned
filenames and maps native `sans-serif` to database `Roboto`.

Source inspection of `0x8a42c` shows priority-ordered style-pattern lookup:
Regular, Italic, BoldItalic, Bold, ThinItalic, Thin, LightItalic, Light,
MediumItalic, Medium, BlackItalic and Black. Each pattern uses its rightmost
occurrence at a positive position; it need not end the name. File registrations
at `0x7f048`–`0x7f168` derive aliases before the last dot and last hyphen;
lookup at `0x8076c` uses the complete NAME before falling back to the default.
Caller configurations define registrations and aliases into the caller's
database; physical matching uses its family membership/style policy. That
generic database behavior is not Minikin/device matching parity. Missing
configuration, embedded NULs and conflicting registrations return typed
errors. Names are bounded to 4096 bytes, with at most 1024 registrations and
1024 family aliases. The captured four-file
resolution proof is separate from paint measurement activation.

### Captured named-face measurement

The [native producer](../../conformance/native_table/text_shaping/named_faces.rs)
and [`table-text-shaping-named-faces.json`](../../conformance/table-text-shaping-named-faces.json),
SHA-256
`52bc6161befde8fe518148ca65631be25ae0712a9d07170caae61820ae211c30`,
execute the same supplied XML/four-file manager, actual NAME selection and
complete span paint helper, then pass the returned MinikinPaint and collection
directly into `LayoutPiece` (`0x9b150`), bundled HarfBuzz (`0xecdcc`) and
Skia/FreeType. Its 110 profiles capture 134 HarfBuzz calls, 437 glyphs/UTF-16
advances, 614 FreeType loads and 110 scaler configurations. Allocation fills,
repeat and independent processes agree byte for byte.

Profiles combine each physical Roboto style with source flags 0, 1 and 4,
covering pairs, marks, ligatures, context, RTL and sizes around the 2048-paint
normalization threshold. Layout font-slot source ID is checked against the
NAME-selected physical file, with no additional slot/fallback. Actual append
and bounded SpanRun entry conversion consume the produced geometry. The
XML omits family language: the HarfBuzz input language is null, while the
actual Font metadata getter returns an empty string. Two Regular controls at
source size 20 capture `9.` and `10.` paint advances 1123/526 and 1123/1123/526.
Hinted configurations use native `0x120208` flags; unhinted/skew/normalized
configurations use `0x10020a`.

The [Rust named-face regressions](../../crates/sdocx/src/render/fonts/native_named_face_tests.rs)
match every generated HarfBuzz input/output, paint layout and logical-entry
bit for 106 whole pieces, comprising 122 calls and 401 glyphs. Four
`mixed_marks` source-flag-4 profiles, one per
physical face, return explicit `UnsupportedComposite` for glyph 2578.
Separately, 429 raw vector advances and 429 ink bounds match native metric
callbacks; eight composite callback requests remain explicit rejections.
These counts are separate from the original eight-suite producer corpus.

Source fake-bold flag 2 is excluded from measurement. Constructor-only
0–7 paint controls do not establish fake-bold metrics. This capture does not
establish Samsung device font configuration, arbitrary fonts, fallback glyph
selection, whole-span execution after paint conversion, wrapping/composition
or raster/SVG output. The four-face producer proof and the production
paragraph admission gate remain distinct.

### Captured consumer text metrics

The [native consumer-metric capture](../../conformance/native_table/text_shaping/consumer_metrics.rs)
and [`table-text-shaping-consumer-metrics.json`](../../conformance/table-text-shaping-consumer-metrics.json),
SHA-256
`a5ce8f4ce74f205300f6fdb6e7ac7fa0a5023c377e659a2527760582feca8041`,
record 52 Regular/normal/LTR profiles through actual NAME/paint selection,
`LayoutPiece`, bundled HarfBuzz, Skia/FreeType and bounded logical-entry
conversion. Ten source sizes (10, 12, 15, 17, 20, 24, 30, 36, 45 and 100)
each measure supplied strings `1.`, `9.`, `10.`, `aa.` and `ABC`; two `AB`
controls use sizes 10 and 30. The corpus captures 52 HarfBuzz calls,
134 glyphs/UTF-16 slots, 186 FreeType loads and 52 scaler configurations.
Three allocation fills, repeated zero-fill and independent compiled drivers
agree byte for byte; the prior 110-profile named-face fixture is unchanged.

The [Rust consumer regressions](../../crates/sdocx/src/render/fonts/native_named_face_tests.rs)
match all 52 profiles and 134 glyphs without rejections: raw metric advances,
24.8 shaping, UTF-16/full-glyph geometry and logical entry widths/ink/positions.
These are separate metric controls for marker/body/object-width consumers;
the native list numbering, object feedback, wrapping and composition algorithms
do not execute. Source strings, size and the pinned XML/font configuration are
caller inputs. Underline, skew, fake-bold, other physical faces, device fallback
and output appearance remain outside this corpus.

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
are separate inputs. Mixed RTL/LTR appearance has no captured reference here.

`RichTextLayout::DoParagraphLayout` obtains ICU's logical-to-visual map at
`0x72eb0`–`0x72ec0`. `ParagraphLayout::m_CopyLayoutData` copies those paragraph
ranks (`0x6aa6c`–`0x6aaa0`); `SetInverseLogicalMap` sorts the selected line's
source interval by them (`0x6ca80`–`0x6cb04`). Positioning consumes that map at
`0x6b6ac`–`0x6b7ac`. This route does not call `ubidi_setLine`: a fresh line-level
L1 whitespace reset would change the inspected behavior. For `Aאב  גZ`, the
first five scalars retain visual source order `[0,4,3,2,1]`, rather than
`[0,2,1,3,4]` from a fresh line reset. Inline object anchors participate in the
same map; their measured object widths replace ordinary glyph advances.

ICU's preliminary `getBaseDirection` scan includes strong characters inside
isolates; its subsequent automatic paragraph resolution excludes those
characters. Rust therefore uses the preliminary RTL default only when the
library's P2/P3 scan finds no outer strong character. `RLIאPDI123` chooses
RTL base and visual order `123א`, while `RLIאPDIABC` still chooses LTR base.
Blindly forcing the preliminary direction, or always using LTR for a neutral
outer paragraph, both differ from the native caller. The library performs
paragraph analysis once; Rust uses its public base-direction helper rather
than duplicating isolate parsing. The preliminary policy appears in
[ICU 76.1's scan](https://github.com/unicode-org/icu/blob/release-76-1/icu4c/source/common/ubidi.cpp#L340)
and native measure/layout callers at `0x78b40`–`0x78c24` and
`0x72878`–`0x728f4`. Independent ICU76 literals and covered-font public tests
exercise LRI/RLI/FSI, neutral numbers, opposite outer strong text and unchanged
native CR/LF boundaries.

ICU also assigns retained X9 controls the following character's level, after
whole-paragraph L1. The policy is identical in inspected
[ICU 60.3](https://github.com/unicode-org/icu/blob/release-60-3/icu4c/source/common/ubidi.cpp#L2294)
and [ICU 76.1](https://github.com/unicode-org/icu/blob/release-76-1/icu4c/source/common/ubidi.cpp#L2289).
Direct ICU76 probes distinguish that policy from `unicode-bidi`'s retained
control levels. Rust applies the source-backed X9 adjustment once, retaining
typed levels and scalar-atomic cluster ranges. ICU's uniform-direction getter
can collapse all levels to the paragraph level; Rust retains resolved levels
instead. Tested uniform cases agree in direction parity and visual ranks,
but canonical full-level equality and the device's ICU version remain unproven.
Internal Unicode B-class paragraph separators remain unsupported for this
native visual map; native CR/LF paragraph handling is unchanged.

Default alignment is independent of detected paragraph direction.
`CalculateParagraphLayout` passes global `layout_direction == 1` at
`0x73eb0`–`0x73ee0`, and `GetLineAlign` uses it for Default4
(`0x6aac0`–`0x6aacc`). Globally LTR exports therefore retain left default
alignment, including naturally RTL paragraphs.

Native painting reads retained per-source glyph IDs and positions
(`0x66df0`–`0x66e1c`, `0x6732c`–`0x673fc`). RTL flushing reverses glyph IDs
and their positions together (`0x67140`–`0x671c0`, `0x67c0c`–`0x67c80`);
it does not reshape reordered Unicode. The Rust SVG adapter admits native
visual positions only after every cluster in the line reproduces its retained
glyph IDs and relative offsets through isolated export shaping. RTL clusters
must be scalar-sized; multi-scalar fragments that resolve RTL under usvg's
LTR paragraph base are rejected even if native shaping was forced LTR.
Mirrored punctuation, joined Arabic, unsupported fonts and split cluster
styles retain the diagnosed fallback. Object lines apply the same visual map
after child preparation and obstacle retries, before justification. Text and
object arrays stay in source order; only their X positions and visual ranks
change. A rejected child plan keeps the whole line in its existing fallback,
including the replacement-character anchor. This malformed-input recovery is
an SDK policy, not established native behavior.

Roboto override/isolate fixtures verify logical Unicode preservation,
wrapped origins, kerning, visual decoration intervals and backgrounds across
placed/flow and normal/replay paths. Chromium scalar-position probes match
independently positioned glyph references pixel-for-pixel. SVG scalar position
lists and browser UTF16 character APIs use different indexing for supplementary
characters. PDF preserves control collisions with `ActualText`; lopdf's plain
text extractor ignores that metadata, so regressions inspect both CMaps and
marked text. These are transport and source-derived contracts, not captured
Samsung RTL appearance parity: the four current HF documents contain no RTL
letters or directional controls.

Covered Hebrew/Arabic regressions use caller-selected DejaVu Sans 2.37:
`tests/assets/fonts/DejaVuSans.ttf`, 759,720 bytes, SHA-256
`57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e`.
Its upstream font notices accompany the fixture. It is test-only; default
fonts and browser/WASM bundles continue to contain the pinned Roboto families.

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
do not establish complete native shaping or RTL endpoint ordering. Drawing also joins
only fully equal spans with matching font IDs, direction and adjacency
(`inSameDraw`, `0x65998`); measurement's narrower join mask does not imply
that decoration changes can be discarded during paint.

### Captured decoration commands and paint inputs

[`table-text-decorations.json`](../../conformance/table-text-decorations.json),
SHA-256 `2aa3eaff8c540215ae798596372875c9f978442157a4fb1f622103644389ac2c`,
contains 368 cases and 438 rectangle commands. The
[Rust capture module](../../conformance/native_table/text_decorations.rs)
executes complete native Text `setTextPaint`, `0x63b98`,
`drawTextDecorations`, `0x666d4`, `GetSpan`, `0x61f3c`, and entry initialization,
`0x65920`, plus Base `RectF::Set`, `0xb10e0`. Every serialized result repeats
with memory fills `0x00`, `0xa5` and `0xff`.

The first endpoint is `offset_x + first_entry_x`. The second is calculated as
`first_endpoint + (last_advance + last_entry_x - first_entry_x)` with f32
operations in that order. The baseline is `offset_y + first_entry_y`.
The routine does not inspect entry direction: supplied RTL direction, reversed
X order and negative advances can retain reversed rectangle endpoints. It
does not normalize those endpoints using a separate RTL branch.
Underline top uses `size.mul_add(1/9, baseline)`; ordinary thickness is
`size * f32(1/18)`. Strikethrough top uses
`size.mul_add(-2/7, baseline)`. These capture the native f32/FMA operations,
independently of the Rust vector transport's arithmetic.

Suggestion style bit `0x10` and correction style bit `0x20` share the special
thickness branch at `0x668ac`–`0x668e4`. Both `size` and `converted_17` are
multiplied by the same f32 reciprocal `1/18`; comparison selects the smaller
result. The bottom is then calculated as `selected.mul_add(2, underline_top)`
at `0x668d0`, rather than adding an independently rounded thickness.
Here `converted_17` is `17.mul_add(unit_scale, font_size_delta)`.
RichTextImpl screen-unit member
188 selects density member 196 for mode 1, scaled density member 200 for mode
2, document-pixel member 192 for mode 3, and unit scale 1 otherwise; size
delta is member 172. These are supplied runtime font-unit inputs, rather than
a universal document-density multiplier.

`SetSuggestionSpanEnabled`, `0x6863c`, controls Drawing byte 512.
When enabled, `setTextPaint` uses underline style mask `0x34`; otherwise it
uses only ordinary underline bit 4. Special underline paint reads its color
from raw span member 32. Correction foreground-enable byte 66 selects main
foreground member 36 independently of correction style bit `0x20`.
Hypertext flag bit 0 and RichTextImpl link-enabled byte 112 force underline
and color `0xff0054ff`, after which enabled correction foreground can replace
that color.

Paint alpha arguments use truncation of `255 * opacity`. The forced-strike
argument also multiplies opacity by 0.4. Recorded ARGB color and separate alpha
setter values are native interface arguments; this capture does not prove how
actual SkPaint combines their alpha. A null RichText implementation produces
the real `GetSpan` fallback of size 17 and style zero.

Cases cover all style values 0–63, suggestion/link/correction gates, five font
sizes including negative and zero, screen-unit modes 0–4, four opacity values,
three color alpha values, supplied RTL/reversed endpoints, negative advances,
fractional origins, differing baselines, forced strike and null implementation.
Retained spans, configuration, entry positions/advances/direction and opacity
are supplied. Paint setters/getters, paint copy/destruction, memory copy and
canvas rectangle recording are host interfaces. Native font selection, shaping,
theme conversion, complete `drawTextRun`, SkCanvas pixels and Composer decoration
painting do not execute.

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
fields, and conditionally computes the natural paragraph width. That width
starts with the `f32` sum of left/right margins (`0x72ca4`–`0x72cb0`),
folds the entry advances with `f32` additions (`0x72d88`), then adds the
`f32` constant `0.001` at rodata `0x26684` (bits `0x3a83126f`).
`frintp`, signed 32-bit conversion and conversion back to `f32` produce
the value stored at paragraph record member 20 (`0x72dd0`–`0x72dfc`).
Width-zero and single-line controls store integer natural width in layout member
128 and expand the available maximum width; explicit-width, non-single-line
word-wrap-disabled controls store the rounded width in paragraph member 20
without expanding layout member 128. Default wrapping executes neither width
fold. These conditional natural widths are separate from the unrounded candidate comparisons in
`GetBlockInfo`. The function passes the same entries into
`CalculateParagraphLayout` (`0x72e14`–`0x72e50`). The latter calls
`ParagraphLayout::DoLayTextOut` at `0x73ee0`. `SetLayout` places ordinary
entries with an independent `f32` addition of X and member 0 for the
right edge and next cursor (`0x6b6fc`, `0x6b73c`). It moves cached ink at
member 32 by removing the old entry point and adding the new X/baseline
(`0x6b70c`–`0x6b734`), then unions that placed ink into line record
member 24 (`0x6b8a8`–`0x6b940`). The ink is not the ordinary wrapping
width. These inspected line
selection and placement functions do not call the Minikin measurement
producer again when a line boundary is chosen. The paragraph-loop capture
below executes these conditional width folds; it does not establish complete
Rust wrapping parity.

Rust retains the paragraph's measured source/cluster mappings and advances,
then selects and positions lines from those results. Native
width comparisons here use float additions and a strict `>` test; no
additional per-line width rounding or special kerning correction is proved.
Native emergency splitting is not proven grapheme/cluster-safe;
keeping graphemes intact is an SDK policy, not established native behavior.
The retained advances also do not prove that
independently reshaping each exported SVG line reproduces the same glyph
positions. The retained Rust document PDF path consumes the measured layout;
SVG text transport retains the limits described below.
The native ordinary drawing path obtains cached `GlyphInfo` at `0x65e04`
and builds each glyph point by adding its retained X/Y offset to the
`MeasureData` run point and supplied draw offset (`0x660c4`–`0x660e8`).
`drawGlyphs` forwards those positions to the canvas at `0x66a94`–`0x66ac8`.
The native contract retains selected glyphs, face choices and cluster positions
from the same measurement result, including clusters spanning several UTF-16
entries. Matching line advance alone does not establish this placement contract.

`ParagraphLayout::GetBlockInfo`, `0x6ab9c`, keeps committed width in
`s10` and pending word width in `s8`. Its candidate is
`f32(f32(committed + pending) + entry_advance)` (`0x6ae04`, `0x6ae18`),
and each entry separately adds to pending width (`0x6af08`–`0x6af0c`).
This grouping is not one continuous fold of all entry advances. It tests
the candidate against the current available rectangle with a strict `>`
comparison (`0x6ae80`–`0x6ae84`): an exactly fitting candidate fits. A
committed break is tracked when the current paragraph-relative index
equals the stored break end minus one (`0x6af18`–`0x6af38`). That break,
spaces and tabs add pending width to committed width and reset pending
(`0x6af34`, `0x6af64`, `0x6af80`); space counts increase by 1 and 4 for
spaces and tabs respectively. The final block width at member 16 is the
`f32` sum of pending and committed widths (`0x6afd0`–`0x6aff8`). On
overflow it uses the last committed
break when its index is at least 1 (`0x6b014`–`0x6b028`), otherwise the
preceding index (`0x6b03c`–`0x6b06c`). This is measured greedy wrapping,
not a character-count estimate.

### Captured paragraph layout loop

The [Rust capture](../../conformance/native_table/text_paragraph_layout.rs) and
[`table-text-paragraph-layout.json`](../../conformance/table-text-paragraph-layout.json),
SHA-256
`d381922f40a06ef49ff43794aa01ae2546676e42f765c22e9512fff4fb948bf4`,
execute the native RichText/default-paragraph constructors and complete
`DoParagraphLayout` → `CalculateParagraphLayout` → `DoLayTextOut` →
`GetBlockInfo`/`SetLayout` chain. Its 25 cases retain 38 lines and 81 placed
UTF-16 entries, with direction/break annotations, pre/post logical maps,
logical/ink rectangles, line metrics and conditional natural-width results.
Actual constructor defaults are single-line false and word-wrap true; actual
setters select the recorded controls. Alignment value 1 is Right and 2 is
Center. Three memory fills and the repeated zero-fill run agree; strict
master-driver replay reproduces the fixture.

The native ICU loader resolves suffix 76 and the pinned host ICU 76.1/Unicode
16 libraries execute bidi and line breaking over retained source buffers.
The break iterator uses type 2 and an empty locale. Source UTF-16, dense
80-byte measured advances, entry kinds, size/metrics 17.125, spacing 1.35,
old points and ink are supplied. Kind classification is a harness input.
The width-zero/single-line controls exercise layout member 128 and expand
maximum width; explicit-width, non-single-line word-wrap false exercises
paragraph member 20 without that expansion. Default wrapping leaves both
conditional folds inactive.

The oversized surrogate control selects inclusive ranges `[0,0]`, `[1,1]`,
`[2,2]`, splitting the supplied surrogate pair. This establishes a UTF-16
emergency result, not scalar/grapheme safety. Empty source is forced through
layout; a newline inside the supplied single paragraph does not establish
paragraph splitting. Font/entry measurement, source classification, spans,
objects/bullets, obstacles/padding, pagination, draw clipping, vector export
and Android device ICU do not execute. The production ordinary slot bridge
below retains SDK break/grapheme/height policy rather than this full loop.

### Captured ordinary wrap arithmetic

The [Rust capture](../../conformance/native_table/text_wrap_numeric.rs) and
[`table-text-wrap-numeric.json`](../../conformance/table-text-wrap-numeric.json)
execute complete native `GetBlockInfo` (`0x6ab9c`) and `SetLayout`
(`0x6b4a4`) against supplied advances, entry kinds, break ends, rectangles,
old positions, ink and visual maps. The fixture SHA-256 is
`b3380614708e4a16712f2bdfa9d9a80ed3610b733f4ff274d50d09da962e09b8`.
Its 17 cases supply 59 UTF-16 slots and capture 55 candidate operand/result
pairs, 16 commit operations and 45 selected/placed slots. Three memory fills,
the repeated zero-fill run and an independent process agree byte for byte.

For supplied advances `[2^24, 1, 1]`, grouped breaks produce block width
16777218 and a continuous pending word produces 16777216. Independent
`SetLayout` cursor additions end at 16777216 in both cases. Controls cover
adjacent `f32` budgets around 30, zero-width continuation, oversized first
entries, negative advances, index-zero breaks, committed versus pending
words, spaces, tabs and justification. Changed supplied ink/old points alter
retained line ink while selected range, block width and new points remain
equal.

This capture supplies the single-line shell, maxima copied from the actual
block, baseline spacing, visual-to-logical map and ordinary cached entry data.
Synthetic large/negative advances are numeric controls, not font-produced
metrics. Native shaping, font selection, ICU break/bidi production, the
automatic paragraph wrapping loop, natural-width ceiling, draw clip gates,
full composition and raster/vector output do not execute.

The [Rust selector/cursor](../../crates/sdocx/src/render/text/native_wrap.rs)
matches the captured candidate/commit operations, ranges, block widths and
independent entry positions. The production
[UTF-16 slot bridge](../../crates/sdocx/src/render/text/wrapping/native_slots.rs)
feeds retained per-UTF-16 advances into that selector before cluster
compression for wholly native-measured paragraphs without objects. Zero
continuation slots survive. Grouped block width controls center/right
SVG/PDF alignment; the separate f32 visual cursor places glyph owners and
distributes justification. SDK bidi scalar maps expand to physical UTF-16
slots.

Production regressions replay all 17 numeric cases and 45 positions, compare
native widths from four shaping captures at zero/prefixed paragraph origins,
and check adjacent-f32 fitting, grouped alignment and compatibility with
embedded objects. Break ends from UAX rules, grapheme boundaries and entry
heights remain SDK policy. Native emergency results that cut a glyph cluster,
surrogate or grapheme, and narrowed intervals with no native block, report
`UnsupportedNativeWrapping` before retaining the SDK emergency/source policy.
Objects and mixed unsupported measurement retain the existing wrapper.
These tests do not establish native ICU/object/full-composition parity.

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
Those horizontal margins come from the measurement context, rather than saved
object X coordinates or the block-spacing option. `RichTextMeasure` initializes
them to zero (`0x78090`–`0x780ac`). Bodytext `updateObstacle` supplies constants
63/64 (`0xb2d88`–`0xb2db8`): their Content records at `0x7dd0` / `0x7de8`
contain logical 4 / 0 with document-density units. Native PDF's
`BodyTextPDFWriter::WriteBodyText` constructs `BodyTextCapture` at `0x375e2c`
and requests its drawn text at `0x375ea4`. The capture constructs a fresh
`BodyTextLayout` (`0xca2ac`), assigns the document (`0xca2b8`) and measures
(`0xca2d0`). `Measure` chooses layout type 2 (`0xb2c14`), whose active layout
branch updates obstacles before updating and measuring text
(`0xb1a40`–`0xb1a68`). Body-flow inline entries therefore reserve four logical
units on each side; a width-30 object at density 1 has advance 38 and local
content bounds `[4,-height,34,0]`. At density 3 these become advance 54 and
`[12,-height,42,0]`. Fresh placed/frame measurement retains zero horizontal
margins. Nested frame layouts use their own producer context, independently
of the style defaults used to paint their text. Rust selects typed
`ObjectMeasurementContext::Body` from flow/capture layout and `Frame` from
placed, shape, code and table frame layout. Reserved advances include both
horizontal margins, while object drawing and culling use content X and visible
width. The same rule applies to measured and diagnosed measurement-fallback
paths; it does not apply body obstacle margins to a child merely because the
child uses flow typography. Density-1/3 synthetic fixtures exercise distinct
body/frame positions, wrapping thresholds and nested child layout ownership.

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
the member. Widget's default/font-span conversion and final span
copying do not clear the object's font.
`GetBlockInfo` takes maxima of entry height and text metric independently
(`0x6aeb4`–`0x6af44`). Both object types set block flag 41
(`0x6ad3c`–`0x6ad54`), so `GetBaseline` adds `0.001f` to both baseline and
vertical cursor (`0x6cc0c`–`0x6cc28`). With zero measurement margins,
`F=20`, `H=100`, default multiplier 1.35 and initial cursor 0 yield cursor
107.001 and baseline 100.001, including an object-only initial paragraph.
Rust retains the font maximum separately from ordinary text height and
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
bounds (`0xab23c`–`0xab2c0`), giving `[0,0,985,217]` here. The union uses
the first and last frame owners; it does not cover every raw slot. Over-pages
table text selects these frames at `0xa6b04`–`0xa6b24`, then offsets by the supplied
draw rectangle minus measured origin at `0xa6b38`–`0xa6b50`. These rules
support prepared geometry, not an arbitrary -0.5 shift of table glyphs;
the composed draw-rectangle origin remains a separate input.

The ordinary, non-spannable drawing branch instead reads each model cell's
rectangle, subtracts the model table origin, and adds the supplied draw origin
(`0xa68bc`–`0xa6b50`). It must retain that distinct coordinate producer. Rust
prepares bounded dense unmerged/merged grids for constraints 1/2 with retained
split lists. Each cell uses the shared Rust engine at the native integer-truncated cell
dimensions and retains that plan for painting. Cold measurement grows saved rows;
warm retries retain caches, shrink or grow rows, and preserve pending page gaps.
Fresh drawing also prepares Normal tables; their callback reservation remains
separate. Sparse grids, nested objects and rotated layouts retain saved-frame
fallbacks. Partial horizontal obstacles report `UnsupportedContent` and preserve
saved-cell painting; the shared cell engine
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
not established as equivalent. Rust retains these phases for dense merged grids.
The [table captures](table-code-findings.md#public-table-measurement-and-layout-lifecycle)
verify raw-slot cold sizing, owner-based warm sizing and endpoint-owner bounds.
Native merged shaping and complete parent pagination remain unverified.

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
There is no captured mixed inline-text case. The SDK validates UTF-16
U+FFFC anchors with half-open source ranges and measures text and objects in
one paragraph stream, preserving neighboring text. Synthetic mixed-layout
regressions do not establish captured inline parity. The prepared-object and
capture-window contracts below describe supported contexts; arbitrary nested
composition and edited capture contexts are not covered by those references.

## Body-flow pagination boundaries

Over-page objects still participate in parent obstacle checks.
`IsOverlappedWithObstacle` (`0x6c6bc`–`0x6c6e0`, Text) passes the object's
over-page flag and minimum first-page height to the obstacle tree. For padding
obstacles, the tree skips a band only when that minimum is effectively zero
or `band.top - candidate.top >= ceil(minimum_height)` (`0x6e7fc`–`0x6e820`).
Otherwise the parent can move past the obstacle (`0x6a830`–`0x6a894`).
Code's minimum adds top padding, the retained title rectangle height,
title/body gap and first body-line height (`0x738f4`–`0x73948`, Drawing).
The title rectangle shares the copy-button height; measured title text height
does not replace it. The [code-layout capture](table-code-findings.md#code-block-chrome-and-split-inputs)
and Rust kernel preserve that arithmetic. Rust retains the minimum beside
measured height and settles parent movement before remeasuring child splits
at the final candidate. Ordinary obstacles always apply. The SDK absent-title
control also retains copy height in this reservation; it does not establish
native absent-title construction.

Table `GetMinHeightInFirstPage` (`0xac9d4`, Drawing) instead measures its first
cached row (`0xac9f0`–`0xacb48`). Nonempty cells contribute first-line background
height plus text top margin; empty cells contribute measured height. A zero
maximum falls back to the first cached frame's height, then adds
`contentRect.top - measuredRect.top`. Cold slot frames begin at the global
drawing half-border offset. Bodytext lays out the child before reading its
minimum (`0xb0e28`, `0xb0f20`–`0xb0f30`), so a missing Rust plan cannot stand
in for native zero. Prepared paged grids retain this minimum and their shared
cell layouts; parent retries update the retained native row state rather than
rebuilding a cold grid.

Empty table cells reserve a cursor rectangle, independently of raw text height
zero. Drawing constructs Widget text children (`0xaacb8`, `0xae628`), whose
empty paragraph retains applicable metadata for ordinal `[0,1)`. The cell's
`getCellLayoutHeight`, `0x8c29c`, reads `GetDefaultCursorRect` bottom and adds
the bottom margin (`0x8c2fc`–`0x8c314`). Its first rectangle starts after top
margin and paragraph-before spacing. Padding bands can move it; a spacing
height larger than the first two sorted bands' gap falls back to plain font
height. Cursor centering truncates `(rectangle_height - caret_height) / 2`
to an integer. Rust derives this metric from shared caret inputs, including
numeric-marker font at caret 0 (`0xd85c4`), while keeping raw layout height zero.

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
measured layout. It does not establish moving/resizing objects or document
repagination.

### Captured page text ranges

The [Rust capture](../../conformance/native_table/page_text_ranges.rs) and
[`table-page-text-ranges.json`](../../conformance/table-page-text-ranges.json),
SHA-256
`768293b00e5f880166add1683c12cf76fe9c5ebf293e1b522803ca69b395b05c`,
execute complete native boundary predicates, page scans, inclusive UTF-16
conversion, vector resize and Base `Rect::Height`. Its 47 cases include 251
boundary queries across three allocation fills; independent replay agrees
byte for byte. Page records, measured-line getters, default cursor and
first-empty rectangle values are supplied.

Crossing lines belong to adjacent sections. Exact page-end starts and exact
page-start bottoms are excluded, while a zero-height line at the page start
can qualify. First-line fallback and empty-text cursor controls use distinct
getters. Page-end arithmetic adds integers before conversion to `f32`.
Rescans preserve overlap; exhausted `[count,0]` and absent `[-1,0]` sections
remain distinct. Paragraph shaping, cursor/first-empty producers and document
repagination do not execute. Rust saved-section handling accepts exact
`[-1,0]` only in exhausted suffixes, preserving earlier saved captures and
leaving absent pages blank. Malformed negatives, absent prefixes/interior
sections and wholly empty sections for nonempty bodies retain compatibility
reflow. These results do not establish an authoritative production measured-line
indexer or replace SDK viewport ink visibility.

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
just splitting the string into equal chunks. The SDK retains saved page text
sections. These records do not establish native parity for recomputed page
breaks or arbitrary obstacle/inline-object behavior.

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
before the object epsilon. Rust retains the raw candidate separately from
its adjusted margin, applies this padding exception to every wrapped line,
and recomputes the margin after moving the raw candidate to an obstacle's
bottom. Independent regressions cover the literal cases above, native f32
probe rounding, and suppression of a preceding object's bottom margin on an
ordinary text line. Ordinary constraint-0 child feedback was not established
by this trace. Child callback origins use their resolved object font:
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
the 37.498 page-boundary move. Preparing at the final drawing origin preserves
that boundary movement and the parent's 411-unit reservation. SVG
comparisons against the hash-checked native PDF differ by less than 0.00012
units for the title, all code lines, and following whitespace heading.
The four wholly clipped code lines remain absent.

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

Rust regenerates bounded dense unmerged/merged grids without rotation through
this same engine and keeps final world geometry in a distinct drawing plan.
It preserves the callback reservation and translates retained cell text once.
Hash-checked comparisons include all four native cell glyph origins, alongside
the body and code references. Sparse, rotated and nested preparation remains
unsupported; native merged shaping/parent placement and complete split-page
border clipping remain unverified. Saved height limits do not cap the traced
export layout; see [table/code findings](table-code-findings.md).

Rust also applies the native paragraph indentation masks: saved RTL indent
adds only a right inset for right/center/default alignment; other indent
directions add only a left inset for left/center/default alignment. Both
alignment applies neither inset (`0x7397c`). Indent direction does not select
the bidi base or mirror markers. Drawing normalizes unknown alignment to
internal default 4 through the literal `[4,0,1,2,3]` conversion tables
(`0x93234`, `0xa5ac8`, `0xd1454`). Default alignment currently follows the
SDK's LTR display policy. Right/center markers share the glyph alignment
offset, matching `m_UpdateBullet`'s caller (`0x6b5dc`–`0x6b61c`).

Both alignment distributes extra width through the shared positioned
line. Native ordinary constructors initialize layout options to zero
(`0x636e0`, `0x6fcfc`), so final lines and leading/trailing whitespace are
included. Only U+0020 and TAB expand, weighted one and four respectively
(`0x77688`, `0x6af4c`, `0x6b838`). The native f32 share is
`(available_width - measured_width) / weight_count`, without a positive
remainder clamp. Markers remain at their reserved block start, and source
ranges remain unchanged. Covered static Greek/Cyrillic positions retain
their advances through justification; unsupported placement still reports its
diagnostic. RTL justification parity remains unverified.

## Rust layout contracts

The shared Rust engine measures shaped runs with actual font faces and wraps
using Unicode line-break opportunities. Paragraph runs are measured once;
line widths use retained advances. Missing usable metrics report
`MeasurementFailure`. Span joining uses size, source color, family, bold and
italic; the inspected Latin policy disables `liga`/`clig`.

The [support matrix](../text-vector-support.md) records supported adapters,
glyph transport, diagnostics and reference coverage. Sparse table preparation
and arbitrary nested composition remain unsupported; native merged shaping,
parent placement, emergency breaking and device ICU/font selection remain
unverified.

Placed-text gravity excludes final paragraph after-spacing while retaining
between-paragraph after-spacing. Glyph Y positions retain five decimal places,
including gravity midpoints; the parent text coordinate uses its older display
precision. Whole-empty measured height remains zero while gravity uses its
separate spacing contract. Empty boxes retain author-supplied highlight summaries.
Leading/terminal separators and empty-line caret metrics have synthetic
coverage; separator-only clipping and empty-line selection remain unverified.

Shared style resolution checks native scaling before using its result.
Finite `f32::MAX` font sizes, pixel spacing or margins can overflow during
f32 multiplication. Invalid summary sizes use the configured default;
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
flow, placed, shape, table and both code text contexts. These probes do not
establish recovery for arbitrary malformed frame rectangles or oversized
aggregate page geometry.

Code preparation retains owned title and body layouts, copy and panel
rectangles. Parent lines prepare these plans before placement; constraints
1/2 replace their reserved height when the measured height differs by more
than 0.001. Other constraints retain their saved reservation. Split bands
are selected using the live parent candidate. Final code/table drawing prepares
its frames at the settled baseline and world X, including parent gravity;
candidate feedback and final drawing both use the shared Rust layout engine.
Validated native capture windows use a single viewport projection.
They retain native source separators, prepare object feedback in group-local
physical coordinates and project the selected viewport once. Typed glyph,
marker and prepared-panel bounds filter painting independently; code child
viewports receive both translations, while table cell frames already include
their vertical offset. Decorations use actual styled segments rather than the
first style in a shared shaping run. Fresh paint registries embed only painted
faces and report visible font resolution; planner issues retain source or
outer-object ownership. Structural and geometry diagnostics can still cover a
whole visible object. The negative-top adapter remains only for fallback
inspection slices. Captured body/code/table origins are covered by the
comparisons above; native merged/sparse parent composition and arbitrary object
composition remain outside those references.

Shared line assembly retains mutable object measurements and child plans
for each paragraph. Body entries with constraints 1/2 apply staged width
feedback before scanning following entries; retries retain the callback's
changed dimensions and child state. Frame entries retain their established
height policy without assuming the Body callback exists there. Native
`m_CheckObjectChanged` compares both
callback height (`0x6c42c`–`0x6c45c`) and width (`0x6c460`–`0x6c490`) with a
0.001 threshold. A changed width updates the visual rectangle and replaces
the measured advance with the callback width directly, then records the source
anchor in a changed-object notification (`0x6c4a0`–`0x6c4c4`). It does not
simply add the old obstacle margins to the new width. Code's measured width
remains its source width
(`0x73340`–`0x73350`, `0x73584`–`0x735b4`), while a table's regenerated column
frames can change its measured width (`0xab168`–`0xab2c0`). The captured table
retains width 985 in both paths and does not exercise this discrepancy.
`GetBlockInfo` tests the current object's fit using its old advance
(`0x6ada4`, `0x6ae18`, `0x6ae80`), then reloads the changed advance and adds
it to the accumulated width for following entries (`0x6af08`–`0x6af0c`).
The returned block width includes that change (`0x6aff8`). Thus an accepted
object can expand on the current line while pushing the following text onto
another line; eagerly rewrapping the entire paragraph would change the
current object's acceptance. The selected capture route commits this pass
without an established convergence loop. Its changed-object notification is
separate from the editor event route, which updates stored span dimensions
and remeasures the changed paragraph (`0xd4c90`–`0xd4e28`). The Rust driver
measures paragraph text once and admits each object against the previous
width snapshot. Changing only the final paint translation or repeating
measurement until stable would not reproduce this capture behavior.

Callback timing also matters. An old-width precheck skips ordinary overflowing
lookahead (`0x6ae28`–`0x6ae30`), but a permitted first-object overflow can enter
the callback path. The exception compares the candidate rectangle width with
the paragraph's full width, not with the object's width (`0x6ada8`–`0x6add8`).
Its scan offset and prior-break tracker reset for every candidate
(`0x6acec`–`0x6ad0c`), so it can apply to continuation lines. A second check
tests the same old-width sum after the callback (`0x6ae80`–`0x6ae84`). With
a prior break, failure returns that break and leaves the current anchor for
another candidate. Without one, the first-object or character-overflow branch
can force inclusion and consume the updated advance (`0x6b03c`–`0x6b108`).
Child layouts survive rejected visits and are reused and remeasured at the
next candidate (`0xb0cac`–`0xb0e2c`). Applying a callback only after acceptance,
to every rejected lookahead, or to regular constraints would lose these
distinctions. Constraints 1/2 use this width-feedback route; ordinary
constraints retain their separate top-of-page shrink branch.

Body width feedback is the minimum of the child's measured width and the
object's transformed width cap (`0xb0ed8`–`0xb0f1c`). Tables override the
maximum-width getter with their raw table property (`0x3dac24`, relocation
`0x495240`). Code inherits the base getter (`0x2cb6cc`): an attached context
can replace a nonpositive or oversized raw maximum with twice its requested
width; a detached object returns the raw maximum. A nonpositive result falls back
to the parent layout width minus its global text margins
(`0xb0e54`–`0xb0ec0`, `0x8b8f0`, `0x8b928`), not paragraph indentation or
the inline object's four-unit margins. The transformed cap scales the saved
drawn width by the cap divided by the saved raw width, in native float
arithmetic; it does not add an unchanged border after scaling.

The owner is runtime context (`ObjectImpl + 56`), distinct from saved owner
page-size metadata. Partial capture clones a detached text box and its spans
(`0xdf30c`–`0xdf358`, `0x4179b8`–`0x417a78`); property copying does not copy
that context. `setBodyText` and `ObjectTextLayout::SetObject` merely retain
the copied model (`0xa90d4`, `0xaff08`–`0xaff10`, `0xd3974`). Consequently
a partial capture's code cap must not assume the physical source page's width.
Full first-page coverage with the native completed-section flag bypasses
copying (`0xaa4cc`–`0xaa4dc`), and missing sections likewise use the original
body (`0xaa478`–`0xaa688`, `0xa8f50`–`0xa8f58`). The original body attaches
to the note context (`0xa0b60`–`0xa0b74`), whose nested text context forwards
its width requester (`0x3e1cfc`, `0x3e1f48`–`0x3e1f64`). That width comes
from a runtime loader argument stored at note implementation offset 128
(`0x90674`, `0x90c20`), distinct from the persisted note width at offset
132 (`0x91e34`). Saved physical page widths, default dimensions and flow
dimensions therefore cannot substitute for it. The SDK reports
`UnsupportedWidthLimitContext` for full-body code caps without that runtime input.

Final object drawing also converts the selected drawn rectangle back into
the original object's raw rectangle (`0x376418`–`0x376474`). The inverse
uses independent X/Y affine ratios against the original drawn rectangle
(`0xe2064`–`0xe2258`). A detached table clone stores only that outer raw
rectangle; fresh grid measurement still uses the saved column widths
(`0xaaab8`–`0xaaad8`). Its final drawn origin includes the unchanged border
inset before the writer rounds it (`0x37e4ac`–`0x37e4f4`). For raw width 20,
drawn width 21, border 1, selected width 61 and selected left 10.5234375,
the cloned drawn left is approximately 11.47582 and the writer floors it to
11. The grid remains 61 units wide. A width cap therefore changes the parent
reservation and clone placement without necessarily shrinking or clipping
the table grid. Code instead requires fresh layout in its cloned raw frame;
its final wrapping cannot simply reuse the callback's original-width plan.

Independent public regressions cover table expansion, shrinkage, strict
callback thresholds, current-object admission, following-text wrapping,
alignment, global margin caps, signed paragraph indents and densities 1/3.
They check ordinary/replay SVG and selectable PDF, including the full table
grid beyond a capped reservation. Partial-capture code regressions use typed
ObjectBase maximum-size metadata and verify fresh final wrapping without
changing the parent's retained callback height. Full-body code reports
`UnsupportedWidthLimitContext` rather than inferring its runtime cap from
saved dimensions. Rotated Body code with over-page feedback remains an
explicit unsupported case. Rejected derived clone bounds preserve the source
replacement marker and never repaint the stale callback plan; the regression
isolates float-coordinate collapse at world X 16,777,216 from an otherwise
valid one-unit code frame. This is SDK recovery behavior, not a native
malformed-input claim.

Rejected derived code geometry retains the original replacement marker and
neighboring text, reports `InvalidBounds`, and never falls back into measuring
and painting the rejected block again. The same preparation boundary checks
normal and split constraints. This is an SDK robustness policy, rather than
an established native malformed-input recovery rule.

The locked PDF comparisons cover five physical pages, including body, code
and table origins. Prepared-object regressions cover native chrome constants,
candidate-relative bands, retained gravity, nested height feedback, rejected
source markers and selectable PDF text. These checks do not establish
device-font or arbitrary composition parity.

The measured paint path supplies retained X positions and Y offsets
for clusters that can be expressed by the current typed SVG text adapter,
and uses filled rectangles for ordinary underline/strikethrough. Unsupported
glyph positioning, clusters crossing a paint-style boundary, or nonfinite
positions produce `UnsupportedGlyphPositioning` diagnostics and preserve
the text through the existing SVG text fallback. Those fallback bounds and
complex cluster placement are not native glyph-geometry parity.
Missing glyph coverage already reports `MissingGlyphs`; it does not also
emit the positioning diagnostic. Captured standalone typography, bidi visual
placement and device-specific fallback faces remain unverified.

The public-API release probe used a 640-unit page, 17-unit text, density 1
and 544 units of usable width. Three release probe runs measured
50k-character rendering medians of 55.2 ms for repeated Latin,
55.0 ms for long URLs and 38.8 ms for spaced prose. All nine 5k/20k/50k
size/type cases retained complete source, line counts and serialized SVG sizes.
These are observations from one machine, not portable performance limits or
mixed-object benchmarks. A separate pinned-font `AV` regression checks retained
paragraph kerning at a line boundary.

The retained scaling probe checks complete source text, contiguous emitted
lines and diagnostics without asserting wall-clock thresholds:

```sh
cargo test -p sdocx --all-features --test text_layout_scaling --release --offline -- --ignored --nocapture
```

## SVG physical-face transport

SVG text styles and embedded `@font-face` rules use a private family
`sdocx-face-<digest>`, where the digest is SHA-256 of the original font source
bytes followed by the big-endian collection-face index. Logical/source font
names, native NAME requests and diagnostic families remain unchanged.
[`ResolvedFace::svg_family`](../../crates/sdocx/src/render/fonts.rs) reuses its
cached alias for the original index and recomputes it if the public index
changes. Distinct TTC/OTC faces retain distinct identities; renaming a logical
family does not change the physical alias.

[`SvgFontFamilies` and `svg_font_resolver`](../../crates/sdocx/src/render/fonts/svg_identity.rs)
map those aliases to existing database face IDs without mutating the database.
The typed usvg resolver handles a generated carrier's first physical family
and delegates logical/generic families and fallback to usvg. The resolver must
be paired with the same database instance in `usvg::Options::fontdb`; the CLI
uses that pairing. Alias identity preserves physical selection, separately
from the bounded Regular-only native paragraph admission gate and browser
glyph-position reproducibility.

[Rust transport regressions](../../crates/sdocx/tests/svg_face_identity.rs)
combine synthetic bold on a selected Regular face with a physically selected
Bold neighbor. Both spans request bold; default usvg lookup picks Bold for
both, while the physical resolver retains Regular/Bold. Retained and raw-SVG
PDFs contain the independently compared source outlines without images or
foreign objects. Unit controls cover collection indices, public index changes,
logical-name changes and logical-family delegation. This establishes Rust
selection/outline transport, separately from Samsung appearance parity.

The [Chromium physical-font regression](../../web/tests/e2e/svg-face-identity.spec.ts)
loads the generated SVG as a standalone artifact. DevTools reports actual
PostScript faces `Roboto-Regular` and `Roboto-Bold` for the two `W` glyphs,
both with CSS weight 700 and one custom-font glyph per span. The SVG retains
source `WW`, two embedded aliases and no images/foreign objects. This proves
standalone SVG physical selection. Application preview uses SVG image URLs,
covered separately by existing embedding/pixel tests; inline SVG in the
application DOM is not established by this probe.

## Retained glyph PDF transport

Document PDF export consumes the shared Rust measurement and canonical
physical placement plans directly. A private registry retains source text,
resolved face bytes and face index, glyph IDs, UTF-8 cluster ranges, origins
and advances in both axes. The bundled SVG converter hooks the corresponding
text nodes under its existing transform, clip, opacity and blend scopes;
it does not reshape those registered glyphs. Registry entries must be handled
exactly once. Missing nodes or effects that bypass the hook fail explicitly.
SVG still supplies the surrounding vector graphics and clipping geometry.
`PdfOptions::from_font_book` retains the font database and native NAME
configuration. `PdfOptions::default` uses that constructor with the default
book, so direct document PDF and supplied-book exports share native measurement
admission. `PdfOptions::new(custom_database)` keeps native configuration absent
and uses compatibility measurement; an explicit supplied book overrides both
database and configuration. The supplied book also controls carrier parsing.
The CLI keeps the default configured book when no font files are supplied;
explicit files construct a caller database with compatibility measurement.
An actual Regular `AV` source-size-17 regression checks captured owner prefix
`265416 / 256 / 100`, f32 world placement, original outlines, selectable/tagged
source and no images across SVG, direct/default PDF and supplied-book PDF.
Inkless runs use a
private ink-bearing carrier that the hook replaces entirely; its glyph and
source never enter the PDF. Arbitrary object-bounding-box effects still derive
bounds from the carrier and are not established as native-equivalent.

`render_document_pdf` and `render_layout_pages_pdf_with_fonts`, including CLI
and WASM document export, use this retained path. `render_svg_pages_pdf` remains
the generic compatibility conversion for arbitrary or serialized SVG; that
round trip has no private glyph registry. The existing SVG text fallback and
its positioning diagnostics remain unchanged. Canonical layout geometry is
therefore separate from either transport's ability to reproduce it.
Raw SVG conversion returns `PdfError::UnsupportedText` when a complete private
physical alias is absent from the supplied database. Retained document plans
continue to draw their original face bytes even if carrier parsing uses a
different database. [Missing-font controls](../../crates/sdocx/src/pdf/font_identity_tests.rs)
verify both outcomes and the retained outline/source.

The native source supports this separation. HarfBuzz extraction stores the
whole cluster advance at its UTF-16 anchor (`0x9cd60`–`0x9cd70`); other entries
remain zero-initialized (`0x9b1e0`–`0x9b1fc`). The result aggregator retains
cluster-relative glyph positions (`0x9dc60`–`0x9dc98`), and `SpanRunFunctor`
caches glyph IDs and positions at the anchor (`0x77390`–`0x77418`). Canvas
painting adds the physical entry position and those retained XY offsets
(`0x660c4`–`0x660e8`). It does not distribute advance across a ligature's
constituent characters or reshape the chosen line for painting. These addresses
refer to the cached `libSPenText.so` ARM64 disassembly cited above; Rust UTF-8
source ranges remain distinct from native UTF-16 indices.

Samsung's PDF transport is narrower than that canvas contract. The ordinary
`getDrawnTextRun` branch retains glyph X (`0x67330`–`0x673fc`) but not each
cached glyph Y. Its common baseline is entry baseline plus caller offset and
RichText member 212 (`0x66d0c`–`0x66d14`, `0x672b4`–`0x672c4`), stored at
DrawnText offset 84 (`0x680e8`). Composer's ordinary-font writer consumes that
baseline (`0x380e28`–`0x380e44`); color emoji selects a different rectangle
endpoint. The [cached-run capture](table-code-findings.md#complete-retained-text-run-emission)
executes the complete ordinary emitter with supplied entries and font interfaces,
including baseline, X offsets, run unions and RTL reversal. It does not execute
shaping or the final PDF consumer. The Rust PDF path preserves full XY from
the canvas contract.
Arbitrary combining-mark Y parity with captured Samsung PDFs is not established.

Native synthesis has separate measurement, canvas and PDF contracts. The
logical drawing masks are bold `0x1`, italic `0x2` and underline `0x4`
(`0x90f88`–`0x90f94`, `0x90f08`–`0x90f14`, `0x90f58`, `0x91088`).
`FontFakery`'s low byte selects fake bold and its high byte selects fake italic
(`0x886bc`–`0x886e4`). The matcher requires requested weight at least 600 and
requested-minus-selected weight at least 200 for fake bold; fake italic requires
requested italic and a nonitalic selected face (`0x91478`–`0x9149c`). Rust retains
those decisions on the measured run after coverage fallback. Placed text requests
bold faces, while Flow text retains its regular-face measurement policy.

Canvas logical italic sets skew to −0.25 (`0x63c64`–`0x63c84`), after assigning
the physical typeface. `TextPaintImplSkia::setTypeface(Font*)` at `0x7be84`
only retrieves that physical typeface (`0x7bf38`–`0x7bf40`); it does not also
apply cached fakery. The separate fakery adapter subtracts 0.25 from existing
skew (`0x887f0`–`0x88814`), but the ordinary canvas call does not accumulate
that additional skew. Canvas path emboldening in `libSPenSkia.so` uses
`FT_MulFix(unitsPerEM, yScale) / 34` and skips already-bold FreeType faces
(`0x27e9ac`–`0x27ea40`). This is distinct from the PDF stroke policy.

Shaping applies base skew to retained offsets through
`x_offset − y_offset * skew` (`0x9cab0`). The active measurement helper
reads literal mask `0x4` for base skew and `0x2` for fake bold
(`0x76b84`, `0x76ba0`), unlike the logical drawing flags. Its caller passes
the same span buffer without shifting flags: `GetSpan` at `0x78d28`,
`AddStyleRun` at `0x78d30`–`0x78d48`, and the helper at `0x76ad4` through
`0x76800`/`0x76880`. `InitMinikinFontStyle` initializes weight 400 and
italic false (`0x76c18`–`0x76c2c`), then `0x76d18`–`0x76d24` copies the
final Typeface weight/italic metadata. The
[complete helper capture](#captured-complete-span-paint-helper) confirms this
override and the source-style setter effects. Device face selection and
fake-bold metric effects remain unverified. Rust paragraph measurement retains
its existing shaping advances, offsets and face-selection policy for this branch.

In cached Samsung Notes 4.4.45.37 ARM64 `libSPenPdf.so`,
`PdfiumTextHandler::DrawText` at `0xa2230` maps logical bold `0x1` to fill plus
stroke mode 2 and a constant **0.25 PDF-point** stroke
(`0xa2334`, `0xa2344`, `0xa2354`–`0xa235c`). Logical italic `0x2` adds
**+0.25 to the PDF Y-up text-matrix c term** (`0xa2348`–`0xa234c`, `0xa24a8`),
equivalent to −0.25 in unrotated Y-down geometry. This handler has no placed/Flow
or selected-bold-face exemption. `appendTextBlock` copies logical flags
(`0x68148`–`0x68158`), and Composer forwards those bits
(`0x3836d0`–`0x3836f4`). The writer scales rectangles and glyph X coordinates
before this handler (`0x380f80`, `0x380fac`–`0x380fb4`). A 1.8 world-to-point
ratio explains the historical 0.45 world-unit stroke; it does not establish a
fixed 0.45 stroke at arbitrary export DPI.

The retained Rust PDF adapter supports synthesized italic and logical bold.
It converts the 0.25-point pen to document units using export DPI. Cached typed
glyph-outline commands are sheared around each retained origin, then stroked
under the unsheared ancestor transform. Embedded selectable fills compensate
glyph origins and advances before applying the run shear, preserving physical
XY placement, including combining marks and nonzero vertical advances. Stroke
paths and font fills share one logical `ActualText` block. Fonts, paths and
derived geometry are validated before mutating the surface; normal neighboring
runs recover their original transform and paint state.

Rust keeps physically italic/oblique selected faces without adding a second
shear. That selected-face policy does not establish equivalence to the native
PDF handler's unconditional logical-italic term or close native font selection.
The 90-degree regression checks Rust's composition and pivot, not arbitrary
Samsung rotation-matrix parity. Variable-font instances remain unsupported;
bold glyphs with color, bitmap or SVG outlines also fail explicitly. SVG retains
the existing Flow 0.45-world-unit stroke and preserves requested synthetic
styles when overriding a coverage-fallback face. Browser SVG synthesis and
generic serialized-SVG PDF conversion do not establish the retained PDF
outline contract or the canvas emboldening policy.

Fallback-run coalescing requires equal retained synthesis metadata. Regressions
use a regular-only DejaVu Sans face with source `لالا`, styling the first two
scalars italic/bold and leaving the second span plain. Standalone, Flow, code
and table text preserve those boundaries, the complete source and the
complex-glyph positioning diagnostic.

Decorations use anchor-entry X and advance endpoints (`0x66758`–`0x6678c`),
not glyph ink bounds. Backgrounds use individual source-entry rectangles;
non-anchor constituents have zero advance. Partial-cluster background geometry
still reports `UnsupportedBackgroundPositioning` conservatively. Neither
transport invents proportional background or decoration widths within a glyph.
Adjacent compatible decorations share joined intervals in both adapters;
non-anchor decoration changes do not split a retained glyph.

Tagged PDF blocks carry full logical source through `ActualText`, independent
of visual glyph order. Viewers that ignore `ActualText` may still extract
ligatures, combining marks or bidi runs differently. The Rust scene records
semantic source scopes separately from paint order. Parent fragments sort by
scalar source position; embedded text descends at its object anchor, with code
title before body and table cells in row-major order. Numbered markers precede
their paragraph content. Each exported page occurrence contributes its own
ordered tag groups, including repeated selections. Glyph positions, transforms,
clipping and z-order retain the existing paint traversal. Readers that ignore
the structure tree can still expose paint order around inline objects.

Independent regressions traverse `StructTreeRoot` children and resolve
page-scoped marked-content IDs instead of assuming content-stream order. A
hand-built PDF has declared reading order `BCA` while its streams paint `ABC`.
Regressions cover inline code, nested objects, title/body, RTL, table cells and
repeated pages without changing the pinned glyph geometry. Numbered
paragraphs retain marker-before-content order. Glyph tests
pin DejaVu Sans bytes (SHA-256
`57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e`), native
HarfBuzz glyphs 5365/1399 and an isolated-run X of 58.173828125, then inspect
embedded outlines and PDF origins. Chromium regressions exercise embedded
fonts and logical-source PDF downloads through the actual WASM exporter.
These transport checks do not establish complete visual parity.

The synthesis regressions pin the same font and GIDs, verify per-glyph contours
independently through `ttf-parser`, and normalize PDF matrices and strokes at
72, 96 and 144 DPI. Additional contract checks cover regular-only fonts through
shape preview/replay and native PDF export, and mixed styles through both
native document PDF and generic SVG
conversion for placed text, Flow, code and table cells. Chromium exercises
regular and synthesized bold-italic Arabic through actual WASM preview and PDF
downloads, checking embedded font bytes, normalized shear, the physical pen,
exact logical source and absence of image resources or canvas.

[Vector text support](../text-vector-support.md) records implemented behavior,
captured native evidence and transport limits.

## Capture coverage limits

Standalone text-box cases for margins, vertical gravity, rotation, wrapping
and text-area modes are synthetic parser/renderer regressions. They do not
establish Samsung-exported standalone typography parity.

Captured coverage does not establish arbitrary mixed families/sizes,
overlapping styles, emoji, combining marks, RTL and CJK fallback selection.
Native font choices, baseline metrics and line breaks can differ from the
pinned-font contracts. Pixel/percentage spacing, indents, justification and
before/after gaps also have route-specific scaling limits.

Locked body/code/table origins do not establish arbitrary inline composition,
obstacle wrapping, explicit page breaks or merged/sparse tables. UTF-16 source
and paragraph ranges remain distinct from glyph clusters throughout these
contracts. Captured Samsung output supplies reference evidence; the authored
layout implementation remains Rust.
