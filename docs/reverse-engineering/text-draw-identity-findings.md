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
bytes 17–23, 41–43 and 68–71 are
not initialized by the captured constructor. Members 44, 52 and 60 are ignored
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

[`table-text-ownership.json`](../../conformance/table-text-ownership.json),
SHA-256 `2773117ab2a4e36bd30348de23f47960ae51cdb3ec2b37b1ad61ca4fd8a253c0`,
contains 32 cases, 108 UTF-16 entries, 78 supplied glyphs, 34 emitted runs and
78 output glyphs. The
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
Its paint `TextStyle` does not carry the complete native span comparison
members, including both raw background colors and correction fields. Foreground
painting keeps RGB rather than the compared native ARGB value.

Measured runs retain a selected `ResolvedFace`, synthesis, direction and shaped
glyphs; they do not retain native bitmap/language metadata. `ResolvedFace.id`
is the current face identity, with no proven equivalence to Samsung's source
instance/cache behavior. HarfBuzz byte clusters are mapped to Rust character
ranges; they are not native per-UTF-16 entry slots. Rust's f64 cluster positions
also differ from the native f32 adjacency predicate.

The retained PDF path can transport supplied clips with selectable text, and
Chromium can transport span clips without breaking the covered joined shaping.
The production table text path still uses its conservative table-wide clip.
Those transport results do not establish native per-run identity or conditional
clipping in the Rust engine. The
[vector support contract](../text-vector-support.md) records the implemented
scope and these independent evidence limits.
