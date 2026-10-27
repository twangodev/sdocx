# InkPen2 null-setting V1 host paint

The [receipt](../../conformance/inkpen2-null-v1-paint-native.json) records two
synthetic Drawing-selected RTV1 captures for the baseline geometry in the
[controls](../../conformance/inkpen2-null-v1-controls-native.json). Both use a
1920 × 1080 screen profile, opaque `#336699`, a 128 × 96 host RGBA8 target and
unit draw scale. The first uses the identity draw matrix and clips against the
target edge; the second translates by (32, 32). Native RTV1 uploaded the same
300-byte, 25-record geometry in both cases and drew 25 instances. Its observed
fragment shader uses derivative-based circular coverage and source-over
blending. A fresh recapture reproduced both full RGBA SHA-256 values on this
host. These are host Mesa outputs, not Android/device pixels, a complete app
canvas, serialized-note admission, or SVG/PDF parity.

The [capture adapter](../../conformance/inkpen2_v1_paint_native.py) checks the
SHA-256 of the joined native harness and seven support scripts before running;
the parent harness checks pinned native libraries. It requires the existing
`scratch/apk-analysis-native`, `scratch/apk-analysis-runtime` and
`tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance` research
tree, plus host EGL/GLES. The support tree is a local prerequisite, not a
portable checked-in capture package; a different Mesa implementation can yield
different pixels. From the project root, recapture with:

```sh
export PYTHONPATH=scratch/straightinkpen2-alias:scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin
python3 conformance/inkpen2_v1_paint_native.py --print-capture > /tmp/inkpen2-v1-original.json
python3 conformance/inkpen2_v1_paint_native.py --print-capture --matrix-translate-x 32 --matrix-translate-y 32 > /tmp/inkpen2-v1-translated.json
```

Compare `geometry_sha256`, `rgba_sha256`, `draw_matrix`, and the complete
compressed RGBA readbacks with the corresponding receipt fields. A bounded opt-in V1 vector approximation and paired SVG/PDF measurements at 0.5×, 1×, and 2× are described in [rendering support](../rendering-support.md). The [scale receipt](../../conformance/inkpen2-null-v1-paint-scales-native.json) retains the native frames and comparison measurements.

The [style capture](../../conformance/inkpen2-null-v1-style-native.json)
adds three synthetic type-0 Drawing controls with the same points and size 4,
using ARGB `ff336699`, `80336699`, and `00336699`. Native ObjectStroke
returns each supplied color and Drawing forwards it unchanged to `Pen::SetColor`
and `PenDrawableRT::SetPenData`, including the alpha byte. The saved redraw
callback stream has the same SHA-256 in all three controls, so these cases
show color-independent geometry, not the effect of alpha on painted pixels.
The capture uses the same identity canvas and synthetic setter boundary as the
geometry controls; it does not deserialize a saved page or establish device
appearance. An attempted type-1 stroke hit an unmapped read in this host
harness, so it does not establish whether native Drawing admits that style.
Recapture with `python3 conformance/inkpen2_v1_style_capture.py`; the adapter
requires the same local native and runtime research trees as the geometry
capture.

The [color and alpha receipt](../../conformance/inkpen2-null-v1-color-alpha-native.json)
adds zero-alpha blue and half/opaque red on transparent and host-cleared opaque
white targets. Zero-alpha blue matches the existing opaque-blue frame byte for
byte; half and opaque red match each other. On white, zero and opaque blue match,
as do half and opaque red. These are controls for one synthetic stroke on host
Mesa, not a general Android or application-canvas claim. The bounded V1 SVG/PDF
route ignores the source alpha byte; public output tests check that zero, half,
and opaque source alpha produce the same SVG and PDF for each of these two RGBs.

A [six-shape host receipt](../../conformance/inkpen2-null-v1-shapes-paint-native.json)
adds short, stationary, turn, reverse, and size 0.2 and 12 synthetic inputs at
unit output scale. In each case the public SVG and PDF profile has a lower
alpha-byte L1 error than the generic fallback against the transparent host frame.
SVG profile/generic errors range from 56/1,590 (stationary) to 1,012/14,790
(size 0.2); PDF ranges include 846/2,076 (stationary), 1,064/1,190 (short),
and 7,211/67,203 (size 12). PDF was rasterized by PyMuPDF at 4/3 pixels per
PDF point, SVG by resvg. These remain synthetic host-Mesa comparisons, not
serialized notes, device output, or full application composition. The Rust test
`inkpen2_v1_distinct_native_shapes_public_svg` validates the SVG comparisons;
set `SDOCX_V1_SHAPE_PDF_CAPTURE` to an existing empty directory to write the profile
and fallback PDF controls there for independent rasterization. Existing files are
never overwritten.

A [rectangle-clip comparison](../../conformance/inkpen2-null-v1-rectangle-clip-comparison.json)
worsened PDF alpha error for all six shapes, so the vector renderer retains circle clips.

At 1440 × 2560, the six-shape [host capture](../../conformance/inkpen2-v1-screen-paint-native.json) retains complete vertex streams. The resolution-dependent model matches all six byte for byte, including the reverse stroke whose geometry and frame differ from 1920 × 1080. The public 1× SVG and [PDF comparison](../../conformance/inkpen2-v1-high-pdf-comparison.json) have lower alpha error than generic output in all six cases. This is host-Mesa evidence, not device or application-canvas parity. At 2×, six additional host frames and public SVG/PDF comparisons also improve against generic output. At 0.5×, the short stroke candidate has worse SVG and PDF alpha error, so that scale remains excluded. See the [capture](../../conformance/inkpen2-v1-high-scales-native.json) and [PDF comparison](../../conformance/inkpen2-v1-high-scales-pdf-comparison.json).

Additional [synthetic geometry controls](../../conformance/inkpen2-v1-extra-controls-native.json) compare zero, full, and alternating pressures, pen sizes 0.5 and 8, and 32-point curved and straight strokes against native saved redraw at 1920 × 1080. All seven vertex streams match the bounded model byte for byte. These controls do not establish paint, serialized-note, or device parity. Recapture with `python3 conformance/inkpen2_v1_extra_controls_capture.py`. Four further callback controls use fractional negative coordinates, irregular timestamps, descending timestamps, and a disabled curve flag. Their native vertex streams also match the model byte for byte. They do not establish SVG or PDF paint parity for these inputs.
