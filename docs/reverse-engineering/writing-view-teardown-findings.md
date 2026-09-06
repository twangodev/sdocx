# Writing-view teardown and prediction callbacks

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenEngine.so`, `libSPenComposer.so` and `libSPenView.so` from the
[identified APK](README.md#sources-and-validation). `SpenWritingView` and
`SpenWritingViewImpl` were freshly decompiled in fallback mode from that
APK's `classes8.dex`. Addresses below are in Composer unless prefixed.

The Java close methods invoke native finalization directly. The native
ownership chain reaches the raster stroke view, predictor and consumer
through ordinary destructor calls. The writing view also cancels its own
Handlers, but those are separate allocations from the per-completion
Handlers in the [prediction queue trace](predictor-queue-findings.md).

This narrows the teardown question to the caller's scheduling and any
behavior inside other cleanup delegates. It does not establish a device
failure or prove that every application call site permits stale delivery.

## Java closes the native view before the draw loop

Fresh `SpenWritingView.close()` decompilation gives this order, with null
checks around optional components:

1. For a non-HWUI draw loop, detach the TextureView listener and remove
   that view.
2. Unregister activity/resource listeners and close the listener manager.
3. Call `mWritingViewImpl.close()` directly.
4. Disable the orientation listener.
5. Call `mDrawLoop.close()`. Clear that field immediately only when the
   view is no longer attached to a window.
6. Clear the saved context and activity-component references.

`SpenWritingViewImpl.close()` first closes its text, latency, control,
hover-icon, sound, view-core, configuration and display delegates. It
then checks `nativeView`:

```text
if nativeView != 0:
    Native_finalize(nativeView)
    nativeView = 0
close native context if present
finalize front-buffer draw pad
```

These two methods contain no direct Handler post, looper wait, callback
drain or deferred native deletion. The zero assignment happens after
native finalization returns. This protects subsequent checks of the Java
field; it does not rewrite consumer pointers already copied into native
prediction payloads.

`SpenWritingView.onDetachedFromWindow()` separately forwards detachment to
the implementation. After the superclass call, it checks native-view
validity; an invalid view triggers draw-loop detachment and clears the
draw-loop field. That is not a delayed native deletion in the close path.

## JNI finalization dispatches native destruction directly

Engine `WritingView_OnLoad`, `0xc84fc`, resolves class string `0x5b541`,
`com/samsung/android/sdk/pen/engine/writingview/SpenWritingViewImpl`, and
registers 41 entries from table `0x1930e0` at `0xc8564`.
Entry `0x1930f8` binds `Native_finalize`, `(J)V`, to `0xc8bdc`.

For a non-null native view, that finalizer:

| Operation | Engine evidence |
| --- | --- |
| Retrieve the parent | `View::GetParent` call at `0xc8c18` |
| If present, cast the parent to `RootView` | Adjust the secondary-interface pointer by -464 at `0xc8c28`, then `__dynamic_cast` at `0xc8c38` |
| If the cast succeeds, remove the root's children | `ViewGroup::RemoveAllViews` at `0xc8c44` |
| Destroy that root through its interface | Slot 112 call at `0xc8c54` |
| Finalize the supplied view | Slot 112 tail-call at `0xc8c6c` |

The imported RTTI at Engine `0x18bad8`/`0x18bae0` names `ViewGroup` and
`RootView`. View `GetParent`, `0x718bc`, reads member 32. The finalizer
itself has no Java message dispatch or queue-drain loop.

For a `NoteWritingView`, primary vtable address point `0x5752c8` binds
slot 112 at `0x575338` to deleting destructor `0x425ad8`. That calls
non-deleting destructor `0x425720`, which eventually enters its
`SmartWritingView` base destructor at `0x545360` through `0x425a6c`.
The latter enters `WritingView` destruction, `0x531ecc`, through
`0x5456ec`.

## The raster owner chain reaches the presenter synchronously

The [raster construction trace](stroke-input-findings.md#the-ordinary-raster-branch-exposes-the-recorders-count)
establishes the view's drawing objects. Their destruction follows:

| Owner and member | Destruction dispatch | Resolved target |
| --- | --- | --- |
| `WritingView`, member 728 | Slot 112 at `0x532200` | `WritingContentView` deleting destructor `0x509768` |
| `WritingContentView`, member 632 | Slot 112 at `0x5096e8` | `WritingMainContentView` deleting destructor `0x50ac98` |
| `WritingMainContentView`, member 648, raster branch | Slot 8 at `0x50abb4` | `WritingViewRasterDrawing` deleting destructor `0x50e8c8` |
| `WritingViewRasterDrawing`, member 64 | Slot 8 at `0x50e834` | `WritingViewDocumentFloatingLayer` deleting destructor `0x512bc4` |
| Floating layer, member 8 | Slot 24 at `0x512bb4` | Stroke-view deletion dispatcher `0x4d2340` |
| `LowLatencyStrokeView` | Dispatcher tail-calls slot 8 at `0x4d2348` | Deleting destructor `0x4d22f4` |
| Stroke view, member 72 | Slot 8 at `0x4d21dc` | `TouchPresenter` deleting destructor `0x4d7698` |

The primary vtable address points that bind those rows are:

| Class | Primary address point |
| --- | --- |
| `WritingContentView` | `0x582410` |
| `WritingMainContentView` | `0x582970` |
| `WritingViewRasterDrawing` | `0x5839f0` |
| `WritingViewDocumentFloatingLayer` | `0x583df8` |
| `LowLatencyStrokeView` | `0x580c78` |
| `TouchPresenter` | `0x580f90` |

The floating-layer constructor stores the newly created stroke view at
offset 8 through `0x50e4f8`; the raster owner stores the wrapper at offset
64 through `0x50e4fc`. Its slot-24 deletion dispatcher is therefore a
deletion call, even though that slot differs from the usual deleting
destructor slot.

These are direct or virtual calls on the closing thread. No callback
message is substituted for the deletion in the listed dispatches. The
existing [presenter teardown trace](predictor-reconfiguration-findings.md#presenter-teardown-deletes-the-proxy-before-its-consumer)
then applies: proxy/predictor deletion, worker join when present, and
consumer deletion at `0x4d7654`.

## The view's own Handler cancellation has different owners

`SmartWritingView` construction allocates and constructs three 80-byte
Handlers, then stores them in members 1528, 1536 and 1416 through
`0x544de4`, `0x544e00` and `0x544e68`. Constructor calls at `0x544de0`,
`0x544df4` and `0x544e60` use imported `Handler()`.

Its destructor explicitly cancels and deletes those Handlers:

| View member | `Handler::RemoveMessages` call | Handler deletion call |
| --- | --- | --- |
| 1416 | `0x54544c` | `0x545464` |
| 1528 | `0x545574` | `0x54558c` |
| 1536 | `0x545598` | `0x5455b0` |

The shared PLT target `0x54fa80`, relocation `0x5a4138`, names
`Handler::RemoveMessages`. The [Handler trace](predictor-queue-findings.md#handler-destruction-cancels-by-handler-id)
establishes that it removes messages for the receiving Handler's ID.
It is not a request to drain every Handler associated with the view.
These three constructor allocations are separate from the Handler
allocated inside each Predictor `OnTouchAsync` call.

The earlier wait at `0x5453e0`, through relocation `0x5ab960`, names
`NNBaseGrouping::WaitForIdle`. Its receiver is the embedded grouping
object at view offset 1144. That is a separate synchronization call from
joining the neural prediction worker or delivering its Java messages.
This finding identifies its receiver and import; it does not recover the
grouping algorithm's full wait implementation.

## Presenter controller cleanup also does not drain callbacks

The presenter member-528 cleanup call at `0x4d75f0` uses
`PredStrokeLengthController` slot 16. Vtable primary `0x580f20` binds
that slot to `0x4d5550`, which tail-calls its own deleting destructor
through slot 8 at `0x4d5558`.

The non-deleting controller destructor, `0x4d5508`, deletes its member-144
scratch allocation through `0x4d5524`; deleting destructor `0x4d552c`
then releases the controller object. It does not visit the predictor,
touch synchronizer, Handler registry or Java queue. This resolves the
previously unnamed controller cleanup operation without treating it as
a callback barrier.

## What remains unresolved

The traced close methods do not demonstrate a wait for queued prediction
callbacks. Main-thread execution alone would not establish that wait:
if native deletion runs while a completion is pending, the completion
still needs to be delivered or cancelled before its consumer is freed.
Whether the application ensures that ordering before entering `close()`
requires its call sites and scheduling. The helper delegates called
during close also remain possible sources of additional constraints.

The APK digest, fresh DEX and three native streams were verified. JNI
registration, import targets, constructor stores, vtable slots and
destructor calls were checked against their bytes. Disposable ownership
reconstruction checked the raster deletion chain, separate cancellation
IDs and pending-callback ordering. No Android/device execution, SDK
change or new SDOCX fixture was used.
