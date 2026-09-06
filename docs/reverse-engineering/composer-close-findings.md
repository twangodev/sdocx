# Main-editor Composer close ordering

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` from the [identified APK](README.md#sources-and-validation).
Java classes were freshly decompiled in fallback mode from that APK:

| DEX | Classes |
| --- | --- |
| `classes2.dex` | `SpenComposerWrapper` |
| `classes7.dex` | `SpenComposer`, `SpenComposerImpl`, `SpenNoteWritingViewManager` |
| `classes11.dex` | `ComposerView`, including its capture callback |

The application classes are under
`com.samsung.android.support.senl.nt.composer.main.base`: the wrapper is in
`widget.view`, and `ComposerView` is in `view.composer`. SDK classes are
under `com.samsung.android.sdk.composer`, with the writing manager in
its `writing` package. Native addresses below are in Composer.

The main editor uses `SpenComposerWrapper`, which extends `SpenComposer`.
Its close entry differs from the generic
[`SpenWritingView` close path](writing-view-teardown-findings.md).
Both reach the native raster owner chain, but the generic JNI finalizer's
root/view destruction order must not be applied to Composer.

## Application release clears the document before closing

`ComposerView.releaseComposerView(firstFlag, secondFlag)` performs these
operations, with guards around optional components:

1. Release menu, text-scale listener, scroll/zoom and view-state helpers.
2. If `firstFlag` is true, call `requestReadyForSave()`.
3. Remove the layout listener and release the listener manager.
4. Call `mView.setDocument(null)`.
5. Call `mView.close()` directly.
6. If `secondFlag` is true, remove the closed view from its Android parent.
7. Release the remaining guide/DVFS helpers and clear `mView`.

`release(boolean)` forwards to `release(boolean, false)`. The two-argument
method clears `mDoc`, invokes the helper inside an exception handler, then
clears `mView` even if the helper threw. A cleared Java reference therefore
does not by itself prove that every cleanup operation completed.

`releaseAfterCapture(callback)` has two branches. Without a callback, it
calls `showCaptureView(view, null)` and immediately invokes
`releaseComposerView(false, true)`. With a callback, it passes an adapter
to `showCaptureView`; that adapter's `onDone` first invokes
`releaseComposerView(false, true)` through `ComposerView.a`, then forwards
the result to the caller. This branch ties closing to the capture callback.
The capture implementation's scheduling and its relationship to prediction
messages remain unresolved.

## Java manager cleanup precedes native ownership cleanup

The final `SpenComposer.close()` method directly calls its implementation's
`close()`, then disables orientation observation and unregisters activity
callbacks. `SpenComposerImpl.close()` closes its document, PDF, text,
recognition and other delegates before this relevant sequence:

```text
close writing manager if present
close hover pointer icon if present
Native_finalize(mNativeHandle)
mNativeHandle = 0
close remaining display and interaction delegates
close draw loop if present
close frame scheduler and renderer if present
```

The writing manager's own `close()` closes latency/view-core delegates,
finalizes its front-buffer draw pad, clears context/view/document fields,
and sets `nativeNoteWritingView` to zero. It does not directly call a native
writing-view finalizer. Native ownership is released later by Composer's
finalizer. Neither clearing that manager field nor subsequently clearing
`mNativeHandle` rewrites the raw consumer pointer already copied into a
[queued prediction payload](predictor-queue-findings.md#each-completion-registers-a-separate-native-handler).

These close methods contain no direct Java message drain or deferred native
deletion. Effects inside their cleanup delegates require separate tracing.

## Composer deletes its contents before removing the native root

`Composer_OnLoad`, `0x30b3a0`, resolves class string `0x1d4195`,
`com/samsung/android/sdk/composer/SpenComposerImpl`, and registers 86
entries from `0x5afce0` at `0x30b404`. Entry `0x5afcf8` binds
`Native_finalize`, `(J)V`, to `0x30bb74`.

For a non-null Composer, this finalizer retrieves its parent through
`View::GetParent` at `0x30bbb0`. If present, it adjusts the secondary
interface pointer by -464 at `0x30bbc0` and casts to `RootView` at
`0x30bbd0`. Imported RTTI at `0x5a1238` and `0x5a1240` identifies
`ViewGroup` and `RootView`.

The finalizer then deletes Composer through slot 112 at `0x30bbec`.
Only afterward, if the root cast succeeded, it calls
`ViewGroup::RemoveAllViews` at `0x30bbf8` and deletes that root through
slot 112 at `0x30bc10`. The generic Engine finalizer uses the other order.

Composer's ordinary native ownership chain is:

| Owner | Dispatch | Resolved destructor |
| --- | --- | --- |
| Composer | Primary `0x565b20`, slot 112 | Deleting `0x3889b0`, non-deleting `0x3885b4` |
| Composer member 528, `ContentsView` | Slot 112 at `0x38873c`; primary `0x571d18` | Deleting `0x4177e8`, non-deleting `0x416d10` |
| Contents member 1872, `NoteWritingView` | Slot 112 at `0x41707c`; primary `0x5752c8` | Deleting `0x425ad8`, non-deleting `0x425720` |

`Composer::GetNoteWritingView`, `0x38bbbc`, independently confirms the
member-528 then member-1872 lookup. The resulting `NoteWritingView`
continues through the existing
[raster and presenter destruction chain](writing-view-teardown-findings.md#the-raster-owner-chain-reaches-the-presenter-synchronously).

Composer also cancels its own member-1272 Handler at `0x38869c` and
deletes it at `0x3887d8`. Its allocation is identified by
`Composer::initLaserViewHandler`, `0x38829c`: allocate 80 bytes at
`0x3882c4`, construct imported `Handler()` at `0x3882cc`, and store the
new pointer at `0x3882d4`. This is a laser-view Handler allocation,
separate from each prediction completion's allocation. The imported
`Handler::RemoveMessages` operation cancels by the receiving Handler's ID;
it does not establish cancellation of all prediction completions.

## Ready-for-save reaches the document image cache

Application `requestReadyForSave()` first calls
`getTextManager().updateBodyTextPage()`, then the writing manager's
`requestReadyForSave()`. The manager returns immediately for a zero native
handle; otherwise it invokes `Native_requestReadyForSave`.

Native glue `0x32183c` dispatches NoteWritingView slot 960 at `0x321884`.
Vtable entry `0x575688` resolves that slot to
`NoteWritingView::RequestReadyForSave`, `0x4271b8`. The method:

1. Optionally calls `NoteWritingViewShapeDetectionAction::RequestCancel`
   at `0x4271d8`, when member 2576 exists and byte 2656 is zero.
2. Invokes the optional callable at member 1920 through its slot 48 at
   `0x4271ec`.
3. Follows members 728, 632 and 648 to the drawing object, then calls its
   slot 320 at `0x42720c`.

In the raster branch, primary `0x5839f0` binds slot 320 to `0x50fc18`.
That loads the raster object's member 32 and branches to `0x53c61c`.
The target logs `DocumentImageCache::SaveCache`, using strings at
`0x230a02` and `0x1e4279`, and processes cache entries. Its complete
cache behavior and the optional member-1920 callback remain unresolved.
The name `requestReadyForSave` alone is not evidence that queued predictor
callbacks have been delivered or cancelled.

## Validation and remaining boundary

The APK digest, defining DEX files and native stream were verified against
the archive. JNI entries, destructor and ready-for-save vtable targets,
RTTI, imported methods and cited instructions were checked against their
bytes. The capture adapter was decompiled with anonymous-class inlining
disabled to retain its `onDone` body.

This trace connects the main editor's release entry to presenter destruction.
It does not establish a device failure, a prediction callback barrier, or
the full effects of `setDocument(null)` and other cleanup delegates. No SDK
behavior or saved-stroke format rule follows from this lifetime trace alone.
