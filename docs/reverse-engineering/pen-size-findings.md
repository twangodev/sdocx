# Pen size settings and document-relative width

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenEngine.so`, `libSPenPenCommon.so`, `libSPenMarker2.so`,
`libSPenComposer.so` and `libSPenDrawing.so`, together with Java
decompilation from the APK identified in the
[knowledge base](README.md#sources-and-validation).

The note-writing manager has separate document-relative and density-based
size-level conversions. On the document-relative branch, the native
utility scales the selected pen's width range by the document's shorter
dimension and interpolates using the requested size level. The density
branch interpolates the pen's separate DP bounds and multiplies the result
by `densityDpi / 160`. The native setting bridge passes the resulting
`size` float directly to the pen's size setter.

On the ordinary raster path, the pen action assigns ViewCore's selected
PenData to the stroke view during down-event initialization. The
low-latency recorder then has a separate, verified copy of pen size from
that input PenData to its recording PenData. These findings do not
establish every application setting or drawing path, or justify
recomputing a saved stroke's width from a UI size level.

## The note-writing manager chooses the conversion

In `com.samsung.android.sdk.composer.writing.SpenNoteWritingViewManager`,
`setPenSettingInfo` returns when the supplied setting or its view core
is null. Its conversion requires both a nonzero `sizeLevel` and an
attached document:

| Condition | Assignment to the supplied setting's `size` |
| --- | --- |
| `isDpSize == false` | `SpenPenUtil.convertSizeLevelToPxSize(name, sizeLevel, document.getWidth(), document.getHeight())` |
| `isDpSize == true`, with a context | `SpenPenUtil.convertSizeLevelToDpSize(context, name, sizeLevel)` |
| No conversion condition applies | Existing `size` is retained |

It then copies the settings into its local pen information and passes the
supplied setting object and display to `SpenViewCore.setPenSettingInfo`.
The Java view-core wrapper calls `Native_setPenSettingInfo` when its
native handle is nonzero and the display is nonnull.

The more general `com.samsung.android.sdk.pen.engine.writingview.SpenWritingViewImpl`
also converts a nonzero size level when a document is attached, using
that document's width and height. Its recovered method uses the pixel
conversion without the note-writing manager's `isDpSize` branch.

These branches were checked in fresh fallback decompilation of both
classes from the identified APK. Neither supplies the current zoom
factor to the document-relative conversion. The density-based branch
uses the application context's display metrics, as traced below.

## Pixel conversion uses the shorter document dimension

Engine `EngineUtilGlue::Native_convertSizeLevelToPxSize`, `0xc2540`,
constructs a native pen-name string. On success it passes that string,
size level, canvas width and canvas height to
`PenUtil::ConvertSizeLevelToPxSize` at `0xc25a8`. Failed string
construction returns zero.

The PenCommon implementation, `0x53cd8`, selects the smaller signed
dimension at `0x53d0c`–`0x53d18`. It looks up a cached pair of pen
bounds by name. On a cache miss, it constructs a PenManager, obtains
PenData by name and calls the pen's slot 40 at `0x53df4` and slot 32
at `0x53e08`. These are maximum and minimum pixel-size getters,
respectively; the pair is cached at `0x53e48`.

For a successfully resolved pen, the following arithmetic uses floats
and rounds at each multiply, divide, subtraction and addition:

```text
short_side = float(min(canvas_width, canvas_height))
minimum = (pen_min_px_size * short_side) / 360.0
maximum = (pen_max_px_size * short_side) / 360.0

if size_level < 2:
    size = minimum
else if size_level > 99:
    size = maximum
else:
    size = minimum + ((maximum - minimum) * float(size_level)) / 100.0
```

The divisor 360 is loaded at `0x53e6c`–`0x53e74`. Bound scaling is
at `0x53e8c`–`0x53e9c`, level selection at `0x53eb4`–`0x53ecc`,
and interpolation at `0x53ed0`–`0x53ee8`. This is not interpolation
using `(level - 1) / 99`: levels below 2 select the minimum explicitly,
while levels 2 through 99 use `level / 100`.

There is no positive-dimension validation in this arithmetic. The caller's
document validity and the utility's string/pen lookup are separate
conditions. Calling the utility directly with level 0 selects its minimum;
the Java manager's level-0 branch instead retains the existing `size`.

## Marker2 supplies a concrete width range

Marker2 primary-vtable slots 32 and 40 resolve to `GetMinPxSize`,
`0x1f008`, and `GetMaxPxSize`, `0x1f014`. They return float constants
at `0x11b7c` and `0x11b78`, approximately `0.546` and `16.135`.
These are the utility's pixel bounds, distinct from Marker2's density
bounds and the common pen setter's global clamp.

The recovered float arithmetic produces these examples, rounded for
display:

| Shorter document dimension | Level 1 | Level 2 | Level 50 | Level 100 |
| --- | --- | --- | --- | --- |
| 360 | 0.546 | 0.85778 | 8.3405 | 16.135 |
| 720 | 1.092 | 1.71556 | 16.681 | 32.27 |

These are arithmetic reconstructions, not measurements from device files.
Changing only the longer document dimension does not change this utility's
result. Doubling the shorter dimension doubles the example widths before
any subsequent setter clamp.

## Density conversion interpolates DP bounds before scaling

`SpenPenUtil.convertSizeLevelToDpSize` obtains
`context.getApplicationContext().getResources().getDisplayMetrics().densityDpi`.
It passes that integer, the pen name and the size level to
`Native_convertSizeLevelToDpSize`; fresh fallback decompilation confirms
the property access and argument order.

Engine's JNI table entry at `0x1926d8` binds that method, signature
`(ILjava/lang/String;I)F`, to `EngineUtilGlue::Native_convertSizeLevelToDpSize`,
`0xc2610`. On successful native string construction it forwards the
arguments to PenCommon at `0xc266c`; failed construction returns zero.

PenCommon `PenUtil::ConvertSizeLevelToDpSize`, `0x53fe0`, caches the
pen's DP bounds separately from the pixel-bound cache. On a cache miss,
the calls at `0x540e4` and `0x540f8` use pen slots 56 and 48 for
maximum and minimum DP size, respectively. It caches the pair at
`0x54138`.

The recovered arithmetic is:

```text
if size_level < 2:
    dp_size = pen_min_dp_size
else if size_level > 99:
    dp_size = pen_max_dp_size
else:
    dp_size = pen_min_dp_size
        + ((pen_max_dp_size - pen_min_dp_size) * float(size_level)) / 100.0

density_scale = float(density_dpi) / 160.0
size = density_scale * dp_size
```

Level selection is at `0x54158`–`0x5416c`, float interpolation at
`0x54170`–`0x54188`, and integer-to-float density conversion at
`0x5418c`. The divisor 160 is loaded at `0x54190`–`0x5419c`;
division and final multiplication are at `0x541b4`–`0x541b8`.
Each arithmetic step uses floats, without integer rounding of the output.

Thus the public method's result already includes density scaling. The
name `convertSizeLevelToDpSize` does not mean it returns an unscaled DP
number that the caller must multiply by density again. The method takes
neither document dimensions nor a zoom factor. Positive-density validation
is not present in this native arithmetic; the ordinary Java caller obtains
its value from display metrics.

Marker2 relocations `0x2eaf8` and `0x2eb00` resolve slots 48/56 to
`GetMinDpSize`, `0x1f020`, and `GetMaxDpSize`, `0x1f02c`. They return
the float constants at `0x11b74` and `0x11b70`, approximately `1.142`
and `33.714`. The density branch therefore produces these reconstructed
widths, rounded for display:

| Density DPI | Level 1 | Level 2 | Level 50 | Level 100 |
| --- | --- | --- | --- | --- |
| 160 | 1.142 | 1.79344 | 17.428 | 33.714 |
| 240 | 1.713 | 2.69016 | 26.142 | 50.571 |
| 320 | 2.284 | 3.58688 | 34.856 | 67.428 |

The ordering differs from the pixel branch: density conversion
interpolates first, then scales, while pixel conversion scales both
bounds before interpolating. Reordering those float operations can
change rounding. Both utilities remain distinct from the common size
setter's subsequent clamp and from the recorded size getter.

## The JNI bridge sets the pen's size float directly

Engine registers 11 methods for `com/samsung/android/sdk/pen/engine/SpenViewCore`
at `0xc61d0`, using the method table at `0x192f60`. Its entry at
`0x192fc0` binds `Native_setPenSettingInfo`, signature
`(JLcom/samsung/android/sdk/pen/SpenSettingPenInfo;J)Z`, to `0xc6454`.

The bridge resolves the Java `size` field with descriptor `F`, reads it
at `0xc6530`, and preserves it in `s8` at `0xc6558`. Its pen-selection
helper, `0xe3a1c`, obtains PenData through the view core's PenManager
at member 56 and stores a successful result in member 72 at `0xe3a60`.
That helper also configures the pen's screen size through slot 112.

The bridge then calls view-core slot 16 at `0xc6a54`, obtains the pen
from PenData member 16, and calls pen slot 16 at `0xc6a84`, supplying
the saved size float via `0xc6a7c`. There is no zoom multiplication or
division between reading `size` and this size-setter call.

For the Composer-created ViewCore, primary-vtable relocation `0x5809d0`
binds slot 16 to `0x4cfec8`, which returns member 72. Composer's
`NoteWritingViewGlue::Native_getViewCore`, `0x320a80`, returns writing-view
member 720, also exposed by `NoteWritingView::GetPenSetting`.

For Marker2, relocation `0x2ead8` binds pen slot 16 to PenCommon
`Pen::SetSize`, `0x45fdc`. It applies the already recovered finite
[0.4–800 size clamp](marker2-rendering-findings.md#size-clamping-and-stamp-geometry)
and stores the result at pen member 24. `Pen::GetSize`, `0x4600c`,
reads that float through Marker2 slot 24.

## Down-event initialization assigns the selected PenData

Composer creates the writing view's ViewCore through `0x4cf708` and
stores it at writing-view member 720 at `0x530e10`. Later,
`NoteWritingView::createActions` supplies that same member as the
`IPenSetting*` argument at `0x42534c`, and supplies the drawing interface
through members `728 -> 632 -> 648` at `0x425340`–`0x425354`.
The `NoteWritingViewPenAction` constructor call is at `0x425358`.

Its constructor, `0x422354`, forwards these arguments to the base
constructor at `0x422368`. That constructor stores the setting pointer
at action member 16 at `0x4fed28`, and the drawing pointer at member
416 at `0x4fed74`. The action therefore reads the same ViewCore object
whose selected PenData is assigned by the native setting bridge.

In `NoteWritingViewPenAction::OnTouch`, `0x422444`, action 0 enters
down-event initialization at `0x422580`. Document/page checks precede
the pen assignment. For an event that reaches `0x4228c8`, the action:

1. Loads the setting from member 16 and calls its slot 16 at `0x4228d4`.
   The ViewCore binding returns its selected PenData at member 72.
2. If the result is null, clears the action's active byte at member 64
   and returns through `0x422940`–`0x422944`.
3. Otherwise, passes the returned pointer unchanged to drawing-interface
   slot 112 at `0x4228ec`, using the drawing object at action member 416.

For the [raster factory branch](stroke-input-findings.md#the-ordinary-raster-branch-exposes-the-recorders-count),
primary-vtable relocation `0x583a60` binds drawing slot 112 to
`0x50f7b4`. It obtains the stroke view through drawing member 64 and
wrapper member 8, then tail-calls that view's slot 152 at `0x50f7c4`.
The input PenData remains in argument `x1` throughout this forwarding
method.

The concrete `LowLatencyStrokeView` binding, relocation `0x580d10`,
resolves slot 152 to `0x4d4220`, which stores the pointer at member 88.
Its tail helper, `0x4d29f4`, compares the pen name with three literals
and updates presenter byte 392; it does not rewrite the pen's size.
This connects the setting bridge to the input used by the width copy
below. Other factory branches and specialized actions require their
own dispatch checks.

## The low-latency path copies width into its recording pen

The previously identified low-latency stroke-view helper, Composer
`0x4d36ec`, maintains input PenData at member 88 and recording PenData
at member 104. The input assignment method, `0x4d4220`, stores its
argument in member 88; primary-vtable relocation `0x580d10` binds it
to slot 152.

The helper obtains or reuses recording PenData by the input pen name,
calling `PenManager::GetPenData` at `0x4d37f4` when needed. It calls
the input pen's size getter, slot 24, at `0x4d3868`, then the recording
pen's size setter, slot 16, at `0x4d3878`. No arithmetic occurs between
the getter and setter. Advanced settings, color and other pen properties
are copied separately.

The ordinary payload builder, `0x4d3304`, calls slot 352 to obtain
PenData and stores it at payload offset 24 at `0x4d3344`. Relocation
`0x580dd8` resolves that getter to `0x4d4780`: it selects member 104
when the same slot-72 predicate that enables the copy is true, and
member 88 otherwise.

`TouchPresenter::PresentTouch` passes payload member 24 to
`TouchStrokeDrawing::SetPenData` at `0x4d77f8`. Drawing's setter,
`0xb817c`, retains the pointer at recorder member 16 at `0xb81ac`.
The recorder subsequently obtains width through that pen's getter when
[creating the stored stroke](view-input-transform-findings.md#recorded-pen-width-comes-from-a-separate-getter).

Together with down-event assignment, this supplies the ordinary raster
path from the selected ViewCore pen to recorded width. The path carries
PenData and copies its size through pen setters; it does not derive width
from the event's inverse coordinate-transform matrix.

## Validation and remaining work

The APK digest and five native library byte streams were verified.
JNI names/signatures, view-core and pen vtable slots, action constructor
arguments, raster forwarding, Marker2 constants, the float arithmetic
and recording-pointer transfer were checked against the binaries.
Fresh fallback Java output confirmed the manager conversion
branches and the density source, and disposable float reconstruction
checked both utilities' example widths and level boundaries. Documentation
links were checked. No SDK code changed.

The ordinary raster assignment is now resolved. Other drawing factory
branches, specialized actions, setting changes during an active stroke
and application decisions that choose `isDpSize` remain separate targets.
Existing decoded widths remain the authority for saved-file rendering:
size-level conversion, setter clamping and live view zoom are
creation-time concerns.
