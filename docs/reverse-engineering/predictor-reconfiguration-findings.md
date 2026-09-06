# Composer predictor reconfiguration and teardown

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenPredictor.so` from the
[identified APK](README.md#sources-and-validation).

The [runtime lifecycle trace](neural-lifecycle-findings.md) found that
replacing a holder can invalidate pointers already copied into a pending
task. Composer's selection setter instead deletes and recreates the
concrete predictor. This caller distinction limits where that earlier
static example applies. Teardown and queued callback lifetime remain
separate questions.

Addresses below are in Composer unless prefixed with Predictor. The
trace establishes local ordering, not application-wide mutual exclusion
or observed device scheduling.

## The presenter records the requested selection and forwards it

The low-latency view forwarding function at `0x4d2b04` loads its
presenter from offset 72, reads the presenter's refresh rate at offset
396, and forwards the requested selection to `setPredictionType`,
`0x4da3bc`.

The presenter stores that selection at offset 400 at `0x4da40c`
and calls `PredictorProxy::SetPredictorType` at `0x4da410`. After
the call it obtains the proxy's prediction time and sets the maximum
prediction time with multi-output enabled at `0x4da424`–`0x4da444`.
The [horizon-selection trace](neural-selection-findings.md) covers that
second operation.

The presenter does not branch on the proxy setter's Boolean return.
Its stored requested selection is consequently not proof that a concrete
predictor was successfully created.

## Changed selection destroys the old predictor before creating a new one

The proxy maps selections 1–4 to neural kind 1 with the corresponding
length ID, and selection 5 to linear kind 2 with length 1. Other selections
map to disabled kind/length zero. This is the
[previously recovered factory mapping](predictor-callback-findings.md#the-proxy-loads-a-bundled-factory);
it does not establish support for neural length 4 in the bundled factory.

Before replacing anything, the setter compares its reported kind and
length with those requested, through slots 80 and 72 at
`0x4daaf8`/`0x4dab10`. If both match, it returns true through
`0x4dacd8`. The refresh-rate argument is not part of these comparisons;
it reaches the factory only if construction proceeds. The independent
[refresh-rate setter](predictor-position-findings.md) is a separate path.

For a different selection, the sequence is:

| Operation | Instruction |
| --- | --- |
| Load the existing predictor from proxy offset 8 | `0x4dab1c` |
| If non-null, call its deleting destructor through slot 8 | `0x4dab2c` |
| Clear proxy offset 8 | `0x4dab30` |
| If disabled, return without creating a replacement | `0x4dab34` |
| Otherwise call `CreatePredictor` | `0x4dab68` |
| Store the returned pointer in proxy offset 8 | `0x4dab6c` |

This sequence applies to a length change within the same neural kind as
well as a change of kind. It does not invoke the old neural predictor's
`SetPredictionLength` to reuse that instance or its holder.

The destructor is called before the proxy pointer is cleared. The local
code does not publish a null pointer first or acquire a mutex around
the sequence. This observation does not establish a concurrently reading
caller; it identifies where external coordination would be needed.

Proxy kind getter `0x4daf00` reports zero if proxy byte 32 is set,
even when a concrete predictor exists. Otherwise it forwards the concrete
kind, or returns zero for a null pointer. The length getter at
`0x4dae78` returns 5 for a null pointer. Therefore the early-return test
compares these getters, not just the caller's last requested selection.

## Model setup occurs before publishing the new predictor

For neural selection, Predictor `CreatePredictor` constructs a fresh
`NNPredictor` at `0x35a3c`, then invokes:

| Setting | Predictor virtual slot | Predictor call site |
| --- | --- | --- |
| DPI | 64 | `0x35a80` |
| Prediction length | 184 | `0x35a94` |
| Refresh rate | 144 | `0x35aa8` |

Only after the factory returns does Composer store the pointer at
`0x4dab6c`. It subsequently installs the VSync callable pair through
predictor slot 48 at `0x4dac68` and registers the saved consumer through
slot 40 at `0x4dac78`.

The model-length setter can rebuild holders during this factory call,
but this object has not yet been published in the proxy or registered
with its consumer. The newly created worker has no prediction task
submitted by this caller. Thus the earlier pending-task/model-replacement
example is not established by this ordinary selection path.

The old predictor's destructor still reaches its worker's stop/join
sequence, so the [pending-stop ownership case](predictor-worker-findings.md#stop-and-model-changes-do-not-inherently-drain-pending-work)
remains relevant to changed selection. Whether a task is pending when
this happens requires runtime or further caller evidence.

## Disabling preserves the selection for later re-enabling

The low-latency view wrapper at `0x4d2af8` forwards an enable Boolean
to helper `0x4da368`, supplying its second Boolean as true.
The helper always stores the enable byte at presenter offset 393 at
`0x4da374`. When the second Boolean is false, it returns without
reconfiguring the predictor.

When reconfiguration is requested, it saves the current selection from
offset 400 and the rate from offset 396. Enabling forwards that saved
selection through `0x4da39c`. Disabling calls `setPredictionType(0)`
at `0x4da3a8`, then restores the saved selection at `0x4da3ac`.

For example, disabling a currently selected M20 predictor deletes that
concrete instance while preserving selection 2 in the presenter.
Re-enabling requests a fresh kind-1, length-2 predictor. Offset 400 is
the requested selection retained across disable, not an ownership handle
or confirmation that the predictor is currently enabled.

## Presenter teardown deletes the proxy before its consumer

`TouchPresenter` destruction at `0x4d758c` first clears drawing hooks,
releases drawing state, and invokes its `PredStrokeLengthController`
cleanup slot at `0x4d75f0`. The [outer teardown trace](writing-view-teardown-findings.md#presenter-controller-cleanup-also-does-not-drain-callbacks)
resolves this as controller deletion, including its scratch allocation.
At `0x4d7604` it calls proxy slot 104 with a null pointer.

Proxy slot 104 resolves to `0x4daf58`, which forwards concrete slot 104.
Both neural and linear vtables bind that slot to Predictor `0x328f8`,
a single pointer store to offset 1760 followed by return. This operation
does not call `SetTouchConsumer`, clear its synchronizer, or cancel
queued messages.

After releasing other presenter delegates, the destructor:

1. Deletes the proxy through slot 8 at `0x4d7640`.
2. Deletes the `PredictorTouchConsumer` at `0x4d7654`.
3. Releases remaining drawing resources and its separate drawing timer.

Proxy destruction at `0x4da958` deletes its concrete predictor at
`0x4da988`. For a neural predictor that invokes the worker destructor,
including its stop/join, before proxy destruction returns. The consumer
therefore remains allocated during that local predictor/worker teardown.

This ordering is not a guarantee that previously queued callback messages
have finished. The [callback trace](predictor-callback-findings.md#registration-captures-a-consumer-and-a-thread-identifier)
shows that asynchronous completion messages copy a raw consumer pointer.
Joining the prediction worker and delivering those messages are distinct
operations. The [Handler and queue trace](predictor-queue-findings.md)
confirms main-looper dispatch and separate per-completion Handlers, whose
destructors cancel by Handler ID. The synchronizer does not retain those
Handlers, and no message-drain call was identified in this presenter/proxy
sequence. The Java close and raster-owner deletion chain are now traced
in [writing-view teardown](writing-view-teardown-findings.md); scheduling
at the application call sites remains unresolved.

## Validation and remaining work

Both native byte streams were matched to the APK. Presenter and proxy
vtables, the concrete destructor and pointer-setter targets, factory
ordering, and documented instructions were checked against ELF bytes
and relocations. Disposable state reconstruction covered unchanged
selection, changed neural length, kind changes, disable/re-enable,
getter overrides, creation failure, and the preserved requested selection.

These findings narrow the earlier holder-replacement example to callers
that change a live instance's model directly. They do not prove all such
callers absent or establish the safety of pending work during destruction.
Callback queue ownership, Java close and raster deletion are now traced
separately; the next boundary is application call-site ordering. No SDK code,
saved-stroke decoding rule, or corpus fixture changed.
