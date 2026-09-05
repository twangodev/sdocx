# Optional stroke finalization and coordinate replacement

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenDrawing.so`, `libSPenModel.so`,
`libSPenMarker2.so` and `libSPenPenCommon.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation).

The [presenter trace](stroke-prediction-findings.md) identified a later
`TouchStrokeDrawing::TransformStroke` call. Its configured implementation
is optional post-stroke processing, not a general conversion from view
coordinates into document coordinates. The ordinary stroke-view constructor
selects no transformer. Other factory selections can change stored color
or replace stored coordinates after event recording.

These findings recover selection, inputs and model mutation. They do not
establish that a device enables the smoother or fully reproduce its spline
solver. The separate [insertion trace](stroke-insertion-findings.md) resolves
page selection and translation after this stage.

## Construction selects no transformer

`LowLatencyStrokeView` installs its primary vtable at `0x4d1efc`. After
creating the presenter, its constructor calls slot 304 with integer
arguments 0 and 30 at `0x4d1fb4`–`0x4d1fc4`.

Slot 304, relocation `0x580da8`, resolves to `0x4d48d0`. This setter passes
the two integers to factory `0x4db914`, replaces stroke-view member 288,
destroys the previous transformer if present, and assigns the new pointer
to presenter member 408 at `0x4d4910`.

The signature string at `0x1f4a92` identifies the factory as
`StrokeTransformerFactory::CreateStrokeTransformer(TochUpStrokeTransformType, int)`.
The spelling `TochUp` is present in the binary. Its dispatch is:

| First argument | Result | Evidence |
| --- | --- | --- |
| 0 | Null | `0x4db92c`, `0x4db9a8` |
| 1 | `TestStrokeTransformer` | `0x4db938`–`0x4db950` |
| 2 | `CSAPSStrokeTransformer`, member-12 flag false | `0x4db9b0`–`0x4db9c4` |
| 3 | `CSAPSStrokeTransformer`, member-12 flag true | `0x4db96c`–`0x4db980` |
| Other values | Log an error and return null | `0x4db988`–`0x4db9a8` |

Thus the constructor's second argument 30 does not activate smoothing:
the first argument 0 selects null. Later runtime selection remains a
separate question; the factory's presence is not evidence of its use in a
saved document.

## Finalization dispatch operates on the recorded object

The presenter checks member 408 before calls to helper `0x4d93a4` at
`0x4d8580` and `0x4d8780`. In the latter branch it first classifies action
1 or 3 as closed using `(action & ~2) == 1` at `0x4d8758`–`0x4d8768`.
The helper configures the drawing canvas, then passes the transformer
and an output rectangle to Drawing `TouchStrokeDrawing::TransformStroke`
at `0x4d9404`.

Drawing implementation `0xb8258` retrieves the ordinary pen drawable and
calls its slot 32, `GetCanvasBitmapType`. A result of 2 returns false
before transformer dispatch at `0xb82b0`–`0xb82b4`. Ordinary Marker2 V1/V2
bind this slot through relocations `0x2edc8` and `0x2ef08` to PenCommon
`PenStrokeDrawableGL::GetCanvasBitmapType`, `0x51f38`, which returns 1.
Marker2 therefore passes this particular gate.

The call at `0xb82cc` invokes transformer slot 16 with:

- the recorder's current `ObjectStroke*`, member 32;
- the recorder's stroke rectangle, member 40.

After a successful return, Drawing can redraw the current stored object
through the ordinary drawable's secondary interface at `0xb837c`, then
union the resulting bounds at `0xb83a0`. This happens after the
[event append path](stroke-recording-findings.md#drawing-and-recording-are-separate-operations).
It is distinct from the drawable's optional coordinate provider and from
the separate prediction drawable.

## The test implementation changes color

Factory type 1 loads GOT entry `0x5a2c00`, which resolves to vtable
`0x581350`. RTTI identifies `TestStrokeTransformer`; its primary slot 16
at `0x581370` resolves to `0x4ec23c`.

For a nonnull object it calls `ObjectStroke::SetColor(0xff0000ff)` at
`0x4ec25c`, optionally obtains its drawn rectangle, and returns true.
It does not convert the object's coordinate system. This implementation
also demonstrates why the interface name alone cannot establish the
meaning of a particular transform.

## CSAPS prepares separate time and coordinate vectors

Factory types 2 and 3 call constructor `0x4dba78`. Its GOT entry
`0x5a2bf8` resolves to vtable `0x581320`, whose RTTI identifies
`CSAPSStrokeTransformer`. The primary slot-16 relocation at `0x581340`
resolves to `0x4dbbb0`. A signature string at `0x1c5648` independently
identifies this method as `CSAPSStrokeTransformer::Transform(ObjectStroke*, RectF*)`.

For each stored point, the method reads:

| Input | Call site | Conversion |
| --- | --- | --- |
| Point count | `0x4dbc78` | Loop bound |
| X and Y | `0x4dbc88`, `0x4dbc90` | Stored float coordinates promoted to double vectors |
| Timestamp | `0x4dbc98`, `0x4dbca0` | Signed 32-bit integer converted to float |

It constructs a separate time vector. Starting with a previous value of
`-1.0f`, it retains a timestamp whose float difference from the previous
value is positive. Otherwise it substitutes `previous + 0.01f` at
`0x4dbccc`. The chosen float is then promoted to double for the temporary
vector, and becomes the previous value at `0x4dc070`.

This is not a rewrite of stored timestamps. It is also not an unconditional
guarantee of strict monotonicity: float rounding can make `previous + 0.01f`
equal to `previous`. For example, the reconstructed arithmetic produces
the same value for both entries of `[100000000, 100000000]`.

## Parameter mapping and weighting are separate stages

The second factory argument is an integer parameter passed to the CSAPS
constructor. The constructor maps it to a stored float, called `mSmooth`
in its log string. For integer `k`, the recovered arithmetic is:

```text
k < 1:    mSmooth = 1
k > 99:   mSmooth = 0
otherwise:
    mSmooth = 1 - expf(-5.841355800628662f * powf(1 - k / 100.0f, 0.6666666865348816f))
```

The division, addition, multiplication and subtraction use float
arithmetic; the constructor implements the inner subtraction as division
by `-100.0f` followed by addition to 1. The constants at `0x1f9198` and
`0x1f9120`, and calls at `0x4dbad4` and `0x4dbae4`, establish the mapping.
The setter at `0x4dbb34` repeats it. Parameter 30 gives approximately
`0.9900000095`, while parameter 99 gives approximately `0.2374837995`.
No application UI meaning for this integer is established here.

The member-12 flag selects the temporary weight vector:

- Type 2 fills weights with `1 / point_count`, then sets the first and last
  weights to 1 at `0x4dc1b8`–`0x4dc1bc`.
- Type 3 uses squared coordinate distances at `0x4dc0e4`–`0x4dc118`.
  For at least three points and positive accumulated distance, each
  interior weight uses the preceding segment's squared length divided by
  the sum over those segments. A zero-length segment inherits the preceding
  weight at `0x4dc1f4`. Endpoint weights are 1. The final segment is outside
  this accumulation loop. Zero accumulated distance takes a logging branch
  before that normalization.

These describe the traced nonempty input branches, not a safe input
contract for importing arbitrary empty or degenerate strokes.

Helper `0x4e1264` then adjusts `mSmooth` using the temporary time and weight
vectors. Its result is converted to float before being promoted to double
at `0x4dc25c`. Two calls to `0x4e145c` build separate X and Y processing
objects; two calls to `0x4e3dc4` evaluate them using the temporary times.
The full solver and all degenerate-input behavior remain unresolved.

## Coordinate replacement preserves the recorded count and channels

After evaluating the two coordinate components, `0x4dc360`–`0x4dc46c`
builds one output point per original stored point. If both evaluated arrays
contain the corresponding index, their doubles are converted to floats at
`0x4dc3a8`–`0x4dc3b4`; otherwise the original X/Y pair is retained. The
method passes this vector to `ObjectStroke::ReplacePoint` at `0x4dc480`.

The Model overload at `0x2dfa20` forwards to `0x2df5f8` with its additional
boolean argument false. This implementation requires the supplied count
to equal the object's existing count at `0x2df668`–`0x2df670`. It reaches
`ObjectStrokeImpl::ReplacePoint`, `0x2e9cec`, through either its history
or direct path.

The implementation replaces the coordinate vector at member 48 and marks
the object and drawing caches dirty at `0x2e9d38`–`0x2e9d50`. It does not
replace the parallel timestamp, pressure, tilt or orientation vectors, or
assign a new logical point count. The wrapper refreshes geometry bounds.

Consequently this path can produce saved X/Y coordinates that differ from
the recorded event coordinates while preserving sample count and recorded
parallel channels. It does not append a predicted endpoint, resample the
stroke to a new count, or prove that the smoother was selected for any
particular document.

## Validation and SDK implications

The APK digest, five library byte streams, factory and vtable bindings,
RTTI, imported bitmap-type methods, parameter constants and replacement
count checks were verified. Disposable arithmetic reconstruction checked
the parameter boundaries and timestamp rounding examples. These are
static results, not execution of Samsung's native smoother or a visual
comparison against a new device fixture. No SDK code changed.

Keep the saved point arrays authoritative during document replay. A live
input filter or optional finalizer may already have changed them; rerunning
that processing would apply it twice. Ordinary pen replay, including its
own mask sampling, remains a separate step.

The [insertion trace](stroke-insertion-findings.md) establishes a later
translation into the selected page for page mode 0. The input transform
before recording remains unresolved, so final saved coordinates still
cannot be equated with the original screen-space event values.
