# Save preparation and shape-recognition cancellation

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenRecogUIFeature.so`, `libSPenView.so` and
`libSPenBase.so` from the [identified APK](README.md#sources-and-validation).
Addresses below are in Composer unless prefixed with RecogUI, View or Base.

The [Composer close trace](composer-close-findings.md#ready-for-save-reaches-the-document-image-cache)
establishes three operations in `NoteWritingView::RequestReadyForSave`:
optional shape cancellation, an optional callable, then image-cache saving.
This trace resolves the cancellation gate and its notification chain. It
does not establish that requesting save preparation finishes an ordinary
in-progress pen stroke.

## Composer mode controls the cancellation gate

The byte at NoteWritingView offset 2656 is initialized to 1 at `0x424b70`,
using the constant loaded at `0x424ac8`. The constructor installs a mode
callback with primary vtable address point `0x575948`; its capture at
`0x424c04` stores the NoteWritingView pointer. Registration at `0x424c18`
uses the supplied context's member 96.

The callback's RTTI identifies
`NoteWritingView::NoteWritingView(IContext const*)::$_0` with a
`void(ComposerMode)` signature. Vtable slot 48, entry `0x575978`, points
to `0x428c70`, whose complete operation is:

```text
view.byte2656 = (mode == 1)
```

The input mode is read at `0x428c70`, compared to 1 at `0x428c78`, and
the Boolean is stored at `0x428c80`. The semantic name of mode 1 is not
assigned by this trace.

`RequestReadyForSave`, `0x4271b8`, checks the shape action at member 2576
and that byte at `0x4271c8` through `0x4271d4`. It calls
`NoteWritingViewShapeDetectionAction::RequestCancel` at `0x4271d8` only
when the action exists and the byte is zero. The constructor default skips
this branch; a mode notification with a value other than 1 enables it.

## RequestCancel sets a recognition flag

Shape-action construction, `0x42317c`, allocates a 288-byte transformer
at `0x4231f8`, constructs it at `0x42322c`, and stores it at action
offset 368 through `0x423244`. Relocation `0x5a94d8` identifies
`StrokeDiagramTransformer(IContext const*, PenData*)` in RecogUI.

RecogUI constructor `0x1295e0` installs primary vtable address point
`0x1e86b8` at `0x129618`. It also allocates a 416-byte
`DiagramRecognitionHandler`, calls its constructor through relocation
`0x1f8140` at `0x129690`, and stores that handler at transformer offset
248 through `0x129694`.

Composer `RequestCancel`, `0x423cdc`, loads the transformer and calls its
slot 40 at `0x423d18`. RecogUI entry `0x1e86e0` resolves this slot to
`AbsStrokeShapeTransformer::CancelRequest`, `0x128bc4`.

The latter is a 64-byte function. It loads the transformer's member-248
handler at RecogUI `0x128bd0`, logs
`ObjectRecognitionHandler::SetCanceledFlag[%d]` using string `0x9f6d0`,
and writes 1 to that handler's byte 248 at `0x128bf4`. It then returns.
There is no recognition-worker join, Java Handler cancellation, prediction
callback dispatch or stroke insertion in this function.

The flag participates in recognition control:

| Operation | RecogUI evidence |
| --- | --- |
| `AbsStrokeShapeTransformer::RequestRecognition(bool)` clears the canceled flag before continuing | Store at `0x128ae4` |
| `AbsStrokeShapeTransformer::recognize()` checks the flag | Load at `0x128de8`, branch at `0x128dec` |
| A set flag takes the early failure path | `0x128e30` through `0x128e3c`, including a false return value |

This confirms a cancellation flag for shape recognition. It does not prove
that an already-running recognition engine has stopped when the setter
returns. The independent neural
[prediction worker](predictor-worker-findings.md) and its
[completion Handlers](predictor-queue-findings.md) are separate owners.

## The cancellation notification releases a gesture lock

After the transformer call returns, shape-action `RequestCancel` loads
its optional callable target at offset 352 and invokes slot 48 through
`0x423d34`. The installed notification chain is:

| Callable owner | Binding evidence | Slot-48 target and effect |
| --- | --- | --- |
| Shape action, callable storage at 320, target at 352 | `NoteWritingView::createActions` captures the view with primary `0x575c08` at `0x4252ac` and assigns the callable at `0x4252c0` | `0x4291c8` invokes the view's optional member-2016 callable |
| NoteWritingView, callable storage at 1984, target at 2016 | `ContentsView::initNoteWritingView` captures ContentsView with primary `0x574138` at `0x41344c` and assigns it at `0x413460` | `0x41f8e0` invokes ContentsView's optional member-1312 callable |
| ContentsView, callable storage at 1280, target at 1312 | `Composer::initContentsView` captures Composer with primary `0x567730` at `0x3867f0` and assigns it at `0x386804` | `0x397dc8` calls `GestureDetector::SetGestureLocked(false, composerAddress)` |

The corresponding slot-48 entries are `0x575c38`, `0x574168` and
`0x567760`. Their RTTI names bind the callbacks to `createActions::$_5`,
`initNoteWritingView::$_2` and `initContentsView::$_14`, respectively.
The captured owner is at callable offset 8 in all three cases.

The final thunk reads Composer member 768 at `0x397dd0`, supplies false
at `0x397dcc`, and retains the Composer pointer as the second method
argument. Its branch at `0x397dd4` resolves through relocation `0x5a6208`
to `GestureDetector::SetGestureLocked(bool, long)`.

View `SetGestureLocked`, `0x830c8`, calls `RefList::RemoveReferrer` on
its member-144 reference list at `0x830fc` for the false branch. Depending
on that operation's result, it writes the false lock byte at offset 184
through `0x8312c` or leaves the byte unchanged.

Base `RefList::RemoveReferrer`, `0xc1b98`, saves the original set size from
member 24 at `0xc1ba8`. A matching referrer is removed and the size is
decremented at `0xc1d54` through `0xc1d5c`. Its return predicate at
`0xc1c54` through `0xc1c5c` is true only when the original size was
nonzero and the resulting size is zero. Removing a missing referrer or
removing one while others remain returns false. Composer therefore clears
the gesture-lock byte only when its removal empties the reference set.

The notification chain contains no ordinary stroke append or explicit
delivery of neural prediction completions. Its terminal operation manages
gesture locking.

## The other save callback still has no identified target

NoteWritingView construction explicitly initializes the callable target at
member 1920 to null at `0x424ae0`. `RequestReadyForSave` checks that
target at `0x4271dc` and invokes it at `0x4271ec` only when populated.
This is a different callable from the shape-cancellation notification at
member 2016.

A populated member-1920 target and its registration site remain unresolved.
The inspected constructor, ContentsView callback setup and writing-manager
JNI initializer do not identify one. This is not proof that the field can
never be assigned through another path. It must not be labeled a stroke
finalizer or a queue-drain operation without further evidence.

## Validation and SDK implications

The APK digest and all four native streams were verified against the
archive. Constructor stores, callable captures, RTTI, vtable slots,
relocations and cited instructions were checked against the binary bytes.
Disposable state reconstruction checked mode-dependent cancellation, flag
reset and the notification's captured-owner routing.

The result narrows what save preparation guarantees: its recovered shape
branch requests cancellation and releases a gesture-lock reference before
the image-cache stage. It does not establish a rule for appending, replacing
or dropping an unfinished stroke in an SDOCX decoder. Recognition-engine
completion behavior, the optional save callback and cache serialization
remain separate investigations. No device execution, SDK change or new
SDOCX fixture was used.
