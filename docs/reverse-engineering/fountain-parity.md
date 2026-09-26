# Fountain pen vector parity

The supported saved V14 fountain profile now uses Rust-generated vector
shading in SVG previews and PDF exports. V14/V16 positions and widths retain
the native reconstruction; no global width adjustment is applied.

Saved V14 (`14;`) and V16 (`18;0;100;`) profiles support input modes 1, 2,
and 3, fixed width, and saved stroke opacity. Their geometry is verified against
the native library. V14 shading is verified in Chromium and vector PDF output;
Firefox currently fails mask visibility and maximum-blending checks. Appearance
is not identical across SVG consumers, and the experimental alternatives below
are not part of the implementation.

The agreed target is Chromium preview and vector SVG/PDF export. Firefox
appearance parity is deferred; its two independent regressions remain expected
failures. Native pixel identity is not established: the measured partial-alpha
differences below remain, despite matching reconstructed geometry.

## Geometry and appearance

V14 retains SmPath's direction vectors, including its small-vector
normalization threshold, zero-vector substitution, and final stamp's retained
direction. Native fixtures compare positions, radii, directions and
original-sample boundaries. Pressure, tilt and movement continue through the
existing native saved-stroke reconstruction.

Previously every V14 stamp was filled opaquely. The native directional gradient
is flat over the central half and falls linearly to 0.07 at its tangent-aligned
ends. `render/fountain.rs` expresses that function as an SVG linear gradient,
transformed with each stamp's native radius and direction.

Opaque grayscale stamps blend with Lighten over black in a luminance mask.
This implements maximum coverage in the vector interior instead of accumulating
opacity where stamps overlap. Stroke color and opacity are applied once to the
result. PDF export preserves the masks as vector forms, the gradients as PDF
shadings, and the Lighten blend mode. There are no image or filter elements in
the generated fountain SVG, and no bitmap API or custom rasterizer.

These are scale-independent vector shapes. The SVG/PDF consumer owns edge
antialiasing. The APK's device-pixel expansion and subpixel compensation are
not baked into document geometry, so this is not a claim of byte-identical
screen output at every zoom level.

## Validation on 2026-09-25

The final source audit passed all eight fountain Rust tests, all five PDF
integration tests, and all eleven Chromium debugger tests after the negative
pressure correction. The browser run confirmed identical completed replay
pixels and preserved SVG nodes across seeks, with zero replay canvases, images,
or filters. Its median frame interval was 16.6 ms (17.4 ms p95); median and
maximum measured seeks were 182.6 ms and 239.3 ms. No Firefox workaround or
experimental geometry dependency was added to production.

- Re-executing the hash-pinned native fixture oracles checked 68 V14 cases /
  4,062 stamps with zero error, and 187 V16 cases / 12,331 stamps with maximum
  error 0.000000954 and identical sample mapping. The real-document WASM
  replay/export check also passed for the calibration document's 77 strokes.
- The available `handwritten.sdocx` contains 2,769 V14 strokes and 56,713 stamps.
  The native saved-redraw oracle and Rust geometry agreed exactly on x, y,
  radius, direction x/y and original-sample boundaries.
- New regressions check gradient orientation and interior maximum blending with
  repeated stamps, including stroke opacity. PDF object checks require native
  shadings and Lighten and reject image XObjects.
- The real SVG is 10,739,219 bytes. The PDF is 42,045,542 bytes and contains zero
  image XObjects across all PDF objects, including mask resources.
- A local Chromium decode-and-draw probe measured about 1.11 seconds for the
  shaded SVG versus 0.16 seconds for the previous opaque-path SVG. This is one
  cold probe, not an interaction benchmark. Vector shading increases document
  complexity and PDF size; it is not flattened to reduce either.

```sh
cargo test --offline -p sdocx --all-features --lib fountain
cargo test --offline -p sdocx --all-features --test pdf_export
```

The optional hash-pinned native geometry oracles are documented in the
[conformance guide](../../conformance/README.md#native-geometry-checks).

## Scope limits

Unsupported saved settings remain explicitly approximate, including
input modes outside 1–3, other historical profiles, and effects. Saved redraw
validation does not establish live prediction or temporary-tip parity.

## Real-document geometry audit

All 7,282 fountain strokes in the seven available SDOCX fixtures select
reconstructed V14 or V16 geometry. Native comparison covers 242,800 stamps
with identical sample boundaries and zero measured stamp-attribute error.
V14 comparisons include both tangent components as well as centers and radii.

| Document | V14 strokes / stamps | V16 strokes / stamps |
| --- | ---: | ---: |
| `quiz.sdocx` | 3,222 / 129,727 | — |
| `cs61bl_su22.sdocx` | 1,112 / 48,903 | 73 / 1,954 |
| `handwritten.sdocx` | 2,769 / 56,713 | — |
| `02-shapes-and-dot-calibration.sdocx` | — | 77 / 3,777 |
| `04-marker4-highlighter.sdocx` | — | 29 / 1,726 |

The other two fixtures contain no fountain strokes. Local input SHA-256:

```text
quiz.sdocx           38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a
cs61bl_su22.sdocx    fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0
handwritten.sdocx   77e3997a066afa0333d0f5020bb428efffeb783c741bc956ae596292e2d5cda3
```

The V14 `--prepared` command previously captured native output without
comparing the supplied geometry. It now compares every prepared stamp and
sample boundary and fails on a mismatch. A deliberately modified tangent
was rejected, then all three real V14 documents passed the corrected check.
Both oracles exclude other pen profiles in mixed documents and reject
unequal prepared attribute lengths before combining their channels.
These results prove geometry agreement for the listed inputs, not complete
native appearance or coverage of absent historical settings.

An input-channel audit checked both profiles with tools 1–3: absent optional
tilt matched an explicit zero-tilt channel, but a null pressure array caused
native reads at addresses zero or 0x50. The emulator originally mapped the
first library's ELF header over these addresses, hiding invalid reads. Its
null page is now unmapped; all 255 current native fixture cases pass, and the
null-pressure probes fail with unmapped-memory errors. Rust therefore retains
its requirement for a complete pressure channel even when the tool's width
calculation ignores pressure values.

After that correction, fresh exports from the current Rust source were checked
against native redraw for all five fountain-containing documents in the table.
All 7,282 strokes / 242,800 stamps still had zero measured attribute error and
identical sample boundaries. The real-corpus geometry result therefore does not
depend on accepting null-page reads.

## Vector replay

The debugger now uses the same Rust SVG renderer as the completed page. Replay
adds stroke indices and prepared-point boundaries without changing geometry or
paint. The UI reveals stamp groups or path prefixes using `sample_ends`; it
does not calculate pen shapes, shading, or opacity. Fountain stamps remain in
their original luminance mask, so partial strokes retain directional shading
and maximum blending. Ordinary ink, page elements, and the Darken highlighter
batch keep the renderer's paint order.

The canvas replay renderer, tile caches, and viewport raster helpers were
removed. A page/color-mode change loads a new SVG; timeline changes retain the
same DOM and reveal existing vectors. This is saved-geometry replay, not a
reconstruction of native live prediction or temporary tips.

Rust regressions compare annotated and normal page appearance in both color
modes. Browser checks cover backward seeks, completed-page appearance, zoom,
scrolling, and resource cleanup. On the dense handwriting fixture, completed
replay matched the normal page's RGBA output at 462 pixels wide. A local
Chromium playback probe measured a 16.7 ms median frame interval and 17.3 ms
p95 with 143,886 SVG nodes and zero replay canvases. These measurements cover
this fixture and host, not every document or device. Large forward/backward
seeks in the latest run took 179.5 ms median and 229 ms maximum. All eleven
Chromium debugger tests passed using the WASM build containing saved-opacity
support, including the independent visibility and overlap checks. After
installing Firefox, eight existing debugger
tests passed and its Chromium-only performance test was skipped. This does not
establish appearance parity: both sides of the completed-page comparison can
render incorrectly in the same way.

Firefox currently renders a production V14 stroke completely transparent when
the SVG is loaded as an image. Inline replay also shows striped stroke interiors.
The isolated image failure persists with one stamp, full-page mask bounds,
explicit mask content units, and different grouping. Removing Lighten makes ink
visible but changes native maximum-coverage semantics, so it is not a parity fix.
The failure depends on placement in the output viewport: fitting the root
viewBox tightly to the same stroke paints visible ink (alpha sum 212,951 at
128 by 128), while retaining page coordinates paints none. Translating the
whole diagnostic toward the viewport origin also restores ink. Rewriting the
mask and stamps into local coordinates inside a translated group or nested SVG
does not resolve it. Nonzero coverage alone does not establish correct shading.
The browser regression `V14 fountain SVG retains visible ink when loaded as an
image` isolates a real production stroke, checks nonzero alpha coverage, and
attaches its SVG. It records Firefox as an expected failure until corrected;
the existing replay comparisons alone must not be cited as Firefox fidelity.

The rejected workarounds, analytical blend probe, and unshipped geometric
partition prototypes are recorded separately in
[fountain-vector-experiments.md](fountain-vector-experiments.md).

## Saved V14 fixed width

V14 redraw supports the saved fixed-width property for input modes 1, 2 and 3. Native
`GLV14::drawPoint` at `0x731e0..0x73234` overrides each radius with half the
saved fixed width and applies the same float32 0.1 minimum. Centers, tangent
directions and sample boundaries still come from the normal redraw. Rust
applies the override before preparing bounds and vector shading.

Twelve stylus fixed-width cases in `conformance/fountain-v14.json` contain 644 stamps
captured from the complete hash-verified native redraw. They cover single
taps, stationary input, short jitter, turns, reversals, pressure and tilt
changes, closed loops, and widths below/at/above the radius floor. The oracle
preserves an explicit zero fixed width; it no longer substitutes pen size
for zero. Missing fixed widths remain unsupported rather than guessed.

The shape-specific quarter-width branch remains excluded.

## Saved V16 fixed width

V16 stylus reconstruction now supports saved fixed width. Native
`GLV16::drawPoint` at `0x79018..0x79048` selects half the saved width and
applies the float32 0.1 radius floor. Rust applies that override to the
completed two-pass geometry, preserving native sampling and width history.
The shape-specific quarter-width branch remains excluded.

The V16 oracle executes native `redraw(ObjectStroke)`, `drawLine`, `endPen`
and `drawPoint`, including fixed-width selection, the radius floor and the
fill-mode check. Its collector intercepts the final `RTV5::AddPoint`
submission. Object initialization and the outer fill/smooth/replay wrapper
remain host reconstructed. ObjectStroke channels and endpoint MotionEvent
access are supplied by the host.

Thirty-one additional cases capture 1,874 native stamps across all existing
trajectories, including pressure, tilt, stationary samples and reversals.
Single-point cases also cover widths 0, 0.1, 0.2, 0.3, 3 and 1024. Native
fixed and variable runs produce identical centers and sample boundaries.
The V16 stylus subset has 57 cases and 3,743 stamps; Rust checks their
positions, radii and sample boundaries. V14's 68 native cases also pass with
the updated shared oracle.

## Saved V14 input modes

V14 saved input modes 1 and 3 now use native reconstruction, alongside stylus
mode 2. `GLV14::redraw` at `0x72d9c..0x72df0` and `0x72e3c..0x72eec`
substitutes pressure 0.5 and tilt 0.8 radians for modes 1 and 3. The tilt is
processed by the same native degree conversion and clamp as stylus input.
`0x72cec..0x72d04` selects a short-movement threshold of 50 for mode 1 and
5 for modes 2 and 3. Midpoint sampling, alternating short-move filtering,
tangent directions, width limiting and endpoint handling remain shared.

Thirty additional native cases contain 1,482 stamps, including fixed-width
input, pressure/tilt changes, taps, jitter, turns, loops, and movements just
below/at/above both thresholds. Native reruns with changed pressure and tilt
arrays produced identical geometry for the first 24 cases. The complete V14
fixture now contains 68 cases and 4,062 stamps; the Rust geometry regression
checks all five stamp attributes and sample boundaries against those captures.

Complete saved pressure and timestamp arrays are still required: native redraw
reads the final pressure while constructing its endpoint event even when the
width calculation substitutes a constant. This does not establish parity for
other input codes, shape redraw, live event processing or temporary tips.

## Saved V16 input modes

V16 now reconstructs saved input modes 1 and 3 alongside stylus mode 2.
Native redraw at `0x78ba4..0x78bcc` and `0x78c68..0x78cd0` substitutes
pressure 0.5 and tilt 0.8 radians for these modes. `0x78aec..0x78b04`
sets the short-movement threshold to 50 for mode 1 and 5 otherwise.

`WidthSmoothManager::resetSmoothing` selects parameters (2, 2) for
non-stylus input. The incremental fill logic finalizes each new entry
immediately, so there is no forward averaging; the 0.15 smoothing step
applies after index 3. Stylus reconstruction retains its existing averaging
and later smoothing start. Native width-history captures verify this
distinction, rather than inferring it from the parameter names.

The new reversal cases also exposed an incorrect early return in Rust:
zero distance from the previous accepted sample can still leave a nonempty
midpoint curve after an alternating short move. Native drawLine consumes
that curve, resolving the undefined direction ratio through its width
floors. Rust now follows that behavior.

The V16 fixture contains 187 cases and 12,331 stamps. The 120 input-mode
cases cover variable and fixed widths and both movement thresholds, including
values immediately below, at and above each threshold. For the first 114,
changing all saved pressure and tilt values leaves native geometry unchanged.
Every fixture checks native centers, radii and saved-sample boundaries.
The oracle now executes the native saved redraw loop for both passes;
the outer two-pass setup remains host supplied.

## Bounded negative saved pressure

Compressed pressure deltas can decode slightly below zero. V14 already allowed
values down to -1, but V16 rejected every negative value and switched the entire
stroke to approximate geometry. Native V16 instead processes these values
through its width and radius floors. Both profiles now accept the same bounded
range; nonfinite values and pressures below -1 still fall back.

Ten additional native V16 cases cover -1/4096, -1, mixed positive/negative
pressure, two pen sizes, single-point taps, and input modes 1 and 3. Their 1,360
stamps and sample boundaries pass the Rust geometry comparison. This fixes a
confirmed fallback path, but none of the existing real V16 corpus strokes used
it, so it is not evidence that this caused the user's original width complaint.

## Saved fountain opacity

V14 and V16 preserve saved ARGB opacity while using native geometry.
`PenDrawableRT::SetPenSettingData` at `0x4a4b8..0x4a500` converts the
ARGB channels to float32 RGBA by dividing by 255. Direct execution verified
alpha values 0, 1, 64, 128, 254 and 255. The saved-redraw composite applies
the resulting alpha once after maximum stamp coverage, as traced in
[draw-time compositing](fountain-rasterization.md#draw-time-compositing).

Previously non-opaque saved colors forced fountain geometry into the
pressure fallback and lost their opacity. Shared Rust preparation now
retains reconstructed stamps and carries the converted alpha into SVG,
PDF and replay paint. V14 applies it to the masked stroke; V16 applies it
to the union path. A regression checks both profiles across the six alpha
values, including overlapping stamps and fully transparent strokes, and
rejects image/filter output. This does not add erasing or rainbow effects.

## V16 native PDF appearance

The paired `02-shapes-and-dot-calibration` and `04-marker4-highlighter`
fixtures use saved fountain settings `18;0;100;` and stylus input. Their
source and PDF hashes were checked against `conformance/corpus.json`.
The upgraded native saved-redraw oracle also matches all 77 calibration
strokes (3,777 stamps) and 29 marker-test fountain strokes (1,726 stamps)
with identical sample boundaries and zero measured position/radius error.
Their prepared geometry is unchanged by the fixed-width and input-mode
extensions; the appearance measurements remain applicable.
This comparison does not validate V14 appearance: no paired native PDF is
available for the V14 handwriting fixture.

The PDFs embed ink alpha masks at one pixel per source document unit. On
page 1, calibration soft mask xref 6 is 933 by 778 pixels with source origin
(79, 125); marker soft mask xref 13 is 565 by 219 pixels with origin
(86, 151). The calibration mask also contains shapes and a line. Comparing
that entire mask against fountain-only vectors gives a misleading result.
The marker mask contains the 29 fountain strokes separately from highlighters.

The candidate was the production Rust SVG with other page content removed,
cropped to those bounds and rendered by Chromium at the native mask size.
For calibration, measurements use the union of the 77 prepared fountain
bounds, padded by three pixels. Visual inspection confirmed that these
regions exclude the shape outlines. The label and dot-calibration groups
are separated at source y=600.

| Region | Candidate alpha coverage versus native | Ink intersection over union |
| --- | ---: | ---: |
| Calibration labels | -1.879% | 97.093% |
| Calibration dots and writing | -1.348% | 97.801% |
| Marker-test fountain writing | +0.071% | 98.203% |

Coverage is the summed alpha difference divided by native summed alpha;
intersection over union uses alpha greater than 32. All differing pixels
in these regions have partial alpha in at least one image. There are no
fully opaque native pixels that are transparent in the candidate, or the
reverse. The twelve calibration dots individually have 1.15–2.45% less
summed coverage in the candidate.

These observations locate the residual discrepancy at edges; they do not
prove its cause is solely antialiasing, or establish identical appearance
at other scales. They provide no basis for a global width reduction.
The implementation remains vector geometry, with edge sampling performed
by the SVG consumer. Native PDF bitmap masks are comparison evidence only.

## V14 embedded native cache appearance

`handwritten.sdocx` contains `media/0@page_0000000.spi`, a native page cache
with SHA-256 `64327fdf9a1940d1fdd7bfa7ec48c1ddbb2bb262d2653ff7de6f16b1555f660c`.
The hash-verified Samsung Base library decoded its 20-byte header and
453,763-byte data block into a transparent 1848 × 3919 image. Both input
blocks were fully consumed and the native output conversion succeeded.

The existing disposable codec harness needed a larger bounded heap and
instruction allowance for this real page: 512 MiB of mapped heap and
300 million instructions per call. Import-only instruction tracing reduced
overhead; native codec code and tables were unchanged. This decoding is
reference extraction, not a production bitmap rendering path.

The source page is 1848 × 7838, but all its prepared ink lies within the
cache's shorter height. Chromium rendered the production SVG with its paper
rectangle removed at one pixel per document unit, cropped to the cache size.
Native and candidate ink bounds at alpha >32 both equal
(81, 84)–(1824, 2477). Inspection of the first 700 × 700 region confirms
matching handwriting and alignment.

The vector candidate has 0.862% more summed alpha coverage, 94.993% ink
intersection over union at alpha >32, and 8.118% absolute alpha difference
normalized by native summed alpha. Every differing pixel has partial alpha
in at least one image; there are no opaque-to-transparent pixel mismatches
in either direction. This supplies a native V14 appearance reference despite
the missing paired PDF. It does not establish why every partial-alpha value
differs, validate appearance at other scales, or justify a global width
adjustment. Cache freshness is supported by matching content and bounds,
not by a separate export timestamp.

### Controlled shading and edge checks

Making every gradient stop white, with identical circles and blend groups,
increases whole-page alpha coverage error from +0.862% to +6.383% and
absolute alpha difference from 8.118% to 8.598%. Its thresholded ink overlap
is higher (96.622%), illustrating why threshold overlap alone is insufficient
to assess shading. This control does not support removing directional shading.

The first 700 × 700 source-unit crop was also rendered at larger pixel sizes
and averaged in non-overlapping pixel blocks back to native-cache resolution:

| Sampling scale | Summed alpha difference | Absolute alpha error | Ink overlap |
| --- | ---: | ---: | ---: |
| 1× | +2.864% | 6.987% | 96.652% |
| 2×, averaged down | +0.890% | 3.940% | 97.985% |
| 4×, averaged down | -1.187% | 3.134% | 98.078% |

The vector geometry and gradients are identical in all three cases. The
changing coverage and smaller error establish a material sampling effect,
but do not fully explain the residual or prescribe an export scale. These
are diagnostic renders only; production remains vector output. Only one
of the 56,713 native V14 stamps is an exact repeated center/radius/direction
within its stroke, so duplicate removal cannot explain the whole-page gap.

### PDF appearance check

A fresh production export of `handwritten.sdocx` is 42,045,542 bytes,
with one 1386 × 5878.5 point page and no image objects. MuPDF rendered its
first 525 × 525 points at 96 dpi, matching the first 700 × 700 source units
of Chromium's production SVG. Inspection confirmed matching content,
directional shading and alignment. Mean absolute RGB difference across
that crop was 0.905 on the 0–255 scale, including the blank paper.

Color conversion matters when comparing coverage: MuPDF rendered the paper
as (251, 252, 251), while Chromium rendered (252, 252, 252). In the
monochrome source region x=180..480, y=265..350, estimating alpha from each
renderer's own paper and darkest ink gives 6.86% normalized alpha error
against the native cache for PDF and 7.00% for SVG. Their estimated coverage
excesses are 2.13% and 2.29%, respectively. This is a local estimate, not
direct PDF alpha extraction or a whole-document fidelity score. It provides
no evidence of a large appearance change introduced by PDF conversion.
