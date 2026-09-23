# FountainPen V14 saved redraw

> `conformance/fountain_v14_native.py` and `conformance/fountain-v14.json` are
> in this checkout. The independent reconstruction
> `conformance/fountain_v14_model.py` was removed during test cleanup; recover
> it from Git revision `40de721` in a separate checkout. The Rust renderer now ports saved stylus V14 positions, radii and sample
> boundaries through the shared SmPath and prepared-dot pipeline. Directional
> shader coverage remains approximate. Validation is in
> [current validation](../../conformance/README.md#native-geometry-checks).

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

All 2,769 strokes in the dense handwriting fixture run successfully, producing
56,713 stamps. This establishes native output to compare a reconstruction
against; it does not yet establish SDK parity. The checked-in reference has
26 synthetic cases and 1,936 stamps, each storing x, y, radius, tangent x and
tangent y. It includes original-sample boundaries, and rerunning the native
oracle reproduces every value exactly on this host.

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

The oracle currently requires stylus input. Finger/tool-3 substitutions,
shape-specific paths, live event prediction, reused-drawable state and the
remaining historical drawing versions still need separate characterization.
Production V14 rendering remains approximate until the geometry and coverage
port is complete.

## Independent reconstruction

`conformance/fountain_v14_model.py` at Git revision `40de721` reconstructs the
saved variable-width stylus algorithm without loading the APK or executing
native functions. It uses float32 arithmetic, midpoint quadratics, adaptive
SmPath subdivision, distance-to-parameter interpolation, analytic normalized
tangents, distance filtering, alternating short-event filtering, the direction
ratio ring, the pressure/tilt width law, width limiting, residual stamp
spacing, and endpoint handling. Python 3.13 or newer is required for
`math.fma`. The script is not in this checkout. Check out `40de721` separately
and run it from that tree:

```sh
python3 conformance/fountain_v14_model.py
python3 conformance/fountain_v14_model.py --prepared /tmp/handwriting-ink.json --native /tmp/fountain-v14-native.json
```

The measurements below were recorded from that script. The Rust port checks
all 26 synthetic cases for positions, radii and sample boundaries in ordinary
CI, with a 0.0001 page-unit tolerance. It does not yet consume shader tangents.
A real-document check of the Rust port reproduced all 56,713 stamps across
2,769 handwriting strokes with identical positions, radii and sample boundaries.
This includes small negative pressures from legacy saved-channel quantization.

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

The archived model remains a research reference. Production now uses its
saved variable-width stylus geometry for settings `14;`, sharing validation
guards, SmPath and prepared-dot rendering with V16. Fixed-width, non-stylus
and live input paths retain their fallback. Circular coverage still approximates
the native direction-dependent shader; this is geometry parity, not pixel parity.
