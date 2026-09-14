# Structural images and media bindings

## Confirmed native serialization

Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`, writes outer image objects as
`0 + 6 + 7 + 3`. The main image is not selected from the final type-3 frame.

| Location | Content |
| --- | --- |
| Type 0 | Common UUID, timestamp, bounds and optional rotation. |
| Type 6 | Shared shape-base component. |
| Type 7 fixed data | Shape type, four `f64` local bounds, `f32` rotation, sized path, one-byte control-point count, then 16 bytes per control point. |
| Type 7 flexible bit 0 | Sized `TextCommon`, when present. |
| Type 7 flexible bit 1 | One-byte text-area mode: margin 0, free 1, path 2. |
| Type 7 flexible bits 2 / 4 | Four-byte signed string IDs for pen name / advanced pen settings. |
| Type 7 flexible bit 5 | `u32` effect byte size, `u8` effect type, then the sized effect payload. |
| Fill effect type 2 | `FillImageEffect`; the normal WDoc payload is 62 bytes. |
| Type 3 | Crop, border, original-image and additional image settings. |

`FillImageEffect::GetBinary` writes a one-byte fill mode followed by the
four-byte signed main media bind ID. The remaining 57 bytes contain stretch
offsets, tiling offsets/scales, transparency, a rotatable flag, and nine-patch
rectangle/width. Negative IDs indicate an absent reference. The alternate
coedit representation uses a 64-byte hash in place of the four-byte ID and
occupies 122 bytes; the SDK does not interpret that hash as a bind ID.

The type-3 writer uses a 17-byte header (one property byte and four field
bytes), no fixed payload, and these flexible fields in ascending bit order:

| Bit | Content |
| ---: | --- |
| 1 | Four `i32` crop coordinates. |
| 3 / 4 / 5 | Four-byte border color, `f32` width, `u16` type. |
| 9 | Four-byte border-image ID. |
| 10 | Four `i32` border nine-patch coordinates. |
| 11 | Four `f32` border widths. |
| 12 | Four-byte border nine-patch width. |
| 17 | Four `f64` original-image rectangle coordinates. |
| 18 | Four-byte original-image ID. |
| 19 | Sized path followed by two 16-byte rectangles. |

The main, border and original-image IDs have different roles. Source addresses
and Java manifest writers are indexed in [`source-map.md`](source-map.md).
These contracts are confirmed from native serialization. The measured
`03-image-placement` fixture below covers images embedded in text flow.

## Resolution and public API

`parse_media_manifest_bytes` exposes the bounded modern media manifest,
including bind IDs, filenames, recorded hashes, reference counts, timestamps,
attached flags and extension bytes. Record sizes exclude their four-byte size
prefix. The empty-hash writer form is a two-byte zero; populated hashes occupy
64 ASCII hexadecimal bytes. Legacy manifests at versions 3001 and below are
not implemented. Malformed modern records fail instead of falling back to
filename guesses. Hashes are retained; normal parsing does not verify them.

For image objects, bind IDs resolve through manifest filenames under `media/`.
This mapping takes precedence over both ZIP order and numeric filename prefixes.
Repeated image references share an asset index across pages, layers and child
objects. Duplicate bind IDs are ambiguous. Missing files, unbound IDs and
unsupported media types produce `UnresolvedImageMedia`; another asset is never
substituted based on encounter order. When the manifest is absent, a unique
numeric filename prefix can resolve an ID with `InferredImageMediaReference`.
That fallback also considers unsupported media filenames when checking ambiguity.

The SDK returns `PageElement::PlacedImage(PlacedImage)` for native images. It
preserves bounds, rotation, main/border/original IDs, crop rectangle and an
optional resolved `media_index`. An unresolved object stays in the model, with
`media_index: None`. Existing caller-created `PageElement::Image` values remain
supported by the renderer. `MediaAsset::archive_id` continues to mean the
filename prefix; use `PlacedImage::media_id` for the authoritative object bind ID.

The SVG renderer embeds the resolved PNG/JPEG/WebP bytes and applies placement
and stored rotation. Unsupported image features generate
`UnsupportedImageFeature` diagnostics. CLI conversion and WASM inspection use
the existing shared report plumbing. The image marker scanner and encounter
counter have been removed; only the native image decoder produces placed images.

## Validation

- Twelve image tests cover reordered assets and mismatched filename prefixes,
  repeated references, missing/unsupported/ambiguous bindings, cross-page and
  nested references, zero/negative IDs, tiny bounds, wider masks, future frames,
  optional fields before the fill, separate border/original references, every
  payload truncation, resource limits, legacy image-value rendering, and matching
  base/shape rotations without misinterpreting the shape angle as a radius.
- Three manifest tests cover bounded records, Unicode filenames, sparse IDs,
  empty/full hashes, extensions and count limits.
- The previous parser at `3dd8f52` returns zero images for the three-image
  synthetic regression; the new parser returns all three with the intended
  blue/red/blue asset references. The comparison used an isolated archived
  checkout and its own Cargo target directory.
- The rich-text conformance fixture passed during the migration. The
  [historical fixture audit](fixture-validation.md) also retained 7,182 strokes
  and 924,442 points and verified all 21 media hashes at versions 5202/5400
  against the actual PNG/PDF/SPI bytes. Those three audit inputs are retired.
- Workspace tests, Clippy, formatting, Rust 1.88 and WASM target checks pass.
- A disposable synthetic archive converted through the CLI to SVG and PNG
  displays the expected blue/red/blue sequence, one rotated placement, and a
  blank missing-ID location with its diagnostic. This is a runtime smoke check,
  not a Samsung visual-fidelity comparison.

## Remaining gaps and next work

Crop rectangles with an original-image placement are rendered by clipping the
original placement to the displayed bounds. Pixel crops without that placement
and border/original media IDs remain incomplete.
Fill transforms, tiling, transparency, active nine-patch, custom shape paths and several
inherited properties remain incomplete. Alternative fill encodings and unknown
fields before a fill are reported without guessing a reference. `.spi`, PDF,
audio and video assets are not raster image render inputs.

The subsequent [shape/line migration](shape-line-findings.md) removes the
remaining UUID/text heuristics. Native setters also confirm that type-7 fields
2/4 reference pen-name/settings strings, correcting the provisional color label.
Standalone image objects still need a Samsung reference pair; text-flow image
coverage does not establish fidelity for every image container or style.

## Images embedded in document text flow

The `03-image-placement` pair at dataset revision `aa89121` contains one
451 × 300 RGB PNG and seven image spans in `note.note`. Its four stored page
records have no image objects; the Samsung PDF has three visible pages with
three, two and two placements. The references exercise original size,
enlargement, 180°/90°/−90°/45° rotation, and a rectangular crop.

Full archive parsing decodes these raw span payloads through the same bounded
`0 + 6 + 7 + 3` reader as standalone images, resolves their media IDs, and
exposes `RichTextObjectContent::Image`. Both `ParsedDocument.note` and the
document's text-flow metadata receive the resolved contents. Standalone
`parse_note_bytes` retains the raw image spans without archive media resolution.
Hidden image spans retain their raw bytes and do not resolve or render media.
Missing and ambiguous bindings remain explicit unresolved images with warnings.

Rendering uses the rotated image footprint for vertical flow advancement.
The image-only flow default has a line height of 1.35 times the font size and
0.35 times the font size between image paragraphs; explicit paragraph spacing
is retained. Continuation pages start at their text margin without the baseline
adjustment used for flowing text. These defaults are measured against this
fixture and do not establish typography for arbitrary mixed text and images.

The cropped span stores `[73, 50, 405, 222]` pixel bounds and an original placement
of `[-26, 587, 428, 890]`. The renderer clips that placement to the image bounds
before applying the object's rotation and flow translation.

Before this support, all three SVG/PNG/PDF pages were blank and the parser
reported no diagnostics. PNG comparison after implementation measures 1.06%,
0.04% and 0.21% changed pixels. PDF comparison measures 0.07%, 0.33% and 0.09%.
Both report no missing foreground at the runner's one-pixel tolerance.
The executable and input hashes are recorded by the
conformance runner; these figures describe this pair and its raster settings.

The initial implementation emitted seven conservative `UnsupportedImageFeature`
warnings for inherited shape/style/path/fill settings. The bounded handling
below removes these warnings for this fixture. Inline, alternate-margin,
cross-page and nested text-object image
layouts are also explicitly reported as incomplete. The corpus now locks both
decoded and resolved embedded-image counts, and a rendering regression checks
the per-page placement counts. Synthetic regressions cover UTF-16 anchors,
media binding, hidden images, truncation, limits, crops and page slicing.
Workspace tests, the full external corpus and Clippy pass. The existing five
formatting SVG pages are byte-identical to the pre-change SDK output.

## Standard image settings and diagnostic precision

The seven images use type-6 magnetic points with no connections, explicit
no-outline color kind 2 and zero line width. Images now reuse the bounded
shape-outline reader. Magnetic points do not change the displayed image;
no-outline paint, transparent solid paint and zero-width outlines require
no additional rendering. Visible outlines and unrecognized inherited settings
still produce `UnsupportedImageFeature`.

Their type-7 shape is rectangle 4, with matching base/local bounds and a
five-command path: move, three lines, close. The coordinates describe the
placement corners after the stored rotation. The SDK recognizes this redundant
rectangle, checking every command and coordinate, with eight `f32` epsilon
units of tolerance relative to the placement coordinate scale. This accounts
for native float geometry serialized as doubles; the 45-degree fixture's
largest corner discrepancy is about 0.0000462 document units. Other templates,
custom or open paths, extra commands and inconsistent placement still warn.
Known path commands are bounded and reject truncated/non-finite coordinates.

Every image fill stores nine-patch width 1080 with rectangle `[0,0,0,0]`.
The width alone does not activate nine-patch rendering: ARM64
`libSPenDrawing.so` function `ObjectImageDrawing::hasNinePatchRect` at
`0x85a94` obtains the rectangle at `0x85abc`, calls `Rect::IsEmpty` at
`0x85ac8` and negates that result at `0x85acc`. The parser consumes the width
without reporting an unsupported effect when the rectangle is all zero.
Nonzero rectangles, stretch/tiling changes, transparency and alternate fill
flags remain diagnostic conditions.

The locked image fixture now expects zero diagnostics while retaining all
seven resolved images and the three/two/two page distribution. New synthetic
cases cover both standalone and embedded images, rotated rectangles, inactive
outline variants, dormant nine-patch width, active effects, custom paths,
unknown properties and malformed path/outline lengths. All three image SVG
pages and all five formatting SVG pages remain byte-identical to the output
before this diagnostic correction.

Validation passes all 259 workspace tests, both external corpus tests,
Clippy with warnings denied, formatting, Rust 1.92 checks and the WASM target
check. The source/reference hashes were verified in an isolated corpus copy;
the existing `hf` checkout was preserved.
