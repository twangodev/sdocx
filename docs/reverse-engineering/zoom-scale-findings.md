# Zoom scale and contents-view coordinates

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenView.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation).

The Composer zoom callback copies the scroller's total X/Y scales and
integer deltas into the contents view's transform. Its separate
`OnZoomUpdated` notification passes zoom and content scale to the writing
view. That writing-view setter updates cutter and eraser components and
an optional diagram transformer; it does not set the ordinary pen size.

This connects runtime zoom configuration to the previously recovered
[inverse view transform](view-input-transform-findings.md). It does not
establish every ancestor transform, every callback's effects or the entire
upstream pen-setting path. No device fixture or native execution was used.

## Composer registers the callback on a concrete scroller

Composer helper `0x385d20` passes the shared `DeltaZoom` at member 656
to factory `0x395e00` at `0x385d54`. The factory reaches the imported
`ViewZoomScroller` constructor at `0x395f84`; helper `0x38e91c` stores
the resulting shared pointer in Composer member 672 at `0x385d64`.

View `ViewZoomScroller::ViewZoomScroller`, `0x8f688`, installs primary
vtable `0xa2f58` and retains the `DeltaZoom` pointer at member 32.
The relevant primary slots are:

| Byte slot | Relocation | Implementation in View |
| --- | --- | --- |
| 40 | `0xa2f80` | `GetScaleX`, `0x90204` |
| 48 | `0xa2f88` | `GetScaleY`, `0x9020c` |
| 56 | `0xa2f90` | `GetZoomScale`, `0x90214` |
| 120 | `0xa2fd0` | `GetDeltaX`, `0x90f00` |
| 128 | `0xa2fd8` | `GetDeltaY`, `0x90f08` |
| 200 | `0xa3020` | `GetContentScale`, `0x94bfc` |
| 392 | `0xa30e0` | `RegisterUpdatedCallback`, `0x92c68` |

Composer registers a closure through slot 392 at `0x385dc0`.
The closure's primary vtable is `0x567158`; relocation `0x567188`
binds its invocation slot to `0x3960a4`. Closure member 8 supplies
the Composer pointer at `0x3960d4`.

The callback skips its updates when Composer member 744 is null.
Otherwise, its contents-view target is member 528. That member receives
the constructed `ContentsView` in `Composer::initContentsView` at
`0x386194`, following the constructor call at `0x38618c`.

## Total scale includes content scale and axis stretch

Scroller `GetScaleX` and `GetScaleY` delegate to `DeltaZoom` getters
`0x7af78` and `0x7aff0`. They multiply float member 196 by float
member 216 or 220, respectively, at `0x7afd4` and `0x7b04c`.

`DeltaZoom::GetStretchedScaleX`, `0x7b4d0`, reads member 216 at
`0x7b50c`; `GetStretchedScaleY`, `0x7b544`, reads member 220 at
`0x7b580`. The scroller's zoom getter reads `DeltaZoom` member 204
at `0x90250`, while its content-scale getter reads member 200 at
`0x94c38`.

The ordinary public scale updates maintain the combined member 196:

- `DeltaZoom::SetContentScale`, `0x7b068`, stores the content scale
  at `0x7b0f8`, multiplies it by the zoom scale at `0x7b0fc`, and
  stores a changed product at `0x7b118`.
- `DeltaZoom::SetZoomScale`, `0x7b2a4`, clamps a finite requested
  zoom between members 208 and 212 at `0x7b324`–`0x7b338`.
  On its changed-scale path, it stores zoom at `0x7b360`, multiplies
  it by content scale at `0x7b368`, and stores the changed product
  at `0x7b37c`.

Consequently, after these updates, the getters implement float arithmetic
equivalent to:

```text
combined_scale = zoom_scale * content_scale
scale_x = combined_scale * stretched_scale_x
scale_y = combined_scale * stretched_scale_y
```

These are distinct inputs. Treating `GetZoomScale()` alone as the full
view scale would omit content scale and any axis stretch. The internal
`setScale(float)`, `0x78be4`, can also assign member 196 directly; the
equation above describes the recovered public update paths rather than
an invariant proven for arbitrary internal calls.

## The callback sets scale and translation independently

The contents-view portion of callback `0x3960a4` performs:

| Scroller getter call | Contents-view setter call |
| --- | --- |
| `GetScaleX`, `0x3960f4` | `View::SetScaleX`, `0x3960fc` |
| `GetScaleY`, `0x396110` | `View::SetScaleY`, `0x396118` |
| `GetDeltaX`, `0x39612c` | `View::SetTranslationX`, `0x396138` |
| `GetDeltaY`, `0x39614c` | `View::SetTranslationY`, `0x396158` |

The delta results are signed integers converted to floats at `0x396130`
and `0x396150`. These calls use `GetDeltaX/Y`, not the separate
`GetPanX/Y` slots 168 and 176. The callback repeats the four assignments
for the optional view at Composer member 568.

View setters `0x709d8`, `0x70a00`, `0x70a50` and `0x70a78`
delegate to the corresponding `Transform` methods on view member 64
and invalidate byte 428. `View::GetMatrix`, `0x709bc`, obtains that
transform's matrix when byte 310 is true.

The base view constructor initializes byte 310 to true at `0x6f834`,
using an address relative to view member 244. `View::SetMatrix`,
`0x70af8`, instead copies an explicitly supplied matrix to member 152
and sets byte 310 from that matrix's identity test at `0x70b24`–`0x70b2c`.
A nonidentity explicit matrix therefore selects that stored matrix in
`GetMatrix`. The zoom callback itself does not change this selection byte.

Under the transform branch, these fields configure the matrix whose
inverse is applied during child-view input dispatch. An exact conversion
still requires the child's position, pivot, rotation, ancestor transforms
and any explicit matrix selection. Saved stroke replay should continue
to use decoded coordinates without reapplying this editing-time zoom.

## Writing-view scale dispatch reaches the removers

The callback obtains zoom through slot 56 at `0x3961f0` and content
scale through slot 200 at `0x396204`, then calls
`ContentsView::OnZoomUpdated(zoom, content)` at `0x396214`.

`ContentsView::OnZoomUpdated`, `0x41b5dc`, obtains its writing view
from member 1872. It reverses the two arguments when invoking slot 952
at `0x41b638`. Relocation `0x575680` resolves that slot to
`NoteWritingView::SetScale`, `0x4284f0`.

That setter follows members `728 -> 632 -> 872` to the remover action.
Controller initialization constructs this action through `0x51bb14` and
stores it at member 872 at `0x509e8c`. Its primary vtable `0x5849d8`
has RTTI naming `WritingViewRemoverAction`.

The action constructor creates two concrete components:

| Action member | Constructor | Primary vtable | RTTI class |
| --- | --- | --- | --- |
| 392 | `0x51d72c` | `0x584c88` | `WritingCutterRemover` |
| 400 | `0x525f68` | `0x585068` | `WritingEraserRemover` |

`NoteWritingView::SetScale` visits both components and invokes their
slot 56 with `(content_scale, zoom_scale)` at `0x428544`. It repeats
that loop at `0x428588`. Both slot relocations, `0x584cc0` and
`0x5850a0`, resolve to the same implementation, `0x51e91c`.

This implementation stores zoom and content at remover members 232
and 236. Using the preexisting float fields, it computes:

```text
field_220 = (field_252 * field_216) * field_228
field_224 = field_220 * field_212
field_248 = field_224 / zoom_scale
```

The division is at `0x51e938`. The physical meanings of the other
fields and the reason for the repeated loop remain unresolved.
This division belongs to the remover components and does not establish
a division rule for ordinary saved pen width.

The writing-view setter also stores zoom in its own member 1508 at
`0x42859c`. If the optional diagram transformer exists, it passes
`content_scale * zoom_scale` to `StrokeDiagramTransformer::SetScale`
at `0x4285c0`. There is no ordinary pen-size setter in this method.

## Validation and remaining work

The APK digest and both library byte streams were verified. Scroller
construction, callback registration, vtable relocations, view setter
imports, scale arithmetic and remover RTTI were checked against the
ARM64 binaries. Documentation links were checked. No SDK code changed.

The [pen size trace](pen-size-findings.md) now resolves the note-writing
manager's document-relative conversion and native size assignment, plus
the alternative density conversion and the stroke view's separate
recording-pen copy. The action that connects those PenData pointers, other
zoom listeners and specialized input modes remain outside the scope of
this callback trace.
