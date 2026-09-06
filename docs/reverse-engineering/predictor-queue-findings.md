# Prediction callback queue and ownership

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenBase.so` and `libSPenPredictor.so`, with the presenter lifecycle
in `libSPenComposer.so`. `SpenHandler` was freshly decompiled in fallback
mode from `classes8.dex` in the [identified APK](README.md#sources-and-validation).
Addresses below are in Base unless explicitly prefixed otherwise.

Asynchronous prediction callbacks use the Java main looper. Each queued
completion has an independent native Handler and a copied consumer pointer.
Deleting the predictor or replacing its consumer does not, through the
traced synchronizer operations, cancel those independent Handlers.

This establishes local ownership and cancellation boundaries. It does not
establish that a particular device has delivered a callback after consumer
destruction, or that every outer application lifecycle permits that ordering.

## The Java bridge explicitly selects the main looper

Fresh decompilation of `com.samsung.android.sdk.pen.util.SpenHandler`
shows these operations:

| Method | Operations |
| --- | --- |
| Constructor | Construct `android.os.Handler(Looper.getMainLooper(), this)` |
| `sendMessage(what, arg1)` | Allocate `Message`, set `what` and `arg1`, call the Android Handler's `sendMessage`, discard its result |
| `handleMessage(msg)` | Forward `msg.what` and `msg.arg1` through the companion/accessor bridge to `native_handleMessage`, then return false |
| `removeMessages(what)` | Forward to the same Android Handler's `removeMessages(what)` |
| `hasMessages(what)` | Return the same Android Handler's `hasMessages(what)` result |

The native bootstrap `Handler_OnLoad`, `0x972b4`, resolves the class
using string `0x2b84f`, constructs it through `0x97348`, and saves its
global Java reference at `0xf7600` through `0x97370`. It caches:

| Java method | Signature | Method-ID storage |
| --- | --- | --- |
| `hasMessages` | `(I)Z` | `0xf7608` |
| `removeMessages` | `(I)V` | `0xf7610` |
| `sendMessage` | `(II)V` | `0xf7618` |
| `sendMessageDelayed` | `(IIJ)V` | `0xf7620` |

The one-entry JNI registration table at `0xf6b70` binds
`native_handleMessage`, signature `(II)V`, to `0x97198`. That bridge
forwards the two integers to `HandlerImpl::HandleMessage`, `0x971a4`.

The [synchronizer's saved thread identifier](predictor-callback-findings.md#registration-captures-a-consumer-and-a-thread-identifier)
only selects inline versus queued delivery. It does not select a looper.
If registration occurs on a non-main thread, matching-thread completion
still executes inline there, while a completion on a different thread
uses the main looper. The recovered API does not guarantee delivery back
to an arbitrary registration thread.

## Each completion registers a separate native Handler

Predictor `TouchSynchronizer::OnTouchAsync`, `0x39670`, allocates a
64-byte payload at `0x396a8` containing the event pointer, copied
40-byte timing entity, consumer pointer and event-deletion flag. It
allocates an additional 80-byte Handler at `0x396f0`, constructs it at
`0x396fc`, and sends its message at `0x39704`.

Predictor GOT `0x41d58` resolves to `TouchSynchronizer::HandlerCallback`,
`0x39778`. The Handler constructor receives that callback and the payload
pointer as a 16-byte pair. Base `Handler(Callback*)`, `0xaf960`, registers
the Handler at `0xaf9a4` and copies the pair through `0xaf9b0`:

| Native Handler offset | Role |
| --- | --- |
| 0 | Vtable pointer |
| 48 | Optional alternate callable, null for this constructor |
| 64 | `HandlerCallback` function pointer |
| 72 | Payload pointer |

The local Handler pointer is used to send the message and is not stored
back into the synchronizer or predictor. The synchronizer remains a
16-byte consumer/thread pair.

`Handler::SendMessage`, `0xafba0`, supplies argument -1 at `0xafbb8`.
`HandlerImpl::SendMessage`, `0x97c84`, passes two Java integers at
`0x97cec`/`0x97cf0`:

```text
Message.what = low 32 bits of the native Handler address
Message.arg1 = -1
```

The Java message carries neither the event nor the consumer pointer.
Those remain in the native payload, reached through the Handler registry.

## Registry lookup uses a 32-bit address key

`HandlerImpl::Register`, `0x977f0`, stores the low 32 address bits as
the map key at `0x97834`. Map helper `0x97ee4` compares signed 32-bit
keys, returns an existing equal-key node through `0x97f88`, or allocates
a 48-byte node at `0x97f50`. Registration then writes the full Handler
pointer to node offset 40 at `0x97858`, including for an existing key.

Incoming dispatch searches the same map by `Message.what` at
`0x97208`–`0x97234`. A missing key returns without a callback. A match
loads node offset 40 at `0x97250` and invokes Handler slot 0 through
`0x97284`. `Handler::HandleMessage`, `0xafbf8`, loads its callback and
payload at `0xafc18`/`0xafc20` and tail-calls the callback at `0xafc40`.

There is no full-address or generation comparison in this lookup. For
example, hypothetical addresses `0x0000000112345000` and
`0x0000000212345000` have the same key. Registering the second would
replace the registry value used by messages bearing that key. This is
an arithmetic consequence of the recovered map operations, not evidence
that this allocation pattern occurs in the application.

The registry critical section also ends before delivery. Dispatch releases
it at `0x97244`, then reads the located node at `0x97250`. The called
`AutoCriticalSection` destructor, `0x99cc8`, invokes mutex unlock at
`0x99cdc`. No retained ownership or second lookup occurs between unlock
and invocation. Concurrent deregistration would therefore need an outer
lifetime rule; this map lock alone does not keep the node or Handler alive
through callback delivery.

## Normal callback completion releases its own resources

For a non-null payload, Predictor `HandlerCallback`, `0x39778`, does:

1. If payload consumer offset 48 is non-null, copy the timing entity and
   invoke consumer slot 16 at `0x397f0`.
2. If the deletion flag is set and the event is non-null, destroy and
   delete that event at `0x39808`/`0x39810`.
3. Delete the Handler through slot 16 at `0x39824` when it is non-null.
4. Delete the payload at `0x3982c`.

Base Handler vtable `0xebf40`, primary address point `0xebf50`, has:

| Slot | Function |
| --- | --- |
| 0 | `HandleMessage`, `0xafbf8` |
| 8 | Non-deleting destructor, `0xafa88` |
| 16 | Deleting destructor, `0xafb34` |

The callback therefore deletes its Handler; slot 16 is not another
message dispatch. The Handler dispatch tail-call also avoids reading the
Handler after that callback returns. The null-payload branch at Predictor
`0x39798` returns without these cleanup operations, but ordinary enqueue
supplies the allocated payload.

## Handler destruction cancels by Handler ID

The Handler destructor clears callback offset 64 at `0xafaa8`, destroys
any alternate callable, then invokes `HandlerImpl::Deregister` at
`0xafae8`. Deregistration, `0x978b8`, holds the registry critical section
while it:

1. Calls `RemoveMessages` at `0x978fc`.
2. Erases the low-32-bit key through `0x9790c`.
3. Releases the critical section at `0x97914`.

`RemoveMessages`, `0x97968`, passes that key to Java at `0x979c8`/
`0x979cc`. The erase helper `0x97ff8` unlinks and deletes the map node
at `0x9805c`/`0x98064`. It does not delete the mapped Handler or payload.
The Handler destructor itself does not free the raw payload at offset 72.

Cancellation is consequently scoped to a Handler ID, not a predictor,
consumer or stroke. Destroying a Handler before dispatch removes its
pending messages when the Java bridge is available, but payload/event
cleanup still requires an owner outside that generic destructor.

## Predictor teardown does not own these cancellations

Predictor `TouchSynchronizer` destruction at `0x39500` is a single return.
Its direct consumer setter at `0x39504` only changes its consumer field.
`PredictorBase::SetTouchConsumer`, `0x301d8`, replaces the 16-byte
synchronizer at `0x301fc`–`0x3021c`. None of these operations enumerates
the Handlers already allocated by `OnTouchAsync`, and queued payloads
retain the consumer pointer copied at enqueue time.

The [presenter teardown trace](predictor-reconfiguration-findings.md#presenter-teardown-deletes-the-proxy-before-its-consumer)
establishes that Composer deletes the predictor proxy, including joining
the neural worker, before deleting the consumer at Composer `0x4d7654`.
That join establishes worker termination; it does not dispatch the Java
queue or destroy these per-completion Handlers.

A local ownership reconstruction therefore permits this sequence:

```text
worker completion copies consumer C into payload P and enqueues Handler H
predictor is deleted and its worker finishes joining
presenter deletes consumer C
main-looper message resolves H and callback reads P's unchanged C pointer
```

The callback checks whether the copied pointer is null, not whether its
consumer is still registered or allocated. Establishing whether the
application actually permits this sequence requires the outer teardown
callers and thread ordering, or a device reproduction. The local trace
does not establish a reproduced use-after-free.

## A true return does not confirm enqueue success

The Java `sendMessage(II)V` wrapper discards Android's send result. Native
`HandlerImpl::SendMessage` also has an early logging return if the saved
Java VM, Java object or send method ID is missing, tested at `0x97cb0`,
`0x97cbc` and `0x97cc8`. It does not call Java on those branches.

Predictor `OnTouchAsync` nevertheless returns true at `0x3971c`; it neither
checks an enqueue result nor releases the Handler/payload after a normal
send return. Under the missing-bridge condition, the local sequence leaves
registered native resources without a queued completion to release them.
Whether that condition is reachable after successful application startup
remains unresolved. This Boolean must not be interpreted as proof of
delivery or cleanup.

## Validation and remaining work

The APK digest, freshly extracted DEX, and Base/Predictor/Composer native
byte streams were verified. JNI strings and registration, imported Handler
bindings, vtable slots, allocation sizes, key truncation, lock boundaries
and cleanup instructions were checked against the native bytes.

Disposable ownership reconstruction covered normal delivery, consumer
replacement, predictor/presenter teardown with a pending message,
Handler cancellation, missing bridge state, colliding IDs, and node
removal between registry lookup and invocation. These are checks of the
recovered operations, not execution of the Android queue or native code.

The [writing-view teardown trace](writing-view-teardown-findings.md)
now follows Java close through the native raster owner chain and separates
the writing view's own Handler cancellations. The separate
[main-editor Composer trace](composer-close-findings.md) covers application
release, capture callbacks and native Composer ownership. Prediction ordering
inside capture and other cleanup delegates remains unresolved. No SDK code,
saved-stroke format rule, corpus fixture or device execution changed.
