# Rendering support

This document maps the Rust SDK's saved-document rendering surface. It separates
structural inspection, semantic decoding, drawing, and evidence of Samsung
behavior. It is not a claim of complete Samsung Notes compatibility.

“Implemented” means a Rust route exists for the stated scope. “Approximate” means
the route preserves content with geometry or appearance that does not reproduce
the native feature. Native evidence may establish a serialized field, an isolated
arithmetic kernel, a bounded producer, or a real-document comparison; these are
different guarantees. Successful parsing and an empty diagnostic report do not
certify visual parity. Chromium preview and vector SVG/PDF are the supported
rendering target. Bounded Fountain V14/V16 witnesses also render in Firefox.

## Object types and locations

The [object identifiers](../crates/sdocx/src/types.rs) and
[physical page decoder](../crates/sdocx/src/page.rs) define the first boundary.
Every stored object retains its declared type, payload location and child
structure through [storage](../crates/sdocx/src/storage.rs). Borrowing its original
payload requires the original uncompressed page bytes. Retention does not imply
that the object has semantic content or a renderer.

| Native ID | Object | Semantic decoding or inspection | Rendering |
| --- | --- | --- | --- |
| 0 | None | Stored/common metadata | No dedicated renderer |
| 1 | Stroke | Sample channels, properties and pen resources | Page vectors; supported reconstructions or generic approximation |
| 2 | TextBox | Rich text, styles, paragraphs and object spans | Page text and supported embedded content |
| 3 | Image | Geometry, media references, supported crop fields and fill-effect transparency | Page and text-flow images with quantized fill alpha; bounded page-image nine-patch geometry; native PNG decode/draw and crop-edge clamp controls verified; partial-source alpha rounding and other effects remain partial |
| 4 | Container | Common metadata and ordered children | Nested page objects; supported parent visibility and root selection |
| 7 | Shape | Geometry, styles, saved paths, controls and embedded text | Supported page templates/paths, solid paint and bounded linear/radial fill/outline gradients; partial styling |
| 8 | Line | Geometry, styles, controls and saved paths | Straight lines and supported paths; bounded linear/radial outlines, including descending two-stop linear gradients with partial alpha and radial endpoint projection using the first ten distinct attached stops; partial styling/routing |
| 9 | DeprecatedDummyStroke | Stored/common metadata | No dedicated renderer |
| 10 | Voice | Stored/common metadata; [own attachment/source contract](reverse-engineering/voice-source-findings.md) is distinct from typed note VoiceData | No voice-object renderer |
| 11 | Formula | Bounded expressions, result/image rectangles, media ID, strokes and label graphs | PNG placement with resolved native bounds; source/answer ink fallback for absent or corrupt images; other image formats unsupported |
| 12 | DeprecatedTable | Stored/common metadata | No dedicated renderer |
| 13 | Web | Stored/common metadata | No dedicated renderer |
| 14 | Painting | Stored/common metadata | No dedicated renderer |
| 15 | StrokeDevelopmentVersion | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 16 | Video | Stored/common metadata; [source/thumbnail binding contract](reverse-engineering/vector-retention-findings.md#physical-video-source-bindings), without a typed Video projection | No dedicated renderer |
| 17 | Link | Stored/common metadata; separate rich-text hyperlink spans exist | No link-object renderer |
| 18 | StrokeBrush | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 19 | Unknown | Explicit Samsung unknown marker | No dedicated renderer |
| 20 | Plot | Bounded plot/graph expressions, colors, styles and visibility | Inspection only; no graph evaluation or plotting |
| 21 | Math | Bounded formula envelopes, margins and connected plot UUIDs | Inspection only; no automatic child-formula drawing |
| 22 | Table | Typed table when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 23 | CodeBlock | Typed title/body when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 24 | AttachedFile | Stored/common metadata, including common attachment reference | No attachment-card renderer |
| 100 | StrokeGroup | Stored/common metadata and children | No group renderer; supported children can be traversed separately |
| Other IDs | Future/unrecognized objects | Stored structure and unknown-type diagnostics | No own renderer; supported children can be traversed separately |

Rich-text routing is separate: [initial table/code decoding](../crates/sdocx/src/note.rs),
[image resolution](../crates/sdocx/src/image.rs), and
[embedded rendering](../crates/sdocx/src/render.rs) support only image, table and
code content. A table inside a text box therefore differs from a physical
type-22 page record. Flattened supported children under an unsupported parent
do not establish that parent's transform or composition semantics.

Native [card contracts](reverse-engineering/card-source-findings.md) distinguish
saved Web/Link/AttachedFile text, action metadata and source bindings from
regenerated card artwork. A thumbnail does not retain the contents of a bound
Web or attached document.

The [formula](reverse-engineering/formula-findings.md),
[math](reverse-engineering/math-findings.md), and
[plot](reverse-engineering/plot-findings.md) findings describe inspection APIs.
Native [formula drawing](reverse-engineering/formula-rendering-findings.md)
has separate image/stroke selection rules; decoded expressions do not require
or establish an SDK evaluator.

Page and text-embedded images with an ordinary image fill can use nine-patch cuts when the source
and destination patches are positive and in bounds. The saved reference width
and page width determine native f32 cut positions in the source-sized processed
bitmap; an active crop takes precedence. The vector reconstruction approximates
that intermediate bitmap. Transformed fills, original/border metadata, and degenerate or out-of-bounds
patches retain the existing fallback. Active nine-patch records retain an
unsupported diagnostic. A [synthetic native nine-color control](../conformance/nine-patch-native-control.json)
compares source-sized preprocessing and fractional placement against public exports.
The nine-patch SVG reduces total RGB error from 57,589 to 8,135 compared with
ordinary image placement, but misses 20 native edge-support pixels; the PDF
reduces error from 77,804 to 42,475. A [translucent native control](../conformance/nine-patch-translucent-native-control.json), using the same direct helper and placement with alpha values from 0 to 255, gives SVG error 4,515 versus 45,141 without nine-patch and PDF error 28,008 versus 53,302. These are single synthetic controls with directly supplied q, not saved-note or device tests. Sampling and seam differences remain, and native pixel parity is unverified. A [nonuniform gradient probe](../conformance/nine-patch-gradient-native-control.json) found that source-sized bilinear precomposition improves the tested SVG but worsens the tested PDF results, so the shared rendering path is unchanged. A [four-bit interpolation probe](../conformance/nine-patch-four-bit-sampler-control.json) reduced intermediate error for opaque and translucent nonuniform sources and improved both tested SVGs, but did not match native pixels; PDF results moved in opposite directions, so it does not establish a common rendering change.

## Ink

Sample coordinates, pressure, timestamps and optional tilt/orientation are
decoded and retained. A retained channel is not necessarily used by a pen's
drawing route. The [pen registry](../crates/sdocx/src/ink.rs) identifies names and
libraries, not a renderer for every registered pen.

Coverage is incomplete. The pinned APK registry has 44 names: 11 have bounded
default page-rendering routes, 5 require explicit profiles, 13 bundled names
still use generic strokes, 11 refer to unbundled plugins, and 4 are editing or
transient tools whose persistent paint is not established. These counts describe
dispatch, not complete version coverage or visual parity.

| Pen family | Current saved-page rendering and missing work |
| --- | --- |
| Ballpoint and ink | DefaultPen and InkPen V1 have bounded default routes. DefaultPen handles gradual motion using its native displacement gate; synthetic native captures cover delayed starts, small successive movements, and inclusive distance thresholds across tools 0–3. Three additional synthetic Drawing controls cover tool 4 in both curve modes and a short stroke; the public SVG and direct PDF route is shared with tool 3 for the paired geometry. InkPen V4 and InkPen2 V9 require explicit display profiles; the latter admits normal-color StraightInkPen2 `9;` and non-null empty-setting aliases for both names. An additional output-scale opt-in enables a bounded V1 approximation for InkPen2 and StraightInkPen2 with null or literal `1;` settings. |
| Fountain | Fountain V14 and saved AS18/V16 have bounded default routes; V17 rainbow is experimental and explicit. MontblancFountainPen remains generic, with test-only saved-path and expanded triangle-mesh checks for four GLV1 controls; painted output is unverified. |
| Pencil and crayon | Pencil, Pencil2, ColoredPencil and Crayon2 remain generic. Pencil2 V14 and ColoredPencil V4 preserve explicit saved RGB for the observed normal stylus profiles; Pencil2 has one nonblack native color control, and its source alpha and textured paint remain unverified in the generic renderer. Pencil3 V3 requires an explicit texture/context profile; its other versions remain generic. Pencil2 V14 has grain and point atlases distinct from Pencil3. Test-only samplers match captured Pencil GLV1, Pencil2 V14, ColoredPencil V4 and Crayon2 V2 geometry, with explicit host RNG profiles where needed. Legacy Pencil GLV1 regenerates a captured 47-record stream from a synthetic public `Stroke`; its test-only paint model matches one native frame under all 12 explicit host arithmetic hypotheses. Native Model construction and top-level Drawing reproduce that stream, atlas and frame. Parsed-note admission, backend arithmetic and SVG paint remain unresolved. ColoredPencil V4 test-only intrinsic SVG paint matches four native host-Mesa color controls exactly. A test-only public Stroke adapter reproduces the controlled native streams with explicit glibc RNG/context; the exported image-atlas PDF has extra support in MuPDF and PDFium. Single-stamp controls show residual reader differences even without the clip, and toggling PDF interpolation does not change the four MuPDF outputs. Production and Android textured paint remain unverified. |
| Highlighters | Marker2, Marker3 V5 and Marker4 V7/V8 have bounded routes, with edge/scale and effect limits below. StraightHighlighter reaches Marker4; StraightMarker reaches the bounded Marker3 V5 route after native literal-name controls matched. Marker defaults to the generic route; an explicit `LegacyMarkerCenterlineProfile` accepts caller-supplied native screen dimensions for supported legacy metadata and geometry, including explicit empty or literal `1;` settings with saved ARGB or the captured omitted-color default, while replay and unsupported cases retain the fallback. Its round caps and overlap approximate native textured paint; device parity is unverified. Its test-only paint model reproduces coverage at all five native draw prefixes, with at most one channel level of color error under the captured host precision profile. |
| Textured brushes | BrushPen remains generic; Brush/SimpleBrush has an experimental opt-in profile. BrushPen has a test-only pipeline regenerating one manual native caller geometry stream from a public `Stroke` and matching its painted frame exactly under an explicit host-Mesa profile; parsed-note admission and SVG/PDF paint are unverified. SimpleBrush has a bounded geometry model and one native host-Mesa paint control; broader exported coverage and blending remain unverified. AirBrush V2 has an explicit `SavedV2` textured-stamp route for normal tool-1 and tool-2 strokes with empty or literal `2;` advanced settings, and tool-0 strokes with empty settings, and tool-3 strokes with empty or literal `2;` settings. Three [synthetic tool-3 controls](../conformance/airbrush-tool3-matrix-native.json) match tool-1 geometry and native frames; their public SVG and direct PDF reproduce the corresponding tool-1 output. Two SVG rasters match their native frames exactly; the third follows the existing bounded 12-channel-error control. Other tool-3 inputs extrapolate from those controls. Two [synthetic tool-0 host-Mesa controls](../conformance/airbrush-tool023-probe-native.json) match tool-2 geometry and frames on curved and zigzag paths; one curved public SVG matches its native frame, while direct PDF alpha errors improve over fallback in MuPDF and PDFium. Other admitted tool-0 inputs extrapolate from these controls. A synthetic tool-1 Drawing/Mesa curved control matches the sampler exactly; its public SVG and direct PDF improve over the generic fallback in [paired comparisons](../conformance/airbrush-tool1-export-comparison.json). A [fresh matrix](../conformance/airbrush-tool1-matrix-native.json) confirms the literal `2;` alias on that control and matches sampler geometry for zigzag and reverse paths with different sizes and colors. Their public SVG rasters match the native frames exactly; the [export comparison](../conformance/airbrush-tool1-matrix-export-comparison.json) records nonzero direct-PDF raster errors in MuPDF and PDFium. Other tool-1 inputs extrapolate from these controls and the shared tool-2 paint model. Its Rust SVG exactly matches transparent and light-paper native host-Mesa controls. Explicit `SavedV1` admits null or literal `1;` setting strokes; its public SVG matches one synthetic native host-Mesa frame, and its sampler matches paired native boundary controls. V1 replay uses inferred boundaries from completed native redraws; native progressive display remains unverified. Admitted empty V1 streams emit no paint, extrapolating from stationary host captures; other settings retain generic rendering. This host control does not establish device or PDF parity. Its native controls also cover low-precision material sampling and exact texel/quad boundary ties at an integer stamp position. Eight additional host controls cover source colors, low/zero alpha, size truncation and a horizontal atlas boundary; the route reconstructs each triangle's F32 UV plane separately. Arbitrary transforms, device appearance and PDF parity remain unverified. ChineseBrush GLV1–V3 and the captured omitted, `0;`, empty and literal `4;` aliases require an explicit profile; one paired synthetic Drawing control shows stored setting 4 selects GLV3 and matches setting 3's native host-Mesa frame. A separate omitted-color control selects black for setting 4. These are bounded controls, not device parity or general V4 reconstruction. |
| Calligraphic nibs | MontblancCalligraphyPen V1 and Oblique V1/V4 have bounded default routes. Oblique V2/V3 and rejected settings remain generic. |
| Paint and effects | MagicPen V1 has a bounded default route. WaterColorBrush, OilBrush3, Smudge, Beautify and TapePen remain generic. WaterColor V1/V2 preserves explicit saved RGB for the observed normal stylus profiles. Rust SVG tests exactly match a low-density V1 host-Mesa frame and a 480-dab native RT density control, including the high-density branch under an explicit host precision profile. A controlled public Drawing V22 test regenerates eight native dabs and exactly matches the full host-Mesa frame; its pinned mediump vertex shader narrows model/MVP arithmetic to f16 and widens clip output to f32. A separate native Model construction and top-level Drawing capture reproduces the nine-pixel host control, with the submitted canvas matrix traced through to the native render message. These are controlled-input test routes, not saved-note rendering; production paint and exported appearance remain unverified. Smudge has a test-only four-pass feedback/blur model matching transparent and two nonuniform native host controls, including an interior stroke whose first pass changes the canvas; a test-only source-to-point model matches straight and bent host controls, while general stroke generation and production/vector paint remain unverified. Tape's test-only source models exactly regenerate native host point streams for controlled two- through five-point curved inputs and one curve-disabled input. The two-point control uses a guarded host `fmodf(0, 0)` and empty-palette path, not verified Android math behavior. A test-only pattern composite matches its host capture's alpha support and stays within one channel value (12 differing channels); Four reveal-mask tests regenerate masks from source-derived point streams, including native-rotated Model points, matching support and differing by at most one value in one pixel per control; a captured-mask attenuation test stays within one channel value. Both are bounded host controls; general mask generation and saved-stroke parity remain unverified. OilBrush3 has test-only GLV1 mesh checks; a synthetic public `Stroke` regenerates the captured 15-record stream under explicit Drawing-context and host-math inputs. Native Model construction and top-level Drawing independently reproduce that stream. Its source-input paint model retains explicit host RNG/math and initialization assumptions. Its Rust CPU paint model exactly matches all 30 native draw-prefix RGBA frames across two bounded host controls, using captured fixed-point lowp sampling and RGBA8 blend quantization. Five 192×192 SVG controls match coverage and alpha with RGB differences up to two levels; five 1920×1080 controls retain exact coverage with RGBA differences up to two levels. Production and exported paint remain unverified. |
| Tools and unavailable plugins | Eraser, SelectPen/SelectPen2 and LaserPen are separate tool/lifecycle questions, not verified persistent paint. Crayon, FadedPen, Glow/Blur/Mosaic families and PatternImagePen lack their plugins in the pinned APK; their straight aliases are not effect renderers. See the [complete native registry](reverse-engineering/pen-selection-findings.md). |

The `pdf` feature also exposes the explicit `legacy_marker_vector_profile`,
using `LegacyMarkerCenterlineProfile` to supply native PenData screen dimensions.
For supported saved Marker metadata and ordinary noncircular source geometry,
it prepares bounded vector paint and a retained PDF form with a shared per-page
budget. Cap texture is represented as a binary nonzero-texel shape; vector
boundary coverage approximates native nearest sampling. Unsupported inputs, replay, descendant strokes, sliced pages, and
budget failures retain the existing fallback. On synthetic straight/bent host
controls, matched-background SVG RGB L1 was 2993/2489; direct PDF L1 was
4721/3796 in MuPDF and 5332/4230 in PDFium. A varied blue control at
720×480 PenData dimensions passes a finer checked grid: SVG/MuPDF/PDFium L1 is
5733/9650/9032 versus 24117/23904/23671 on fallback. All grid retries share the
same per-stroke and per-page work limits. A self-overlap host control now
passes bounded clipping and measured Scene reservation: SVG/MuPDF/PDFium L1 is
4255/6699/8784 versus 25199/25098/26880 on fallback. These checks preserve the
existing page limits. Two successive native Drawing submissions of an identical
top-layer stroke also improve over fallback in SVG and both PDF readers; a second
control with a blue, offset stroke also improves and checks paint order. Other
multi-stroke compositions remain unverified. A synthetic straight control with checked 0.5× and 2×
output transforms improves over fallback in SVG, MuPDF and PDFium; other
transforms remain unverified. This is an experimental approximation. For effective widths below 3, SVG retains the fallback; direct
PDF retains it below 6 because synthetic narrow-stroke controls favored the
fallback in those outputs. Texture behavior across versions, raster precision, arbitrary
transforms, saved-note and device parity remain unverified. The profile is unavailable without the
`pdf` feature. See the [Marker findings](reverse-engineering/stroke-rendering-findings.md#legacy-marker-source-to-mesh-controls).

An experimental `SimpleBrushRenderProfile::HostV25` opts eligible strokes into
ordered radial-gradient paint. It currently admits null or empty settings, tools 2 and 3,
curve-enabled strokes without saved stylus channels, with two through 224 points and finite sizes from 3 through 32; unsupported
inputs, transformed descendants and replay retain generic paint. The profile
ignores saved alpha as the captured shader did, and assumes one page unit per
output pixel for its fringe. Each scene has a shared stamp budget, and sliced
exports omit off-slice stamps while retaining boundary intersections. It is not validated for general saved notes,
other scales, device rendering, or replay parity.
Two synthetic three-point controls at [size 28](../conformance/simplebrush-size28.json) and [size 32](../conformance/simplebrush-size32.json) match native geometry. Profile SVG overlaps all 522 and 612 native pixels, respectively, with lower alpha error than fallback. Direct document PDF rasterization overlaps 492/522 and 577/612 support-union pixels, versus 126/522 and 131/612 for fallback, and reduces RGB error. Other newly admitted sizes and stroke shapes are extrapolated from these host controls.
Three synthetic [empty-setting controls](../conformance/simplebrush-empty-setting-host.json) match the corresponding null-setting native geometry and host-Mesa frames; broader combinations remain extrapolated. A separate [literal `1;` control](../conformance/simplebrush-literal-one-host.json) matches null-setting geometry and host-Mesa paint for one synthetic two-point stroke; the same opt-in accepts this setting for the existing bounded profile, extrapolating beyond that control.
Short movements are skipped against the last accepted point for tool 2; all-short sequences emit one stamp, while short terminal movement after accepted points uses the captured terminal curve. Synthetic [first-skip](../conformance/simplebrush-first-skip.json), [repeated-skip](../conformance/simplebrush-repeat-skip.json), [terminal](../conformance/simplebrush-terminal.json), and [short-sequence](../conformance/simplebrush-short-large.json) controls match native geometry and improve SVG/PDF comparisons.
Four synthetic controls at sizes 3 and 24 with seven and sixteen points also match native geometry; their SVG and PDF comparisons improve on generic paint. Admission between controls is extrapolated.
The opt-in also admits tool 0 at sizes 4 through 32 for two to sixteen points. A fresh [four-point size-32 host control](../conformance/simplebrush-tool0-size32-native.json) matches tool 2 native geometry and the complete RGBA frame; its public SVG overlaps all 698 native support pixels with alpha error 1,464, versus 303 pixels and error 98,940 for fallback. Direct PDF output matches tool 2. Other paths and sizes extrapolate from these controls. A fresh [four-point size-20 host control](../conformance/simplebrush-tool0-multi-large-native.json) matches tool 2 native geometry and the complete RGBA frame; its public SVG overlaps all 462 native support pixels with alpha error 1,177, versus 210 pixels and error 66,747 for fallback; direct PDF output matches tool 2. Intermediate sizes and other paths extrapolate from this control. Two additional [paired host controls](../conformance/simplebrush-tool0-large-native.json) at sizes 20 and 32 match tool 2 native geometry and complete RGBA frames; their public SVG and direct PDF outputs are compared with tool 2. Intermediate sizes and other two-point trajectories are extrapolated from these controls. Two-point strokes admit short movements; multi-point strokes require the final movement to reach 1.5 units on an axis, except three-point strokes whose first movement reaches that threshold. [Short-terminal host controls](../conformance/simplebrush-tool0-terminal-native.json), including an exact 1.5-unit first movement, match tool 2 geometry and the full frame; their public SVG and PDF match tool 2. SVG alpha error against the native crops is 954 and 394, versus 25,005 and 14,423 for fallback, respectively. A four-point excursion and return also matches natively but remains outside this admission. Other trajectories extrapolate from these controls. The capture harness resides in ignored scratch files and is not independently reproducible from a clean checkout. Three additional paired size-8 host captures cover short interior movements, seven points, and sixteen points with varying pressures; tool 0 and tool 2 match native geometry and full frames, and their public SVG and direct PDF outputs match. The reason for this tool-selection equality is not established. Seven [paired host controls](../conformance/simplebrush-tool0-native.json), covering vertical, reverse, and curved strokes at size 8 and two-point and curved strokes at sizes 4 and 12, match tool 2 geometry and full frames; their public SVG and direct PDF outputs match tool 2. A separate two-point control improves SVG alpha error against the generic fallback (706 versus 13,387). Fresh native host-Mesa controls cover a short two-point movement and a six-point stroke; their public SVG improves alpha error over fallback, and direct PDF matches the corresponding tool 2 output. See the [short](../conformance/simplebrush-tool0-short.json) and [six-point](../conformance/simplebrush-tool0-six.json) receipts. Other shapes within this gate extrapolate from these controls; device and saved-note parity remain unverified. Four synthetic [tool-3 controls](../conformance/simplebrush-tool3-native.json) at size 8 match the corresponding tool-2 native geometry and host-Mesa frames byte for byte, including vertical, reversed, and short movements. Additional three- and seven-point controls match tool-2 native geometry and host frames; their profiled SVG and direct document PDF are compared in the test suite. Four-point first-skip and short-terminal controls also match tool-2 native geometry and paint; their SVG and direct PDF are compared in the test suite. Fresh tool-3 controls with 16 and 64 points at size 8 and 16 points at size 20 match the corresponding tool-2 native geometry and full host frames. A 224-point size-8 control matches native geometry and the complete painted crop. Their public SVG and direct document PDF are compared in the test suite. Admission of other combinations extrapolates from these controls. Other sizes, pressures, and admitted point combinations extrapolate from these controls. Public direct document PDFs for the horizontal and vertical pairs are byte-identical across the two tool types.
Two-point host controls at [size 4](../conformance/simplebrush-two-4.json), [8](../conformance/simplebrush-two-8.json), and [16](../conformance/simplebrush-two-16.json), plus size-8 [vertical](../conformance/simplebrush-two-vertical.json) and [reversed](../conformance/simplebrush-two-reversed.json) controls, match native geometry and improve SVG/PDF comparisons over generic paint. Synthetic [0.01-unit](../conformance/simplebrush-two-short.json) and [zero-distance](../conformance/simplebrush-two-zero.json) controls yield one native sample and improve SVG/PDF comparisons over generic paint. Additional [geometry controls](../conformance/simplebrush-two-short-pressure-geometry.json) cover vertical sub-gate movement, unequal pressures, and two pressure/size extremes. Other sub-gate sizes, pressures and positions are extrapolated. Other directions and combinations remain unverified.

Three intermediate-size synthetic host controls—[three points at 10](../conformance/simplebrush-size-3-10.json), [seven at 5.5](../conformance/simplebrush-size-7-55.json), and [sixteen at 20](../conformance/simplebrush-size-16-20.json)—match native geometry byte for byte. Profile SVG overlaps 167/167, 312/312, and 2008/2009 support-union pixels versus 50/167, 144/312, and 931/2008 for generic paint. MuPDF PDF overlaps 141/167, 270/327, and 1916/2016 versus 50/167, 144/312, and 967/2008. Both outputs reduce alpha error in each control. Admission of other sizes in the interval and other size/point combinations extrapolates from these captures; device, saved-note, and general PDF parity remain unverified.
Additional five-, seven-, and sixteen-point host controls at sizes 8, 12, and 8 respectively match native geometry byte for byte. Their SVG support intersects all native painted pixels, with at most two extra edge pixels; the corresponding PDF overlaps 241/273, 529/574, and 895/985 native pixels, compared with 123/273, 254/574, and 483/985 for generic paint. Admission for intermediate combinations is an extrapolation from these controls.
Synthetic size-8 [17-point](../conformance/simplebrush-17-8.json), [32-point](../conformance/simplebrush-32-8.json), and [64-point](../conformance/simplebrush-64-8.json) controls also match native geometry byte for byte. Profile SVG overlaps all native painted pixels; MuPDF PDF overlaps 938/1068, 1821/2063, and 3703/4191 of the respective union pixels versus 504/1036, 984/1993, and 2007/4045 for generic paint. Both outputs have lower alpha error than generic paint. Other point-count and size combinations are extrapolations; device and saved-note parity remain unverified.
Synthetic size-8 [128-point](../conformance/simplebrush-128-8.json) and [192-point](../conformance/simplebrush-192-8.json) controls match native geometry and improve SVG and PDF comparisons over generic paint; intermediate point counts and other sizes remain extrapolations. The 192-point profile SVG overlaps 12,192/12,208 native support-union pixels versus 5,631/12,192 for fallback; PDF overlap is 11,182/12,648 versus 6,063/12,192. A [224-point size-8 control](../conformance/simplebrush-224-8.json) also matches geometry; profile SVG overlap is 14,233/14,251 versus 6,587/14,233 for fallback. Public document PDF overlap is 13,053/14,839 versus 7,080/14,233, and its RGB error over the opaque page is 416,281 versus 2,190,796.
Two six-point synthetic host controls at sizes 4 and 16 match native geometry bit for bit. Their SVG support overlaps native support at 211/211 and 623/623 pixels, versus 95/211 and 248/623 for generic paint. Rasterized PDF overlap is 188/211 and 582/623, versus 105/211 and 267/623 for generic paint. These are bounded host controls.
For one clipped synthetic size-8 SimpleBrush host-Mesa control, the profile's SVG has
137/137 native support overlap,
versus 40/137 for the generic fallback; its total alpha error is 737 versus
23,129, and its raw RGBA error is 2,178 versus 66,288. For the same
input, a MuPDF 1.28.2 rasterization of the generated PDFs gives 115/137 versus
50/137 support overlap and alpha error 3,845 versus 23,138. The captured shader
ignores input alpha; the generic Brush route also uses whole-stroke opacity 1
for this control. A second clipped size-12 control with the same points has SVG support overlap
185/185 versus 62/185 for the generic route, with alpha error 733 versus
33,240. The paired PDF rasterization gives 172/185 versus 60/185 overlap and
alpha error 3,574 versus 32,976. These 1× transparent-background comparisons do
not establish saved-note admission, general PDF behavior, other scales or device parity.
A separate four-point control matches native geometry at both sizes. At size 8,
the profile SVG overlaps 214/214 native pixels versus 87/214 for fallback;
at size 12 it overlaps 295/297 versus 119/295, with two additional fringe
pixels. MuPDF rasterization of the size-12 PDF overlaps 275/297 versus 119/295
for fallback. These remain synthetic host controls.
For the three-point [size-4 control](../conformance/simplebrush-paint-v25-size4.json), geometry matches the native vector. Profile SVG overlaps 80/80 native pixels versus 31/80 for fallback, with alpha error 761 versus 13,579. A 1× MuPDF PDF raster overlaps 68/83 versus 31/80, with alpha error 3,154 versus 13,210. These remain synthetic host controls.
Two four-point size-4 host controls, a [bent stroke](../conformance/simplebrush-paint-v25-four-size4.json) and a [zigzag](../conformance/simplebrush-paint-v25-four-size4-alt.json), match native geometry. Profile SVG support overlaps 128/128 and 151/151 versus 55/128 and 72/151 for fallback. Their MuPDF PDF overlaps are 115/134 and 128/157 versus 61/128 and 74/151 for fallback; alpha error also decreases in both. These synthetic captures do not establish device parity.
The three-point [size-16 control](../conformance/simplebrush-paint-v25-size16.json) also matches native geometry. Its profile SVG overlaps 249/249 native pixels versus 70/249 for fallback, with alpha error 847 versus 43,721. A 1× MuPDF PDF raster overlaps 223/249 versus 80/249, with alpha error 4,146 versus 43,735. Two four-point size-16 controls, a [bent stroke](../conformance/simplebrush-paint-v25-four-size16.json) and a [zigzag](../conformance/simplebrush-paint-v25-four-size16-alt.json), also match native geometry. Their profile SVG overlaps 384/384 and 406/406 native pixels versus 149/384 and 181/406 for fallback; alpha error is 1,418 and 1,223 versus 57,960 and 58,724. MuPDF PDF overlaps are 353/384 and 383/407 versus 165/384 and 181/406 for fallback, with alpha errors of 5,856 and 4,889 versus 58,310 and 59,055. These are synthetic host controls, not device parity.
Four captured geometry streams also pass through a test-only public `Stroke`
adapter with an explicit identity coordinate context. Its synthetic saved curve
flag uses property bit 0, which native controls identify as `IsCurveEnabled`;
the remaining synthetic settings are not native saved-note evidence.
Parsing normally populates public RGB from saved ARGB. For eligible null-setting
SimpleBrush strokes whose public color was subsequently cleared, the generic
route recovers explicit saved RGB. The single native color control supports this
mapping; forwarding other colors is inferred from the shader and remains
unverified end to end.

The SDK chooser also lists TriangleMosaicPen, which is absent from the pinned
native registry; that entry does not establish a loadable or rendered plugin.

Saved-page rendering rejects sources with fewer than two points before pen
selection, matching native Drawing. The standalone `prepare_stroke` API retains
its compatibility tap geometry.
A [native stroke-frame capture](../conformance/object-stroke-channels-native.json)
checks a real 204-point frame, three float-channel and six XY delta controls.
Compressed coordinates and float channels round each addition to float32;
outer SDK loading is not exercised.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Fountain V14, saved `14;` | Reconstructed stamps/directions with [saved defaults](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-defaults-native.json); vector directional shading, colorizer and maximum blending; tools 1–3 and fixed width | Native geometry, real-document comparisons and [host shader captures](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-field-native.json) a [full saved-stroke replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-real-native.json) and an [original-frame Drawing check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-persisted-drawing-native.json). V14 colorizer witnesses render in Chromium and Firefox with vector-only PDF export. Native [expansion parameters](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-addpoint-native.json) and [synthetic thin-stamp shader checks](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-fringe-native.json) cover fringe behavior. An explicit `FountainV14OutputScaleProfile` paints scale-aware radial fringes and minimum-width alpha before maximum blending; 180 compact shader tiles and a saved handwriting-cache comparison show lower alpha error. Default output is unchanged. The profile lowers SVG alpha error but worsens 1× MuPDF alpha error on the saved handwriting reference; small-mark mask sampling remains under investigation. Thresholded ink support and native PDF parity remain unresolved. |
| Fountain V16, saved `18;0;100;` | Reconstructed stamps with pressure/tilt/time/movement and fixed width; saved size/tolerance defaults and relative endpoint time; tools 1–3; ordered source-over radial stamps with ARGB alpha per instance at canonical unit inverse scale | Executed native geometry, [saved Drawing defaults](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-defaults-native.json), [endpoint timing](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-event-time-native.json), [radial field records](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-addpoint-native.json) scalar coverage checks and a [persisted native queue replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-saved-render-native.json). Native queue controls with zero, partial and opaque ARGB establish per-instance alpha and source-over accumulation. Pixel-center source-over matches the partial-alpha native field far more closely than MAX; SVG/PDF edge sampling still differs. Full-coverage circular stamps use vector circles in PDF with per-stamp saved ARGB alpha at unit scale; opaque stamps also retain the existing scale support. A 76-stamp opaque persisted AS18 host-Mesa comparison reduces union alpha MAE from 27.44 to 3.76 bytes in MuPDF and 27.52 to 3.14 in PDFium at fixed 96 DPI. At unit scale, ordinary V16 uses native quad bounds in SVG, including partial saved alpha and width coverage, reducing union alpha MAE from 4.07 to 0.14 bytes in the opaque AS18 resvg control with native-quad boundaries. A controlled native size-1 fork of the same 204-point source reduces thin-stamp alpha MAE from 3.57 to 0.43 bytes at ARGB alpha 128 and 7.05 to 0.42 at alpha 255; its pixel support matches, while thin PDF retains radial shadings with larger errors. An explicit `FountainV16OutputScaleProfile` resamples ordinary and rainbow-flag-ignored V16 for uniform output scale; [controlled scale captures](reverse-engineering/fountain-parity.md#explicit-v16-output-scale) match native uploads and improve SVG alpha error. A six-case rainbow-on native control matches the rainbow-off uploads and full RGBA frames. Missing, invalid and over-budget profiles preserve the existing canonical path; device zoom and native PDF parity remain unverified. This setting selects drawing V16, not V18. |
| DefaultPen, ordinary saved tools 0–4 | Native positions/radii rendered as ordered source-over circles; omitted size uses the native 0.4 clamp | Native geometry, replay-boundary and [admission controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-admission-native.json); paired vector exports improve on the generic fallback. [Joined host-Mesa captures](reverse-engineering/stroke-rendering-findings.md#defaultpen-curve-enabled-branch) check RGB-only shader alpha behavior and 56 size/phase/rotation controls; vector consumers retain edge-coverage differences. Device pixel parity remains unverified. |
| ChineseBrush GLV1–V3, saved `1;`–`3;`, plus captured omitted, `0;`, empty and literal `4;` aliases; explicit source-device dimensions | Reconstructed ordered dabs, native 144×144 atlas, saved RGB and selector opacity; hard-circle coverage or optional output-scale fringe masks | Paired SDK SVG/PDF exports improve host-capture error over generic paint. Explicit uniform-scale fringe masks reduce alpha/color error further but can lose faint edge pixels. A separate synthetic Drawing capture shows stored setting 4 selects the GLV3 drawable and produces the same 80-byte AddPoint stream and full host-Mesa frame as setting 3; it does not establish general V4 behavior. Device appearance and dark/invert parity remain unverified; unsupported sources and exceeded budgets retain generic rendering. |
| Marker2 | Sampled circular stamps, native integer size and saved alpha | [Native caller controls](../conformance/marker2-saved-caller-native.json) check full XY streams and size defaults; [host paint controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker2-zero-paint-native.json) confirm zero-width strokes produce no ink. A [12-case V2 host-Mesa matrix](reverse-engineering/marker2-rendering-findings.md#v2-changes-edge-smoothing-for-thin-strokes) verifies finite 2×2 coverage and post-MAX alpha at widths 1–3; paired SVG/PDF readers retain thin-edge coverage differences. A sampled, disjoint pixel-run experiment matched all 12 controls in resvg at the captured grid, but stays pixelated when enlarged and is not a scalable production route. V2 thin-edge shading, partial-alpha output and complete device appearance remain incomplete. |
| Marker4 / StraightHighlighter V7, saved `7;` | Stylus midpoint sampling, saved style size and fractional rounded rectangular tip | Native geometry and [Drawing size/alias controls](../conformance/marker4-saved-caller-native.json); public support remains approximate because filtering/zoom coverage differ. |
| Marker4 / StraightHighlighter V8, saved `8;` | Sampled rounded rectangular tip centered on saved AddPoint positions, native integer size, tool-specific rotation and saved alpha | Executed Drawing size/alias controls and controlled native RTV4-to-Mesa frames verify saved XY through the instance VBO and rotation about the centered quad; filtering/zoom coverage remain approximate. A +0.5 height candidate improved isolated host-frame coverage but worsened a paired 600-pixel PDF comparison while improving the 1200-pixel comparison, so it remains experimental. |
| Other pens, unknown names or rejected settings | Generic straight segments with pressure width, polyline or tap circle | No pen-specific native texture, smoothing or width-law guarantee. |
| Rainbow, eraser and straighten effects | Metadata retained; InkPen2 V9 and saved Fountain V17 rainbow have explicit profiles | Bounded AS18 variable-width Fountain inputs retain plain GLV16 geometry by default even with the rainbow flag, matching native factory selection and controlled callback arguments; real archived rainbow and GPU appearance remain unverified. An experimental host profile instead applies GLV17 rainbow paint using an explicit palette, period and offset. An optional declared uniform output scale adjusts SVG fringe coverage; paired SDK SVG exports match native support at 1×, 2× and 4×. Native CPU colors and bounded RTV6/Mesa phase/alpha controls are checked against public SVG/PDF exports. Full-coverage profile stamps use ordered vector circles in PDF, improving color and support agreement over radial shadings in five canonical controls; partial-coverage or over-budget strokes retain gradients. Native initialization, device appearance and arbitrary-scale parity remain open. Other native effect semantics remain unsupported. V8 accepts straighten/fixed-width bits without implementing their separate effect semantics. |
| Live prediction and temporary tips | Saved samples and saved-vector replay only | Native live-state research does not establish a production live renderer. |

Marker2 also admits a non-null empty advanced setting into the shared saved geometry route. A [paired synthetic native control](../conformance/marker2-empty-setting-paint-native.json) matches empty and `2;` geometry and host-Mesa frames; public SVG and direct PDF exports are identical between those settings. Against that native frame, SVG RGB absolute error falls from 9,637 for the generic fallback to 2,269, and direct PDF error falls from 9,576 to 2,711 at 96 dpi. Empty can retain a previous native renderer version, and V1/V2 thin-edge coverage differs; the vector paint remains approximate. Unsupported settings and metadata retain generic fallback. This control does not establish serialized-note or device parity.

A [native RTV4 paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-rtv4-paint-native.json)
replays three V14 tuples, a synthetic control and a complete 24-stamp source
tuple stream through native constructor, upload and Draw code on host Mesa.
The full-stroke 1× crop matches all 64 shader-reference support pixels, with
maximum channel error 6 and active-region mean error 1.4844. Color and matrix
controls remain bounded; natural Drawing admission and device pixels are unverified.

Native [matrix-scale captures](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-inverse-scale-native.json)
cover changed and cached V14/V16 getters under five controlled canvas matrices;
the full zoom-dependent Drawing/RT pipeline remains unverified.

Admitted Fountain V14/V16 strokes with neither public RGB nor saved ARGB use
native default black through the existing page theme. The [color oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-saved-color-native.json)
replays two embedded corpus frames and eight separate synthetic Drawing controls;
it does not establish outer SDK loading or final native pixels. Paired 1×
Chromium, Firefox, MuPDF and PDFium renders lower mean error against the host
shader references for one real V14 stroke and one synthetic V16 control;
PDF sampling differences remain.

A separate [native Fountain PDF writer](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-pdf-writer-native.json)
emits circle paths into a mocked sink. Its connection to public Notes export and
final PDFium output remain unverified; it is not the GL paint reference.

Profile admission and validation are narrower than the names above. See
[stroke rendering](reverse-engineering/stroke-rendering-findings.md),
[fountain parity](reverse-engineering/fountain-parity.md),
[Marker2](reverse-engineering/marker2-rendering-findings.md),
[Marker4 V7](reverse-engineering/marker4-v7.md), and
[Marker4 V8](reverse-engineering/marker4-rendering-findings.md).

A [Crayon2 saved-playback paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/crayon2-saved-playback-paint-native.json)
joins native source getters, 285 packed records, queued upload and host Mesa Draw,
with clipped/interior and transparent/opaque controls. That V1 route supplies
renderer size through a host write and uses modeled RNG. A separate
[V2 Drawing oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/crayon2-drawing-admission-native.json)
checks native settings dispatch and Model size/color forwarding through the same
RT, shader uniform and six opaque/partial/zero-alpha readbacks. Persisted-note
admission, production vector paint and device parity remain open.

The [Pencil3 V1 source trace](reverse-engineering/pencil3-source-findings.md)
separates saved samples from generated particles and configured paper coverage.
Pencil profiles default to the generic pressure approximation. An explicit
`Pencil3RenderProfile` admits bounded V3 intrinsic-texture paint for ordinary
tool-1, tool-2 and tool-3 strokes with either curve flag, empty/`4;` settings, supplied screen/canvas dimensions and
a chosen paper sampler; canvas dimensions must match the page. Paper sampling
uses a bounded intrinsic texel plane to preserve nearest repeat/clamp coverage
across fractional tile seams; stroke geometry remains vector. A test-only
Pencil3 V1 sampler matches ten saved native cases and eight boundary witnesses.
Remaining pencil work: reconstruct and validate production texture and paint
for Pencil, Pencil2 and ColoredPencil; establish Pencil3's saved-note runtime
context and compare its SVG/PDF output against native device and real-note
exports. Then evaluate a better default where the required context is known.
For ordinary V3 strokes, the generic fallback retains saved RGB and remains
invisible for fully transparent saved color; partial-opacity fallback is still approximate. A
half-opacity whole-stroke candidate reduced color error in one host control but
did not improve support or reproduce native per-particle compositing.
The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-saved-caller-native.json) captures
null/V1 and empty/V3; V1 matches all 48 particles with a modeled bionic RNG.
The [V3 particle kernel](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-v3-native.json)
matches all 48 particles in two redraw phases, including private RNG state and
reset behavior. The [V3 saved sampler](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-sampler-inputs-native.json)
reconstructs complete native particle streams from source channels across five
profiles, including varied pressure, tilt, tools and context dimensions. The
tool-1 and tool-3 adapters match their captured particle streams; saved settings
and native appearance for those routes remain unverified.
A [V3 host paint join](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil-v3-paint-join.json) replays those
captured streams through the extracted shader and masks at three scales, with
synthetic projection, color and paper phase; native device/FBO paint is unverified.
A [native V3 draw handoff](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-draw-handoff-native.json)
captures color, projection, paper scale and texture bindings across five explicit
profiles using mocked renderer resources and context. A [scale-producer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-scale-native.json)
checks native `min(screen width,height) / 1440` through task dispatch to the RT
field in four contexts; application paper inputs, paint and device pixels remain
unverified. A [bitmap-consumer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-consumer-native.json)
joins the Drawing producer to the queued RT depth-map setter and cached dimensions.
That capture stops at a missing host framebuffer target. A separate
[native paper paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-paint-native.json)
completes same-RT Mesa Draw with the native paper resource and white/zero controls,
retaining full RGBA readbacks. Bitmap/texture ownership and lifecycle remain host
adapters; natural context and device parity remain open.
A [one-particle vector sampling witness](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil-vector-sampling.json)
matches the host shader at three scales with group-scoped SVG `crispEdges`,
including embedded paper. That vectorized-texel hint is lost in PDF; the
intrinsic-image route below remains subject to consumer sampling.
Fresh selectors distinguish empty/`4;` (V3) from `3;` (V1). V3 preparation has
source/output budgets and native size clamps. Public SVG/PDF reuse the original
brush and paper textures with vector stamp geometry; these are intrinsic image
assets, not rendered-stroke bitmaps. Matching sampler controls improve on the
generic fallback across five consumers; PDF residuals remain. A fresh 1×
[host-control comparison](../conformance/pencil3-finite-plane-host-comparison.json)
of the current finite-plane route against the first native Drawing invocation measured RGB mean
absolute error of 1.53 in resvg 0.45.1 and 20.38 in MuPDF 1.28.2 for clamp, versus 65.64 and
56.95 for generic fallback on each renderer's shared support region. The two
regions differ; these are not device or cross-renderer parity claims. Supplied-paper
sampling is producer-owned, while built-in paper repeats. Context defaults,
real-document/device validation remain integration gaps. Profile-aware replay
uses source-event particle cutoffs; viewport-aware metadata checks the same
canvas, particle and paper-plane eligibility as paint. A later image allocation
or encoding failure can still make rendering fall back after metadata preparation.
Slice rendering culls off-slice particles while retaining their original replay
indices; a same-viewport resvg control matches the unculled scene. In a
separate full-page versus one-unit-slice control, resvg produced channel
differences up to 186. Rendering the identical slice SVG into a full-height
target restores the full-page row in that control; cross-viewport texture
sampling remains unresolved.
An isolated single-dab SVG/PDF control points to texture sampling inside the
dab: at 1× and integer alignment the solid control matched, while brush-only
paint differed between resvg 0.45.1 and MuPDF 1.28.2. A per-dab mask
rearrangement did not improve the tested slice residual. These controls compare
consumers, not native pixels. Setting PDF image interpolation true for both
images and their alpha masks produced identical MuPDF output at 1× and 2× in
the clamp and repeat controls. A vectorized brush-row candidate increased PDF
error against the existing source SVG in a one-dab control and required 6,043
rectangles; it is not selected as a rendering fix.
An intrinsic cropped-row alternative reduced average error in the host control
but added PDF support pixels in MuPDF, so the atlas transport remains unchanged.
Marker3 and StraightMarker V5 page exports use native-sampled compound disks for the captured
tool-2 and [tool-3](../conformance/marker3-tool3-native.json) base-pen profiles with explicit saved size/color and ordinary
style. Marker3 tool 1 with explicit `5;`, size 4–8 and explicit ARGB also uses this route; intermediate sizes are interpolated from host controls. Other profiles retain the approximation. The sampler checks stamp
positions, radii and callback boundaries in ten
[synthetic native cases](../conformance/marker3-saved-native.json).
The [Drawing caller capture](../conformance/marker3-saved-caller-native.json)
checks null selecting V1, empty/explicit settings selecting V5, source ARGB
forwarding and linked XY streams; the V5 sampler excludes null settings. Six additional native literal-name controls select the same factory/drawable and identical complete streams for Marker3 and StraightMarker under null, empty and `5;` settings. This establishes bounded alias routing, not independent device or pixel parity.
Its [positive paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-paint-positive-native.json)
executes native mask/composite draws on separate complete framebuffer targets
in host Mesa, retaining the full RGBA hash and GL error checks. The
[scale capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-paint-scale-native.json) retains full masks
and RGBA output for 17 linked-path and isolated-stamp cases across sizes 4/8/12
and scales 1/2/4, checking uploaded XY against the saved caller buffers. Points
enter directly at RTV1. A [joined Drawing paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-joined-paint-native.json)
retains the Drawing-selected RT and its original 176-byte vector through native
Init/Update/Draw, with a full host-Mesa RGBA readback and 239 zero GL error checks.
The [queue control](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-queue-handoff-native.json) executes
native `SendDataToGPU` with correct host queue wiring and detects missing
dispatch with the wrong queue; no host vector-slot assignment is needed.
In the joined paint capture, renderer-message and vector-delete
tasks remain deferred; complete queue/lifetime integration, vector edge coverage
and device pixel parity remain outstanding.
Further host controls isolate the edge law to finite 2×2 fragment-quad
derivatives; a continuous-gradient approximation is insufficient. See
[Marker3 coverage](reverse-engineering/stroke-rendering-findings.md#marker3-fragment-coverage).
DefaultPen's sampler checks
position/radius streams in sixteen synthetic
[cases including degenerate paths](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-saved-native.json).
Its [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-saved-caller-native.json) verifies
native Model getters, factory selection and identical 132-byte Drawing/saved
streams on the same RT under an explicit 1920×1080 display profile.
A [redraw-boundary capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-redraw-boundaries-native.json)
retains all 22 stamps and native history/end-call boundaries for a six-point
stroke; saved replay constructs a native event with source 0.
DefaultPen's [Draw capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-draw-native.json) retains
native blend setup and shader text that replaces input alpha with screen-space
coverage. The [joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-joined-paint-native.json)
passes that Drawing-selected RT’s 132-byte vector through native upload and
draw to a host RGBA8 target, with an explicit same-vector-object handoff.
Admitted saved profiles render the native disks in order; other states retain
the generic approximation. Natural queue/lifecycle setup and native shader edge
coverage remain outstanding.
The [InkPen V4 source trace](reverse-engineering/inkpen-v4-findings.md) establishes
selected saved curve width/sampling and cache boundaries. Canonical-page SVG/PDF
now reconstruct ordinary V1 null-setting geometry for tools 0–4 in both curve
modes, using saved ARGB or the native opaque-black default when it is absent;
a bounded compound contour approximates native coverage at one SVG pixel per
source unit. Replay preserves source-event cutoffs and folds the terminal suffix
into the final event. Unsupported inputs and exhausted budgets retain fallback;
the context-free preparer remains unchanged. Ordinary V1 and observed V4
(empty, `4;` or `18;0;100;` settings) use saved RGB when the public stroke color is absent,
and apply saved ARGB alpha once per stroke in SVG/PDF. An explicit
`InkPenRenderProfile` now selects bounded V4 geometry for ordinary strokes with
saved ARGB and a caller-supplied display size; the default remains generic. The
exact `18;0;100;` setting also admits the native opaque-black default when ARGB
is absent, as observed in a synthetic serialized-frame derivative; this color
is retained when geometry falls back.
The optional `inkpen_v4_output_scale` adjusts the coverage contour for a known
uniform output scale of 1, 2, or 4; absent and other values preserve the prior
contour. The display profile is still required. Synthetic native host-Mesa
controls cover two trajectories and partial-alpha color; SVG comparisons and
direct document PDF comparisons in MuPDF and PDFium improve at 2× and 4× on
the straight control. The PDF comparisons do not cover the bent control, and
device, arbitrary transforms, and other scales remain unverified.
The separate `inkpen_v1_output_scale` opt-in applies the same contour adjustment to bounded InkPen V1 strokes at 1×, 2×, 3×, or 4×; absent and other values retain the prior contour. The [host-Mesa capture](../conformance/inkpen-v1-output-scale-native.json) covers two synthetic trajectories at 1×, 2× and 4×, plus three 3× controls including a narrow stroke at a fractional phase. Against native frames, public SVG and direct PDF output rendered by MuPDF and PDFium reduce error at the tested higher scales. Device pixels, arbitrary transforms, and saved-note parity remain unverified.
For fixed-width V4 strokes, the opt-in additionally admits fixed widths 0 through 200 for tools 0–4 in either curve mode. Synthetic Drawing and Mesa controls cover widths 0, 0.25, 0.5, 1, 3, 6, 12, 20, 30, 50, 100, and 200 and two geometries; all tools and curve modes are covered at width 6. Their complete geometry streams match, and public SVG and direct PDF comparisons reduce pixel error against these host captures. Widths outside that interval remain on fallback; intermediate widths are admitted but have not been individually captured. Its contour approximates native coverage, and V4 replay event boundaries and
other output scales remain unverified.
The numeric core matches twenty-one
configured-context native redraw cases and four setter cases, including both
curve modes, fixed width and a degenerate curve. The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-saved-caller-native.json)
captures null settings selecting V1 and empty settings selecting V4 under an
explicit 1920×1080 display profile. The V4 numeric model reproduces the complete
312-byte Drawing stream bit for bit, plus all streams in a
[seven-case display matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-display-profiles-native.json);
null settings remain outside that V4 model. The separate V1 core matches
all fourteen emitted streams in the [V1 matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-saved-native.json),
including no-curve, duplicate-point, timing and display controls; Drawing rejects
the separate one-point case before pen creation. A [nine-case size/tool capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-admission-native.json)
checks omitted, zero, subminimum and oversized source sizes plus tool codes 0–4.
The test-only model matches every captured callback word, using the saved style
size (missing means zero) and native 0.4–800 clamp rather than the public width.
A [reuse capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-reuse-native.json) checks short/long first
strokes and a distinct-object control on the same pen, drawable and RT; every
target stream matches the fresh 312 bytes. Direct redraw of the distinct object
returns without callbacks after Drawing clears its cache-dirty flag. Broader
reuse states and vector edge coverage remain open under the opt-in V4 profile.
The [property helper oracle](../conformance/object-stroke-curve-native.json)
executes native property read/write and getter/setter controls, confirming
serialized bit 0 selects curve state; full-record deserialization is not exercised.
The [flag audit](../conformance/object-stroke-properties-native.json) checks
137 zero/one-hot/pair masks against named native getters and the Rust parser;
bit 2 is separately exercised through tilt/orientation channel storage.
A [joined paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-joined-paint-native.json) executes
native Renderer factories and RTV1 Init/Update/Draw, checking the 312-byte upload
and full RGBA readbacks. With correct host queue wiring, native `SendDataToGPU`
installs the saved vector and `Update(false)` uploads it without a host vector-slot
write. A [three-color capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-color-native.json) checks an
unchanged MAX mask and one source-over composite for zero, partial and opaque
alpha under identity host color conversion. A [V1 joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-joined-paint-native.json)
checks null settings, the unchanged 300-byte upload and partial-alpha paint on
the same Drawing-selected RT. The
[scalar derivative check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen_coverage_scalar.py) reproduces
all 32,768 bytes of the captured 128×64 MAX mask using fine 2×2 differences.
This is one host Mesa witness, not a universal device derivative contract.
A [V1 scale replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-scale-native.json) checks fresh
1×/2×/4× native Draw targets and separate identity-transform controls. Its
pixel-dependent edge band prevents treating a fixed 1× contour as a general paint law.
A [V4 contour comparison](../conformance/inkpen-v4-vector-host.json)
improves alpha and color error over the generic route in resvg, MuPDF and
PDFium for one synthetic stroke at opaque and partial alpha; zero alpha stays
empty. These controls do not establish device or general saved-note parity.
A [V1 reuse oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-reuse-native.json) checks source-event
and terminal vector prefixes, short/long preceding strokes and distinct source
objects. Rust matches the complete first/target streams and their native boundaries.
A [17-case V1 boundary matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-boundaries-native.json)
checks complete native Drawing/redraw vectors and source-event/terminal prefixes,
including curved tools 0–4 and one no-curve tool1 control; Rust matches every word
and cutoff. These synthetic setter controls do not establish serialized-note admission.
A [serialized-frame replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-serialized-drawing-native.json)
loads embedded string-table and stroke bytes through native Model attachment
and Drawing. Controlled Fountain-to-InkPen derivatives select V1/null and V4;
V1 defaults absent ARGB to opaque black and emits 68 records with exact event
prefixes. These are synthetic derivatives, not app-captured InkPen notes.
A [ten-case no-curve tool-3/4 capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-nocurve-tool34-native.json)
checks pressure and size boundaries; Rust matches complete vectors and native
event cutoffs. Paired SVG/PDF exports improve native-referenced error across
1×/2×/4× host comparisons; tool5 and advanced V4 retain generic fallback.
General V4 admission, app display setup, device parity and exact scale-dependent
coverage remain outstanding.
The [InkPen2 V9 saved trace](reverse-engineering/inkpen2-saved-findings.md)
establishes selected time/history radius and derived cache geometry. An explicit
`RenderOptions::inkpen2_profile` enables bounded V9 vector stamps for ordinary
solid-color tool-2 strokes with `9;`, zero tolerance, and complete saved channels. It selects
fresh per-stroke state and identity inverse scale. `enhanced_antialias` selects
MAX coverage with one saved-alpha composite; false retains per-stamp source-over.
It does not infer the app's display or antialias policy.
Without that option, InkPen2 and StraightInkPen2 retain the generic approximation.
For normal-color StraightInkPen2 `9;` inputs and non-rainbow, normal-color inputs with a non-null empty setting for either name, the same opt-in profile is admitted. A [four-way Drawing capture](../conformance/inkpen2-empty-setting-native.json) matches all callback geometry for the empty and explicit settings on both names. A separate [four-way host-Mesa paint capture](../conformance/inkpen2-empty-setting-paint-native.json) independently matches the full uploaded records and RGBA readback for these cases; it does not establish Android device or full application canvas parity. Null settings remain outside the V9 route. A [separate null-setting control](../conformance/straightinkpen2-null-alias-native.json) selects V1 and matches the callback sequence for both names. Its [captured geometry](../conformance/inkpen2-null-v1-geometry-native.json) differs from the existing InkPen V1 model on the same synthetic input, so that model cannot be reused for these names. General V1 paint and saved-stroke behavior still require validation. A [paired V1 host-Mesa capture](../conformance/straightinkpen2-null-v1-paired-paint-native.json) compares both names for four synthetic shape, color, and scale controls: full geometry and RGBA frames, draw calls, and shader programs match. This does not establish device output or serialized-note behavior.
The separate `RenderOptions::inkpen2_v1_output_scale` opt-in selects `InkPen2V1OutputScale::Half`, `One`, or `Double` for a known uniform output scale. It also requires an explicit `1920 × 1080` or `1080 × 1920` InkPen2 profile, or `1440 × 2560` at 1× or 2× output scale, with enhanced antialiasing off, a normal-color InkPen2 or StraightInkPen2 stroke with a null setting or literal `1;` for either name, tool type 2, a pen size between 0.2 and 12, and supported ordinary stroke metadata. Conflicting color overrides and unsupported inputs fall back to generic rendering; replay and descendant objects remain generic. The V1 geometry model matches [14 synthetic native streams](../conformance/inkpen2-null-v1-controls-native.json). Its vector paint approximates a screen-space shader using scale-adjusted radial gradients, so output at other scales and transforms is not covered by the selected profile. For one translated synthetic stroke, [host-Mesa controls and production exports](../conformance/inkpen2-null-v1-paint-scales-native.json) show alpha-byte absolute-error sums for SVG/PDF of 453/2,575 at 0.5×, 886/5,736 at 1×, and 1,790/12,397 at 2×; generic output gives 3,802/4,011, 21,194/20,972, and 95,087/96,270 respectively. These are measured against transparent native host frames with resvg and PyMuPDF, not Android device pixels or final app composition. Six additional synthetic shapes also improve against the generic fallback in SVG and PDF; their [portrait-profile host captures](../conformance/inkpen2-v1-screen-paint-native.json) match the 1920 × 1080 geometry and RGBA frames, and public SVG and PDF outputs match across the two profiles. At 1440 × 2560 the reverse stroke differs in geometry and paint, but the resolution-dependent model matches all six captured vertex streams. The 1× public SVG and [PDF](../conformance/inkpen2-v1-high-pdf-comparison.json) exports improve alpha error against generic output for each shape. At 2×, the [six host controls](../conformance/inkpen2-v1-high-scales-native.json) likewise improve SVG and [PDF](../conformance/inkpen2-v1-high-scales-pdf-comparison.json) alpha error against generic output. At 0.5×, a temporarily enabled candidate has worse SVG and PDF error for the short stroke, so that scale remains generic for this profile. A [13-length size-4 host sweep](../conformance/inkpen2-v1-half-length-native.json) crosses from worse to better between 0.3 and 0.5 in SVG and between 2 and 3 in PDF for one trajectory, which does not establish a general length threshold. A [paired native capture](../conformance/inkpen2-literal-v1-paired-paint-native.json) matches null and literal `1;` Drawing geometry and host paint across four synthetic shape, color, and scale controls; public SVG and PDF outputs also match for each pair. [Four-way host controls](../conformance/straightinkpen2-literal-v1-paired-paint-native.json) also match StraightInkPen2 with literal `1;`. Broader inputs, full serialized-note loading, and output environments remain unverified. A [host color control](../conformance/inkpen2-null-v1-color-alpha-native.json) found that the source alpha byte did not change these V1 frames for blue or red on transparent or opaque-white targets; the opt-in route consequently ignores that byte. Public SVG and PDF outputs for zero, half, and opaque source alpha are identical for each tested RGB.
A [paired literal-name Drawing capture](../conformance/straightinkpen2-v9-alias-native.json)
selects the same V9 drawable and matches all callback geometry for one synthetic
control. A [paired host-Mesa paint capture](../conformance/straightinkpen2-v9-paired-paint-native.json)
also matches the full RGBA frame and uploaded records after native Drawing and
saved redraw for one synthetic color control. The target and replay are host-managed;
application canvas composition, device output and other settings remain unverified. The V9 core
matches every RTV3 float in three fresh, type-0 saved redraw cases. The [Drawing caller capture](../conformance/inkpen2-saved-caller-native.json)
checks null settings selecting V1, empty/explicit settings selecting V9, and a
25-record V9 buffer match. The Rust core checks both Drawing and saved replay
using native body-normalized sampling state. The [reuse oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-reuse-native.json)
and Rust sequence test retain the native alternation byte between strokes:
a shared Drawing retains it across distinct source objects too. A short
preceding stroke changes tool-1 output, while the captured long
history and tool-2 controls match fresh receivers. General caller/runtime
state, shape modes and paint integration remain open. A
[source controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-source-caller-native.json) check six
V9 streams and one-point Drawing rejection under an explicit host profile.
The model consumes native getter arrays and selected drawable size, including
the 0.4–800 clamp; these remain synthetic setter controls. The
[RTV3 paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-paint-native.json) checks native
MAX coverage, source-over color and final composition in six synthetic host
Mesa cases, plus four direct-branch source-over cases. Both include source ARGB
opacity controls. A test-only paint model matches every full-target readback;
direct same-color overlap accumulates while the alternate branch preserves
its MAX result. The [joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-joined-paint-native.json)
passes one Drawing-selected V9 receiver's unchanged 900-byte stream through
native upload and RGBA paint, with an explicit host canvas/context bridge.
The [R8 mask oracle](../conformance/inkpen2-max-mask-native.json) checks every
mask byte at 1×/2×/4× in six bounded host captures; SVG rasterization and PDF
appearance remain separate validation gaps.
Native global-AA configuration reaches the per-pen setting and queued RT flag.
The solid-color enhanced route uses opaque grayscale fields before MAX blending;
Rust regressions check exact duplicate invariance and pointwise MAX at subpixel
edges, with vector-only PDF export. Intermediate RGBA8 color normalization can
differ from the ideal vector field. Natural app canvas setup
and device defaults remain unverified. For one 25-record native stream at 1×/2×/4×, the explicit profile
improves support and color error over the generic fallback in Chromium, Firefox,
MuPDF and PDFium; a half-alpha 1× control also improves. SVG support IoU is
about 0.992–0.996, while PDF remains about 0.812–0.955. Independent single-stamp
PDF controls retain the radial-shading sampling error even without the
converter’s shape bounds or RGB gradient; gradient transport is not pixel parity.
Nine enhanced-AA controls (1×/2×/4×, alpha 0/128/255) retain the same native VBO.
The enhanced projection improves SVG and partial-alpha PDF error against those
native frames compared with selecting the direct route. Opaque PDF error is
worse at 1×/2× and better at 4×; zero-alpha output is blank and PDFs remain vector-only.
The explicit profile also admits active type-1 rainbow with AA disabled, a
1080-pixel minimum screen dimension, positive distance, nonnegative offset and
an explicit 2–256-color palette. Admission requires unit coverage throughout
and at most 512 records; palette RGB remains separate from theme conversion.
Enhanced-AA gradients and thin coverage retain fallback. SVG uses direct radial
gradients and PDF retains vector shading; native-referenced PDF appearance and
real-document evidence remain open. See the V9 saved trace above.
The [Calligraphy V1 source trace](reverse-engineering/calligraphy-source-findings.md)
establishes orientation-derived nib geometry and retained drawable state.
Production SVG/PDF rendering now uses its three native mesh lanes for admitted
ordinary saved strokes, preserving source RGB, core alpha gradients and native
draw order without image objects. State follows Standard export bitmap batches;
containers and Formula fallback lists retain their enclosing Drawing state.
The [Drawing caller capture](../conformance/calligraphy-saved-caller-native.json)
checks fresh/reused source conversion and two-stroke native list traversal;
Rust matches complete buffers and retained state. The
[saved-wrapper capture](../conformance/calligraphy-saved-wrapper-native.json)
covers fixed width, raw tool normalization and paired curve flags. The
[native defaults capture](../conformance/object-stroke-defaults-native.json)
checks omitted size and ARGB; the adapter uses effective size 0.4 and opaque
black. The [queued Draw capture](../conformance/calligraphy-draw-native.json) records uploads,
uniforms and blend calls. Unsupported inputs retain the generic approximation;
unknown state stays in fallback until a batch reset. Standalone `prepare_stroke`
also retains that approximation because it has no Drawing lifetime. Object/group
opacity, unsupported model routes, PDF raster edges and device pixel parity
remain open; host captures and vector-only PDFs do not establish full parity.
AirBrush has samplers matching all stamp bits and captured state in
40 synthetic native V2 saved-redraw controls, including source/tool gates,
short-path jitter, counter reset and terminal endpoints. Three legacy V1 streams
are also retained. Native V2 reproduces V1 records and CPU pattern bytes at a
synthetic SPTextureBitmap UpdateBitmap boundary. The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-saved-caller-native.json)
selects V1 for null settings and V2 for empty settings. The numeric model matches
both 416-byte streams bit for bit under an explicit 1920×1080 display profile.
A paired [literal-setting capture](../conformance/airbrush-v1-literal-one-native.json) selects V1 for null and `1;`, with identical native stamp geometry and host-Mesa frames. Public SVG and direct PDF outputs match for this synthetic two-point input; the SVG matches the native frame, while the scene PDF raster has RGBA error 169 across nine pixels in PyMuPDF 1.28.2 at 96 dpi. This does not establish device or general PDF parity.
For the short V1 host control, PyMuPDF 1.28.2 rasterization of the actual PDF at
96 dpi differs from native and the matching SVG at eight pixels; its transparent
RGBA absolute channel error totals 227, versus 489 for generic PDF fallback.
The [comparison receipt](../conformance/airbrush-v1-pdf-comparison.json) records
the pixels and the [script](../conformance/compare_airbrush_v1_pdf.py) reproduces
the comparison from PDFs captured by the focused test. This is one viewer and
resolution, not evidence of general PDF or device parity.
A first-stamp triangle-clip experiment with the second stamp unchanged still rendered center alpha 22 versus native 52 in PyMuPDF at 96 dpi.
In PyMuPDF alone, higher-resolution center sampling reduced 3×3 alpha error from 81 at 1× to 37 at 8×, while removing both stamps’ masks gave 80 at 1×; sampling and coverage remain possible causes rather than isolated findings.
A further [texture probe](../conformance/airbrush-v1-pdf-texture-probe.json) found identical MuPDF output with interpolation enabled or disabled and with nearest-neighbor texture upsampling; sampling, transformed coverage, and alpha compositing remain unresolved for this one synthetic control.
Admitted source-owned tool-2 profiles preserve saved RGB or native omitted-color
black in generic rendering. Native per-stamp alpha is not applied as whole-stroke opacity.
The [color oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-color-native.json) checks all 256 source
alpha bytes and ten full Drawing routes: native output alpha is
`clamp(trunc(f32(f32(alpha) * 50) / 255), 3, 50)`, preserving RGB.
This includes source alpha zero becoming output alpha three; GPU compositing
is not established by this getter contract. A native
[texture capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-texture-native.json) now checks Renderer
allocation, nearest/clamped sampling and exact GL upload bytes. An
[instanced Draw capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-draw-native.json) checks native
buffer uploads, attributes and blend calls with a separately captured stamp
stream injected by the host. A [joined paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-joined-paint-native.json)
checks the Drawing-produced vector through native task-body publication, upload
and full RGBA readback. Link-time shader snapshots and live interface/sampler
queries verify the native shader and nearest/clamped pattern; the target is zero
before Draw. Scheduling and Renderer capabilities are host-supplied; general
admission, vector paint and device parity remain outstanding.
The [BrushPen native oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-native.json) captures
default GLV10 selection and 11-float instance records. The separate
[Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-caller-native.json) selects GLV10
with an explicitly empty saved setting and captures 26 records under a
1920×1080 context. The numeric model reproduces all 1144 bytes using the native
`1440 / min(display limits)` speed factor; direct-wrapper cases retain the
constructor’s 1440×2560 limits. The [reuse capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-reuse-native.json)
checks short/long first strokes and a distinct-object control on the same pen,
drawable and RT: all second streams equal the fresh 1144-byte stream. Native
redraw clears retained speed, and the next accepted segment seeds radius.
The observed cache slot is not established as the drawable cache key. A
[texture capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-texture-native.json) reaches intercepted
native texture/instance uploads, attribute setup, blend mapping and Draw-binding
calls. A test-only source sampler matches
every instance bit in ten synthetic MotionEvent cases and nine
[saved-wrapper style profiles](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-wrapper-native.json),
including tools 1–3 and both curve modes.
A [positive paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-paint-positive-native.json)
executes native Draw and shaders on host Mesa with a fixture-linked instance,
a separate output framebuffer and an unused-atlas-cell sensitivity check.
The [same-object capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-same-run-native.json) checks
26 native-generated records through native upload and host paint, including
a full-stream unused-atlas-cell control. The [handoff capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-handoff-native.json)
also completes native endPen and callback destruction, then executes the queued
`SendDataToGPU` task. The [queued paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-queue-native.json)
completes native clip setup, upload, draw and deferred vector deletion with a
distinct cleared native canvas on host Mesa. Its style/color and scheduling are
explicit host inputs; general caller admission, vector paint, production
integration and Samsung device parity remain outstanding.
A [Drawing-to-paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-drawing-paint-native.json)
joins synthetic Model input, native queued color/data tasks, the unchanged
1,144-byte upload and same-RT uniform color to a complete host Mesa draw.
It also checks target preclear and deferred deletion; production vector paint remains open.
An additional [filter ablation](../conformance/brushpen-second-native/filter-ablation-receipt.json)
compares the retained source-cell prototype with native host frames and PDFium.
Removing its grain filter improves PDF agreement in both synthetic controls,
but changes the image; removing coverage while retaining the filter triggers a
resvg assertion. A separate fixed-pixel-grid vector candidate was close to one
host frame at 1×, but its alpha errors rose sharply under half-pixel translation
and 2× projection. The local control is retained under
`tmp/vector-parity-plan/brushpen-e62/`; it uses a standalone vector export, not
the public pipeline, and does not establish a production transport fix.
The [MagicPen Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-saved-caller-native.json)
checks null/`1;` selecting V1 and empty/`2;` selecting V2. It retains actual
RTV1 vectors separately from callback arguments for synthetic Model inputs;
the [Drawing-to-paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-drawing-paint-native.json)
joins native color, the unchanged upload and interior partial-alpha paint on
transparent/opaque host Mesa targets, retaining full RGBA buffers. Scratch texture
initialization is host-supplied. Canonical SVG/PDF now reconstruct ordinary
tool-2 curved and NoCurve null/`1;` sources with explicit size, ordinary style codes and
native replay cutoffs, using one compound vector contour and one source-alpha
composite. Omitted ARGB uses native opaque black, verified with
[Drawing defaults controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-admission-defaults-native.json);
empty advanced settings remain distinct. Omitted stroke/color type fields use the native zero defaults,
verified by a [saved-frame Model check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/object-stroke-style-defaults-native.json).
Paired public exports
improve native-referenced error in 12 host comparisons across four engines at
1×/2×/4×. The fixed contour now preserves the canonical radial coverage integral,
including positive area for minimum-size dots. Edge controls improve across
Chromium, Firefox, MuPDF and PDFium, but still underfill at 4×; resvg can drop
the smallest dot at 1×. A [16-phase host comparison](../conformance/magicpen-phase-export-host.json)
shows resvg dropping all 1× minimum-size dots, while direct PDF rendered by
MuPDF paints some phases absent in native Mesa; both underfill at 4×. These
are synthetic host-renderer results, and scale-dependent shader edges and
device parity remain open. Unknown effects and budgets retain fallback.
A [NoCurve paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-paint-native.json)
retains the same native source stream and full RGBA buffers at 1×/2×/4× on
transparent targets, with separate opaque-background blend controls.
The [edge replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-edge-paint-native.json) adds
18 captures covering duplicate points, rejected moves, minimum-size dots,
subpixel phase and zero-alpha ink, with exact native uploads and full RGBA buffers.
A [paint-control matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-paint-controls-native.json)
checks eight scale/phase views plus GLV1/GLV2 and opaque-background controls.
The source-derived shader model matches transparent support and stays within
three RGBA bytes on these host captures; it does not establish export parity.
A [GLV1 source matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-glv1-source-native.json)
checks native source getters and complete XYZ streams; the Rust source model
matches all 35 two- to eight-point cases, including duplicate/nearby-event
admission, native width transitions and the 0.4–800 size clamp. An omitted
saved size uses the native zero default and 0.4 floor. Three minimum-size
host controls improve over the generic omitted-size fallback in SVG and PDF.
The native default comes from synthetic Model/Drawing input, not a serialized
MagicPen note. A
[NoCurve event matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-source-events-native.json)
adds 18 controls for admission cutoffs, first-admitted width seeding, terminal
ranges, direct dots and thick-stroke sampling, with exact complete streams. A separate
[curved event capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-curved-source-events-native.json)
records line/terminal callback ranges for 33 bounded inputs, including rejected
final points, duplicate endpoints, direct dots, size limits and low-factor
terminal pressure controls. Rust matches every
captured XYZ word and callback range. Pinned native loops exclude line endpoints
and include terminal endpoints; terminal arguments retain the raw final input.
Curved SVG/PDF admission uses the same bounds and approximate contour. The
[four-profile curved paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-curved-paint-native.json)
supports paired comparisons at 1×/2×/4× in five engines: active-support error
improves over fallback in all 60 rows, while subpixel and zoom coverage still differ.
The [ChineseBrush caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-saved-caller-native.json)
checks seven setting states and captures five four-float records from synthetic
Model input. A [source matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-source-native.json)
retains complete four-lane streams for 52 two- to eight-event controls across
GLV1/2/3; a shared bounded source-event model matches callback Vector4/scalar
inputs and every packed stream word in these bounded captures.
An authored synthetic SDOCX regression checks note-string resolution and all five
parsed source channels before comparing 33 packed native records.
Parsed GLV1/2/3 fallback colors retain saved RGB or ObjectStroke's omitted-color black;
brush geometry and textured paint remain approximate.
An [adversarial extension](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-source-extension-native.json)
adds 24 cases for pressure, size and method-specific speed/width precision;
The established Rust matrix matches 1,762 native records across 143 cases, with generated
long/short controls extending coverage through 128 events, including
rejected terminal movements and zero-time speed seeding. Bounded sampling
checks source channels and output budgets; terminal geometry retains the raw
endpoint and pressure while V2/V3 widths retain accepted-move speed.
NoCurve controls retain rejected raw terminal segments, skip duplicate tails,
and place all-rejected dots at the last source point with pressure-scaled width.
Additional Rust controls pin 77 paint-input records at sizes 0.4, 1, 3.144 and
24; 18 versioned controls verify cumulative source-event record boundaries
for curved and NoCurve saved redraws. These cuts do not establish live replay policy.
A [reuse matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-reuse-native.json) checks fresh-equivalent
target streams after short, long and different preludes and version changes,
with explicit host lifecycle adapters. A bounded
[GLV3 paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-paint-native.json) joins the
synthetic source stream to native queued VBO/color uploads, texture bytes and a
full host Mesa readback. These host captures do not establish device pixel parity.
An additional [stored-setting 3/4 pair](../conformance/chinesebrush-glv3-setting4-native.json)
uses a separate synthetic Drawing input: stored getter 4 selects the GLV3 drawable,
and both cases have identical AddPoint bytes and full 1920×1080 RGBA8 readback.
Its compact 11-pixel fixture is checked by reconstructing and hashing the complete
frame. This proves only the captured pair, not general V4 behavior or SVG/PDF pixels.
An [empty-setting pair](../conformance/chinesebrush-empty-setting-host-native.json)
also selects GLV3 and matches the explicit `3;` control's producer stream and
complete host-Mesa readback for one synthetic stroke. The opt-in renderer
therefore treats an empty setting as GLV3; this does not establish device parity.
Live program/sampler queries identify the alpha-atlas shader and native nearest/
clamp sampling. Opaque, partial and zero source-alpha controls produce identical
host readbacks. Production paint requires explicit source-device dimensions.
`ChineseBrushFwidthOutputProfile::pixels_per_page_unit` separately opts into
64-sector fringe masks at a declared uniform output scale; the default keeps
hard-circle coverage. Public SDK exports reduce alpha error in 116 of 117 paired
host-capture comparisons across resvg, MuPDF and PDFium, with one tie, but lose
some faint edge support. Identical-radius masks are shared and output budgets
are checked before emission. Texture minification, arbitrary output affines and
device pixel parity remain unverified. Three captured output transforms also show
that an analytic radial derivative differs from native finite helper-lane
derivatives before SVG/PDF rasterization; the masks remain an approximation.
A thin host-replay diagnostic also identifies nearest-atlas boundary sensitivity
to tiny UV interpolation differences, separate from the fringe approximation.
The [Oblique V4 source trace](reverse-engineering/oblique-v4-source-findings.md)
establishes an integer-size diagonal core and separate fixed-width bounds.
Hash-pinned oracles capture saved traversal, core/fringe triangles and host GLES
coverage/compositing. Production SVG/PDF uses the bounded V4 mesh in canonical
page coordinates for ordinary strokes with color type 0 or absent. Omitted
size/ARGB use the [native defaults](../conformance/oblique_defaults_native.json)
of size 0.4 and opaque black; unsupported settings and Descendant objects retain
the generic approximation.
Source RGB/alpha and max coverage are preserved without image objects.
The [native Draw oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/oblique-draw-native.json) also verifies
explicit ARGB forwarding and executed MAX/source-over blend descriptors, with
synthetic resource adapters and no device-pixel claim. The
[reuse witness](../conformance/oblique-saved-caller-reuse-native.json) checks a
three-point stroke followed by a short stroke on the same drawable/RT; the
second callback stream and geometry match its fresh reference. Standalone
`prepare_stroke` remains approximate because it lacks a canvas context. Fixed
width, alternate stroke modes, arbitrary reuse and device pixel parity remain
open. Paired host exports improve on the pre-integration renderer, but broader
light-background PDF cases show material coverage/color differences at low
raster scales; SVG stays close in those captures. PDF transport remains under
investigation.
Selected V1 now uses the shared RTV1 vector transport in canonical page
coordinates for ordinary tool-0–3 strokes with the same native style defaults,
excluding fixed width and effects. The
[stress](../conformance/oblique-v1-stress-native.json) and
[edge](../conformance/oblique-v1-edge-native.json) oracles verify callback pairs,
return-state boundaries and complete mesh buffers, including short and
duplicate curve-disabled transitions. The [tool oracle](../conformance/oblique-v1-tool1-native.json)
verifies tool 1’s 50-unit distance threshold (5 for other tools), including
exact-threshold callbacks and tool-3 curve/linear meshes. Paired 1×/2× SVG checks against vendor shader
replay are close; vector-only PDFs retain material active-region errors.
PDF triangle fields use rounded 8-bit vector coverage contours within precision
and work limits, retaining the existing calculator field otherwise. Two of seven
paired host cases improve; five preserve the previous PDF bytes. Device parity
and broader contour coverage remain open.
That replay and the host-modeled overlap controls do not establish native
multi-stroke paint, device pixels or complete parity.

## Text and embedded layout

The detailed [text support table](text-vector-support.md) owns the admission
rules and native capture boundaries. These rows summarize it rather than
extending its guarantees.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Body, standalone and supported shape text | Shared measurement/layout; density, margins, gravity, wrapping and paragraph spacing | Selected native origins/wrap kernels verified; all standalone modes and complete native layout are not established. |
| Text fit and overflow | Saved text retained; no typed auto-fit/ellipsis projection | Present auto-fit/ellipsis fields produce extension diagnostics. Native dots/cached cutoff and separate triangle cues are derived output; full fitting and Free/Path semantics remain unverified. See [saved options](reverse-engineering/text-box-findings.md#persisted-auto-fit-and-ellipsis). |
| Fonts and Unicode | Pinned/caller fonts, coverage fallback including indivisible CJK graphemes across supplied faces, bidi and embedded physical-face identities; supplied SVG-table color glyphs preserve vector paint and source text in PDF | Plain/gzip SVG, currentColor and square/non-square viewport transport have Rust regressions and paired MuPDF checks. Native production measurement has bounded pinned-Regular admission; bundled CJK/emoji coverage, device fallback order and variable-font instances remain incomplete or unverified. |
| Flow italics | Native-name-selected Regular face with synthetic shear; generic fallback when native name resolution is unavailable | Native style captures and one paired export verify the face policy; PDF and eligible SVG text use native −0.25 shear. Positioned mixed-style fragments preserve source order and neighboring positions. Eligible non-bold combining clusters use per-glyph SVG outlines with independent origins and retained source text; unsupported mixed positioning, fonts and styles retain fallback. Rust geometry/tagging and transformed-clip tests cover transport; complete font/device parity remains unverified. |
| Complex shaping and emoji | Source-preserving SVG fallback; retained PDF glyph geometry where available | SVG cannot reproduce every retained cluster; drawable kind-4 states and native emoji resource production remain unsupported. See [resource ownership](reverse-engineering/text-draw-identity-findings.md#line-feeds-and-emoji-resources). |
| Spans, headings, lists and links | Supported styles, underline/strike/background bands, vector list markers including empty bullet paragraphs, and gated SVG hyperlinks | Native conversion confirms empty-bullet visibility; Mobile controls distinguish body/cell sizing and large-font growth. Complete marker appearance remains unverified. Intra-cluster background changes and unsafe object positions are conservative; PDF hyperlink annotations are absent. |
| Composition/suggestion/correction styles | Supported modern composition appearance; ordinary text preserved elsewhere | Legacy/incomplete composition and special suggestion/correction appearance produce diagnostics. |
| Tables | Dense unmerged/merged preparation, vector fills/borders/artwork and measured content | Sparse/invalid grids use diagnosed fallback; rotated/nested preparation and general split-page composition remain incomplete or unverified. |
| Table text clipping | Conservative table-wide scope; native per-run clipping in certified ordinary one-page contexts | Certification is bounded by source, font metrics, geometry and transforms; arbitrary cell-loop/per-run parity is not established. |
| Code blocks | Shared title/body layout, panels, chrome/minimum geometry and continuation handling | Captured supplied-metric geometry is verified separately from complete native split-page composition. |
| Pagination | Saved inspection sections, full-source fallback reflow, single-page continuous bodies and bounded native page-range routing | SVG, replay and PDF use the full body when its inspection node is source-owned; quiz’s image at UTF-16 112 is retained. Inspection data still selects 0..36. General native repagination, obstacle production and page-bound construction remain outside the verified route. |

## Graphics, paper and composition

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Shapes and lines | Basic templates, saved move/line/quadratic/cubic/close and bounded Arc/Oval paths (Model normalization for shapes, Drawing semantics for lines), rotation, solid fills/outlines and embedded text; bounded type-7 linear/radial fill/outline gradients and simple type-8 linear/radial outlines with bounded linear and radial line-paint metadata rotation; named positive-width dashes and bounded straight-line arrowheads | Native helper/load captures verify admitted path geometry separately from paired appearance. Gradients require admitted common frames and consumed stops. Rotatable linear/radial line paint follows metadata rotation without transforming saved body geometry; radial V4 pins the off-center center/radius and cache, with its native 0°/30° pair changing RGB while alpha stays fixed; descending two-stop cache steps preserve partial-alpha stop colors, with four synthetic `.75`/`.5` controls confirming the same 128/128 split across alpha/RGB variants, plus native bucket-zero and off-grid controls. Paired synthetic 0°/30° captures verify geometry and visible paint change; SVG/PDF sampling residuals remain. Two-stop type-8 radial-line cache steps preserve captured stop alpha and RGB, including cache bucket 0, zero-alpha and nonprimary controls. Synthetic source-grid SVG matches V35–V37 and opaque V41 exactly, and V38/V42 within 1 LSB; vector PDFs retain measured MuPDF color residuals. The corpus still has no saved-gradient witness. Rectangular/other gradients, other type-8 styles and saved radial-gradient appearance, image fills, specialized templates, connectors, pen simulation and compound styles remain incomplete. Arrowheads require simple solid-color straight lines with admitted coordinates, widths, lengths and separate cleanup regions; curved/short/overlapping contexts remain diagnosed. Style inputs and SDK vector transport are checked separately from paired appearance coverage. See [outline styles](reverse-engineering/shape-style-findings.md#rust-dash-projection), [arrow admission](reverse-engineering/shape-style-findings.md#bounded-rust-arrow-projection), [paint admission](reverse-engineering/shape-fill-findings.md#consequences-for-this-codebase) and [shape/line findings](reverse-engineering/shape-line-findings.md). |
| Images | Media resolution, placement, rotation, original-placement cropping and in-bounds source-pixel crop geometry; exact source-pixel crop for simple unrotated RGB8/RGBA8 PNGs, preserving sBIT/sRGB metadata and using displayed bounds even when original placement is stored; field 17 original bounds use native integer conversion | Empty and inverted crop rectangles use the native inactive-crop path. The native nonuniform-alpha upscale control confirms crop-edge-clamped sampling; exact source-row copying improves the paired crop-edge result. The PDF pair reduced MuPDF alpha error from 1,006 to 137 (maximum error 31 to 1); PDFium remained at 203. The real fixture 03 crop reduces page-3 changed pixels from 0.0822% to 0.0118% and crop-interior RGB error from 3.33 to 1.92 bytes; its other pages are unchanged. This does not establish full sampler or rounding parity. Other image inputs retain the geometry path; native filtering, out-of-bounds crops, border/original references, fill transforms, tiling and active nine-patch remain incomplete. Nonrectangular inherited image paths produce diagnostics; the traced native effect route uses a rectangle. See [image findings](reverse-engineering/image-findings.md) and [image-filled vector shapes](reverse-engineering/image-effects-findings.md#ordinary-image-effect-paths-differ-from-image-filled-vector-shapes). |
| GIF image sources | No typed GIF asset admission or animation renderer | Native animation opens the current main file; reader replacement has separate loader/restart gates. See [GIF source findings](reverse-engineering/image-findings.md#native-gif-import-and-current-file-animation). |
| Page paper | Solid color and supported built-in ruled/dotted templates | Image/URI-backed and rotated paper are rejected by [template admission](../crates/sdocx/src/page_background.rs); other templates are retained without drawing. |
| Page custom objects | No typed custom-list decoding or rendering | Separate from layer objects and their diagnostics. Sticky notes can bind nested `.sdocx` source; original page/archive bytes remain required. See [custom source ownership](reverse-engineering/vector-retention-findings.md#page-custom-objects-and-attached-source). |
| Physical layers and visibility | Saved current layer; hidden recognized objects/subtrees omitted | Inactive layers remain structurally available; no captured multilayer appearance comparison. See [layer selection](reverse-engineering/page-layer-selection-findings.md). |
| Object order and containers | Stored order within Base/Top/Masking root passes; selected containers recurse in place | Child render IDs do not reselect root passes. Saved child rotation is not applied again through a parent transform. Full mixed-container pixel parity remains unverified. |
| Root intersection filtering | No native geometric selection filter | Native collector behavior is traced, but comparing stored bounding boxes alone does not implement it. See [selection findings](reverse-engineering/object-selection-findings.md). |
| Light/dark treatment | Shared modes, native lightness reversal, local contrast and alpha-preserving paint | [Native Context-to-Pen dispatch](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/drawing-color-theme-native.json) is verified for three explicit host-selected themes. Android theme discovery is unverified; paper/contrast selection remains export policy. See [themes](render-themes.md). |
| Highlighter blending | Whole Top batch uses Darken on light paper and Lighten on dark paper | Capture-route selection/shader math has static evidence; native Standard list PDF uses Darken. SDK shares its SVG policy with PDF; native pixel parity is not claimed. |
| Tape visibility/reveal | Saved note preference and per-stroke reveal flag retained | Generic pen approximation does not implement native presentation controls or destination attenuation. See [Tape findings](reverse-engineering/stroke-metadata-findings.md#tape-visibility-and-reveal-controls). |

Native [image editing](reverse-engineering/image-edit-source-findings.md) can replace
the main resource independently of its original binding. SPI sources under `media/`
are retained as opaque resources but remain unsupported image inputs;
complete original archive bytes remain a separate carrier. Detailed parsing can
opt into retaining original page sources.

The [SVG composition description](svg-rendering.md) and
[native capture findings](reverse-engineering/capture-composition-findings.md)
give the ordering and export-route boundaries. “Masking” is a pass name; it
does not mean the SDK implements eraser subtraction. Layer alpha-lock/shadow
metadata and runtime object draw-state alpha are not a proven general saved
group-opacity contract.

## Output and verification

| Output | Implemented contract | Limits |
| --- | --- | --- |
| SVG preview/export | Typed Rust elements, validated numeric/path values, escaped content, vector ink and embedded fonts | Supported geometry and text transport do not imply complete native appearance. |
| Replay | Reveals the same generated vector geometry at saved sample boundaries | No pen geometry in the UI; no native live prediction or all-layer replay-state guarantee. |
| Document PDF | Shared scene plus retained selected fonts/glyphs/XY, vector masks/shadings/clips and selectable logical source with authoritative outer PDF text replacements; embedded image resolver/constructor rejections become export errors | Inherits rendering gaps; source images remain images; deferred image decoding and universal PDF accessibility are not certified. |
| Arbitrary SVG to PDF | Compatibility conversion through the SVG importer | Can reshape text and rasterize imported filter effects; lacks the document exporter's private retained-glyph guarantee. |

The [conformance guide](../conformance/README.md) records fixture identities,
independent native capture boundaries and real-document reference coverage.
The locked corpus contains four document/PDF pairs, not an exhaustive feature
inventory. Synthetic archives establish SDK behavior and malformed-input
handling; they cannot establish Samsung visual equivalence for absent features.
The corpus has no real gradient witness. Parsed Rust SVG and vector PDF
gradient checks verify synthetic transport, separately from native
helper/dispatcher captures and paired Samsung appearance.
The browser displays parser diagnostics separately from Rust's text, embedded-object,
shape/line geometry and fill/outline paint notices for previews and completed
exports. Legacy WASM and debugger SVG methods remain payload-only. See the
[diagnostic transport boundary](reverse-engineering/vector-retention-findings.md#diagnostic-interpretation).
The [real-document inventory](reverse-engineering/rendering-corpus-findings.md)
records feature occurrence separately from native research. At that revision its only
parse warnings were PDF-backed paper in two local research notes. Rust now retains
all ordered signed PDF records, version-dependent rectangles and raw template type;
modern manifest bindings and `media/` source bytes are retained separately. See
[page background findings](reverse-engineering/page-background-findings.md).
PDF export now preserves source PDF paper for a bounded continuous-note class:
authoritative source PDFs with contiguous, top-aligned, full-width integer
paper records. When multiple source PDFs are used, each catalog must contain only
its page tree so document-level context is not silently lost. Source-page indices may repeat or appear in a different order;
the native segment selector starts a new group when indices are not consecutive,
and the SDK imports the selected source pages in record order. Source CropBox
origins and quarter-turn rotations are preserved; UserUnit must be 1. The note must use the
native-default background and supported saved unrotated strokes.
Native-derived query bands select whole strokes;
source pages retain their physical boxes, while continuation pages use the
native 600-point width and integer-truncated height, independently of SDK DPI.
An explicit `PdfOptions::pdf_paper_fixed_axis_size` normalizes saved page/stroke
geometry using native f32 rules; its default preserves the stored coordinate
profile. Body coordinates and source PDF boxes stay in their own domains.
Empty or LF-only body text with supported formatting is accepted, including
foreground/font-size spans, finite margins and valid saved text sections;
bullets remain unsupported. LF-separated resolved, unrotated block images are
also accepted when the page and body flow axes agree; every image must have a
unique source anchor and measured owner. Body-aware page cuts use
the same cached layout as painting. A bounded ASCII and newline body is also
accepted at document density 1 when the native measured-layout certificate
admits its source, font, geometry and flow settings; space-only measured lines
remain unsupported. At density 3, a separate PDF-paper profile also accepts
bounded ASCII body text with nonempty paragraphs and no supplied paragraph
metadata, subject to the existing font, geometry and flow guards. The captured level-1 LTR indentation record with implicit font size is accepted
for one nonempty paragraph, two paragraphs with a nonempty first paragraph, or
three with nonempty first and last paragraphs, or a leading empty paragraph followed by a nonempty paragraph. Native controls cover limited
text and widths, so broader input is an inference. Other paragraph shapes with indent remain unsupported.
A single leading, interior, or trailing empty paragraph is also accepted for a
parsed implicit-default body. The captured 24-byte whole-text font-size span
profile accepts a leading or interior empty paragraph; leading requires size 50
and interior size 17 or 50. Other empty-paragraph styles remain unsupported. The prevalence of
this span encoding in saved notes is unverified. Native
scale-three controls cover line geometry, page-index boundaries and the
line-range and substring-visibility primitives, but do not execute the complete
BodyTextContext visibility method. A synthetic public PDF test checks 81 text
baselines across source-page cuts; MuPDF and PDFium extract the same 81
characters across its first four pages. A separate long indented-body control
crosses page cuts after increasing the shaping-window limit to 512; native layout
and independent PDF-reader checks cover the synthetic examples. Worst-case
performance at the new limit remains unmeasured. The density-one controls also cover
native line-range and visibility primitives. Both sets use a changed host libc;
public PDF tests check source resources, overlay clipping and text placement. Boundary text
uses stable source ranges and physical-clip ownership; duplicate visible pieces
are painted as outlines when supported. MuPDF and PDFium each extract all 160
lines once in a repeated-line public export fixture. Against the previous PDF,
MuPDF rasters at 1x and 3x differed only in a small number of antialiased color
channels around an outlined boundary glyph. Unsupported outlines or uncertain
ink geometry use a conservative text fallback and can still produce duplicates.
These checks do not establish device pixel parity, native PDF appearance, or
saved-device parity. The real quiz exports seven pages with an
explicit 1080 load width. MuPDF and PDFium render every page; its image pixels
are preserved and its page-5 bounds match native model placement within
0.00002 points. Two-image and blank-gap controls match native line geometry and
queries; multi-image SDK export has synthetic coverage. This is model/transport
validation, not native pixel parity.
A saved stroke's generated flag alone does not exclude it. Other visible text,
image transforms, gaps between records, multi-source documents with global
catalog context and unverified cases still fail explicitly. SVG previews continue to report omitted
PDF backgrounds; converting those diagnostic-aware pages to PDF fails explicitly.
The [PDF preservation constraints](pdf-paper-preservation.md) connect the
recovered contracts to the Rust model and verified vector PDF transport.
Rust assembly tests cover scoped resources, overlay tags, reordered/duplicate
pages and ordinary tails; public export tests cover source preservation,
cross-band whole-stroke admission and DPI-independent physical page sizes.
Source annotation identities are reserved across selected pages before graph
cloning. A cross-page `/IRT` now points to the annotation in its selected owner's
`/Annots`, with `/P` set to that output page. Repeated output pages receive
separate page-bound annotations; references from other pages use the first
selected occurrence. Assembly tests cover cyclic replies, cross-page `/Popup`
and `/Parent` identity with reordered and duplicate pages, indirect annotation
arrays and aliases, shared intermediary objects, and rejection of references
to annotations on unselected pages, including those without `/P`. A public export
test also checks cross-page reply identity. For that public export, MuPDF
1.27.2.3 reports the second-page reply's `/IRT` as the first-page annotation;
PDFium 150.0.7869.0 opens and renders both pages and reports their text
annotations. Cross-page `/Popup` reader behavior and interactive annotation
behavior remain unverified. Cyclic references in an annotation list are rejected.
Page-box geometry accepts either corner order while preserving the source values;
blank source pages with direct or indirect null content can receive overlays.
Overlay grafts clear unfinished source paths before clipping; a synthetic manual
check in MuPDF and PDFium confirmed that this prevents paint outside the clip.
The real `cs61bl_su22` note exports 20 pages: both MuPDF and PDFium render them,
and source page boxes, text, font streams and image bytes survive. This verifies
SDK transport; a connected native multi-record export comparison remains open.
Real-source and retained-text transport match MuPDF/PDFium at 1×/2×. The stroke
selector matches native identities and order across all 11 observed bands of
one quiz archive loaded at width 1812: 3,228 unique strokes and 3,261 admissions,
including cross-band repeats. This verifies query admission in that loaded
state, not exported appearance. General native body layout and placement remain
open beyond the bounded LF-plus-image route.
A [native source-PDF export probe](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf_paper_pdfium_quiz_source_native.py)
preserves one retained document's image bytes and page geometry while adding a
vector path. A [PNG overlay capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-pdfium-image-export.json)
also executes native PNG decoding and image placement for a synthetic rectangle.
A [Composer branch capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-composer-export.json) verifies
the imported-paper DrawPDF skip and two manual-background extent controls;
its AddPages bridge does not join native PDF serialization.
A later archive-backed SingleX capture loads and imports the quiz's actual
cached PDF, then stops at FountainPen loading before completed output.
A separate native loader control verifies FountainPen initialization and factory
construction; the joined archive export remains unverified.
A [SingleX segmentation capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-single-segments.json)
uses retained quiz dimensions with explicit host text/object queries; full
export and final page appearance remain unverified.
The [body-writer transform capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-body-writer-matrix.json)
observes native note-to-PDF scale and marker rectangles for rotated pages with
two CropBox origins, then calls PDFium matrix helpers separately. The
[image-adapter capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-adapter-native.json) joins the
native String-overload constructor/setters to PDFium image insertion and export
for four rotations and shifted origins, with host-supplied placement and PNG.
The [bitmap-overload capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-bitmap-native.json) executes
native bitmap methods, adapter and libpng encoding with synthetic pixels,
simulated host GL and pinned host zlib. The separate
[RGBA bitmap capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-rgba-native.json) executes native
texture/subbitmap readback through actual host Mesa textures and framebuffers,
then native adapter/libpng encoding. Two synthetic 2×2 controls preserve alpha
and verify rounded, clamped RGB unpremultiplication, including zero alpha;
queue scheduling and zlib remain host bridges. An archive-backed Standard probe
also completes a [native plain-bitmap draw and deactivation](reverse-engineering/fountain-rasterization.md#presenter-copy-replaces-transparent-pixels-too)
with host EGL/GLES adapters. A later archive-backed Standard attempt completes
an 1800 × 2545 native RGBA readback, including pack-state restoration and the
native error check. That intermediate contains opaque ruled-background pixels;
its Composer producer remains unresolved, and it is not a whole-note ink oracle.
Complete PDF output remains unverified. The native raster-batch producer and
Drawing-to-insertion and general saved-note/body placement remain outstanding;
the production PDF-paper subset above does not establish those contracts.

The native [shape style contracts](reverse-engineering/shape-style-findings.md)
and [image effect contracts](reverse-engineering/image-effects-findings.md)
have static evidence for features absent from these documents; they do not add
real-document appearance coverage. See also
[PDF findings](reverse-engineering/pdf-export-findings.md).
The [solid shape paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/shape-solid-paint-native.json)
adds bounded native fill/outline raster evidence for an axis-aligned rectangle;
odd-width edge coverage and broader paired appearance remain open.

The [vector retention findings](reverse-engineering/vector-retention-findings.md)
separate source-byte ownership, semantic decoding and drawable output. Physical
payload offsets do not own their bytes, and vector output can still quantize or
omit source geometry. Recovered
[path](reverse-engineering/shape-path-findings.md),
[fill](reverse-engineering/shape-fill-findings.md) and
[connector](reverse-engineering/connector-routing-findings.md) contracts identify
additional typed geometry and resource boundaries without adding SDK support.
[Container edits](reverse-engineering/object-transform-findings.md) and
[eraser cuts](reverse-engineering/eraser-preservation-findings.md) can modify saved
child or stroke geometry before drawing. The
[brush and painting findings](reverse-engineering/brush-record-findings.md)
distinguish compatibility dispatch and editable resources from previews.
The [painting source contract](reverse-engineering/painting-source-findings.md)
identifies a separate archive with ordinary stroke packets, replay channels and
bitmap state. The shared native reducer uses different coordinate widths from
WDoc; neither a thumbnail nor decoding WDoc frames preserves that source.
