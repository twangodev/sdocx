# Native pen color, coverage and opacity

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64 libraries
from the APK identified in the [knowledge base](README.md#sources-and-validation).
The inspected copies of `libSPenDrawing.so`, `libSPenView.so`, `libSPenBase.so`,
`libSPenPenCommon.so`, `libSPenRenderer.so`, `libSPenDefaultPen.so` and
`libSPenMarker.so` through `libSPenMarker4.so` match their APK entries.

This investigation follows the fixed-opacity dispatch for five plugins and
the color/coverage path of `Marker2StrokeDrawableRTV1`. It does not establish
identical behavior for every brush, every Marker2 version or every export
mode. No new Samsung document or rendered comparison was used.

## Fixed opacity depends on the pen interface

Drawing `ObjectDrawing::drawObjectStroke`, `0x81a20`, obtains the pen's
morphable interface through virtual slot 288 at `0x81eec`. A null result
branches past its settings at `0x81ef0`. For a nonnull interface, the renderer
reads `ObjectStroke::IsFixedOpacityEnabled` at `0x81f98` and passes the boolean
to sub-interface slot 64 at `0x81fac`.

The concrete vtables resolve as follows. Relocation addresses belong to the
library named in the first column; implementation addresses are local to
that library unless qualified.

| Plugin | Main-interface slot 288 relocation | Result | Fixed-opacity setter |
| --- | --- | --- | --- |
| DefaultPen | `0x1f6c0` | `GetMorphable`, `0x16600`, returns `this + 96` | Sub-interface relocation `0x1f790` resolves to `0x166f0` |
| Marker | `0x4e4c8` | `GetMorphable`, `0x2acc4`, returns `this + 96` | Sub-interface relocation `0x4e598` resolves to `0x2adb4` |
| Marker2 | `0x2ebe8` | PenCommon `Pen::GetMorphable`, `0x46468`, returns null | Drawing skips the interface settings |
| Marker3 | `0x55a78` | Same null-returning base implementation | Drawing skips the interface settings |
| Marker4 | `0x7f1f8` | Same null-returning base implementation | Drawing skips the interface settings |

DefaultPen and Marker's bound `IPenMorphable::SetFixedOpacityEnabled(bool)`
implementations return true without reading the argument or storing state.
Their following getters, `0x166f8` and `0x2adbc`, return false. Thus a
successful setter return does not prove that this setting changes rendering.

The [serialized flag](stroke-metadata-findings.md#properties-and-polarity)
still has a valid storage contract. These bindings do not justify dropping
it, treating it as a global alpha override, or assuming that other plugins
also ignore it.

## Stroke ARGB reaches the pen settings

Drawing reads the stroke's color type at `0x81bcc`, then its pen size at
`0x81be8`. For pens advertising attribute 2, it reads the color at `0x81c1c`,
passes it through context virtual slot 80 at `0x81c34`, and calls pen slot 80
at `0x81c48`. Marker2's `GetPenAttribute`, `0x1f038`, accepts attribute 2.

All five inspected plugin vtables bind pen slot 80 to PenCommon
`Pen::SetColor(int)`, `0x4602c`. The store at `0x46034` retains all 32 bits at
pen member 28. `Pen::getSettingData`, `0x464a0`, returns `this + 24`, so the
packed color occupies settings member 4.

The context conversion preserves alpha in the inspected light, dark and
high-contrast themes:

- Drawing supplies theme selector 3 at `0x81c30`. View `Context::GetColor`,
  `0x6c39c`, chooses the context's active converter at color member 8.
- View `Color::SetColorTheme`, `0x8aef0`, constructs LightColorTheme for 0,
  DarkColorTheme for 1 and HighContrastColorTheme for 2; it assigns the
  converter at `0x8af84`.
- Base `LightColorTheme::GetColor`, `0xe54f8`, and
  `HighContrastColorTheme::GetColor`, `0xe54b4`, return the input unchanged.
- Base `DarkColorTheme::GetColor`, `0xe51a4`, saves the original ARGB value,
  converts an opaque copy and replaces only its low 24 bits at `0xe51bc`.
  The original alpha byte survives the RGB conversion.

This addresses the ordinary color-conversion call. Drawing also contains
separate reveal/effect branches; their resulting color overrides require
their own traces.

## Marker2 queues the packed color without discarding alpha

Marker2's constructor obtains `Pen::getSettingData` at `0x1ed44` and stores
the pointer in Marker2Data member 0 at `0x1ed4c`. Its GL V1 constructor keeps
that data pointer at member 72 (`0x20b78`) and its render-thread drawable at
member 16 (`0x20b94`).

The object-stroke redraw wrapper, `0x21c58`, constructs a MotionEvent from
the stored channels and dispatches through slot 144 at `0x21d1c`. Relocation
`0x2ee38` binds that slot to `RedrawPen(MotionEvent const*, RectF*)`, `0x21504`.
The method queues `PenDrawableRT::SetPenData(float,int)` at `0x215ac`, using
settings member 0 for size and member 4 for packed color. GOT relocation
`0x30400` identifies the member function. The queue helper at `0x21998`
copies both arguments into the task; its executor at `0x21dec` restores the
float and integer before calling the saved member function.

PenCommon `PenDrawableRT::SetPenData`, `0x4a558`, unpacks the integer into
normalized RGBA at drawable members 44, 48, 52 and 56. The shift table at
`0x2c3b0` contains -16 and -8 for red and green; the remaining lanes extract
blue and the high-byte alpha. All four channels are divided by 255 at
`0x4a594`. `Marker2StrokeDrawableRTV1::getAlpha`, `0x246a0`, reads member 56.

Marker2's constructor initially sets color `0x7f000000` at `0x1ed98`–`0x1ed9c`.
That is a plugin construction default. The later object-drawing color setter
means it cannot be substituted for a missing serialized field or treated as
a mandatory opacity for every saved Marker2 stroke.

## Marker2 V1 separates mask coverage from color composition

`Marker2StrokeDrawableRTV1::Init`, `0x23178`, creates a mask shader and a
composite shader. Its blend-state setup uses two different equations:

| Pass | State member | Native color settings | Native alpha settings |
| --- | ---: | --- | --- |
| Stroke mask | 232 | Function 4, factors 1 and 1 | Function 4, factors 1 and 1 |
| Color composite | 224 | Function 0, factors 6 and 7 | Function 0, factors 1 and 7 |

The composite settings are assigned at `0x23478` and `0x2348c`; the mask
settings at `0x234c0` and `0x234d4`. Renderer
`GLES::BlendStateObject::Activate`, `0x514cc`, converts these enums through
the tables at `0x308b8` and `0x30880`, then calls
`glBlendEquationSeparate` at `0x514f8` and `glBlendFuncSeparate` at `0x5151c`:

| Native enum | OpenGL value | Meaning |
| --- | --- | --- |
| Function 0 | `0x8006` | `GL_FUNC_ADD` |
| Function 4 | `0x8008` | `GL_MAX` |
| Factor 1 | `0x0001` | `GL_ONE` |
| Factor 6 | `0x0302` | `GL_SRC_ALPHA` |
| Factor 7 | `0x0303` | `GL_ONE_MINUS_SRC_ALPHA` |

`drawMask`, `0x24050`, activates member 232 at `0x2406c`. Its fragment shader,
`Marker2MaskShader::szFragmentShader` at `0x11e2a`, computes antialiased circle
coverage from distance and the fragment derivative. It writes coverage into
the red channel. The maximum blend equation retains the greatest coverage
where primitives overlap in this mask, rather than adding alpha per stamp.

`Draw`, `0x23d64`, calls `drawMask` at `0x23e54`, updates the quad and then
calls `drawComposite` at `0x23f0c`. The composite activates member 224 at
`0x241b4`, selects ordinary drawing mode with `uIsEraserMode = 0` at
`0x24208`, and supplies drawable RGBA from member 44 to `inputColor` at
`0x24218`. The shader constructor at `0x1fa6c` binds those named uniforms;
the input-color uniform occupies shader member 16.

The ordinary branch of `Marker2CompositeShader::szFragmentShader`,
`0x12033`, outputs straight RGB and the product of mask coverage and color
alpha. For sampled mask coverage `m`, color alpha `a`, straight source RGB
`Cs` and destination premultiplied color `Cd`/alpha `Ad`, the shader and
blend state together establish:

```text
As = m * a
Cout = Cs * As + Cd * (1 - As)
Aout = As + Ad * (1 - As)
```

Zero-coverage fragments are discarded. Overlapping primitives in one mask
use `m = max(coverage_i)`. This does not establish maximum blending between
separate strokes or between the finished highlighter batch and the page.

The shader's separate `Alpha` uniform participates in its eraser branch;
the ordinary branch uses `inputColor.a`. The StrokeTip composite shader at
`0x12fe8` has another `inputOpacity` multiplier and emits premultiplied RGB.
Those shader differences are further reason to scope the V1 result narrowly.

## SDK implications and remaining work

The SDK preserves complete ARGB in stroke metadata, while ordinary `Stroke`
decoding currently retains RGB. A future semantic rendering change needs
both alpha preservation and the right scope for coverage/composition.
Giving each overlapping stamp an independent opacity would differ from the
inspected maximum-coverage mask. Applying opacity after constructing one
stroke mask is a closer model for this specific path.

The [capture compositor](capture-composition-findings.md) and
[Standard PDF writer](standard-pdf-composition-findings.md) apply their own
blend operation to the resulting highlighter batch. That later Darken or
Lighten operation is separate from the pen's mask and alpha calculation.

The [pen selection trace](pen-selection-findings.md) resolves stored name
and setting IDs and Marker2's choice between GL V1 and V2. The subsequent
[V1/V2 comparison](marker2-rendering-findings.md) confirms shared color-alpha
composition and identifies V2's thin-stroke smoothing change. Its static
call-site audit finds no identified callers of `PenDrawableRT::SetAlpha`
inside the APK. Other brush plugins, StrokeTip opacity, reveal/effect
overrides and dynamic state changes remain outside these conclusions.

Useful new pairs are one self-crossing highlighter stroke, two separate
overlapping strokes of the same color, and strokes crossing text or images,
with pen identity and export mode recorded. These distinguish internal mask
coverage, between-stroke composition and final page blending. The present
findings are static contracts; visual equivalence remains unmeasured.
