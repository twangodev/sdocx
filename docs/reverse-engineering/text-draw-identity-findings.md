# Native text draw identity and glyph ownership

## Evidence

These findings concern Samsung Notes 4.4.45.37 ARM64 native text producers and
draw-run grouping. The APK SHA-256 is
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are virtual addresses in the named ELF, before harness relocation.

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenWidget.so` | `cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenText.so` | `5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenPdf.so` | `cdc62f9e02a3ef60e0dc504dbb13c4352accb811fb1ec629a7c8648954dd8f04` |
| `libSPenPdfiumB.so` | `4bd55ef116541205cb8aaf04812a317fe11911c0ff5a5d44526d98d6b0e48854` |
| `libSPenLibxml2.so` | `46753f76c8c007e78777e8fe7de7b57202f966f9494d4fba2b675c3540e35dbd` |
| `libSPenSkia.so` | `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d` |

The [entry/run-bound capture](table-code-findings.md#retained-text-entry-and-run-bounds)
and [complete run emitter](table-code-findings.md#complete-retained-text-run-emission)
establish grouping with supplied metrics, glyph caches and font interfaces.
The captures below separately exercise the producers of span identity, font
metadata and glyph-cache ownership. They do not establish native font selection
or shaping for an actual document.

## Span identity

Text `inSameDraw`, `0x65998`, compares spans through `RichTextSpan::operator!=`,
`0x8db40`. It compares selected members, rather than the complete 72-byte record.

| Offset | Representation | Producer meaning |
| ---: | --- | --- |
| 0 | `f32` | Resolved font size |
| 4 | `u32` ARGB | Foreground color |
| 8 | `u32` ARGB | Background color |
| 12 | `u32` ARGB | Composing background color |
| 16 | Style bits | Bold 1, italic 2, underline 4, strikethrough 8, suggestion `0x10`, spell correction `0x20` |
| 24 | Owned `SPen::String` pointer | Font name, compared by string value |
| 32 | `u32` ARGB | Suggestion/spell-correction underline color |
| 36 | `u32` ARGB | Spell-correction foreground color |
| 40 | Flag byte | Hypertext bit 0, object-span bit 1 |
| 66 | Boolean byte | Spell-correction foreground enabled |

Font-name equality uses `String::CompareTo`; different allocations with the
same name compare equal. Case changes compare different. A null name is the
default absent name and differs from a nonnull empty name. Font size uses
native floating-point comparison: positive and negative zero compare equal,
while NaN does not compare equal. The captured semantic comparison also covers equal names
containing Chinese characters and a surrogate pair. Padding is not identity:
bytes 17–23, 41–43 and 68–71 are not initialized by the captured constructor.
Members 44, 52 and 60 are ignored
by this predicate. Inline byte 64, over-pages byte 65 and math-answer byte 67
also do not participate in this span comparison.

Widget's default vector element constructor, `0xc0d38`–`0xc0dac`, writes size
17, foreground `0xff000000`, zero background/composing background/style,
null font name, and `0xff000000` for both underline and correction foreground.
Flags and the remaining named fields are zero. `ObjectTextLayout::updateSpan`
replaces the object defaults at `0xd4fdc`–`0xd5004`: size is
`(17 + delta) * scale`, and foreground is the theme interface's result for
`0xff262626` with usage 3. This default size operation is unclamped.

### Object span conversion

Widget `ObjectTextLayout::convertTextSpanImpl`, `0xd7ba8`, applies a concrete
source span to a slot. Its dispatch recognizes foreground, font size, font name,
bold, italic, underline, hyperlink, composing background, composing underline,
background, composing tag, strikethrough, suggestion and spell correction.
Other dispatched types leave these members unchanged.

The ordinary conversion resolves font size as
`max(1, stored_size + delta) * scale`. Foreground assignment also sets the
math-answer flag when the source color type is 1. That flag can differ while
the compared foreground and complete draw identity remain equal. Ordinary and
composing backgrounds remain separate members. Background assignment skips an
object-marked slot; setting the object bit first can therefore change the
result of subsequent conversion calls.

Suggestion conversion writes style bit `0x10` and its underline color.
Spell correction writes style bit `0x20`, correction foreground and underline,
and the foreground-enable byte. Enabling strike sets bit 8; a false strike
property leaves an existing bit 8 unchanged, including when correction
foreground is disabled. Disabling correction foreground
does clear byte 66. A hyperlink type
in 1–9 sets the hypertext bit; 0, 10 and the captured unknown maximum value
clear it. Composing-tag conversion assigns theme-mapped `0x19252525` to
ordinary background when enabled, or adds style bits 1, 2 and 4 in the alternate
branch. Neither branch clears the other fields. Composing underline (type 16)
unconditionally sets underline bit 4; its false property does not behave like
an ordinary underline span's false property, which clears that bit.

`ObjectTextLayout::convertTextSpan`, `0xd56b8`, traverses
`ComponentText::FindSpans` results and applies each span to the intersection of
its half-open UTF-16 range and the update range. Every selected code unit gets
its own slot; later traversed spans can overwrite the same member. The
capture supplies conversion call order directly, so it does not prove native
`FindSpans` ordering for a serialized document.

`moveTextBackgroundColor`, `0xd5d9c`, chooses nonzero composing background
ahead of ordinary background for its separate background data. It does not
clear either raw span member. Equal painted backgrounds can therefore retain
different span identities. `TextLayoutUtil::convertSpanImpl`, `0xdb894`, is a
separate producer with different size, theme and correction behavior; it does
not replace the object-text conversion contract.

### Span capture

[`table-text-span-identity.json`](../../conformance/table-text-span-identity.json),
SHA-256 `42f53231b1f764cd66325609a13995e84964cb4d8b3978922b1b77fb289710f7`,
contains 70 independently constructed span pairs. The
[Rust harness module](../../conformance/native_table/text_span_identity.rs)
executes native default vector allocation/initialization, the object-default
assignment window, complete object span conversion, actual Model property
getters, Text flag setters, Base string construction/copy and member comparison.
Named outputs repeat with memory fills `0x00`, `0xa5` and `0xff`.

Source types/property buffers, layout delta/scale and theme inputs are supplied.
The theme interface uses an explicit color XOR transformation in one case;
it does not implement Samsung theme behavior. Allocation, deletion and memory
initialization are host interfaces. Serialized span decoding, range/caret
selection, shaping, font selection, run emission, clipping and rendering do
not execute in this capture.

### Native binary boundaries

[`table-text-span-binary.json`](../../conformance/table-text-span-binary.json),
SHA-256 `0e8437fead4285c0ead18349f8708309219c74c6aae8a2acc83bec9fdbbc1c7b`,
contains 237 cases: 17 full version-7 records, 20 full version-8 records and
200 truncated version-8 records, using document type 2. The
[capture module](../../conformance/native_table/text_span_binary.rs) executes
`TextStyleFactory::CreateSpan`, `0x415db4`, complete native
`ApplyBinary`/`GetBinary`/`GetBinarySize`, property getters, Base String
conversion and native List operations. Native WDoc header reading at `0x40cfb4`
and buffer checks at `0x2784a4` execute unchanged. Every serialized result
repeats with memory fills `0x00`, `0xa5` and `0xff`.

| Span | ApplyBinary | GetBinary | GetBinarySize | Captured binary contract |
| --- | --- | --- | --- | --- |
| Foreground, type 1 | `0x40a858` | `0x40a78c` | `0x40a728` | ARGB `u32`; version 8 adds color-type `u32`, and the writer emits both |
| Font name, type 4 | `0x4097fc` | `0x40974c` | `0x4096d0` | Reserved bytes, `u16` byte length including NUL, then string bytes |
| Composing background, type 15 | `0x415684` | `0x41567c` | `0x415674` | Reads ARGB and skips the version-8 tail; size is -1 and writing returns false |
| Composing, type 16 | `0x4150a8` | `0x4150a0` | `0x415098` | Reads one nonzero-normalized boolean, skips three padding bytes and the version-8 tail; size is -1 and writing returns false |
| Composing tag, type 18 | `0x415c0c` | `0x415c04` | `0x415bfc` | Same boolean/padding/tail layout; size is -1 and writing returns false |
| Suggestion, type 21 | `0x418bf8` | `0x418a30` | `0x418908` | Suggestion type, underline ARGB and string count as `u32`, followed by `u16` UTF-16 unit count and units per string |
| Spell correction, type 22 | `0x419770` | `0x419768` | `0x419760` | Applying and writing return false; size is -1 |

These methods distinguish an in-memory span from a writable persisted span.
In particular, the successful Widget correction conversion above does not
establish a persisted correction payload. The native correction binary methods
reject the operation before consuming even the supplied header.

Font-name version 7 skips four reserved bytes; version 8 skips eight. The writer
always reserves eight bytes without initializing them. Capture output buffers
are explicitly zeroed, so their zeros are capture initialization rather than
native serialization of a meaningful zero field. Factory construction creates
a nonnull empty String; the captured binary format has no null-name marker.

Base `String::Set(char*)`, `0xc3934`, reads the supplied NUL-terminated name.
Native decoding accepts both UTF-8 and CESU-8 for `字体𝄞`. Native writing emits
the supplementary character as two encoded surrogates
`ed a0 b4 ed b4 9e`, increasing the byte count relative to UTF-8.
An embedded NUL in `Ro\0boto` truncates the native String to `Ro`, while the
reader's consumed count advances through the complete declared field. The
writer then emits only the canonicalized name. Missing terminators and
zero-length fields are outside this capture because their native C-string
read is not bounded by the declared field. Rust's bounded rejection of those
inputs is a separate safety policy.

Suggestion strings use UTF-16 unit counts, including surrogate pairs. Empty
entries are omitted from the decoded list and subsequent serialization. A
count of `0xffffffff` produces an empty list through the native signed loop
condition. The native consumed-byte counter adds each string's unit count
rather than twice that count, even though its read pointer advances through
the correct UTF-16 bytes. The fixture records this discrepancy: the two-string
48-byte case reports 40 bytes consumed. It does not normalize the native
counter into a correct byte offset.

Truncated records can leave partial header or property mutations after failure;
the capture records those states. They retain the complete initialized backing
record and reduce only the native available-count argument; they do not place
an unmapped-memory boundary at that offset. Every source record is at most 256
bytes, names at most 64 bytes, and suggestion strings at most 64 UTF-16 units.
Supplied suggestion lists contain at most four entries apart from the explicit
negative-count case. All records use the 16-byte WDoc header; the legacy
12-byte document header path is outside this capture.
Allocation, deletion, memory copy/move,
initialization, string-length and single-thread mutex operations are host
interfaces; diagnostic logging is isolated. Whole-document parsing, native
span traversal, Widget conversion, shaping, grouping and rendering do not
execute in this binary capture.

## Measurement identity

Native measurement joining is a different predicate from retained draw
identity. Text `RichTextSpan::JoinableForMeasureTo`, `0x8dc28`, rejects an
object-span flag on either side, then compares:

| Member | Equality used for measurement |
| --- | --- |
| Font size, offset 0 | Native f32 comparison |
| Foreground ARGB, offset 4 | Complete `u32` equality, including alpha |
| Style bits, offset 16 | Difference masked by `0xc3` |
| Font name, offset 24 | Nullable pointer identity, or semantic native String comparison |

Equal positive and negative zero sizes join. A quiet NaN does not join,
including a self comparison; equal positive infinities join in this isolated
predicate. This does not establish that document producers admit such values.
Foreground transitions that differ only in alpha split measurement even when
the painted RGB is unchanged. The predicate ignores both backgrounds,
underline/correction colors, hyperlink bit 0, other nonobject flag bits,
link metadata and inline/over-pages/correction/math-answer flags. Underline,
strike, suggestion and correction style bits below `0x40` do not split it.
Draw identity separately compares those paint fields.

Widget foreground conversion at `0xd7c04`–`0xd7c20` passes the complete source
color through context usage 3 and stores mapped ARGB in span member 4, without
a raw-zero or alpha-zero guard. Color type is queried afterward at
`0xd7c28`–`0xd7c34` and sets only the ignored math-answer byte 67.
Hyperlink blue is applied later to retained draw output at
`0x68164`–`0x68178`; correction foreground is applied later in text-paint
setup at `0x63cd0`–`0x63ce8`. The measurement predicate therefore compares
mapped span foreground before those display overrides. Equal rendered link
or contrast colors do not establish equal native measurement identities.

Two null font names join, and a shared nonnull pointer joins immediately.
One null name and one nonnull empty name do not join. Independently allocated
equal or empty names join through Base `String::CompareTo`, `0xc49e4`, and
native UTF-16 comparison at `0xc204c`; different case or length can split them.
UTF-8 and CESU-8 inputs that construct the same UTF-16 surrogate pairs join.
Embedded-NUL constructor inputs retain their native prefix; the capture does
not establish preservation or comparison of trailing data after the NUL.

[`table-text-measurement-join.json`](../../conformance/table-text-measurement-join.json),
SHA-256 `45cfbe69af0ba5a931c5a06a93b289af44e3723f68790bb82c0c6da59b218e2b`,
contains 291 cases, including all 256 right-hand style values, f32 boundary
cases, full-ARGB transitions, object flags, ignored members and nullable/string
variants. The
[Rust capture module](../../conformance/native_table/text_measurement_join.rs)
executes complete native joining and String comparison, with native Base
construction from UTF-8/CESU-8 or explicit UTF-16 units. Forward, reverse and
both self comparisons repeat with memory fills `0x00`, `0xa5` and `0xff`.
Span members and bounded string inputs are supplied; allocation, deletion,
initialization, string length and memory copy are host interfaces. Widget
conversion, document parsing, shaping, font resolution, the producer's
measurement-run loop and painting do not execute.

## Preview and Composer backgrounds

Native ordinary and composing backgrounds have different consumers. Their
separate raw span members cannot be replaced by one resolved paint color.

| Span | Object conversion | Native table preview | Native Composer vector export |
| --- | --- | --- | --- |
| Composing background, type 15 | Theme-mapped ARGB into member 12, unless the Widget slot is an object | Nonzero mapped composing ARGB overrides ordinary background | Composing background is not copied to retained `DrawnText`; ordinary background remains |
| Composing underline, type 16 | Sets underline bit 4 regardless of the source boolean value | Underline flag | Underline flag retained and painted |
| Composing tag enabled, type 18 | Theme-mapped `0x19252525` into ordinary background member 8 | Ordinary background, unless composing overrides it | Ordinary background retained |
| Composing tag disabled, type 18 | Sets bold, italic and underline bits 7 | These style flags | These style flags retained |

Widget's object update establishes object identity before ordinary spans:
`updateSpan`, `0xd4e80`, calls `convertObjectSpan` at `0xd5014`, then text
conversion at `0xd502c`; object conversion marks the slot at `0xd54f4`.
Its type-15 and type-17 handlers test that identity at
`0xd7de8`–`0xd7df0` and `0xd7e20`–`0xd7e28`. Type 18 has no corresponding
object-background guard. Drawing's `convertTextSpanImpl`, `0x90de8`, has no
type-15/type-17 object guards in its handlers at `0x9102c`–`0x91050` and
`0x91058`–`0x9107c`.

Tables use the Widget producer: Drawing `ObjectTableCellLayout` construction at
`0x8c008` creates Widget `ObjectTextLayout`, and Drawing table initialization supplies
its content through `SetObject` at `0xaad30`. Widget copies effective
backgrounds to a separate vector at `0xd5080`; Drawing performs the same
nonmutating selection in `moveTextBackgroundColor`, `0x8da30`.

Drawing `ObjectTableCellLayout::DrawTextContent`, `0x8c0c0`, reaches Text
`DrawRect`, `0x8b578`, `RichTextDrawing::drawRect`, `0x64bec`, and its background
painter at `0x64f18`. The painter obtains the raw span at `0x64fdc`, reads
ordinary/composing colors at `0x64fe0` and selects composing when its complete
ARGB value is nonzero at `0x64fe4`–`0x64fe8`. This tests the complete color
word after theme mapping, not alpha or the raw source value alone. Widget
type 15 calls the theme interface at `0xd7dfc`–`0xd7e14`, with usage 3,
before storing mapped member 12 at `0xd7e18`. Base `ViewContext::GetColor`,
`0x6c39c`, selects the active theme at `0x6c3a4`–`0x6c3c4`, without a zero
or alpha guard. Base dark-theme conversion at `0xe51a4` temporarily forces
opaque alpha at `0xe51b4` for HSL conversion and restores source alpha at
`0xe51bc`. It maps transparent black `0x00000000` to `0x00ffffff`, and
transparent white `0x00ffffff` to zero. Consequently, transparent black can
override ordinary background in dark preview, while transparent white can
fall back to ordinary. An alpha-zero color with nonzero mapped RGB still
overrides ordinary background selection.

[`table-text-background-theme.json`](../../conformance/table-text-background-theme.json),
SHA-256 `77bacb34244a83f35e2b7aeea51ab6fc92fe6a71f18dabebb08163c29da7edb3`,
records 12 dark-theme color cases, repeated with memory fills `0x00`, `0xa5`
and `0xff`. The
[Rust capture module](../../conformance/native_table/text_background_theme.rs)
executes Base's complete theme constructor at `0xe5168`, `GetColor` at
`0xe51a4`, `getColorByLightControl` at `0xe51d0`, `ColorToHSL` at `0xe48f8`,
`RGBToHSL` at `0xe4964` and `HSLToColor` at `0xe4b34`. Only libc `fmod` is
host supplied; its two calls per case retain double argument/result bits.
Cases cover black, near-black, white and a colored sentinel at alpha zero,
their alpha-one counterparts, and black/white at alpha 128 and 255. Nonzero
alpha-one sentinels remain nonzero after mapping. Recorded input/output
nonzero predicates describe those integers, rather than executing Widget
background selection. This color-only capture has no density or geometry
inputs and no span conversion, painting or pixels; it does not establish
general bit-exact Rust/native RGB rounding.

The native background painter has no object-span exclusion. At `0x65004`–
`0x65018` it reads an 80-byte retained entry, derives its rectangle at
`0x65030`–`0x6505c` and paints at `0x6507c`. An enabled composing tag can
write ordinary background on an object slot and reach that rectangle path.
This is a source-traced converter/painter contract, without a complete
embedded-object preview capture.

### Captured embedded-object background geometry

[`table-text-object-background.json`](../../conformance/table-text-object-background.json),
SHA-256 `adc0d81004f5b372cf3f6cddc6c34aade8d9728339bc25056a5494cf71b0e076`,
contains 40 cases and 37 rectangle commands. The
[Rust capture module](../../conformance/native_table/text_object_background.rs)
executes native entry initialization, `0x65920`, complete object-span
measurement, `0x779d0`, placement through `SetLayout`, `0x6b4a4`, and
`GetBaseline`, `0x6cb0c`, followed by `GetSpan`, `0x61f3c`, and complete
`drawBackgroundColor`, `0x64f18`. Actual Base rectangle helpers supply setting,
width/height, offset and union. Results repeat across memory fills `0x00`,
`0xa5` and `0xff`.

Native preview background geometry uses the placed entry's line band, rather
than only the embedded object's visible rectangle. Its X is entry X plus draw
offset X; its Y is layout top plus draw offset Y; width and height come from
the actual entry layout rectangle at `0x65030`–`0x65070`. A 30-unit inline
object with left/right margins 4 and 4 measures and paints 38 units wide.
A block object reserves 100 units of advance in the supplied 120-unit context
with 10-unit side margins, but its background paints the visible 30-unit
width. With object height 40, font metric 17 and line-spacing multiplier 1.35,
the captured default layout band is about 45.951 units high, including leading
and the object-metric epsilon. Stored/supplied top margins use a separate
placement branch.

NaN width and height independently fall back to the supplied font metric.
A zero-width entry still dispatches a zero-width rectangle. Background selection
uses nonzero mapped composing ARGB ahead of ordinary ARGB and skips a zero
selected word; it has no alpha-only gate or object-kind exclusion. Transparent
nonzero colors therefore still produce recorded rectangle commands.

Dimensions, margins, font metric 17, context width/margins, mapped span colors,
offsets and a single-entry logical map are supplied. Line/block metrics are
explicitly supplied using the native measured advance/height plus that font
metric and margins. `GetBlockInfo`, wrapping and obstacle selection do not
execute. Paint construction/style/color/destruction, memory copy and rectangle
recording are host interfaces. Native font resolution, shaping, Widget object
conversion, theme mapping, glyphless retained-run emission, Composer/PDF
background policy and pixels are outside this capture. It establishes preview
entry geometry independently of the export route.

Composer table `GetDrawnTextData`, `0x37ef00`, reaches Text drawing through
Drawing `ObjectTableCellLayout::GetDrawnTextData`, `0x8c084`, whose dispatch at
`0x8c098` reaches Text `TextLayout::GetDrawnText`. Text `appendTextBlock`, `0x67ebc`, stores
ordinary span member 8 into `DrawnText` member 144 at `0x68140`/`0x68160`; it
does not copy composing member 12. Composer's table background writer,
`0x37f308`, reads member 144 at `0x37f480`; ordinary text's writer,
`0x380b58`, reads it at `0x380ce0`. Text retains the style byte at
`0x68148`/`0x68158`; Composer tests the table underline bit at
`0x37ef84`–`0x37ef88` and dispatches its paint at `0x37ef9c`.

Composer table glyph writing skips object records at `0x37f55c`–`0x37f560`;
the table background writer at `0x37f308` instead gates background alpha byte
147 and has no object exclusion. Current Body export instead checks member
120 at `0x349e00`–`0x349e04` and skips the background call at `0x349e30` for
objects. Its following loop dispatches objects separately at
`0x349e54`–`0x349e6c`. The legacy Body caller makes the same exclusion at
`0x375ef0`–`0x375ef4`. Table callers at `0x3507d0`–`0x3507e4` and
`0x37ef38`–`0x37ef50`, code title/body callers at
`0x378b28`–`0x378b40`/`0x378ff0`–`0x379008`, and the placed caller at
`0x380790`–`0x3807a4` have no such object exclusion.

The shared newer `TextPdfExporterUtil::CreateTextBackgroundPath`, `0x355120`,
gates foreground alpha byte 139 at `0x355150` and reads layout member 104 at
`0x355184`, without its own object exclusion. Legacy background writers gate
background alpha byte 147 instead. The caller's object policy and the helper's
alpha policy are therefore separate contracts; the direct newer helper does
not establish that Body callers paint object backgrounds.

This producer/consumer trace establishes the different selected background
inputs. Supplied span-conversion and retained-emitter captures separately
exercise raw members and copied `DrawnText` values. They do not capture a
complete table preview image or native PDF for these composition spans.

### Captured retained object runs

[`table-text-object-runs.json`](../../conformance/table-text-object-runs.json),
SHA-256 `23d9bc46664c98c445f7f08c42c9a0d64f41c7827f3df77bc8fbc21c472b7985`,
contains 80 cases repeated across three memory fills: 54 published retained
object records and 26 isolated-helper controls. The
[capture module](../../conformance/native_table/text_object_runs.rs) executes
native measurement construction, `0x78090`, initialization, `0x790d0`, the
`SpanRunFunctor` window `0x77324`–`0x77894`, object measurement, `0x779d0`,
`SetLayout`, `0x6b4a4`, cached `GetGlyphInfo`, `0x78274`, and complete public
`GetDrawnText`, `0x68418`, retained emission and `appendTextBlock`, `0x67ebc`.

Producer-window cases supply one or two shaped records owned by UTF-16 index
zero, codewords/positions/ink, advance 1000 and a cached Font interface with
ID 7, bitmap false, language `en` and drawable true. The real native window
appends those records at `0x773e0`–`0x774e4`, then object measurement at
`0x77844` overrides their dimensions. Supplied single-line/block metrics
propagate captured object advance/height with font metric 17, spacing 1.35,
cursor 3.25 and block bounds. Native placement and emission publish object
flag member 120, layout band member 104 and ordinary background member 144.
Composing background is not copied, including composing-only cases; alpha-zero
nonzero ordinary ARGB is retained. This connects actual native object
dimension override and line placement to the retained fields consumed by
the table background writer, independently of that writer's alpha gate.

The isolated-helper controls omit upstream glyph accumulation and call object
measurement directly on the native-initialized empty cache. That helper does
not populate it; this does not establish empty object caches in production.
The full upstream operator normally assigns font/drawable state at
`0x77590`/`0x775d4` before its object-helper call.

With the isolated empty cache, retained emission reaches `appendTextBlock`,
`0x67ebc`, whose unconditional first-codeword load at `0x68144` reads a null
glyph-vector pointer. Guest address zero is explicitly unmapped, preventing
the Model ELF header from supplying accidental glyph bytes. The capture
records the terminal read and incomplete allocated fields; no retained text
record is published. The default-entry control reaches the same precondition.
This is evidence for an omitted producer input, rather than an application
crash or a valid empty retained object record.

Retained span/context geometry, mapped colors, a single U+FFFC source and
offsets are supplied. Producer records and cached font interfaces bypass
actual font construction/selection and shaping. Allocation, deletion, bounded
copy/move, mutex operations and font getters are host interfaces. Full Widget
conversion, upstream span dispatch, line/block metric production/wrapping,
outer `GetDrawnTextData` dispatch, Composer background policy/painting,
clipping and final output remain outside the capture. Body export has separate
object filtering; published table-consumer inputs do not establish universal
native object composition parity.

### Captured object export caller policy

[`table-text-object-export-policy.json`](../../conformance/table-text-object-export-policy.json),
SHA-256 `63bca0a1f43a8a828c5127c4899d5ac63837e59d78ced56da34cedaf0520134c`,
contains 204 cases across 14 native instruction windows. The
[capture module](../../conformance/native_table/text_object_export_policy.rs)
executes seven background-caller iterations, five alpha gates and two Body
object/text dispatch iterations. Each case repeats across fills `0x00`,
`0xa5` and `0xff`; independent captures are byte-identical.

Both Body background callers skip nonzero object member 120 and subsequently
dispatch those records to object export. Table, code title/body and placed
background callers request background export for non-null object records.
The captured legacy gates read background alpha byte 147: Body
`0x376230`–`0x376254`, table `0x37f354`–`0x37f374`, code
`0x37978c`–`0x3797ac` and placed `0x380ba0`–`0x380bc0`. The shared newer
gate at `0x355150`–`0x355184` instead accepts nonzero foreground alpha byte
139. These observations establish caller filtering and helper branch decisions
separately from retained object-field production.

Inputs supply a zero-initialized 160-byte `DrawnText`, layout rectangle at
member 104, object flag 0/1/255 at member 120, foreground/background ARGB at
members 136/144, a pointer or null, and bounded stack/register state. Four
foreground/background pairs distinguish alpha 0, 1 and 255. Native call targets
are intercepted to record the requested background/object/text dispatch and
return. Caller windows stop before advancing their vectors; gate windows stop
before allocation or rectangle access. Full callers, object/font producers,
PDF allocation, clipping, paths, painting, pixels and native SVG consumption
do not execute. In particular, the newer foreground-alpha rule is captured,
but complete native output for those alpha-edge cases is not.

### Captured PDF alpha transport

[`table-text-pdf-alpha.json`](../../conformance/table-text-pdf-alpha.json),
SHA-256 `4cc777240688534f6a967f4e741bef858d851c615d260a7a827327fed25222d1`,
contains 60 cases repeated across three memory fills. The
[capture module](../../conformance/native_table/text_pdf_alpha.rs) executes
Composer color/alpha instruction windows with complete native PDF paint/engine
setters and getters, followed by the native PDF text RGBA extraction window.
Independent captures are byte-identical.

Composer's color window, `0x383710`–`0x383724`, forwards complete foreground
ARGB through PDF `PDFPaint::SetColor`, `0x665b4`, and its engine setter,
`0x73338`. An alpha setter subsequently replaces that stored alpha byte; it
does not multiply the previous byte. The path through `0x665a0` and `0x73314`
stores the truncated integer result of f32 alpha times 255. Native getters
at `0x7585c`/`0x7583c` expose the final color/alpha. The RGBA extraction window,
`0xa2368`–`0xa2390`, forwards that byte to the intercepted PDFium fill-color
call.

| Legacy writer path | Captured alpha calculation |
| --- | --- |
| Table foreground, `0x37f5f4`–`0x37f62c` | Signed loading of source alpha treats bytes 128–255 as the full-alpha branch. Bytes 0–127 use source alpha / 255; the result is multiplied by writer opacity and quantized once. |
| Code foreground, `0x3799f0`–`0x379a04` | Writer opacity replaces the source foreground alpha. |
| Background, `0x37978c`–`0x3797a4` and `0x3798b4`–`0x3798e0` | Unsigned source background alpha / 255 is multiplied by writer opacity, then quantized once. |
| Foreground color without a later alpha setter | Complete source ARGB survives the captured paint transport. |

Cases include alpha 0, 1, 127, 128, 254 and 255, with supplied writer opacity
0, 0.5 and 1. Alpha-zero background cases execute setter transport after the
omitted caller gate; they are not evidence of reachable background paint.
The full Body writer, `0x3765d8`, and placed writer, `0x380d4c`, have no later
`SetAlpha` after paint setup in the inspected source. Their complete functions
are not executed by this capture.

Retained colors, writer opacity, paint/engine wrappers, native function vtables,
raw paint storage and live register state are supplied. The PDFium fill-color
call is intercepted without allocating or painting a PDF object. Full wrappers,
export-route selection, font data, shaping, clipping and pixels are excluded.
The [Standard export route](standard-pdf-composition-findings.md#text-writer-route-selection)
does not select these standalone helper paths for every page-object context.
Newer export information member 60 is scale, not opacity: native text handling
multiplies font size by it at `0xa1dd0`–`0xa1de0` and forwards source ARGB
directly at `0xa1e28`–`0xa1e4c`.

## Font metadata

Text `Font::GetSourceId`, `0x85d7c`, forwards through implementation virtual
slot 48. `FontImplMinikin::GetSourceId`, `0x877f8`, forwards through source
slot 80; `MinikinFontImplSkia::GetSourceId`, `0x88ad4`, reads source member 120.
The source constructor reads the created SkTypeface's member 16 at
`0x880d4`–`0x880d8`. Skia `CreateFromStream`, `0x2197b8`, allocates a fresh
typeface; `0x219848`, `0x219860` and `0x219868` obtain the counter, increment
it and store the instance ID. This is source-instance identity, rather than a
family/style pair or hash of font bytes. The getter capture alone does not
establish font-manager reuse; the live-registry capture below bounds reuse to
its initialized registrations.

`Font::IsBitmapFont`, `0x85d98`, forwards through
`FontImplMinikin::IsBitmapFont`, `0x87814`, to the source predicate at
`0x88ae4`. That predicate scans the supplied typeface's table tags for exactly
`CBDT` (`0x43424454`). A null typeface returns false. `COLR`, `SVG ` and `sbix`
alone do not satisfy this native predicate; “bitmap” here names that particular
native test, rather than every possible color-font format.

`Font::GetLanguage`, `0x85db0`, forwards to the implementation getter at
`0x87834`, which returns its NDK string at member 32. Constructors at
`0x873c4` and `0x87564` copy supplied language; weight and italic occupy
separate members 24 and 28. XML family parsing at `0x7cf3c`–`0x7cf64` reads
the `lang` attribute, defaulting to the empty string. The value passes through
`readFont`, `0x7d11c`, and constructor helpers `0x7ee10`, `0x83068` and
`0x83150`.

The complete emitter splits after a bitmap font or a font whose language is
exactly the eight bytes `und-Deva`, even when `inSameDraw` otherwise passes.
This is font-family metadata. It is not a check of the text's Unicode script
or the script passed to HarfBuzz, and a Devanagari-shaped run alone does not
establish that this gate applies.

[`table-font-metadata.json`](../../conformance/table-font-metadata.json),
SHA-256 `68a3f9315805a112c4930bcc0459e5c36c668491a01b9daca02036df5c01fccb`,
contains 60 cases: 12 table-tag variants across five metadata/null states.
The [capture module](../../conformance/native_table/text_font_metadata.rs)
executes the complete bitmap scan and actual Font/implementation/source getters
with supplied source IDs, short/allocated language strings, interface vtables
and typeface table-tag results. Every result repeats across three memory fills;
the cases perform 132 table scans per fill. Font factories, native XML parsing,
constructor chains, source reuse, font selection and shaping do not execute in
this capture.

### Line feeds and emoji resources

Native entry kind 4 marks a line feed: Text `RichTextMeasure::IsLineFeed`
tests member 48 against 4 at `0x78490`–`0x78498`. `measureParagraph` writes
that kind to the paragraph-leading separator at `0x78a88`–`0x78a8c`, zeros
its advance and skips it during subsequent span measurement.

Emoji collection instead operates inside drawable glyph emission.
`getDrawnTextRun` requires an optional `EmojiFontSlices` output and selected
Font, then calls `FontManager::IsColorEmojiFont` at `0x674d4`. It stores the
physical source ID at slices member 40 (`0x67504`), groups glyphs by high byte
(`0x67500`) and keys the inner map by low byte (`0x67888`, `0x6799c`).
The map retains owning source UTF-16 units, including kind-3 continuation slots
(`0x6792c`–`0x67940`, `0x67a00`–`0x67a80`); it contains no glyph pixels.
Canvas `drawGlyphs` separately selects `drawCacheEmoji` when its bitmap-font
predicate is true and its caller flag is false (`0x66a48`–`0x66a90`). The cache
draws the glyph into an allocated TextBitmap (`0x61410`–`0x61490`, `0x693ac`–`0x693e8`),
then canvas DrawBitmap consumes it (`0x66c38`–`0x66c3c`).

Composer `PDFWriterUtil::LoadBitmapFont`, `0x383878`, obtains font bytes,
length and face index from the selected physical face's runtime registry
(`0x3838ac`–`0x3838bc`), then passes its descriptor and copied slice map to
writer slot 176 (`0x383910`–`0x38392c`). These source bytes are not proof of a
font asset owned by the `.sdocx`. Pdf `PdfiumImpl::LoadBitmapFont`, `0x8dc94`, forwards
bytes and maps to PdfiumB `FPDFText_LoadBitmapFont`, `0x58d168`. That branch
loads CFX_Font with face index zero (`0x58d228`), creates Type3
font resources (`0x58d3f0`–`0x58d428`) and renders selected glyphs through
FreeType (`0x593ca0`/`0x593cb0`). RGB image streams with an optional alpha mask
back glyph procedures using `/X... Do` (`0x593b18`–`0x593b58`). This proves
derived image-backed glyph transport, not vector outlines or font-program
embedding in the final PDF. Arbitrary color/pixel-mode parity is unverified.

This is static source evidence, separately from the cached emitter fixtures.
No drawable kind-4 emoji producer is established. Rust rejects drawable kind-4
states and lacks this resource producer; its retained PDF outline path rejects color/image glyphs.
The [Rust transport boundary](text-layout-findings.md#retained-glyph-pdf-transport)
preserves source text and physical font bytes without general text rasterization.

### Captured font-family language

[`table-text-font-language.json`](../../conformance/table-text-font-language.json),
SHA-256 `e1e50b79052bf8fd6f3501ff91f8b8fa41258c511c795385a295755b48364149`,
contains 26 supplied XML cases and 39 intercepted font records. The
[capture module](../../conformance/native_table/text_font_language.rs) executes
actual native libxml parsing at `0x75890`, root/property access at
`0x7cfdc`/`0x7dacc`, and Text `FontListParser` construction/readFamily at
`0x7c8e0`/`0x7ce7c`. Native `readFont` is intercepted at its PLT entry,
`0xeffb0`, before execution. It records the supplied family language/name and
parsed font filename, then returns false without creating a font record;
`readFamily` consequently returns false.

The captured producer preserves absent language as the literal empty string,
not `und` or a guessed script. It also preserves explicit empty, `und`,
`und-Deva`, casing, spaces, comma lists, allocated UTF-8 strings, supplementary
characters, XML entities/newline references and duplicate records. A supplied
family named Roboto and its filenames are inputs, not evidence of Samsung's
selected Roboto font or language. Three memory fills and independent repeats
produce identical observations.

Host interfaces provide allocation, byte operations, single-thread bookkeeping
and fixed time/random state. Font loading, Skia typeface creation, numeric
source IDs, bitmap metadata, shaping, selection and rendering do not execute.
Text `DeviceFontManager`, `0x84684`, reads device `/system/etc/fonts.xml` and
`/system/fonts/`, with conditional additional configuration. The inspected APK
has no `fonts.xml`, `fonts_additional.xml`, `fallback_fonts.xml` assets or Roboto
font bytes. The fixture does not recover those device inputs.

Separately, the source-traced file-font creation chain at
`0x87930`/`0x87a94`/`0x87ef0` reaches Skia's fresh typeface allocation and
process counter. The successful branch increments global counter `0x2ac318`
at `0x219848`–`0x219868`; Text copies the created typeface ID into source
member 120 at `0x880d4`. This identifies each created instance, rather than
font-file bytes. It does not establish arbitrary font-manager cache reuse.

### Captured file-font source instances

[`table-text-font-source.json`](../../conformance/table-text-font-source.json),
SHA-256 `10056b7117f979ba9e8bf846e4eb2b7f38aba4737191fdd66a1b4d5e0d0ea485`,
contains 24 cases and 48 independently created native sources. The
[capture module](../../conformance/native_table/text_font_source.rs) executes
Text's file-font constructor, `0x85604`, its load path at `0x88214`, Skia
`CreateFromStream`, `0x2197b8`, and bundled FreeType initialization/open-face
at `0x107eb4`/`0xff684`. The supplied font is the crate's 515,100-byte
Roboto Regular, SHA-256
`56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d`.
This is a pinned test input, not a recovered device font choice.

Constructing the same file twice creates distinct source IDs. Counter seeds
0/41 and an intervening actual `NewFontID` call, `0x21a85c`, bound the
process-history dependency. Native Font source IDs equal their actual created
Skia typeface member-16 IDs. Actual `Font::GetImpl`, `0x85dd0`, and the
implementation getter, `0x87850`, preserve existing implementation/source
pointers and IDs when copying their shared references. This establishes
instance creation and aliasing for these calls, rather than font-byte identity
or arbitrary manager-cache reuse.

Native table-tag parsing at `0x21a05c` supplies 19 actual SFNT tags for each
created source; the complete bitmap predicate at `0x88ae4` returns false
because that file lacks CBDT. Directly supplied language values include empty,
`und`, exact `und-Deva`, different casing, UTF-8 and allocated long strings;
native constructors/getters retain them without inferring script. Requested
weight/italic metadata does not establish synthesized style or shaping.

Every case repeats at allocation fills `0x00`, `0xa5`, `0xff` and a second
zero fill; independent full captures are byte-identical. Host file adapters
supply fixed descriptor/stat/mapping of the pinned bytes; allocation, standard
byte operations, single-thread bookkeeping and a successful setjmp path are
host interfaces. Device XML, family selection, font-manager reuse, missing-file
or parser-error paths, actual bitmap fonts, shaping, rendering and cross-process
ID equivalence are excluded.

### Captured live font registry

[`table-text-font-registry.json`](../../conformance/table-text-font-registry.json),
SHA-256 `41d5adbc3de53576d298b052395a22842be356308d186340513d7c891a983a30`,
contains 154 requests across four separately initialized native registries.
The [capture module](../../conformance/native_table/text_font_registry.rs)
uses the initialized FontManager, supplied family XML and four pinned Roboto
files from the
[NAME evidence](text-layout-findings.md#captured-span-font-name-and-default-selection),
whose fixture SHA-256 is
`20ca51223fa1b5a010786586a12ab4d4c5937cd5ba4e1362ff661d6fab069754`.
Native NAME/default resolution and the complete span paint helper execute;
all requests in each configuration share that live registry without a reset.

The 130-request omitted-language baseline has four source-instance equality
classes:

| Registered file | Native source ID | Requests | First request index |
| --- | ---: | ---: | ---: |
| Roboto Regular | 1 | 82 | 0 |
| Roboto Bold | 2 | 20 | 20 |
| Roboto Italic | 3 | 14 | 40 |
| Roboto Bold Italic | 4 | 14 | 50 |

Each request records its class's first request index. Actual selected-source,
parent Font and implementation pointers equal those from that first request;
the implementation's source member also equals the selected physical source.
Raw pointers are omitted from the fixture. The numeric IDs come from the
native source getter with the supplied process counter seed 0. They identify
these native instances while the registry remains alive, rather than font
bytes, caller database IDs or identities comparable across separately
initialized registries or processes.

`GetParentFont`, `0x88b78`, copies the selected source's `std::any`; the capture
uses the actual cast handler and type information used by `SpanRunFunctor` at
`0x77524`–`0x77590`. Actual Font and FontImpl source-ID, bitmap and language
getters execute and agree. The copied `std::any` is cleaned up natively at
`0x6a278` after each observation; the initialized registry remains alive
throughout the request sequence. This establishes reuse across these temporary
copies, without establishing identity or validity after registry teardown or
cache invalidation.

The other three supplied XML configurations use explicit empty, `und` and
`und-Deva` family language. Each makes eight requests, partitioned 4/2/1/1
across Regular/Bold/Italic/Bold Italic. Actual Font/FontImpl language getters
return exact empty strings for both omitted and explicit-empty language, and
retain exact `und` and `und-Deva`. No host language result is supplied. The
complete native CBDT predicate returns false for all four pinned fonts;
this capture contains no bitmap font file.

Three memory fills, a repeated zero fill and independent full captures produce
identical output. The pinned NAME fixture specifies the library/font hashes
and host XML/file/ICU/libc/libm/allocation and single-thread interfaces; the
host one-C-string `snprintf` service also accepts the native `und%s` format
reached by these explicit-language controls. XML, files and counter seed are
supplied inputs, not recovered device configuration. Equality is bounded to
four unique registrations in one family per registry: duplicate family/font
registration, arbitrary cache invalidation and teardown lifetime are excluded.
The capture does not construct later UTF-16 glyph/cache entries, perform glyph
fallback/itemization or shaping, emit retained runs, or establish full device
font selection, grouping or clipping parity.

### Dynamic locale and custom fallback ownership

The following contracts are source-only findings from the pinned Text library,
SHA-256 `5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b`;
they do not extend the 154-request capture to live replacement or teardown.
Java `SpenFontManager.setLocaleList()` forwards ordered default-locale language
tags. Native `SetLocaleList`, `0x84a78`, registers their comma-separated list
and stores its ID at `0x84bbc`. The registrar, `0x940c8`, caches exact input
strings, calls dynamic ICU normalization, omits invalid/duplicate locales and
interns at most twelve accepted locales in order. It appends new list IDs or
reuses existing ones; this process-local locale-list ID is separate from a
physical font source ID, collection-face index and raw font-language string.
The source does not determine device ICU outputs or fallback priority.

Despite its nullable Java language/buffer signature, native
`SetCustomFallbackFont`, `0x80a1c`, accepts only exact `ko`, nonnull positive
bytes and an existing designated fallback-family slot (`0x80a64`–`0x80ae0`).
System-family parsing creates that slot on the inspected missing
`SamsungKorean-Regular.ttf` branch; null bytes do not remove an installed font.
The accepted memory Font requests index 0, weight 400 and nonitalic
(`0x80b4c`–`0x80b58`). JNI copies Java bytes to a temporary buffer, and the
source constructor copies them again into its retained program
(`0x88488`–`0x884a0`); that program does not borrow the Java array. File sources
instead retain a read-only shared mapping (`0x88268`–`0x882d0`), which is not a
proof of immutable file contents under external modification.

Replacement installs new shared family/collection implementations and inserts
a new source-ID entry (`0x80d08`–`0x80da0`, `0x896c0`–`0x896c8`). The inspected
replacement/default/path-change routes preserve older source-ID map entries;
existing-key insertion does not overwrite the retained Font. Thus an old lookup
remains backed by its shared Font while that parser registry lives, rather than
being rebound to the replacement program. `GetFontData`, `0x80864`, lends the
selected program pointer, length and requested face index; it returns no new
buffer owner. Reporting an index does not prove nonzero collection-face
selection by the imported stream factory.

Parser destruction releases map references (`0x7c9dc`→`0x820ac`→`0x81d00`),
while Font implementations separately retain shared physical sources. Map
lookup lifetime, borrowed-buffer lifetime and retained run ownership are
therefore distinct. These bodies do not establish global cache refresh, all
measured-run validity after teardown or reference-device font order. Rust's
[registered source token](../../crates/sdocx/src/render/fonts/registered_source.rs)
remains restricted to the admitted default Regular registration; physical
program/index identity does not replace that source-instance identity.

## UTF-16 entry ownership

Text `RichTextMeasure::createMeasureData`, `0x788f0`, obtains source length
from `String::GetLength`. `initMeasureData`, `0x790d0`, allocates one 80-byte
entry and one 40-byte `GlyphInfo` slot per UTF-16 code unit. The entry
constructor, `0x65920`, starts with kind 3 and zero metrics.

`SpanRunFunctor` maps each supplied shaping record's source-cluster member 40
to a global UTF-16 owner index (`0x77390`–`0x773ec`). Each 12-byte glyph record
contains a codeword and XY offsets; several records can append to one owner's
cache (`0x7740c`–`0x774e4`). Native production unions glyph ink at `0x7763c`,
stores the owner's supplied advance divided by 100 at `0x77644`–`0x77668`,
and assigns font size at `0x7766c`–`0x77674`.

The ordinary codeword is the complete shaped glyph ID, not a packed source
character. Text `0x9c908` reads HarfBuzz glyph-info `codepoint` after shaping;
`0x9ca0c`/`0x9ca78` append it unchanged to the layout-piece glyph vector.
Aggregation copies it into record member 16 at `0x9dc8c`, and
`0x773f0`–`0x7740c` copies that u32 unchanged into the owner cache. Synthesis
flags travel separately from shaping record member 8 into cache bytes 32/33
at `0x775a8`–`0x775cc`. The retained first-codeword-high-bits field is therefore
glyph ID shifted right by eight, not a Unicode character.

Native UTF-16 decoding at `0xa05a4` anchors a decoded surrogate pair at its
leading code unit (`0xa070c`–`0xa0714`, `0xa0740`–`0xa0760`). Aggregation
owners are relative to the shaping request; the producer adds the request's
global base at `0x773a4`. These source findings do not establish that Rust and
native shaping select identical glyphs or cluster owners.

Unowned code units retain empty caches and kind 3. A surrogate continuation,
ligature continuation or combining character therefore need not own an
independent native drawing entry. Ownership depends on the shaping result;
these source categories do not prescribe one fixed owner map. The draw loop
skips a false drawable cache (`0x66e20`–`0x66e24`, `0x66f48`–`0x66f54`),
without replacing previous font/span state or extending rectangle unions.
Captured font, size and foreground changes on those skipped continuations do
not split the real glyph run.

Run source ranges still include skipped code units. A trailing unowned entry
is included in the final run's inclusive range. With a leading unowned entry,
the captured emitter produces an initial empty `[0, 0]` run, default size 17
and font ID `0xffffffff`, followed by the real glyph run. Empty ranges/records
cannot be inferred solely from the set of glyph-owner indices.

Owned zero-advance glyphs are a separate case. The first run entry directly
initializes its layout rectangle, preserving a zero-width rectangle such as
`[4.25, 8, 4.25, 35]`. Text `0x672a4`–`0x672b0` compares the current source
index with the run start; the equal branch copies both rectangles directly at
`0x672c8`. Later entries call Base `Rect::Union` at Text `0x672e0` for ink
and `0x672ec` for layout. Base `0xb1538` skips an incoming rectangle when
left is greater than or equal to right (`0xb1540`–`0xb1544`), or top is
greater than or equal to bottom (`0xb1550`–`0xb1554`). An empty accumulator
instead accepts the next nonempty rectangle at `0xb15ac`–`0xb15b8`.
A drawable owner's zero advance or zero ink therefore does not make it an
unowned continuation; it can still contribute glyphs and identity.

[`table-text-ownership.json`](../../conformance/table-text-ownership.json),
SHA-256 `a06f8c07d2167aa2dead23d220870da8dec664fa3f32147b4b51bce796b5f683`,
contains 38 cases, 124 UTF-16 entries, 94 supplied glyphs, 40 emitted runs and
94 output glyphs. The
[capture module](../../conformance/native_table/text_ownership.rs) executes
the native entry constructor, the `SpanRunFunctor` window
`0x77324`–`0x77894`, `ParagraphLayout::SetLayout`, `0x6b4a4`, complete
`getDrawnTextRun`, `0x66c98`, and `appendTextBlock`, `0x67ebc`.
Every serialized result repeats with memory fills `0x00`, `0xa5` and `0xff`.

Glyph codewords, cluster owners, positions, ink, advances, logical order,
direction, spans, font getters and line/block metrics are supplied. Owner
caches start drawable with an empty vector and supplied Font wrapper,
bypassing native font creation; unowned caches start nondrawable. Host
interfaces provide allocation, deletion, memory copy and mutex operations.
Scenario labels such as ligature, surrogate, combining and emoji ZWJ describe
source and supplied owner maps, rather than native shaping outputs. Native
source-to-owner selection, font selection, bidi ordering, wrapping, real objects,
emoji rendering, Composer clipping and final PDF painting do not execute.

### Captured nonzero owner bases

[`table-text-owner-bases.json`](../../conformance/table-text-owner-bases.json),
SHA-256 `504bbe262a6bde8f9db3ecd1f5162039f6ed644cc9d85fe7c766e2e941a390ed`,
contains nine cases, 70 UTF-16 entries, 18 owned slots and 21 supplied glyph
records. The [capture module](../../conformance/native_table/text_ownership.rs)
executes each native entry constructor, `0x65920`, then the actual producer
window `0x77324`–`0x77894`. Native output slots are read from memory after
request-relative owner indices are added to supplied nonzero absolute starts
3, 4 or 5. Three memory fills and independent captures match byte-for-byte;
the original 38-case ownership fixtures remain unchanged.

The source-vector base is a separate input, zero or three in these cases.
With base three and requested half-open range `[4,6)`, native source-unit
lookup subtracts the base and classifies the space at absolute index four as
kind 1. Supplementary prefixes/continuations, combining multiple-owner inputs,
ligature gaps, leading/trailing unowned slots and supplied RTL glyph order
exercise those source-index relationships without deriving shaping owners.

Inputs supply document/source UTF-16 units, requested ranges, source-vector
base, relative owners, glyph IDs/XY/ink, per-code-unit advances and font
wrappers. Owned caches are preset drawable with empty vectors and font
wrappers to bypass font creation. Allocation, copy and mutex operations are
host interfaces. HarfBuzz/Minikin shaping, layout-piece/chunk owner normalization,
font selection/creation, native bidi/direction assignment, placement, retained
emission and painting do not execute. These cases establish the producer's
supplied-owner base arithmetic, not full Rust/native shaping equivalence.

The [Rust source-index comparison](../../crates/sdocx/src/text_index/fixture_tests.rs)
projects supplied request-relative UTF-16 owners through substring scalar
indexes and back to global source ranges. Across all nine cases, it matches
the 21 supplied glyph IDs at their actual native owner entries and checks
rebasing. This verifies owner starts and checked source-index projection;
it does not establish native cluster extents, shaping or layout parity.

### Captured cached-entry snapshots

Three fixtures retain the actual pre-emission cached inputs alongside the native
outputs:

| Fixture | SHA-256 | Cases | UTF-16 entries | Output records | Output codewords |
| --- | --- | ---: | ---: | ---: | ---: |
| [`table-text-cached-runs.json`](../../conformance/table-text-cached-runs.json) | `e0f60a1216fc515c800a368e60279fab9b209f3ee883588987fa2246a96665bb` | 230 | 1,116 | 528 | 1,180 |
| [`table-text-cached-ownership.json`](../../conformance/table-text-cached-ownership.json) | `44b7fd9d1c4de0f803aca18782c281b6971d0ef4c5360afc4969f36dbfef63c0` | 38 | 124 | 40 | 94 |
| [`table-text-cached-object-runs.json`](../../conformance/table-text-cached-object-runs.json) | `de3b95b24e236c14ef8af01af89c1e58478599a260aa8c09f7f2e3db8b084b45` | 82 | 82 | 56 | 80 |

The shared [snapshot reader](../../conformance/native_table/text_cached_snapshot.rs)
records source UTF-16, logical map, compared span fields, cached glyphs/font
metadata and native f32 entry geometry immediately before emission. Native
`SetLayout`/`GetBaseline`, cached `GetGlyphInfo`/`GetSpan`, complete
`getDrawnTextRun` and `appendTextBlock` execute as in their existing captures.
Ownership cases additionally execute entry construction and the supplied
`SpanRunFunctor` producer window. Removing `cached_input` from the run and
ownership snapshots reproduces their original fixtures exactly. The first 80
object scenarios reproduce the original object cases after removing the new
snapshot and kernel-admission fields. The expanded fixtures repeat across
three memory fills and independent captures.

The object snapshot capture extends the original 80 object scenarios with two
supplied cache overrides after `GetGlyphInfo` and before public emission.
Native kind 5 with a nonempty cache publishes a record when drawable is false,
and also when the font wrapper is null, in which case font ID is `-1`.
Fifty-six cases publish records, including the original 54; 26 missing-upstream-
glyph controls remain excluded from positive kernel comparisons. Those controls
use an explicitly unmapped guest-zero page and fault on the attempted null
first-word load at `0x68144`. They do not describe production empty caches or
application crashes.

Codewords, shaped source owners, positions/advances, spans, font interfaces and
line/block metrics remain supplied inputs. The recorded codewords are opaque
test cache values; they do not identify glyphs from actual selected fonts.
The ordinary glyph-ID transport proof above is source-derived. Captured directions are 0/1
and cache auxiliary flags are zero. Cached Y offsets are captured
even though the retained native output records store X positions only. These
snapshots do not establish a Rust-character-to-native-UTF-16 ownership bridge,
actual font selection/shaping, bidi/wrap selection, object/emoji rendering,
Composer clipping or final PDF painting.

The ownership fixture includes two leading-unowned default-empty records.
Their stable range/default fields can be compared, but they have no observed
first-codeword-high-bits field. Native append still performs an unsafe first-word
read with an empty cache; this capture supplies mapped guest-zero memory.
That supplied word is not a legitimate glyph or evidence of an application
crash. It is separate from the object fixture's deliberately unmapped-zero
precondition controls.

## Chromium span-clip transport

The [Chromium regression](../../web/tests/e2e/svg-text-clipping.spec.ts) uses
typed SVG DOM creation and embedded DejaVu Sans. For ordinary text, Arabic,
`ffi`, a combining sequence and a surrogate pair, a single text node,
two adjacent `tspan` nodes and a full clip on the first span produce identical
pixel images in Chromium 153. Moving the first span's clip entirely outside
the image removes its owned glyphs while the remaining glyphs stay visible.
The `ffi` case produces no ink: the ligature belongs to its first span even
though the second span contains `fi`. A clip on the entire text removes all
ink in every case.

Programmatic DOM Range selection and `getSelection().toString()` retain the
complete source for every variant, including fully clipped text. This does
not test mouse hit testing, clipboard copying or PDF extraction. Canvas pixels
are measurement output for this supplied-SVG transport check; product text
remains SVG. The regression does not establish native ownership, font selection,
clip selection or appearance, and does not claim Firefox/WebKit parity.

### Captured cell producer identity

The [14-case cell measurement capture](table-code-findings.md#captured-cell-text-measurement)
connects actual Model text and Widget conversion to native NAME/shaping,
measurement, placement and cached-run emission: 68 UTF-16 slots, 61 cached
glyph records and 24 emitted runs. Its entries, kinds, Font wrappers and glyph
caches are native-produced rather than supplied to the emitter. Continuations
retain kind 3 and zero height; the actual newline kind 4 flushes emission.
Actual Font language is a C++ empty string under the supplied XML, rather than
an absent optional value; its source ID identifies the selected live registry
instance. Complete table/merged lifecycle, Model callbacks, world origins and
final writer consumption remain outside that capture. It does not activate a
complete Rust cached-run producer. The separate
[17-case emission capture](table-code-findings.md#captured-cell-text-emission)
retains actual 72-byte `GetSpan` projections, paragraph state, gravity and
inverse logical maps immediately before emission: 78 UTF-16 slots, 67 cached
glyph records and 27 runs. Leading/consecutive/newline-only kind-4 controls
flush without an empty append. This supplies actual producer evidence for that
branch, independently of a final Rust kernel parity claim.

## Current Rust representation

Rust retains raw serialized spans and decodes ordinary font/style properties.
`decoded_font_name_value()` accepts valid native UTF-8/CESU-8 names, borrowing
UTF-8 or converting encoded surrogate pairs to Unicode. Style selection and
painting use that decoded value while the original span payload stays intact.
The accessor uses the modern eight-reserved-byte framing. Version-7 input
framing remains unsupported, and bounded Rust decoding rejects embedded NUL
fields rather than reproducing native prefix truncation.
Typed modern composition decoders require complete eight-byte payloads and
preserve nonzero boolean normalization and ARGB. `RichTextSuggestion` decodes
raw type/underline ARGB and bounded UTF-16 strings, discarding empty entries
and treating signed nonpositive counts as empty. It advances actual byte
lengths rather than reproducing the native consumed-counter discrepancy;
truncated lists and malformed UTF-16 return no decoded value. Decoder
regressions compare 132 native modern reader inputs and 60 native suggestion
writer outputs. No spell-correction or legacy-version-7 composition decoder
contract is claimed.

`StyleIndex` decodes source span patches once and resolves their overlapping
properties in source order. Its typed selections retain ordinary/composing
ARGB separately. Glyph `TextStyle` projects the selected foreground and
font/decorations; background painting reads the typed selection directly,
rather than keeping another background copy in glyph style. This does not
provide complete native entry identity, and glyph foreground painting keeps
RGB rather than the compared native ARGB value.

`TextSpanProducer` explicitly distinguishes Widget conversion for body/capture/
table text from Drawing conversion for placed/code text. This is separate from
flow versus placed geometry: code flows through the text engine with Drawing
span rules. Drawing body construction/update at `0xafed0`/`0xb1a54` reaches
Widget `ObjectTextLayout`; code measurement at `0x73694` updates Drawing
`ObjectTextDrawing` at `0x73728`. Drawing's span update initializes ordinary
styles and calls text conversion at `0x8d0e4`, without Widget's object-span
conversion step. Rust applies type-15/type-17 object guards only for Widget
source ranges backed by validated embedded objects; bare U+FFFC characters do
not acquire an object guard. Type 18 stays unguarded in both producers.

`ResolvedTextStyle` projects paint and a typed
[`NativeDrawSpan`](../../crates/sdocx/src/render/text/native_identity.rs) from the
same selection. The available native projection retains f32 size, complete
theme-mapped foreground ARGB before hyperlink paint, separately mapped ordinary
and composing backgrounds, nullable semantic font name, source style bits,
underline color, hyperlink/Widget object flags and inactive correction defaults.
The native source style includes bold/italic/underline/strike/suggestion bits
before hyperlink paint adds underline. Correction foreground defaults to
`0xff000000` with its enable flag false only when no active correction is
present. Widget/Drawing object guards remain part of that source projection.

Active unsupported correction, malformed recognized payloads, invalid font
metrics and recovery that differs from native produce an unavailable identity. This includes
default-size arithmetic where Rust's paint recovery differs from the native
unclamped f32 result. Those cases retain existing measurement recovery but do
not fabricate comparable native draw fields.

The hash-pinned projection regression classifies all 70 span fixture pairs:
59 compare every supported member and native equality in both directions;
ten require unavailable identity, comprising seven correction pairs, two
native-null-getter cases without a valid persisted source payload and one
default-size recovery mismatch. The remaining pair checks constructor defaults
separately from object-text defaults. The correction pair with a supplied XOR
theme stays unavailable; it does not establish actual theme conversion parity.

`TextMeasureStyle` derives from the same native candidate and retains f32 size,
native-theme-mapped foreground ARGB before hyperlink paint, and a nullable shared font name before
empty-name lookup normalization. Its style mask retains the supported bold and
italic bits under native mask `0xc3`; opaque native bits `0xc0` are outside
the implemented source-style subset. Native span color mapping is separate
from the export policy's contrast-aware paint color. Different source colors
can produce identical dark paint while retaining different measurement keys.
Parsed whole-text foreground spans remain in `RichTextBox.spans` alongside
their RGB summary, so their original alpha still reaches the typed measurement
selection. A manually supplied box with only the RGB summary has an opaque
measurement fallback; that summary does not contain a recoverable alpha value.

`ParagraphMeasurer` coalesces adjacent segments by measurement identity,
keeping paint separately. Script, bidi and tab boundaries still split shaping;
inline objects are measured in separate text chunks. The ordinary measurement
regression compares typed StyleIndex/key projection against 89 finite,
supported native cases, including forward/reverse/self results. Independent
Roboto `AV` advances verify retained kerning or separate shaping at alpha and
null/empty-name boundaries; hyperlink paint and background/decorations do not
introduce measurement splits. Those comparisons do not cover nonfinite sizes,
object flags, opaque style bits, native font resolution or paragraph-heading
style phases.

The [native predefined-style factory capture](text-layout-findings.md#predefined-style-span-factory)
establishes ordinary source spans for heading/body styles. Rust retains their
authoritative size/bold values and uses 15 for its Heading 3 size fallback.
Later explicit spans still override those
source properties. Rendering paragraph metadata alone does not append native
Model editing spans or synthesize their bold flags.

Integration regressions cover six `AV` identity cases across body, standalone,
code, table and nested table/code contexts: native join baseline, alpha-only
foreground, null/empty family, hyperlink-only paint, differing source colors
under two blue links, and different native dark keys with identical dark paint.
The 30 unwrapped and 30 midpoint-width observations compare SVG background
width/suffix origin and retained PDF glyph origins with independent Roboto
shaping. The font is the bundled regular face, SHA-256
`56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d`.
A width strictly between kerned and separately shaped advances produces the
expected one versus two baselines. SVG source and PDF ToUnicode text retain
`AV|` or `AV`, with no image replacements. These compare Rust layout/transport
with independent font advances, rather than executing complete APK shaping.

Retained PDF glyph runs require available, equal `NativeDrawSpan` identities
and equal actual `NativeTextPaint` within the same measured run. This preserves
native span distinctions even when rendered paints coincide, and source and
blue link colors across joined measurement while keeping one `NativeTextBlock` logical source.
Per-glyph PDF fill-color regressions verify both plain/link and two-link cases;
removing the paint-equality guard makes the joined plain/link case lose the
link glyph's blue color.

Six [retained-run regressions](../../crates/sdocx/src/render/text/paint/identity_tests.rs)
exercise actual measurement, wrapping and retained painting. Ordinary/composing
background, underline, strike, suggestion color and hyperlink identity split
PDF glyph runs while preserving their shared measured `Arc`, glyph origins and
source. Equal semantic font names with different reserved payload bytes and
explicit false underline remain joined. Alpha-only foreground differences
still split measurement. Unavailable correction identity prevents coalescing
adjacent shaped clusters while preserving their shared kerning and source.
An identity boundary inside `e` plus combining acute remains one shaped source
cluster; this implementation does not subdivide intra-owner glyphs.

PDF integration checks across four text contexts verify exact CID source and
one complete `ActualText` block across these run boundaries. Extractor-inferred
line breaks between runs are separate from stored source. These comparisons
establish the supported span predicate and its retained consumer, without
changing shaping or implementing full native draw-run grouping. UTF-16 entry
ownership, f32 position adjacency, font source/language gates and conditional
per-run clip selection remain separate contracts.

Hyperlink styling follows the captured native type gate: types 1–9 enable
hypertext styling; type 0, 10 and the maximum unknown value do not add blue
foreground, underline or an anchor. Rust regressions cover these five captured
type values across body text, placed text, table cells and code text.
Valid modern composition spans are implemented: type 16 sets underline
regardless of its boolean, and type 18 selects its background or bold/italic/
underline branch. Preview and replay backgrounds choose nonzero composing
ARGB after theme mapping ahead of ordinary; retained document PDF chooses
ordinary. Generic SVG-to-PDF conversion retains the supplied SVG's preview background rather
than resolving source spans again. These preserve the traced consumer
distinction without claiming complete native visual parity.

Rust integration regressions cover body, standalone, code, table and nested
table/code contexts. They check composition flags, overlapping background
selection, mapped transparent sentinels and object-only diagnostics across
SVG/replay, retained PDF and supplied-SVG PDF. PDF source assertions decode
stored text through its ToUnicode map, separately from extractor-inferred
spacing. Text-only cases retain text without image replacements.

SVG/replay object backgrounds use measured line bands for supported inline/block
placements, including inline margins and block visible width. Rectangle paint
order follows source order. Retained document PDF uses ordinary object
backgrounds in Frame contexts: standalone, table and code text. Body context
omits object backgrounds, matching the native caller's object exclusion.
Preview uses the mapped composing override in both contexts. Missing or
ambiguous reordered object positions report `UnsupportedBackgroundPositioning`
with the object's source/owner. Valid enabled composing tags do not report
an unsupported style merely because their source includes an object. Disabled
composing tags retain bold/italic/underline on text.

The [integration tests](../../crates/sdocx/tests/text_styles.rs) cover 20 object-band combinations across the five text
contexts, plus 30 producer/source-order observations. They distinguish Widget
type-15/type-17 object guards from Drawing conversion, unguarded composing tags,
and before/after overlap order. SVG/replay bands and surrounding text positions
are checked. Retained PDF assertions verify gray composing-tag bands with
alpha 25 in standalone, code, table and nested table/code contexts, their
absence in Body, band bounds, source and per-glyph colors. Source-order cases
separately verify ordinary retained backgrounds versus preview-only composing
backgrounds. Resolved image objects preserve `AB` or empty parent source and
one embedded image; these are Rust output checks against the captured fields
and caller policy, rather than complete native PDF executions.

Validated object anchors participate in background segment boundaries. A whole-line
composing tag followed by a transparent ordinary background leaves only the Widget
object band visible; Drawing clears the band. The mixed `A\uFFFCB` regression
checks all five preview/PDF contexts. Removing the object boundaries makes its
Body preview assertion fail. The
[background unit regressions](../../crates/sdocx/src/render/text/background.rs)
also compare 36 native placed-entry rectangles and 40 supported legacy
export-caller cases, and preserve object ownership for unsafe positions.

Legacy/incomplete composition payloads also report
`UnsupportedCompositionStyle`;
suggestion and correction appearance remains unimplemented and reports
`UnsupportedSuggestionStyle` or `UnsupportedCorrectionStyle`. Typed suggestion
decoding is distinct from its decoration rendering. Diagnostics retain source
ownership, including nested objects; empty, invalid, surrogate-interior and
separator-only ranges do not report those appearance diagnostics.

The [typed cached-entry emitter](../../crates/sdocx/src/render/text/native_runs.rs)
accepts supplied UTF-16 entries, native f32 geometry, span/font metadata and
cached glyph payload references. Its
[fixture comparisons](../../crates/sdocx/src/render/text/native_runs/fixture_tests.rs)
match 324 snapshot cases, 1,296 entries and 624 output records, including source ranges,
glyph order/owner/stored X, origin, ink/layout rectangles, font ID/size,
foreground/style/ordinary background and object flag. Geometry comparisons
use exact f32 values. The adapter maps opaque payload indices back to captured
codewords; it does not interpret those indices as real font glyph IDs.

The kernel implements nondrawable-slot skipping, native boundary decisions,
first-entry rectangle initialization, later native rectangle unions and whole-run
RTL reversal. Cached Y does not enter the native retained output. Default-empty
records carry an explicit classification without inventing first-codeword bits.
Nondrawable kind-4 newlines flush a pending run without an empty append.
Drawable kind-4 states and unknown kinds, drawable owners with empty caches,
unavailable font state and nonfinite/overflowing geometry fail explicitly. Bounded inputs
permit at most 250,000 UTF-16 entries and 1,000,000 cached glyphs.
The comparisons cover the original 268 kind-0/3 cases and 56 published kind-5
object records. They include kind 5's nonempty false-drawable and null-font
branches, while keeping 26 glyphless precondition controls separate.
The [actual cell comparisons](../../crates/sdocx/src/render/text/native_runs/cell_fixture_tests.rs)
also replay all 17 emission cases, 78 entries, 67 cached glyph records and 27
runs with native span/paragraph/gravity/logical-map inputs. Leading,
consecutive and newline-only cases distinguish flushing from an empty append.
These comparisons cover the emitter independently of its bounded production
paint-plan adapter below.

Production [TextSource](../../crates/sdocx/src/text_index.rs) keeps sealed,
consistent character, UTF-8 byte and UTF-16 ranges. One `TextIndex` constructs
these ranges; checked `relative_to` retains all three through measured glyphs,
retained glyphs and `NativeGlyph` block-relative source. The retained registry
recomputes the expected ranges against the actual block text and rejects a
mismatch before passing the validated byte range to Krilla. Compatibility transport uses this checked projection of Rust shaping
ownership. The certified paint-plan comparisons below separately verify native
run UTF-16 source ranges and paragraph maps within their captured profile.

Compatibility PDF grouping calls the same `native_run_boundary` with its available
span projection. It does not supply native entry kind/direction, f32 horizontal
adjacency or font source/bitmap/language metadata. A different or unavailable
span splits; equal spans return an unavailable full native boundary, leaving
the existing measured-`Arc` and actual-paint checks to govern coalescing.
The cached-entry emitter is verified with supplied inputs and, separately, the
actual cell producer above. Outside the paint-plan certificate, the renderer
uses its partial span boundary; unsupported font/ownership profiles retain that
separate compatibility contract.

Measured runs retain a selected `ResolvedFace`, synthesis, direction and shaped
glyphs. `ResolvedFace::is_bitmap_font()` retains the exact CBDT table-directory
presence of the selected SFNT/TTC face. Its 21 font tests include other color
table tags, cached/cloned faces and both collection indexes. This metadata does
not reproduce the native draw-run bitmap gate in production by itself.
The default book's [registered-source token](../../crates/sdocx/src/render/fonts/registered_source.rs)
retains the admitted pinned Regular/index-zero registry instance, empty C++
language string and false bitmap flag for default-family measurement. Explicit
NAME selections do not receive that token. Equality uses retained registry identity,
not a font-byte hash or `fontdb` ID; clones preserve it, while database or native
configuration replacement invalidates it. PDF options preserve the supplied
book's instance when its database/configuration still match. The source metadata
checks 82 Regular requests from the actual live-registry capture. Other
faces/configurations have no certified token. Retained entry classification and
height facts are additionally limited to captured sizes 17 and 50. The bounded
paint-plan adapter consumes those facts for complete run grouping within its
certificate. HarfBuzz
byte clusters map to Rust character ranges in the compatibility path. Admitted
native-measured ordinary paragraphs retain captured f32 geometry and UTF-16
entry slots. A bounded mixed path also retains native text slots around SDK
object anchors and uses native f32 selection/cursor arithmetic. Compatibility
profiles and object classification/height/preparation/break policies remain SDK
behavior. Native cached-run horizontal adjacency outside the paint-plan
certificate remains separate from these bounded producer comparisons.

## Certified whole-source paint plans

[NativePaintPlan](../../crates/sdocx/src/render/text/native_paint_plan.rs)
builds one dense UTF-16 entry array from the complete retained cell text layout
and invokes `native_runs` once. It retains native entry kinds, f32 local
positions and rectangles, span ARGB/style, registered source/language/bitmap
identity, cached glyph IDs/offsets and paragraph inverse logical maps. Newline
kind 4 belongs to the following display paragraph and flushes without an empty
run. Cached glyph Y is ignored by the native emitter; every emitted glyph uses
its run's common f32 baseline. Local geometry is retained before the separate
f64 world translation.

The [actual producer regressions](../../crates/sdocx/src/render/text/native_paint_plan/fixture_tests.rs)
match captured run ranges and paragraph maps, glyph IDs/X/common baseline,
rectangles, font size/paint and physical-font hash for 12 admitted cases,
20 runs and 50 glyphs from the 17-case cell-emission capture. Native numeric
font IDs and exact per-glyph source-owner values are not compared by these
tests; registered-source identity and checked source containment remain
separate contracts. A newline-only case independently retains two
paragraph maps and zero runs. Unsupported font/tab/RTL controls return typed
fallback; empty source retains the native rejection rather than inventing a
run. This certificate admits default-family pinned Regular/index-zero,
nonvariable nonsynthetic LTR text at sizes 17 or 50, opaque foreground and the
captured local cell layout. Objects, bullets, predefined text styles,
exclusions, nonzero gravity and Both justification are outside it.

The [shared dispatcher](../../crates/sdocx/src/render/text/paint/native_plan_paint.rs)
projects the completed plan onto visible source windows. SVG remains selectable
text and validates a rigid browser shaping projection before using retained
positions; internal mark displacement it cannot reproduce falls back with a
positioning diagnostic. Retained PDF consumes the same projected run's exact
font/glyph IDs and source ownership through
[NativePaintBridge](../../crates/sdocx/src/render/text/native.rs), adding the
world translation once and recording only projected source in `ActualText`.
The [source-input comparisons](../../crates/sdocx/src/render/text/native_paint_plan/source_fixture_tests.rs)
extend the producer proof to eight admitted LTR profiles, 14 runs, 33 glyphs
and 33 UTF-16 slots, including nonzero cached offsets and stacked marks at sizes
17/50. Five RTL, `.notdef` or mixed profiles remain outside the certificate.
Preparation uses caller text/style/width controls and the separately proven
constructor size 50, foreground `#252525` and initial Center paragraph;
post-output Model getter observations are assertions, not reconstructed inputs.
The compared run/geometry/paint/map fields retain the limits described above.
Paired stacked-mark consumers preserve native glyph IDs, source and origins
in retained PDF; SVG emits `UnsupportedGlyphPositioning` for the geometry its
rigid shaping projection cannot represent.

Table Drawing preserves the plan's world translation only when every line's
f32-transformed X, baseline, top, background top, bottom and post-cursor equals
its local value plus the f64 world origin exactly. Otherwise it clears the
certificate and retains compatibility vector transport. This admits checked
one-unit translations at sizes 17/50 and rejects nonrepresentable ordinary or
large `2^24` controls. It adds the accepted world translation once, without
reconstructing geometry or using a tolerance.

These writer transports do not establish native per-run clip selection, legacy
Table/Code opacity overrides, arbitrary font/script/gravity behavior or native
warm wrapper-cache lifetime. Local producer geometry and checked SDK translation
are separate from complete native writer world-positioning parity.

The retained PDF path can transport supplied clips with selectable text, and
Chromium can transport span clips without breaking the covered joined shaping.
Production table text retains its conservative table-wide clip outside the
[certified ordinary one-page composition](table-code-findings.md#certified-ordinary-one-page-cell-clipping),
which supplies the admitted Model/source and placement context for conditional
per-run clipping. The helper transport results alone do not establish complete
production native per-run identity or clipping outside that certificate. The
[vector support contract](../text-vector-support.md) records the implemented
scope and these independent evidence limits.
