# Marker2 V1 and V2 rendering

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenMarker2.so` and `libSPenPenCommon.so` from the APK identified in
the [knowledge base](README.md#sources-and-validation). The
[pen selection trace](pen-selection-findings.md#advanced-settings-select-marker2s-version)
establishes when Marker2 selects GL V1 or GL V2. This document compares their
ordinary stroke paths; StrokeTip and preview drawables remain separate.
The [presentation trace](stroke-prediction-findings.md) subsequently
identifies the V2 StrokeTip drawable as a separate live prediction path.

V2 retains V1's color-alpha composition and circle-mask model. Confirmed
differences include mask-fragment precision and a thinner antialiasing ramp
for drawing sizes below 3. These are shader and instruction-level findings,
not measured visual equivalence with Samsung output.

## V2 keeps the same color-alpha composition

V2's GL constructor stores Marker2Data at member 72 (`0x21f18`) and creates
the V2 render-thread drawable at `0x21f30`. Its redraw path queues the same
PenCommon `PenDrawableRT::SetPenData(float,int)` operation as V1:

| Operation | V1 call site | V2 call site |
| --- | --- | --- |
| Queue size and packed ARGB during start | `0x20f88` | `0x22328` |
| Queue size and packed ARGB during redraw | `0x215ac` | `0x228b0` |
| Draw the stroke mask | `0x23e54` | `0x25e58` |
| Composite the colored mask | `0x23f0c` | `0x25f10` |

Both queue paths reference GOT entry `0x30400` and the argument-copy helper
at `0x21998`. The shared size/color unpacking is recorded in the
[pen opacity findings](pen-opacity-findings.md#marker2-queues-the-packed-color-without-discarding-alpha).
V2's `getAlpha`, `0x262cc`, reads the same normalized color-alpha member 56.

`Marker2StrokeDrawableRTV2::Init`, `0x2517c`, creates the V2 mask shader at
`0x25338` and V2 composite shader at `0x25404`. It configures the same two
blend states as V1:

- Composite state member 224: color function/factors 0, 6, 7 at `0x2547c`;
  alpha function/factors 0, 1, 7 at `0x25490`.
- Mask state member 232: color and alpha function/factors 4, 1, 1 at
  `0x254c4` and `0x254d8`.

The [Renderer enum mapping](pen-opacity-findings.md#marker2-v1-separates-mask-coverage-from-color-composition)
identifies these as source-over composition and maximum mask coverage.
V2 activates the mask state at `0x25f84` and composite state at `0x26110`.
The composite explicitly sets ordinary mode at `0x26164` and supplies
drawable RGBA to the input-color uniform at `0x26174`.

The V1 composite fragment shader at `0x12033` and V2 shader at `0x1266b`
are byte-identical. For sampled mask coverage `m` and color alpha `a`,
their source alpha is `m * a`; the finished color uses the same source-over
equations. Internal mask overlap takes maximum coverage. The later
capture/PDF highlighter-batch blend remains a separate operation.

## Size clamping and stamp geometry

Marker2's pen slot 16 resolves through relocation `0x2ead8` to PenCommon
`Pen::SetSize`, `0x45fdc`. For finite input, that setter clamps size to
0.4–800 before storing it at pen member 24. The minimum is the float
constant at `0x2c1e8`; the maximum is immediate `0x44480000`.

`PenDrawableRT::SetPenData`, `0x4a558`, converts that float to an unsigned
integer with `fcvtzu` at `0x4a588` and stores the result at drawable member
60. For the finite nonnegative sizes from this setter, the conversion
truncates the fractional part. The stored integer participates in both
stamp geometry and V2's edge-smoothing selection.

V2 `Update`, `0x25d04`, converts member 60 back to float and calls
`setRectData` at `0x25d1c`. The rectangle builder, `0x25c58`, uses offsets
`0.5 - size / 2` and `0.5 + size / 2` in each axis and pairs the four corners
with normalized texture coordinates. Its operations match V1's builder at
`0x23c54`. These offsets belong to the native drawable's coordinate space;
they do not by themselves establish a universal half-pixel translation in
exported SVG coordinates.

V2 `AddPoint`, `0x259c8`, appends an X/Y pair to the point buffer. `Update`
uploads that buffer and sets the instance count from its byte length divided
by eight at `0x25d34`–`0x25d48`. The mask's vertex shader combines each point
with the shared rectangle offsets and applies the projection matrix.
V1's vertex shader at `0x11ceb` and V2's at `0x12308` are byte-identical.

The normal mask therefore uses one stamp size for all its point instances.
The object-to-MotionEvent wrappers retain pressure channels, but this
particular mask's per-instance data contains X/Y, not a pressure-derived
radius. A pressure-varying outline cannot be assumed to reproduce Marker2
merely because the stored stroke has pressure samples.

## V2 changes edge smoothing for thin strokes

The mask shaders compute circle coverage from distance to the normalized
center `(0.5, 0.5)`. V1's fragment shader at `0x11e2a` declares `mediump`
precision and uses an antialiasing scale of 1. V2's shader at `0x12447`
declares `highp` and uses uniform `aaScaler`:

```text
distance = length(fragment_coordinate - (0.5, 0.5))
ramp_width = fwidth(distance) * aaScaler
coverage = 1 - clamp((distance - (0.5 - ramp_width)) / ramp_width, 0, 1)
```

The shader writes coverage into the red channel. Its maximum blend state
combines overlapping stamps before the packed color's alpha is applied.

`Marker2MaskShaderV2` binds `aaScaler` at shader member 8 through the string
at `0x114ea` and constructor call `0x2066c`. `drawMask`, `0x25f58`, reads
the unsigned integer drawing size at `0x25fa8`, compares it to 3 at
`0x25fb8`, and selects the uniform at `0x25fc0`:

| Drawing size | V1 scale | V2 scale |
| --- | ---: | ---: |
| Less than 3 | 1 | 0.5 |
| At least 3 | 1 | 1 |

The V2 setter call is `0x25fcc`. The threshold is applied after the size
conversion described above; it is not a comparison against pressure,
stored color alpha or UI size level. At equal local derivatives, the
smaller scale narrows the edge ramp. Actual output still depends on the
projection, rasterization and GPU precision behavior.

## Shared point generation

Disassembly comparison establishes the same operations and control flow for
the corresponding normal draw routines after resolving local branches and
V1/V2-specific call targets. Several routines differ only in log strings or
the queued target class:

| Routine | V1 | V2 |
| --- | --- | --- |
| MotionEvent dispatch | `0x20c7c` | `0x2201c` |
| ObjectStroke redraw wrapper | `0x21c58` | `0x22e14` |
| Point append | `0x239c4` | `0x259c8` |
| Stamp rectangle | `0x23c54` | `0x25c58` |
| Buffer upload | `0x23d00` | `0x25d04` |
| Render-thread mask/composite sequence | `0x23d64` | `0x25d68` |

V2 redraw initializes its previous input point and midpoint from the first
historical X/Y pair at `0x228ec`–`0x22924`, emits the first point at
`0x22928`, and resets the sampling residual from spacing at `0x22938`.
The constructor constant at `0x11bd0` initializes spacing to 1 and the two
distance thresholds to 2 and 20.

`drawLine`, `0x22c10`, rejects motion below its first threshold at
`0x22c78`–`0x22c80`. When its boolean sampling flag is enabled, motion below
the second threshold also passes through an alternating skip branch at
`0x22c84`–`0x22ca4`. Redraw derives that flag from tool type 1, or tool type
2 with source `0x1002`, at `0x2295c`–`0x229d4`.

For accepted motion, the routine starts a quadratic path at the previous
midpoint, uses the previous input point as its control point and ends at
the midpoint between the previous and incoming input (`0x22cdc`–`0x22d08`).
It obtains the path length, samples positions with `SmPath::getPosTan` at
`0x22d58`, appends them at `0x22d68` and carries the unused spacing into the
next segment at `0x22d88`–`0x22da0`. The matching V1 routine at `0x21a54`
has the same operations; its differing literal identifies V1 in the log.

These steps explain why connecting raw input coordinates directly is not
the complete native point-generation model. The subsequent
[sampling and completion trace](marker2-sampling-findings.md) resolves
`SmPath`'s bounded distance approximation and confirms that normal redraw
and end routines add no separate stamp at the final input coordinate.

## The standalone alpha setter has no identified static callers

PenCommon exports `PenDrawableRT::SetAlpha(float)` at `0x4a5a0`, which
overwrites drawable member 56. The existence of that setter previously
left an open question about the ARGB-to-draw path.

An APK-wide scan of all 107 ARM64 libraries found its mangled symbol only
in PenCommon. That library has no relocation to the setter, including
relative-address relocations that could populate a vtable, and its complete
text disassembly has no identified branch or direct address formation to
the setter. The Marker2 V1/V2 queue paths instead set RGBA through
`SetPenData`, and their composites consume that RGBA member.

This provides no evidence of a standalone `SetAlpha` override in the traced
ordinary Marker2 paths. It is a static call-site result, not proof against
dynamic symbol lookup, code outside this APK or other direct/inlined state
changes. It also says nothing about StrokeTip's separately named opacity
operations or higher-level object-alpha composition.

## SDK implications and validation

The shared Marker2 model can retain one color alpha and one stamp size per
stroke, with maximum coverage within its mask and a separate final batch
blend. Renderer version remains necessary for edge smoothing. Coordinate
smoothing, size conversion and mask composition need to be evaluated
together before changing the SDK's visible stroke output.

Shader strings were compared byte for byte, enum assignments and uniform
bindings were checked against instructions/relocations, and paired V1/V2
routines were compared across the above paths. Extracted library bytes and
the APK digest were rechecked. No SDK rendering code changed in this step.

Useful new comparisons include thin Marker2 strokes around drawing size 3,
fractional sizes, one self-crossing stroke, two overlapping strokes and
strokes recorded with different input tools. Their stored name, advanced
settings, width and export scale should be recorded with the PDF. The
[touch-recording trace](stroke-recording-findings.md) distinguishes stored
samples from these stamps and identifies a live/replay source difference.
Upstream event preprocessing, StrokeTip opacity and the remaining brush
plugins are still available for APK-only investigation.
