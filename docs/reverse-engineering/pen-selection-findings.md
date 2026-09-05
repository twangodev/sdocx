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

The prior stroke metadata implementation had swapped the two reference
names and consequently mislabeled the legacy field. The parser now matches
this getter/reader/writer agreement. Each stored reference remains independent;
inspection does not overwrite field 7 with field 0 or resolve missing IDs by
guessing a pen.

The regression uses a normal-WDoc field mask `0x82` with distinct IDs:
field 1 refers to `2;`, and field 7 refers to
`com.samsung.android.sdk.pen.pen.preload.Marker2`. Before the correction,
resolving `pen_name_id` returned `2;`. The corrected parser returns the
Marker2 name and advanced settings separately. Additional cases retain
legacy-only references, conflicting legacy/modern names, a modern -1
sentinel and absent advanced settings.

The change affects metadata field identities, including their Serde names.
The previously introduced `legacy_advanced_pen_setting_id` is replaced by
`legacy_pen_name_id`. Ordinary rendered stroke color and width do not use
these IDs and retain their existing behavior.

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
The SDK should retain the requested identity even when a renderer needs a
fallback or reports unsupported brush behavior.

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
Token conversion follows the native C routine, so these observations are
not a recommendation to parse arbitrary future settings as an integer.

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
[V1 opacity findings](pen-opacity-findings.md) must not be silently applied
to every Marker2 stroke on the strength of its name alone.

## Validation and remaining work

The string-reference regression failed on the earlier mapping and passes
after the correction. The 11 stroke-metadata tests also cover truncated
fields, unknown-field stops, signed IDs, limits and shared color/width
alignment. Native addresses, relocation targets and registry library
presence were checked directly against the APK's extracted binaries.
Workspace tests with all features, Clippy with warnings denied, Rust 1.92
checking, the WASM target and the cached `01-basic-formatting` corpus check
passed. The corpus check retained its locked parser/layout expectations;
it does not validate the newly traced pen rendering behavior.

Remaining APK work includes the complete Marker2 V2 draw path, per-plugin
advanced-setting overrides, alias-specific configuration and the later
effect of render-thread alpha setters. New SDOCX/PDF pairs should record
pen selection and export mode; the stored string table can then distinguish
the exact name and settings used in each visual comparison.
