# Capture scheduling and document detachment before editor release

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 from the
[identified APK](README.md#sources-and-validation). `FlashViewManager`
and its nested draw listener/Runnable were freshly decompiled from
`classes11.dex`, with anonymous-class inlining disabled. The synthetic
`com.samsung.android.sdk.composer.a` Runnable was freshly decompiled from
`classes7.dex`. The freshly extracted Composer and writing-manager classes
used in the [close trace](composer-close-findings.md) supply the other Java
methods. Native addresses below are in ARM64 `libSPenComposer.so` unless
explicitly prefixed with View for `libSPenView.so`.

This extends the close trace through capture scheduling and the null-document
branch. A draw callback and a document-change post have different purposes;
neither Java callback inspects pending prediction completions.

## Capture release is posted by the first draw callback

`FlashViewManager.showCaptureView(composer, callback)` returns immediately
if `mCaptureView` is null. Otherwise it aligns that ImageView vertically
with Composer and calls `composer.captureCurrentView()` directly.

A returned bitmap becomes the capture view's background. If Composer's
background is a state-list drawable, the method first paints its current
color behind the captured pixels using `DST_OVER`. A null bitmap selects
the configured container background color instead. Both branches make
the capture view visible and continue to the same callback setup.

With a null callback, the method returns after showing the image. With a
callback, it installs a `ViewTreeObserver.OnDrawListener` containing an
`isCalled` Boolean initialized to false. The signal comes from the capture
view's tree observer; bitmap capture was invoked before listener installation.
Its first `onDraw`:

```text
if isCalled:
    return
isCalled = true
mCaptureView.post(completionRunnable)
```

The Runnable calls `callback.onDone(null)` first, then removes the manager's
saved draw listener and clears `mOnDrawListener`. The supplied callback is
the adapter from `ComposerView.releaseAfterCapture`, so the resulting
ordering is:

```text
capture bitmap and show capture image
first draw notification from the capture view's tree observer
post completion Runnable
run completion Runnable
    releaseComposerView(false, true)
    forward caller callback
remove capture draw listener
```

The 300 ms alpha animation in `removeCaptureView` is a separate operation.
It is not a delay inside `showCaptureView` or the release callback.

The recovered methods also establish these boundaries:

| Condition | Local behavior |
| --- | --- |
| Capture ImageView absent | `showCaptureView` returns without invoking or posting the supplied callback |
| Bitmap capture returns null | Show the fallback color; callback setup still proceeds |
| No tree-observer draw notification | The draw listener has not posted release; this method has no timeout fallback |
| Additional draw notifications | `isCalled` suppresses additional posts from that listener |
| `View.post` returns false | Its result is discarded; `isCalled` has already been set, with no retry in the listener |
| Caller callback throws | Listener removal follows the callback without a local `finally` block |

These are control-flow conditions, not claims that any device encountered
them. The no-callback branch of `ComposerView.releaseAfterCapture` still
closes directly after `showCaptureView` returns, including an early return
for a missing ImageView. The callback branch relies on the posted adapter.

## Bitmap capture enters the native drawing path

`SpenComposerImpl.captureCurrentView()` returns null for a zero native
handle or zero screen dimensions. Otherwise it allocates an `ARGB_8888`
bitmap at the screen dimensions and calls native capture synchronously.
A normal false native result recycles the bitmap and returns null. Its
Throwable handler logs and returns the saved local bitmap reference, which
can be null or already allocated; it does not report a prediction status.

JNI entry `0x5b0130` binds `Native_captureCurrentView`,
`(JLandroid/graphics/Bitmap;)Z`, to `0x30d464`. The bridge obtains
the parent, casts it to `RootView`, converts the Java bitmap, and invokes
`RootView::CaptureCurrentView` at `0x30d510` through relocation `0x5a3288`.

View `RootView::CaptureCurrentView`, `0x6e4a8`, creates a bitmap and canvas
through `SPGraphicsFactory` at `0x6e568` and `0x6e58c`. If a canvas was
created, it calls `View::Draw` at `0x6e5cc`. It releases the canvas at
`0x6e5d4`, obtains the destination bitmap buffer at `0x6e5f8`, invokes
bitmap slot 32 at `0x6e610`, and releases the graphics bitmap at `0x6e618`.
View relocations `0xa77a8` through `0xa77d0` identify those factory,
drawing and buffer operations.

This connects the application capture to drawing and pixel transfer. It
does not recover every child-view drawing delegate or graphics backend,
and therefore does not establish a prediction-completion barrier inside
those delegates.

## Document detachment posts only initialization flags

`SpenComposerImpl.setDocument(document)` returns false immediately if its
native handle is zero or the document compares equal to `mDocument`.
Otherwise it stores the new Java document, updates its text/conversion
delegates and performs the native `Native_setDocument` call before updating
the writing manager.

The writing manager's `setDocument` only compares/stores `mDocument` and
returns whether it changed. It performs no native document detach itself.

For draw-loop types 2 and 3, the implementation subsequently constructs
`Handler(Looper.getMainLooper())` and posts a synthetic Runnable with
discriminator 1. Its default branch invokes the implementation's `a`
accessor, which calls `setDocument$lambda$10`. That function contains only:

```text
mDocInited = true
mDocInitedForOnLayout = true
```

The post also occurs for a changed document set to null. Native detachment
has already returned, and the caller proceeds to `close()` without waiting
for this Runnable. Its body neither invokes close nor consumes prediction
messages. For draw-loop type 1, the method sets `mDocInited` directly;
it creates a twin view only for a non-null document.

## Native detachment clears the writing adapter and action pointers

JNI entry `0x5afe00` binds `Native_setDocument`,
`(JLcom/samsung/android/sdk/pen/worddoc/SpenWNote;)Z`, to `0x30c31c`.
Null Java document input branches at `0x30c338` to the zero native argument
at `0x30c3fc`; `0x30c430` calls `Composer::SetDocument`, `0x3889fc`,
through relocation `0x5a31f0`.

Composer stores the argument in member 744 at `0x388a5c`. The null branch
at `0x388a70` reaches `0x388d5c`, and passes null to its member-528
`ContentsView` through `0x388d88`. The existing
[page-mode adapter trace](stroke-insertion-findings.md#construction-binds-page-mode-to-the-append-implementation)
documents the non-null construction branch. The null branch instead:

| Operation | Evidence |
| --- | --- |
| Store null as the contents document | `0x417a00`, member 2048 |
| Clear the writing adapter | `0x417a78`, member 2056 |
| Clear the associated shared-pointer storage | `0x417a7c`, members 2080/2088; release the previous owner through `0x417b34` when present |
| Pass the null document and adapter to NoteWritingView | `0x417b5c` through `0x417b68` |
| Delete the former writing adapter after the child updates | Slot 8 at `0x417ba4`, when the saved old adapter exists |

`NoteWritingView::SetDocument`, `0x425b24`, skips work when its stored
adapter already equals the new one. On a change, it calls `0x5324c8`
at `0x425b50`, stores the new adapter in member 2600 at `0x425b5c`,
and updates its SmartWritingView base at `0x425b60`.

The first helper reaches `WritingMainContentView` through members 728 and
632, calling its document setter `0x50af4c` at `0x532654`. On a changed
document, that setter:

1. Calls drawing-object slot 88 at `0x50af80`.
2. Iterates twenty action pointers starting at member 696, passing the new
   document to each non-null action's slot 88 at `0x50afa0`.
3. Stores the new document at member 656 through `0x50afb0`.

The note pen action's slot-88 entry, `0x574b68`, resolves to `0x500268`.
That function consists of storing the argument at action offset 24 and
returning at `0x50026c`. Clearing this action pointer is not cancellation
of the consumer pointer copied into a prediction payload.

In the raster branch, drawing slot 88 at `0x583a48` resolves to
`0x50f64c`. It stores the document at raster offset 24, updates its document
image cache through `0x53bd20` at `0x50f674`, and updates the member-48
helper through `0x511ac8` at `0x50f684`. The null branch at `0x50f688`
then skips the non-null document's rendering-parameter updates. The cache
setter clears its document at `0x53bd7c` and enters cache reload at
`0x53bd98`; the other helper clears its document at `0x511b4c` and skips
new-document listener registration at `0x511b50`.

Composer's return value is whether member 744 is non-null, computed at
`0x388e34` through `0x388e3c`. Thus a normal null-document detach returns
false after these updates. `releaseComposerView` discards that result and
continues to close; false alone does not identify failed detachment.

## Prediction ordering remains a separate constraint

The [prediction queue trace](predictor-queue-findings.md) gives each
completion its own Handler and copied consumer pointer. The capture draw
listener and posted Runnable do not refer to those Handlers, their
registry, or the worker. The document-change Runnable only sets Java flags,
and the pen-action setter only changes its document pointer.

Even assuming successful posts into one FIFO queue, observing a draw before
posting close is insufficient by itself: the draw can post close, a worker
can finish and post its completion afterward, and close can then run first.
Reversing those two posts permits completion before close. This is a static
ordering example conditional on other delegates adding no cancellation or
wait, not an observed Android failure.

Unresolved behavior includes other document-change actions, SmartWritingView
recognition cleanup, and drawing/backend delegates. Those require their own
traces before claiming either safe completion ordering or stale delivery
for the full application lifecycle.

The APK digest, fresh DEX definitions and both native streams were verified.
JNI bindings, relocations, vtable targets and cited instructions were checked
against the binary bytes. Disposable state reconstruction checked one-post
capture behavior and the two completion/close orderings. No device execution,
new SDOCX fixture or SDK change was used.
