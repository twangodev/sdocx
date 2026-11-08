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

The [capture adapter](../../tools/inkpen-capture/inkpen2_v1_paint_native.py) checks the
SHA-256 of the joined native harness and seven support scripts before running;
the parent harness checks pinned native libraries. It requires the existing
`scratch/apk-analysis-native`, `scratch/apk-analysis-runtime` and
`tmp/vector-parity-plan/chinesebrush-paint-scale-work/conformance` research
tree, plus host EGL/GLES. The support tree is a local prerequisite, not a
portable checked-in capture package; a different Mesa implementation can yield
different pixels. From the project root, recapture with:

```sh
export PYTHONPATH=scratch/straightinkpen2-alias:scratch/apk-analysis-runtime/python:scratch/apk-analysis-runtime/binutils-aarch64/usr/bin
python3 tools/inkpen-capture/inkpen2_v1_paint_native.py --print-capture > /tmp/inkpen2-v1-original.json
python3 tools/inkpen-capture/inkpen2_v1_paint_native.py --print-capture --matrix-translate-x 32 --matrix-translate-y 32 > /tmp/inkpen2-v1-translated.json
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
Recapture with `python3 tools/inkpen-capture/inkpen2_v1_style_capture.py`; the adapter
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

Additional [synthetic geometry controls](../../conformance/inkpen2-v1-extra-controls-native.json) compare zero, full, and alternating pressures, pen sizes 0.5 and 8, and 32-point curved and straight strokes against native saved redraw at 1920 × 1080. All seven vertex streams match the bounded model byte for byte. These controls do not establish paint, serialized-note, or device parity. Recapture with `python3 tools/inkpen-capture/inkpen2_v1_extra_controls_capture.py`. Four further callback controls use fractional negative coordinates, irregular timestamps, descending timestamps, and a disabled curve flag. Their native vertex streams also match the model byte for byte. They do not establish SVG or PDF paint parity for these inputs.

A further [host capture](../../conformance/inkpen2-v1-pressure-paint-native.json)
compares zero, full, and alternating pressure on the same four-point stroke,
and a diagonal stroke at unit scale. All three pressure controls produce the
same native frame; the diagonal produces a different frame. The existing public
profile has lower alpha-byte L1 error than generic output in all four SVG and
SVG-derived PDF comparisons. Historical measurements with MuPDF 1.28.2 and
PDFium 150.0.7869.0 at 4/3 pixels per point gave these alpha-byte absolute-error sums:

| Control | MuPDF public / generic | PDFium public / generic |
| --- | ---: | ---: |
| Zero pressure | 5,737 / 16,591 | 4,626 / 16,495 |
| Full pressure | 5,737 / 16,482 | 4,626 / 16,337 |
| Alternating pressure | 5,737 / 20,758 | 4,626 / 18,973 |
| Diagonal | 6,649 / 28,295 | 2,086 / 26,882 |

These are synthetic transparent host-Mesa controls, not saved-note or device
parity. Native inputs remain reproducible with
`python3 tools/inkpen-capture/inkpen2_v1_pressure_paint_capture.py` using the local
prerequisites above. The focused `inkpen2_v1_pressure_and_diagonal_native_pixels`
Rust test retains native hashes, SVG error assertions, and PDF construction checks.
The optional export hook, reader comparator, and derived PDF receipt were removed.

Historical local synthetic landscape controls compared 2560 × 1440 with 1440 × 2560 at 1× and 2×. Their sampled native geometry and host-Mesa frames matched the portrait counterparts, and public SVG and direct PDF reduced aggregate error against generic output in resvg, MuPDF and PDFium. The original control covered tools 2 and 3; expanded tool-3 controls sampled both pen names and null or literal `1;` settings. The [landscape](../../tools/inkpen2-v1-landscape/receipt.json) and [expanded](../../tools/inkpen2-v1-landscape-expanded/receipt.json) receipts retain the captured streams and frames; the historical reader-comparison scaffolding is not packaged. Other trajectories extrapolate from these host controls. The V1 landscape profile admits both names and settings at those scales. The separate [experimental half-grid viewport](../../tools/inkpen2-v1-tablet-half/README.md) opts into an asserted fixed 0.5× grid with per-stroke fallback and page work limits. Device and saved-note parity remain unverified.

Original half-grid control receipts are preserved in the screen-paint
fixture; restore them for the local capture workflows without overwriting files:

```sh
for f in white-native-turn.json nonwhite-native-turn.json overlap-native.json overlap-colors-native.json; do test ! -e "tools/inkpen2-v1-tablet-half/$f" && jq -jr --arg name "$f" '.half_grid_color_original[$name]' conformance/inkpen2-v1-screen-paint-native.json > "tools/inkpen2-v1-tablet-half/$f"; done
for f in mixed-scene-native-line-getters-receipt.json mixed-scene-native-line-receipt-v2.json mixed-scene-offset-input.json mixed-scene-rotated-input.json; do test ! -e "tools/inkpen2-v1-tablet-half/$f" && jq -jr --arg name "$f" '.mixed_scene_original[$name]' conformance/inkpen2-v1-screen-paint-native.json > "tools/inkpen2-v1-tablet-half/$f"; done
test ! -e tools/inkpen2-v1-tablet-half/color-native.json && jq -r '.color_matrix_original_gzip_base64' conformance/inkpen2-v1-screen-paint-native.json | base64 -d | gzip -dc > tools/inkpen2-v1-tablet-half/color-native.json
```

The line rerun and its offset and rotated variants can be reconstructed in
memory from the tracked order capture. Restore the selected input above, then
run this with `mixed-scene-offset-input.json` or `mixed-scene-rotated-input.json`
as the argument. The original input hash is also accepted.

```sh
capture_input=tools/inkpen2-v1-tablet-half/mixed-scene-offset-input.json
python3 - "$capture_input" <<'PY'
import hashlib, sys
from pathlib import Path
root = Path('tools/inkpen2-v1-tablet-half')
source = root / 'mixed_scene_order_capture.py'
text = source.read_text()
anchor = '  stage("changed_color_counts=" + json.dumps(changes.most_common(12)))\n'
assert text.count(anchor) == 1
text = text.replace(anchor, anchor + '  raise SystemExit(0)\n')
start = text.index('final = """  final_pixels=framebuffer_snapshot(')
end = text.index('\nwrapper = wrapper.replace(anchor, anchor.replace', start)
line = 'wrapper=wrapper.replace(anchor,"code = code.replace(\\"  p.u.hook_del(draw_branch_hook)\\", "+repr(inject)+" + \\"\\\\n  p.u.hook_del(draw_branch_hook)\\")\\n"+anchor)'
text = text[:start] + line + text[end:]
assert hashlib.sha256(text.encode()).hexdigest() == 'bd40302bcc076fb7c7385bb87b16ffd36caf9b1f159f79caeee3d4aaac79220a'
input_hash = hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()
assert input_hash in {'667ce1567150447bca4e5358f18a2f3f6fd8a8335d3d88b7f7da6c39a02d4069', '4953b7126678c85bf6355d3a1919ff3d40db581beff723cea4ffa2fd4d445031', '2fe8e1e46cce97e322ecfcbc53d16e8f716b391e3eace5763331d9b1cc52adc6'}
text = text.replace('667ce1567150447bca4e5358f18a2f3f6fd8a8335d3d88b7f7da6c39a02d4069', input_hash)
sys.argv = [str(source), '--input-json', sys.argv[1]]
exec(compile(text, str(source), 'exec'), {'__name__': '__main__', '__file__': str(source)})
PY
```


The [tool-2/tool-3](../../tools/inkpen2-v1-tool3/receipt.json) and [extended](../../tools/inkpen2-v1-tool3/extended-receipt.json) host receipts retain paired geometry and frame hashes. The uncurved zigzag supports the native tool alias only; the public profile uses its standard curve. The `inkpen2_v1_tool3_native_alias_exports`, `inkpen2_v1_tool3_extended_native_alias_exports` and `inkpen2_v1_landscape_expanded_exports` Rust tests check public SVG and direct PDF equality; the extended test validates decoded native hashes. The PDF reference composites transparent host frames over paper. Reader comparisons are historical and do not establish saved-note, Android device, arbitrary-transform or complete application parity.

Local recapture uses `tools/inkpen2-v1-tool3/capture.py` and `extended_capture.py`. They require the native prerequisites of `tools/inkpen-capture/inkpen2_v1_paint_native.py` and output JSON for comparison with their receipts. The Rust export tests require the `pdf` feature.
