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
| `libSPenText.so` | `5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b` |
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

## Font metadata

Text `Font::GetSourceId`, `0x85d7c`, forwards through implementation virtual
slot 48. `FontImplMinikin::GetSourceId`, `0x877f8`, forwards through source
slot 80; `MinikinFontImplSkia::GetSourceId`, `0x88ad4`, reads source member 120.
The source constructor reads the created SkTypeface's member 16 at
`0x880d4`–`0x880d8`. Skia `CreateFromStream`, `0x2197b8`, allocates a fresh
typeface; `0x219848`, `0x219860` and `0x219868` obtain the counter, increment
it and store the instance ID. This is source-instance identity, rather than a
family/style pair or hash of font bytes. Font-manager reuse of those source
instances is not established by the getter capture.

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

## Current Rust representation

Rust retains raw serialized spans and decodes ordinary font/style properties.
`decoded_font_name_value()` accepts valid native UTF-8/CESU-8 names, borrowing
UTF-8 or converting encoded surrogate pairs to Unicode. Style selection and
painting use that decoded value while the original span payload stays intact.
The accessor uses the modern eight-reserved-byte framing. Version-7 input
framing remains unsupported, and bounded Rust decoding rejects embedded NUL
fields rather than reproducing native prefix truncation.
Its paint `TextStyle` does not carry the complete native span comparison
members, including both raw background colors and correction fields. Foreground
painting keeps RGB rather than the compared native ARGB value.

Hyperlink styling follows the captured native type gate: types 1–9 enable
hypertext styling; type 0, 10 and the maximum unknown value do not add blue
foreground, underline or an anchor. Rust regressions cover these five captured
type values across body text, placed text, table cells and code text.
Composition, suggestion and correction span appearance remains unimplemented.
The Rust engine reports `UnsupportedCompositionStyle`,
`UnsupportedSuggestionStyle` and `UnsupportedCorrectionStyle` for valid source
ranges with rendered characters. Diagnostics retain source ownership, including
nested objects; empty, invalid, surrogate-interior and separator-only ranges do
not report those appearance diagnostics.

Measured runs retain a selected `ResolvedFace`, synthesis, direction and shaped
glyphs. `ResolvedFace::is_bitmap_font()` retains the exact CBDT table-directory
presence of the selected SFNT/TTC face. Its 21 font tests include other color
table tags, cached/cloned faces and both collection indexes. This metadata does
not reproduce the native draw-run bitmap gate by itself. Native language
metadata is not retained. `ResolvedFace.id` is the current face identity, with
no proven equivalence to Samsung's source instance/cache behavior. HarfBuzz
byte clusters are mapped to Rust character ranges; they are not native
per-UTF-16 entry slots. Rust's f64 cluster positions also differ from the native
f32 adjacency predicate.

The retained PDF path can transport supplied clips with selectable text, and
Chromium can transport span clips without breaking the covered joined shaping.
The production table text path still uses its conservative table-wide clip.
Those transport results do not establish native per-run identity or conditional
clipping in the Rust engine. The
[vector support contract](../text-vector-support.md) records the implemented
scope and these independent evidence limits.
