# Native pen identity and renderer selection

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenModel.so`,
`libSPenDrawing.so`, `libSPenPenCommon.so` and `libSPenMarker2.so` from the
APK identified in the [knowledge base](README.md#sources-and-validation).
The native registry is cross-checked against that APK's library entries and
Java `com/samsung/android/sdk/pen/pen/SpenPenManager.java`.

This trace establishes stored-reference identity, built-in library lookup,
fallback selection and Marker2's GL renderer version. It does not establish
visual equivalence or a universal advanced-settings grammar for all pens.

## Stored reference identity

The stroke reader and the public native getters use these fields:

| Flexible bit | Implementation member | Meaning | SDK field |
| ---: | ---: | --- | --- |
| 0 | Temporary value, conditional fallback into 16 | Legacy pen-name ID | `legacy_pen_name_id` |
| 1 | 32 | Advanced-settings ID | `advanced_pen_setting_id` |
| 7 | 16 | Pen-name ID | `pen_name_id` |

Model `ObjectStrokeBinaryHandler` retains its `ObjectStrokeImpl*` argument
at handler member 8 in its constructor, `0x2ebbc0`. The flexible reader,
`0x2ed720`, accesses that implementation through `0x2ed74c`. It stores field
1 at member 32 (`0x2ed8d0`) and field 7 at member 16 (`0x2eda34`). Field 0
is read at `0x2ed798`; `0x2eda38`–`0x2eda68` copies it into member 16 only
when that member is -1 and the legacy value is not -1.

`ObjectStroke::GetPenName`, `0x2de974`, accesses the object's implementation
through object member 32, reads implementation member 16 at `0x2de994`, and
passes it to `StringIDManager::GetString`. `GetAdvancedPenSetting`,
`0x2dec00`, reads implementation member 32 at `0x2dec20` and uses the same
lookup. Without an attached string manager, their alternate paths return
the detached string pointers at implementation members 8 and 24.

The writer independently agrees:

- `m_GetBinary_FlexibleData`, `0x2ec5dc`, loads member 32 at `0x2ec648`,
  writes four little-endian bytes at `0x2ec6bc`–`0x2ec6ec`, and sets field
  bit 1 at `0x2ec750`.
- It loads member 16 at `0x2ec810`, writes four little-endian bytes at
  `0x2ec84c`–`0x2ec87c`, and sets field bit 7 at `0x2ec8e0`.

Each stored reference remains independent;
inspection does not overwrite field 7 with field 0 or resolve missing IDs by
guessing a pen.

The regression uses a normal-WDoc field mask `0x82` with distinct IDs:
field 1 refers to `2;`, and field 7 refers to
`com.samsung.android.sdk.pen.pen.preload.Marker2`. The parser returns the
Marker2 name and advanced settings separately. Additional cases retain
legacy-only references, conflicting legacy/modern names, a modern -1
sentinel and absent advanced settings.

## Built-in native registry

PenCommon `PenManagerST::buildList`, `0x4da60`, reads 16-byte pairs from
`0x78968`: a full pen class name followed by a library stem. Its loop limit
at `0x4dbac` is 45 pairs. The table contains 44 named pairs and a final
zero/zero pair at `0x78c28`; only the named pairs are listed below.

Every name has prefix `com.samsung.android.sdk.pen.pen.preload.`. Library
filenames have the form `libSPen<stem>.so`. Presence means an ARM64 library
entry exists in this APK; absence does not prove a device cannot obtain the
library through another loader path.

| Name suffix | Library stem | In APK |
| --- | --- | --- |
| DefaultPen | DefaultPen | Yes |
| AirBrushPen | AirBrushPen | Yes |
| Beautify | Beautify | Yes |
| Brush | SimpleBrush | Yes |
| BrushPen | BrushPen | Yes |
| ChineseBrush | ChineseBrush | Yes |
| Crayon | Crayon | No |
| Crayon2 | Crayon2 | Yes |
| Eraser | Eraser | Yes |
| FadedPen | FadedPen | No |
| FountainPen | FountainPen | Yes |
| InkPen | InkPen | Yes |
| InkPen2 | InkPen2 | Yes |
| MagicPen | MagicPen | Yes |
| Marker | Marker | Yes |
| Marker2 | Marker2 | Yes |
| Marker3 | Marker3 | Yes |
| Marker4 | Marker4 | Yes |
| MontblancCalligraphyPen | MontblancCalligraphyPen | Yes |
| MontblancFountainPen | MontblancFountainPen | Yes |
| MosaicPen | MosaicPen | No |
| ObliquePen | ObliquePen | Yes |
| OilBrush3 | OilBrush3 | Yes |
| Pencil | Pencil | Yes |
| Pencil2 | Pencil2 | Yes |
| Pencil3 | Pencil3 | Yes |
| SelectPen | SelectPen | Yes |
| SelectPen2 | SelectPen2 | Yes |
| Smudge | Smudge | Yes |
| ColoredPencil | ColoredPencil | Yes |
| WaterColorBrush | WaterColorBrush | Yes |
| StraightHighlighter | Marker4 | Yes |
| StraightMarker | Marker3 | Yes |
| LaserPen | LaserPen | Yes |
| GlowPen | GlowPen | No |
| PatternImagePen | PatternImagePen | No |
| StraightInkPen2 | InkPen2 | Yes |
| StraightGlowPen | GlowPen | No |
| BlurPen | BlurPen | No |
| StraightMosaicPen | MosaicPen | No |
| StraightBlurPen | BlurPen | No |
| MosaicPen2 | MosaicPen2 | No |
| StraightMosaicPen2 | MosaicPen2 | No |
| TapePen | TapePen | Yes |

The registry has 37 distinct library stems; seven are absent from the APK.
Java's `BUILTIN_PEN_LIST` at line 75 lists 42 names. Its `getPenInfoList`
at lines 220–239 additionally filters some entries by preload mode. The
Java list is not the complete native registry or proof that a listed plugin
can be loaded. It omits DefaultPen, SelectPen and SelectPen2, and includes
TriangleMosaicPen, which is absent from this native table. A shared library
mapping also does not prove aliases have identical configured behavior.

`PenManagerST::createPen`, `0x4deec`, compares the full requested name with
registry entries at `0x4df38`. A match passes the associated stem to
`loadlibrary` at `0x4df5c`. That loader appends `libSPen`, the stem and `.so`
at `0x4eca4`–`0x4ecc0`, then calls `dlopen` at `0x4ece4`.
The factory resolves `createPenInst` at `0x4df70` and invokes it at `0x4df78`.
Thus the registry supplies the library name; arbitrary stored strings are
not simply used as filesystem paths by this lookup.

## Drawing lookup and fallback

Drawing `ObjectDrawing::drawObjectStroke` obtains `GetPenName` at `0x81a80`.
A null result exits this path before pen-manager lookup. The renderer copies
the name, expands the exact short name `Eraser` to the fully qualified
preload name at `0x81ab4`–`0x81ac8`, and calls `PenManager::GetPenData` at
`0x81ad4`.

PenCommon `GetPenData`, `0x4cefc`, first searches its cached PenData entries
by name. On a miss, it asks `PenManagerST::GetPen` at `0x4cf7c`. If that
returns null, the fallback path searches the cache for the fully qualified
InkPen name (`0x4d000`–`0x4d044`). A matching entry is reused at `0x4d150`.
If no InkPen entry is cached, it requests the fully qualified DefaultPen
name at `0x4d074`, stores the returned pen and caches that entry under its
DefaultPen name.

This fallback depends on manager state. It does not establish a single
universal substitution for an unsupported saved pen, and a missing pen-name
pointer does not take the same path as a nonnull name that fails lookup.

## Advanced settings select Marker2's version

Drawing checks pen attribute 4 at `0x81ba4`. If supported, it passes the
stroke's `GetAdvancedPenSetting` result to pen virtual slot 120 at
`0x81bc4`. Marker2's relocation `0x2eb40` binds that slot to PenCommon
`Pen::SetAdvancedSetting`, `0x46160`; its attribute mask accepts attribute 4.

The base setter copies a nonnull string, tokenizes it with semicolon at
`0x461f4`, and converts tokens through `atoi` at `0x4623c`. Only the first
token position can assign version member 88 (`0x46248`–`0x46254`). It then
replaces its stored advanced-setting string with the current integer
version followed by a semicolon (`0x46270`–`0x4628c`). This base behavior
does not describe plugins that override the setter.

Relevant input distinctions are confirmed by the branches:

| Input to base setter | Version member behavior |
| --- | --- |
| Null pointer | Reset to 1 at `0x462a4`–`0x462ac` |
| Empty string | Leave current version; the token loop is skipped |
| First token -1 | Leave current version at `0x46240`–`0x46244` |
| First token 0 or 1 | Store 1 |
| First token greater than 1 | Store the parsed value |

The comparison at `0x46250` is unsigned. Other negative parsed values are
retained by this base setter; it is not a signed clamp to at least 1.
Token conversion follows the native C routine. These observations cover
Marker2's version token, not arbitrary advanced settings.

Marker2's constructor builds `2;` and passes it to slot 120 at
`0x1ed64`–`0x1ed88`. `GetStrokeDrawableGL`, `0x1f050`, obtains the current
version from `Pen::getVersion`, clamps its selection index to the signed
range 1–2, and indexes `versionTable`. GOT relocation `0x30330` points to
the table at `0x34b60`, whose entries are 0, 1 and 2. The resulting value
selects the GL V1 constructor at `0x1f0e0` or GL V2 constructor at `0x1f0ec`.
An existing drawable is reused only if its saved selected version matches;
otherwise it is destroyed at `0x1f0ac` and recreated.

Consequently, null advanced settings select V1 for Marker2, while a stored
`2;` selects V2. Empty settings and the -1 token can retain the constructor's
version or a previously applied version on a cached pen. The
[V1/V2 comparison](marker2-rendering-findings.md) confirms their
shared alpha composition and identifies a thin-stroke smoothing difference;
the pen name alone still does not establish the renderer version.

## Validation and evidence limits

The string-reference regression failed on the earlier mapping and passes
after the correction. The 11 stroke-metadata tests also cover truncated
fields, unknown-field stops, signed IDs, limits and shared color/width
alignment. Native addresses, relocation targets and registry library
presence were checked directly against the APK's extracted binaries.
The locked `01-basic-formatting` parser/layout expectations do not validate
all pen rendering behavior identified by this registry trace.

[Curve sampling and ordinary completion](marker2-sampling-findings.md) and
[stroke recording](stroke-recording-findings.md) establish the saved-sample
paths. StrokeTip opacity, per-plugin setting overrides and alias-specific
configuration are outside this registry trace. Stored string-table references
identify pen names/settings separately from the selected PDF export mode.

The [AirBrush native capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-saved-native.json)
executes the real AirBrush constructor, whose version is 2; controlled version
field overrides select V1/V2 drawable constructors. Three synthetic saved-redraw
cases execute both GL constructors and redraw paths with bit-identical stamps.
The actual V2-owned RTV1 object builds the same 96×96 CPU pattern and passes its
buffer through a synthetic SPTextureBitmap texture-buffer `UpdateBitmap` slot.
Graphics vtable provenance is hash-pinned in the oracle; MotionEvent, factory
and bitmap interfaces remain synthetic, with no texture upload, GPU pixels or
full application parity established.

An offline comparison of the retained `libSPenAirBrushPen.so` (SHA-256
`92ebe10993ebfcb42fa3f5c55e86f6d469ccac6638421bbd71f92e0558587423`)
finds that the entire 308-byte GLV1 `drawPoint` function at file offset
`0x25ff8` is byte-identical to GLV2 `drawPoint` at `0x2735c`. These offsets
also equal the symbol virtual addresses in this library. `readelf -Ws` reports
separate GLV1 and GLV2 `drawLine` and `RedrawPen` functions; the identical
`drawPoint` bytes alone do not establish class inheritance or equivalent
sampling, selector behavior, or rendered pixels across versions.

## Pencil2 current renderer control

A controlled four-point tool-2 native Drawing capture selects
`Pencil2StrokeDrawableGLV14` and `Pencil2StrokeDrawableRTV4` for both a fresh
empty advanced setting and explicit `14;`. Each emits 84 records of 14 floats
(4,704 bytes); Drawing and saved redraw agree byte for byte, and both settings
produce stream SHA-256
`770651fc1b27bfa8a73731f1a3e24cf1a37847e920581aa36855c5107835c2fe`.
The raw ObjectStroke size is approximately 0.228; PenCommon clamps the effective
size to 0.4. The capture declares a synthetic 1920×1080 context and a host glibc
`sincosf` bridge. It establishes neither Android math equivalence nor pixel,
device or PDF parity. A separate unset-setting Drawing control selects GLV1; it does not establish V1 geometry. At effective size 2, five V14 one-factor controls retain identical Drawing/replay streams: pressure and orientation ramps change the records, while the tested timestamp and below-threshold tilt ramps match the neutral baseline.

The test-only Rust sampler checks all 14 float lanes bit for bit in three
84-record streams, including combined pressure/orientation and separate
pressure controls. Its compact [native fixture](../../conformance/pencil2-v14-native.json)
retains the input, native pressure-alpha state/returns and hashes; the packed
record file is 14,112 bytes. Portable `libm` math matches these host captures. A test-only public Stroke adapter derives the controlled fixed-opacity-disabled alpha state and reproduces all six streams. Empty and `14;` settings match in one paired native Drawing control; parsed-note context remains unverified. A controlled RTV4 replay at inverse scale 0.5 also matches all 84 reconstructed records. A separate bounded native Drawing canvas probe applies the supplied matrix through native Graphics and invokes Pencil2 GLV14 `getInverseScale`: identity and the native PDF exporter’s translation-only matrix return (1, 1), supplied scale (2, 2) returns (0.5, 0.5), and (2, 4) returns (0.5, 0.25). The [compact receipt](../../conformance/pencil2-v14-drawing-scale-context-native.json) records the matrix and results; drawable selection and canvas creation use host stubs, and object paint is suppressed. This establishes neither a full saved-note Drawing stroke nor device behavior.

The native V14 path uses its own 150×150 grain and three 100×100 point masks;
it cannot inherit Pencil3's atlas or record layout. Direct native texture
methods reproduce the embedded grain and point arrays as 90,000- and
120,000-byte uploads under synthetic GL callbacks. A separate controlled RTV4
replay runs native Init and Draw with host Mesa sampling and framebuffer
readback; it does not establish Android device output. Pencil2 remains generic
in production pending output transport and broader validation. Compact capture receipts and the pinned candidate are
retained under the ignored `tmp/vector-parity-plan/pen-family-audit/` workspace.
In a matched-context resvg 0.47 control, rows 78–79 yield alpha 37 at (28, 182) both alone and within all 84 rows, so the earlier alpha 39 came from a differently defined pair; isolate the definition differences before attributing the residual to compositing (receipt: `pencil2-v14-svg-paint-v8-context-subset-audit/matched-render-receipt.json` under that workspace).

An ordered native RTV4 host-Mesa prefix replay yields alpha 0, 12, 38 and 38 at (28, 182) after 78, 79, 80 and 84 records respectively; the full-prefix frame matches the earlier capture. The isolated native rows yield 12 and 27, consistent with rounded source-over, while the matched SVG rows yield 10 and 28 and compose to 37. The per-stamp difference therefore suffices to explain this pixel, but does not establish general blend parity. The reproducible wrapper and compact receipt are retained in `tmp/vector-parity-plan/pen-family-audit/pencil2-v14-prefix-probe/`. This is a controlled host replay, not Android, saved-document, PDF or production rendering evidence.

A resolution-specific scalar model evaluates the shader and rounded source-over for every potentially covered pixel of that 192×192 frame. It differs from the native alpha at 22 pixels, with maximum error 4 and absolute error sum 29; the V8 SVG differs at 32 pixels, with maximum 19 and sum 93. A precomposed PNG alpha mask embedded in SVG renders the modeled alpha exactly with resvg 0.45.1 at that size and retains those 22 differences from native. The synthetic black-stroke SVG is 1,214 bytes, including its 589-byte PNG. This is a resolution-specific raster mask, not resolution-independent vector paint. It covers one scale and stroke; the mask needs recomputation at each output resolution and does not establish device, PDF reader, arbitrary-color or production parity. The reproducible scripts and compact receipts are in `tmp/vector-parity-plan/pen-family-audit/pencil2-v14-full-frame-scalar-model/`.

A controlled host-Mesa RTV4 replay translates the same 84 records by six pixels in the draw matrix. After alignment with the baseline, alpha differs at 37 of 52 nonzero-union pixels (absolute difference sum 577, maximum 54); the grain offset uniform remains (0, 0). The fragment shader samples grain using `gl_FragCoord`, so this is consistent with grain fixed in device coordinates as geometry moves. A 75-pixel translation, the nominal grain repeat period, still differs at 14 of 52 pixels (sum 226, maximum 45); the aggregate frame does not isolate the cause or establish exact periodicity. The [compact receipt](../../conformance/pencil2-v14-device-grain-transform-native.json) identifies the scripts and controls. This is not full Drawing, saved-note, Android, or production output evidence.

A [90-degree rotation control](../../conformance/pencil2-v14-rotation-native.json) differs at 42 of 52 alpha pixels after alignment, with absolute error 462 and maximum 39. The grain offset remained zero; device-coordinate sampling and rasterization may both contribute. This host capture does not establish a scalable vector paint contract.

A host-Mesa RTV4 target-height control replays the same 84 records and draw matrix on 192×192 and 192×267 RGBA8 targets. At the same bottom-origin readback coordinates, the nonzero alpha union contains 52 pixels and only one differs (33 versus 30); comparing with a 75-pixel vertical displacement gives 52 differences. This constrains framebuffer-origin behavior for this controlled crop, but does not prove exact size invariance or a portable tiling contract. The [capture script](../../conformance/pencil2_v14_target_height_capture.py) and [compact receipt](../../conformance/pencil2-v14-target-height-native.json) retain the inputs and results. This is not full Drawing, saved-note, Android, PDF, or device evidence.

A separate host-Mesa control holds the geometry and matrix fixed and overrides only the grain-offset uniform from (0, 0) to (75, 0). Despite the nominal one-period shift, 11 alpha pixels change (absolute difference sum 127, maximum 34). This isolates the difference from translated geometry; finite-precision texture coordinates are a possible explanation, not an observed cause. The [receipt](../../conformance/pencil2-v14-grain-offset-native.json) pins the capture and reproduction script. This host adapter control does not establish saved-note or device behavior.

Four additional host-Mesa controls override that same uniform to -0.001, 0.001, 74.999 and 75.001 pixels. The frames at -0.001 and 74.999 have identical RGBA hashes, as do the frames at 0.001 and 75.001. Relative to zero offset, the negative and positive controls change 2 and 34 alpha pixels, respectively. The result is consistent with sensitivity near grain texture sampling boundaries, but does not establish the exact GPU arithmetic, other scales, or device behavior. The [receipt](../../conformance/pencil2-v14-grain-offset-epsilon-native.json) retains the controls and reproduction command.

A controlled host-Mesa RTV4 replay of the same 84 black-stroke records changes only the preclear from transparent black to RGBA (64, 32, 96, 128). All 52 changed pixels agree within one byte per channel with source-over predictions from the transparent-target alpha; the observed blend calls use SRC_ALPHA and ONE_MINUS_SRC_ALPHA for RGB, and ONE and ONE_MINUS_SRC_ALPHA for alpha. The [compact receipt](../../conformance/pencil2-v14-background-native.json) records the control and reproduction command. This aggregate observation does not establish exact fragment arithmetic, full Drawing, saved-note, Android, device, or production behavior.

The test-only geometry sampler also matches all 84 native Drawing records for a synthetic stroke with translucent color `0x80336699`, including its RGB lanes and pressure alpha. The [receipt](../../conformance/pencil2-v14-color-80336699-native.json) and binary retain that control. This extends the captured geometry profile only; it does not establish textured paint or arbitrary color parity.

## Literal StraightMarker control

Six native Drawing controls request the literal Marker3 and StraightMarker names
with null, empty and `5;` settings. Both load `libSPenMarker3.so`, return pen
vptr `0x5055958`, select GLV1 for null and GLV5 for empty/`5;`, and emit identical
complete streams for each matched pair. The controlled two-point tool-2 source
has explicit size 8, ARGB `0xff336699`, and curve enabled. The retained capture
SHA-256 is `8537e503568d6a63512c8acf0777168a8adac85189afee8408032b1eb5c118e9`.
This supports sharing the existing bounded V5 page handler while preserving its
settings, tool, style and point-count restrictions. It does not establish
archive deserialization, omitted style defaults or independent pixel parity.
