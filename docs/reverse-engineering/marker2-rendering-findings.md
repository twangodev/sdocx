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

A bounded host-Mesa replay now pairs V2 widths 1–3 with current SDK SVG/PDF
exports. All 22 sampled X/Y pairs match bitwise, including sample ends
`[1, 5, 14, 22]`; the PDFs contain no image objects. Native final color and
SDK output still differ at thin edges: support IoU ranges from 0.907 to 1.0
across resvg, MuPDF and PDFium. This uses synthetic saved-stroke setters, an
unchanged native point stream copied into the render-thread buffer, and
opaque color only. It does not establish archive loading, partial-alpha or
Samsung device appearance parity.

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
`-(size / 2 + 0.5)` and `size / 2 + 0.5` in each axis and pairs the four
corners with normalized texture coordinates. In particular, the `FNMADD`
instruction at `0x25ca4` negates the whole multiply-add; it does not compute
`0.5 - size / 2`. An isolated execution of its instruction word `0x1f210403`
with size 1, 2 and 3 yields lower bounds -1, -1.5 and -2. The resulting
quad is centered on the supplied point and has half-extent `(size + 1) / 2`.
Its operations match V1's builder at `0x23c54`. Bounded native RTV2 captures
confirm the final 64-byte `aRectPoint` buffer uses half-extents 1, 1.5 and 2
for widths 1, 2 and 3, with normalized UVs and unchanged mask pixels.

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

For the original canonical host-Mesa widths 1–3 replay above, computing normalized
radial distance at pixel centers, taking finite differences within even
2×2 window-pixel pairs, quantizing each stamp and applying MAX reproduces
the captured red coverage plane byte-for-byte. The model uses source-derived
centers without fitting pixel values. It does not predict the readback alpha
channel or establish Samsung-device derivative behavior. A continuous
angular derivative or constant radial feather does not reproduce these
fields exactly; this distinction matters when approximating them in vectors.
Further controls at translation `(0.25, 0.5)` and canonical ARGB alpha 128
match five red planes exactly, with one width-3 translated pixel differing by
one byte level. Typed composite color uniforms and output alpha confirm that
partial ARGB alpha is applied after MAX coverage; all three alpha-128 red
planes remain identical to their opaque canonical controls.

A later 12-case host-Mesa RTV2 matrix preserves the 22-sample saved-redraw
vector at widths 1–3, two fractional caller-matrix phases and ARGB alpha
255/128. The finite 2×2 model matches every red mask plane and composite
alpha byte for byte, with packed alpha applied after MAX. Continuous-derivative
vector shells can reduce intensity error but lose support overlap in resvg and
MuPDF, so they are not used by the Rust renderer. These captures explicitly
bridge the cleared RT+88 render vector; they do not establish native ownership
transfer, archive or Samsung-device parity.

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
the complete native point-generation model. The
[sampling and completion trace](marker2-sampling-findings.md) resolves
`SmPath`'s bounded distance approximation and confirms that normal redraw
and end routines add no separate stamp at the final input coordinate.

## The standalone alpha setter has no identified static callers

PenCommon exports `PenDrawableRT::SetAlpha(float)` at `0x4a5a0`, which
overwrites drawable member 56.

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

A bounded native RTV2/Mesa llvmpipe mask capture confirms the thin-edge
branch for integer widths 1, 2 and 3. With one explicit stamp at
`(128.25,128.5)` in a 256×256 subbitmap, caller identity produces the
subbitmap projection; `Draw` multiplies that projection by the caller matrix.
On pixel row 128, the nonzero red mask values are `85,215` at x=127–128
for width 1, `255,255,94` at x=127–129 for width 2, and `50,212,251,142`
at x=126–129 for width 3. The native `aaScaler` uniform is respectively
`0.5,0.5,1`. The capture checks the active instance buffer, projection,
framebuffer and shader bindings, with no GL errors. Its instance vector is
explicit host input replacing the saved caller's vector; these are mask
controls, not full-page or Samsung device appearance comparisons.

`ink/marker2.rs` reconstructs that saved model for V1 and V2: one stamp
radius, one path opacity, and midpoint sampling. Overlapping stamps in one
stroke are a single filled path, so coverage is a union rather than stacked
transparency. Root strokes selected into the Top pass are painted after the
Base pass and before the Masking pass. Their shared group uses Darken on light
paper and Lighten on dark paper; see [composition](../svg-rendering.md) and
[theme policy](../render-themes.md). The V2 thin-stroke edge ramp is still not
ported; the current hard-circle radius also omits the quad's additional
half-unit before shader coverage. `Marker` and `Marker3` retain the pressure
approximation; supported
Marker4 settings use separate [V7](marker4-v7.md) and
[V8](marker4-rendering-findings.md) vector stamp paths.

Shader strings were compared byte for byte, enum assignments and uniform
bindings were checked against instructions/relocations, and paired V1/V2
routines were compared across the above paths. Extracted library bytes and
the APK digest were rechecked. The geometry port is checked against the
synthetic chord in the sampling note and against a Darken raster of cyan over
red. It is not a Samsung PDF comparison.

The [touch-recording trace](stroke-recording-findings.md) distinguishes stored
samples from stamps and identifies a live/replay source difference. The
geometry checks do not establish device appearance for fractional widths,
thin V2 edges, self-crossings or every input tool.

An explicit empty setting is stateful: a fresh Marker2 manager selects V2,
while a reused manager can retain V1. In two synthetic fresh-manager host-Mesa
controls, empty and `2;` produced identical geometry, mask and composite
frames. Reusing the `2;` approximation reduced RGB error against native from
9637/9576 to 2269/2711 and from 21491/21188 to 2777/2788 in SVG/PDF. A third
2× control improved from 56634/56311 to 18457/18607. These host comparisons do
not establish cached V1 equivalence, device appearance or saved-document manager
history, so empty settings remain on the generic route.

Native capture runners and receipts are retained under
`tmp/vector-parity-plan/marker2-{empty,explicit}-paint*`; public export and
comparison scripts are in `tmp/vector-parity-plan/marker2-empty-exports/`.
Runner manifests were repinned after capture to a retained pre-update witness;
the receipts preserve capture-time hashes. Host adapters materialize native
RT vectors, and the synthetic source is not archive deserialization.
