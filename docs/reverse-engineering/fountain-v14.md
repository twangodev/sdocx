# FountainPen V14 saved redraw

> `conformance/fountain_v14_native.py` and `conformance/fountain-v14.json` are
> in this checkout. Rust reconstructs saved V14 geometry through the shared
> SmPath and prepared-dot pipeline.
> [Vector parity](fountain-parity.md) records directional shading and appearance
> limits; [current validation](../../conformance/README.md#native-geometry-checks)
> describes the retained native checks.

The existing Samsung Notes 4.4.45.37 APK includes GLV14, selected by saved
settings `14;`. No additional APK is needed for this version. This is a
different drawing implementation from GLV16 (`18;0;100;`), not a settings alias.

`conformance/fountain_v14_native.py` executes the complete native
`redraw(ObjectStroke, RectF, bool)` at `0x72c20`. The native loop calls its own
drawLine, endPen, drawPoint and SmPath implementations. Host stubs supply
ObjectStroke channel pointers and endpoint MotionEvent access; a collector
replaces only RTV4::AddPoint. Native drawPoint therefore handles the radius
floor and zero-direction fallback itself. The oracle initializes a fresh
drawable per stroke, canonical inverse scale 1, and the saved tolerance.
Reuse of an already populated drawable is not covered by this initialization.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v14_native.py
cargo run --offline -p sdocx --features render,serde --example ink_geometry -- tmp/stroke-conformance/handwritten.sdocx > /tmp/handwriting-ink.json
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v14_native.py --prepared /tmp/handwriting-ink.json --output /tmp/fountain-v14-native.json
```

The checked-in reference has 68 synthetic cases and 4,062 stamps, storing
x, y, radius, tangent x, tangent y and original-sample boundaries. The Rust
geometry regression checks all five attributes and boundaries. The dense
handwriting fixture contains 2,769 strokes and 56,713 stamps; the measured
native comparisons are recorded in [vector parity](fountain-parity.md).

## Control flow differences from V16

- The saved outer entry at `0x727dc` reads `GetInitialToloerance` at `0x72a74`
  into the drawable's distance threshold. It does not use V16 PenTolerance's
  farthest-dropped-event state or direction-aware acceptance.
- The saved loop includes every sample after the first, including the final
  sample, before calling endPen. V16 excludes that final sample from drawLine.
- V14 has no width-history collection/smoothing pass. Its width limiter feeds
  interpolation directly, retaining the three-entry direction ratio ring.
- Tool 2 uses saved pressure capped at one, optional saved tilt, and a
  five-unit alternating short-movement threshold. The initial width is
  half pen size times the first capped pressure.
- Initial time is loaded from the last sample (`0x72d58`), unlike V16's saved
  initialization. The oracle runs that native behavior rather than correcting
  what might look like a mistake.
- SmPath receives a tangent output pointer. drawPoint forwards that tangent
  to RTV4's direction-dependent shader. A zero tangent becomes `(0, 1)`.

The native oracle covers saved input modes 1, 2 and 3, including fixed width.
Shape-specific paths, live event prediction, reused-drawable state and other
historical drawing versions are outside these geometry captures.

## Archived reconstruction measurements

`conformance/fountain_v14_model.py` at Git revision `40de721` reconstructs the
saved variable-width stylus algorithm without loading the APK or executing
native functions. It uses float32 arithmetic, midpoint quadratics, adaptive
SmPath subdivision, distance-to-parameter interpolation, analytic normalized
tangents, distance filtering, alternating short-event filtering, the direction
ratio ring, the pressure/tilt width law, width limiting, residual stamp
spacing and endpoint handling. This archived research script is not part of
the implementation or current validation suite.

The measurements below were recorded from that archived script's original
26-case stylus subset. They are distinct from the current Rust/native fixture
coverage above and include small negative pressures from legacy saved-channel
quantization.

The 26 synthetic cases reproduce all 1,936 stamps and original-sample
boundaries. Maximum coordinate error is 0.00001526; maximum tangent-component
error is 0.00000620. The dense handwriting comparison reproduces all 56,713
stamps across 2,769 strokes, with identical radii and sample boundaries.
Maximum coordinate error is 0.000244141 page units (at most two float32 ULPs
on that fixture); maximum tangent-component error is 0.000001267.

The verifier checks coordinate differences against the larger of 0.0001 and
two float32 ULPs, and radius/tangent differences against 0.0001. The small
coordinate/tangent rounding differences are retained as measured limitations;
this is not a bit-exact claim for SmPath. Zero-length quadratics also differ
from residual-overrun rejection: the former retains the newly computed
midpoint, while the latter restores the previous midpoint. The independent
model follows that distinction.

Production uses the Rust saved-stroke reconstruction for settings `14;`,
sharing validation guards, SmPath and prepared-dot rendering with V16. Saved
fixed width and input modes 1, 2 and 3 are supported. Geometry parity does not
establish native pixel identity or live-input behavior.
