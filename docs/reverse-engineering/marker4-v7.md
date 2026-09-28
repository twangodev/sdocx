# Marker4 V7 vector reconstruction

The available corpus contains six Marker4 `7;` strokes in `quiz.sdocx`, and
no InkPen/InkPen2 strokes. V7 previously used the generic pressure fallback:
thin round-ended segments, capped width, and lost saved alpha. It now shares
the Marker4 midpoint sampler and rounded-rectangle vector renderer, with its
own fractional tip dimensions. SVG, PDF and replay use the same Rust geometry.

## Native contract

Samsung Notes 4.4.45.37 ARM64 `libSPenMarker4.so` SHA-256:
`23b29d7baa2a766909d942dffb50ce4191790d5ed3537648fc0120cdb75fddff`.

- GLV7 `redraw` (`0x52a80`) reads saved points, processes interior samples,
  then calls `endPen` (`0x5177c`). Required pressure/time channels are still
  read to construct the endpoint event, although stylus geometry ignores them.
- `drawLine` (`0x52348`) measures midpoint quadratics with native SmPath.
  At unit inverse scale, stamp spacing is 1 and minimum movement is 2.
  `endPen` finishes the terminal quadratic and emits the endpoint, or the
  previous point if nothing has been emitted. Stylus stamps have angle zero.
- `drawPoint` (`0x52e30`) clamps half the fractional pen width to radius 1.
  RTV3 `AddPoint` (`0x5c0fc`) expands this radius by 0.5 in both dimensions;
  horizontal extents are multiplied by 0.77. The tip is centred on the point.
  V8 instead truncates pen size and offsets its tip vertically by 0.5.
- RTV3 `CreatePenCanvas` (`0x5bd80`) draws a rounded rectangle from 1 to 199
  in a 200-pixel mask, with corner radii 50. V7 vectors retain these proportions.
  ARGB alpha is applied once to the stamp union, within the existing Darken batch.

The supported V7 path requires stylus tool 2 and widths 0.4–800. Shape,
fixed-width, eraser and rainbow settings remain excluded. Native texture
filtering and scale-dependent coverage are approximated by vector outlines;
the public support classification therefore remains `Approximate`.

## Validation on 2026-09-26

`conformance/marker4_native.py` executes native saved redraw, SmPath and stamp
submission. Object initialization at unit inverse scale and external channel
access are host supplied. Native `MakePointRect` supplies independent tip bounds.
The oracle checks library hashes and leaves the null page unmapped.

The 36 committed synthetic cases contain 510 native stamps. Rust checks their
centres, sample boundaries, angle and tip dimensions. Six real V7 strokes
contain 18,660 stamps: all centres and sample boundaries match native execution
with zero measured error. A moved-stamp negative control must fail the oracle.
Rendering tests cover V7 and V8 width and overlapping-stamp opacity, rejecting
SVG images/filters. The Chromium regression also checks the six V7 strokes
through WASM, identical export/replay paths and a visible translucent sample.

## Native appearance comparison

Source `quiz.sdocx` SHA-256:
`38fd0ef0729d3a113e1c14bcc10557dcc263e5a3582fd80a3cf99c8c2c4ad40a`.
Its upper-layer cache `media/14@page_1000000.spi`, SHA-256
`6a3d2dbd3c6fe5d3da0e7e686252c7ef8dfb75f2df32f2cbe21b1037e73b3ada`,
decodes through the native Base codec to 1812 × 3843 RGBA pixels. The ordinary
ink cache is separate and contains no highlighter colour in this region.

The comparison isolates the production SVG's Darken highlighter group over
white, rendered by Chromium at one pixel per document unit. The native upper
layer is also composited over white. Region `(120,1740)–(710,1920)` contains
two highlights; RGB chroma greater than 35 selects coloured ink. Both versions
use the same prepared bounds. The fallback control routes only V7 strokes
through the unchanged generic renderer; it is not a separately archived build.

| Measurement | Generic fallback | V7 vectors |
| --- | ---: | ---: |
| Missing native ink, one-pixel tolerance | 81.572% | 0% |
| Extra ink, one-pixel tolerance | 0% | 0% |
| Mean RGB error in shared ink, 0–255 | 50.418 | 0.289 |

Exact thresholded ink overlap for V7 is 98.412%, with 38,988 candidate pixels
versus 38,377 native pixels. This is close appearance for these two highlights,
not pixel identity or validation at every zoom. Cache freshness is supported
by matching content, not a separate export timestamp. A production PDF of the
same cropped page contains zero image XObjects.

With decoded native and Chromium candidate images at the same scale:

```sh
uv run --project conformance --locked python conformance/ink_visual.py \
  geometry.json native-top.png candidate-highlighters.png \
  --profile Marker4 --settings '7;' --region 120 1740 710 1920 --chroma-threshold 35
```
