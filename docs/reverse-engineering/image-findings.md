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
| 19 | Sized saved attribute path, then [saved crop and original-placement integer rectangles](image-effects-findings.md#original-placement-precision-and-coedit-span-state), 16 bytes each. |

The main, border and original-image IDs have different roles. Source addresses
and Java manifest writers are indexed in [`source-map.md`](source-map.md).
These contracts are confirmed from native serialization. The measured
`03-image-placement` fixture below covers images embedded in text flow.

## Resolution and public API

`DocumentMetadata.media_manifest` and `parse_media_manifest_bytes` expose the bounded
modern media manifest, including bind IDs, filenames, recorded hashes, reference counts, timestamps,
attached flags and extension bytes. Record sizes exclude their four-byte size
prefix. The empty-hash writer form is a two-byte zero; populated hashes occupy
64 ASCII hexadecimal bytes. The [unversioned WDoc `EOF` layout](file-format.md#mediamediainfodat)
retains hashes and timestamps; it differs from the plain NoteDoc CRC manifest.
Both remain unsupported. The SDK's initial u32 read cannot classify an
unversioned u16 count plus record bytes as a version. Native New loading uses
`EOF`/`EOFX` trailers; 3001 is a Java writer threshold with mutable flag state.
Malformed modern records fail instead of falling back to
filename guesses. Hashes are retained; normal parsing does not verify them.
`archive_resources` retains `media/` source files; `ArchiveResourceResolver` borrows
their bytes independently of typed image admission.

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
the shared report plumbing. Only the native image decoder produces placed images.

### Native loaded manifest identity

The pinned [WDoc/New manager route](source-map.md#native-libraries) differs
from the SDK's duplicate-ID policy. `WNoteLoadHandler::Load` reaches
`MediaFileManagerNew::Load` through manager slot 160 (`libSPenWDoc.so`,
`0xa85b8–0xa85dc`; Model wrapper `0x292c30`, loader `0x290f0c`).

The loader reads the recorded 64-byte file hash (`0x2911a8`), constructs its
string (`0x2911bc`), pairs it with newly loaded metadata (`0x291618`) and
inserts into the hash-keyed tree (`0x291628`, helper `0x29839c`). This key is
the recorded file hash, separate from Java's computed metadata hash.
Its string constructor uses `strlen` (`0x277cb4`), so an embedded NUL ends
the key; ordinary 64-byte ASCII hexadecimal hashes use all 64 bytes.
The comparator orders key bytes with `memcmp`, then length (`0x298378–0x29838c`).

Equal keys retain the metadata already in the tree: helper
`0x2983d0–0x2983f8` returns without replacing the payload or merging reference
counts. In a fresh empty manager, this retains the first admitted record with
that hash; preexisting manager state can instead supply the retained record.
The loader still updates its timestamp maximum after insertion (`0x29163c–0x291650`).

Distinct hash keys with the same bind ID survive this insertion. ID lookup scans
the tree in hash order and stops at the first matching metadata ID
(`GetFilePathById`, `0x28f9b4–0x28fa08`), then builds the path from that record's filename.
The selected record therefore follows hash order, rather than serialized
first/last duplicate-ID order. `WNote::GetPDFFilePathById` reaches this lookup
through the same manager's slot 80 (`0x96af8–0x96b04`).

The stored bind ID and filename remain independent metadata fields. Filename
equality does not deduplicate this insertion, and its numeric prefix does not
remap an ID. Separate runtime name matching and fresh-ID allocation do not
define saved-manifest admission.

These static findings cover tagged `EOF`/`EOFX` records reaching insertion
after native name, record, attachment and file-access gates. They do not cover
no-marker hash recovery, arbitrary malformed input, later context updates or
the plain NoteDoc CRC manager. No duplicate archive or native execution was tested.
The tagged insertion path does not verify the recorded hash against file bytes.

Rust deliberately keeps duplicate bind IDs ambiguous. Ordered manifest records
and retained raw source bytes remain available independently of resolution;
serialized first/last selection would not reproduce the native route above.

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
  blue/red/blue asset references.
- The rich-text conformance fixture passed validation. The
  [historical fixture audit](fixture-validation.md) also retained 7,182 strokes
  and 924,442 points and verified all 21 media hashes at versions 5202/5400
  against the actual PNG/PDF/SPI bytes. Those three audit inputs are retired.
- A disposable synthetic archive converted through the CLI to SVG and PNG
  displays the expected blue/red/blue sequence, one rotated placement, and a
  blank missing-ID location with its diagnostic. This is a runtime smoke check,
  not a Samsung visual-fidelity comparison.

## Rendering limits

Crop rectangles with an original-image placement are rendered by clipping the
original placement to the displayed bounds. Pixel crops without that placement
and border/original media IDs remain incomplete.
Fill transforms, tiling, transparency, active nine-patch, custom shape paths and several
inherited properties remain incomplete. Alternative fill encodings and unknown
fields before a fill are reported without guessing a reference. `.spi`, PDF,
audio and video assets are not raster image render inputs.

The [shape/line decoder](shape-line-findings.md) uses declared frame boundaries.
Native setters confirm that type-7 fields 2/4 reference pen-name/settings strings.
The [locked image fixture](../../conformance/README.md#image-and-media-regressions)
contains seven text-flow images and no standalone page images. Standalone
image placement and style fidelity remain unverified by a Samsung reference pair.

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

The bounded handling below accepts the fixture's inherited shape/style/path/fill
settings without `UnsupportedImageFeature` warnings. Inline, alternate-margin,
cross-page and nested text-object image layouts are explicitly reported as
incomplete. The corpus locks both
decoded and resolved embedded-image counts, and a rendering regression checks
the per-page placement counts. Synthetic regressions cover UTF-16 anchors,
media binding, hidden images, truncation, limits, crops and page slicing.

## Standard image settings and diagnostic precision

The seven images use type-6 magnetic points with no connections, explicit
no-outline color kind 2 and zero line width. Images reuse the bounded
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

The locked image fixture expects zero diagnostics while retaining all
seven resolved images and the three/two/two page distribution. Synthetic
cases cover both standalone and embedded images, rotated rectangles, inactive
outline variants, dormant nine-patch width, active effects, custom paths,
unknown properties and malformed path/outline lengths. The diagnostic-only
correction left all three image SVG pages and all five formatting SVG pages
byte-identical to the preceding output.

## Native GIF import and current-file animation

These are static Java and ARM64 findings from Samsung Notes 4.4.45.37,
APK SHA-256 `daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
No native insertion, playback, save/reopen or paired appearance comparison was
executed. Addresses below are ELF virtual addresses in the named library.
Java paths start at `sources/com/samsung/android/support/senl/nt/` in the
identified APK decompilation.

| ARM64 library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenObjectControl.so` | `3211b70ad105e285b57aaa085e2bcd543f197238d702f9233aedea1c7caa1aea` |
| `libSPenGraphics.so` | `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |

Graphics (871,768 bytes) and Base (980,664 bytes) match their pinned APK members
byte-for-byte; the others use the established pinned inputs.

### Gallery insertion and current saved identity

`composer/main/base/presenter/menu/option/AttachMenuPresenter.java:139–143`
dispatches gallery request 30000; `:76–87,109–113` obtains the URI list and
executes TaskAddImage. `composer/main/base/presenter/task/TaskAddImage.java:56–83` rejects absent
MIME/video, calls DownloadBitmap.saveImageFromUri and requires a nonempty
existing returned file. Storage checks and handler publication are separate
(`:88–114,117–130,230–235`).

`model/base/utils/image/DownloadBitmap.java:411–428` selects MIME containing
`gif`, checks BitmapInspector validity (`:330–335`), names a GIF destination
and calls UriFileUtils.saveUriToFile. It uses this file route rather than the
ordinary saveBitmapFromUri branch. `:445–446` exposes the path only on mSuccess;
`base/common/util/UriFileUtils.java:1004–1008` returns whether its save helper
reported integer zero. These checks do not establish multiple frames or exact
copied bytes after downstream processing.

`composer/main/base/presenter/task/TaskAddImageHandler.java:39–57` checks file existence
and constructs SpenObjectImage in its span route,
calls setImage(path), then ObjectManager.insertImage. The SDK throws
if that setter returns false; this span body has no local catch. The page route
(`:78–99`) reaches `composer/main/base/model/composer/util/ObjectManagerHelper.java:382–418`, which
catches failed setters, omits those objects and inserts the admitted images.
This producer is separate from the [GIF copy for external-editor preparation](image-edit-source-findings.md#external-editor-preparation-uses-current-main-pixels).

The [registered String setter](image-edit-source-findings.md#returned-editor-source-conditionally-clears-the-original-binding)
and [current writer](image-edit-source-findings.md#generated-spi-source-failure-path-and-ordinary-save-are-separate)
remain conditional: Model ComponentImage::SetImage `0x3a17dc` admits a temporary
fill path before attempting live SetFillEffect (`0x3a18c8–0x3a18e4`).
ImageData::SetImage retains a detached path (`0x2b80dc–0x2b80e4`) or calls manager
Bind(path), stores its ID, rejects -1 and checks managed-path resolution
(`0x2b8080–0x2b80a8`). Register has the same retained-path Bind/-1/resolution
gates (`0x2b86b8–0x2b86c4,0x2b873c–0x2b8740`). ComponentImage::GetImagePath
requires fill type 2 and delegates to GetImageUri (`0x3a1ac8–0x3a1afc`). The
ordinary fill writer reads the current media ID (`0x3b90fc`) and writes it
(`0x3b91c4`); original ID remains separate type-3 bit 18. Neither establishes
source-file availability or archive inclusion.

### Animation opens the current main file

Composer PageForegroundView::loadGifAnimation calls GifAnimationDrawing::Load
(`0x3fe1f4`). Load enumerates page objects through FindObjectInRect and
AppendObjectList (`0x3f68d0–0x3f68dc`); its type-3 branch invokes appendGif
(`0x3f69a0–0x3f69b4`). appendGif requires type 3, reads current main GetImagePath
(`0x3f6f94`), then requires successful Image::GetInfo with codec 5
(`0x3f6fa8–0x3f6fb8`). Base GetInfo confirms that codec through DGifOpenFileName
(`0xab700`) followed by value 5 (`0xab718–0xab724`). Codec identity alone does
not prove multiple frames. The admitted branch constructs AnimatedImage with
that main path (`0x3f70d0–0x3f70d4`); separate live-view/membership gates precede
DoLoad and StartOrResumeAnimation (`0x3f6c10–0x3f6c24`).

ObjectControl AnimatedImage copies the supplied path to member +40
(`0x1102f4–0x110300`); DoLoad passes it to Graphics SPGifAnimationLoader
(`0x110ef8–0x110f10`). Graphics copies it to loader +8 (`0xbf930–0xbf938`), then
FrameContext passes it to Base SPenGifAnimation (`0xbf054–0xbf05c`). The latter's
constructor calls reopenFile (`Base 0xdc70c`), which opens the path read-only
(`0xdc858`) and invokes DGifOpenFileHandle (`0xdc878`). Graphics FutureFrame
requests successive Base GetNextFrame results (`0xbf4a4`) and transports frame/time
data (`0xbf4b4–0xbf4f4`). Composer drawGif invokes AnimatedImage::OnDraw
(`0x3f7de0`). These consumers read the main file, rather than selecting the
separate original binding or the [ordinary cache-first bitmap path](image-effects-findings.md#shape-effects-and-image-pixels-have-separate-drawing-calls).

### A changed path does not establish immediate reader replacement

Composer checkGifPathChange compares current main path with AnimatedImage's
stored path (`0x3f6da0–0x3f6dbc`); a difference calls ChangeFilePath (`0x3f6dcc`).
ObjectControl stores the path. With an existing loader it dispatches slot 64
(`0x1112a4–0x1112bc`); Graphics relocation `0xd2248` resolves it to ChangeFilePath
`0xc0134`, which only updates the loader String without reopening its reader.
With no loader, ObjectControl constructs one (`0x1112c0–0x1112e8`). Graphics
constructor slot 32 (`0xbf9c4–0xbf9d0`, relocation `0xd2228`) reaches Restart
`0xbfca4`; FrameContext construction (`0xbfd7c`) takes the reader-opening route
above. AnimatedImage::ChangeFilePath therefore is not universally String-only.
Graphics FrameContext::Restart separately passes the current loader path to
Base reopenFile (`0xbf140–0xbf148`). These calls do not certify successful reload,
immediate frame switching or atomicity.

If a main-binding manifest record names an existing GIF archive entry without
a typed asset, current Rust media resolution reports unsupported media: the
typed asset filter admits JPG/JPEG/PNG/WebP. The new source finding is the reached
main-file animation route; existing GIF entries are retained as opaque resources,
without animation decoding or playback. This trace does
not establish original-image bytes or ordinary processed cache pixels as
substitutes for that playback source, nor prove those references must differ.
Caller-retained original archive bytes remain a [separate carrier](vector-retention-findings.md#original-page-bytes-are-external-to-the-parsed-model).

Native binding can resize an image when configured dimensions are exceeded
(Model `0x28c8cc–0x28c948`); import-copy admission does not establish identical
or still-animated managed GIF bytes. Existing [media save gates](vector-retention-findings.md)
remain required. SPI temporal caches belong to the [Maetel codec](spi-media-findings.md),
not this GIF reader. Complete disposal/blending/delay/loop behavior, worker/frame
lifetime, reload success and PDF/export frame selection remain unverified.
