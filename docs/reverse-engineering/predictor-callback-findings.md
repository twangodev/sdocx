# External predictor selection and callback delivery

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenPredictor.so` from the APK identified in
the [knowledge base](README.md#sources-and-validation). The predictor
library is bundled in that APK; its ARM64 entry is 1,087,584 bytes.

This trace connects the external predictor to Composer's
`TouchPresenter::OnPredictTouch`. It establishes selection, registration,
payload copying and event lifetime. The input and output are temporary
live-touch events, separate from the [stored-stroke path](stroke-prediction-findings.md).

## Composer creates a proxy and registers its consumer

The `TouchPresenter` constructor at Composer `0x4d6f20` creates a
40-byte `PredictorProxy` and stores it in presenter member 64 at
`0x4d7094`. GOT `0x5a2b18` resolves to its vtable at `0x5810a0`,
with primary address point `0x5810b0`; RTTI names
`SPen::PredictorProxy`.

The constructor then allocates a 16-byte `PredictorTouchConsumer` at
`0x4d70a4`–`0x4d70a8`. It stores that consumer in presenter member
368 and the presenter pointer in consumer member 8 at
`0x4d70bc`/`0x4d70c4`. GOT `0x5a2bd8` resolves to consumer vtable
`0x580fc8`, whose primary address point is `0x580fd8`.

At `0x4d70c8`–`0x4d70d0`, the presenter registers the consumer through
proxy slot 40. That slot is `0x4dae14`, which saves the consumer at
proxy offset 16 and, when a concrete predictor already exists at
offset 8, forwards registration through its slot 40. A later predictor
creation also forwards the saved consumer at `0x4dac6c`–`0x4dac78`.

## The proxy loads a bundled factory

`PredictorProxy::SetPredictorType`, Composer `0x4daa1c`, is identified
by the signature string at `0x1e00a5`. Its dynamic loading sequence is:

| Operation | Composer instruction | Evidence |
| --- | --- | --- |
| Open `libSPenPredictor.so` | `0x4dabc8` | String `0x1cf437`; PLT `0x55e920` resolves to `dlopen` |
| Resolve `CreatePredictor` | `0x4dabe0` | String `0x1f49f1`; PLT `0x55e930` resolves to `dlsym` |
| Call the resolved factory | `0x4dab68` | Function pointer held at `0x5b7860` |
| Store the predictor | `0x4dab6c` | Proxy offset 8 |

The integer mapping at `0x4daa84`–`0x4daaec` supplies the factory's
first two integer arguments as follows:

| Requested prediction value | Factory kind | Length argument |
| --- | --- | --- |
| 1 | 1 | 1 |
| 2 | 1 | 2 |
| 3 | 1 | 3 |
| 4 | 1 | 4 |
| 5 | 2 | 1 |

Other requested values take the disabled selection branch. An existing
matching selection can return without allocation; the table describes
the factory arguments when creation proceeds, not the active device
configuration.

The exported Predictor `CreatePredictor`, `0x359f0`, resolves those
kind values concretely:

- Kind 1 allocates 1920 bytes and calls `NNPredictor(bool)` at
  `0x35a28`–`0x35a3c`.
- Kind 2 allocates 1776 bytes, constructs `PredictorBase`, then installs
  `LinearPredictor`'s vtable and minimum count 11 at
  `0x35a48`–`0x35a6c`.
- Other kind values return null at `0x35ab0`.

The factory applies DPI, prediction length and refresh rate through
slots 64, 184 and 144 at `0x35a78`, `0x35a90` and `0x35aa4`.
Predictor vtables `0x40868`/`0x40988` identify the linear and neural
classes. Both bind slot 40 to `PredictorBase::SetTouchConsumer`,
`0x301d8`.

## Registration captures a consumer and a thread identifier

`SetTouchConsumer` stores the consumer at predictor offset 32,
replaces its `TouchSynchronizer`, and stores the new synchronizer at
offset 40. The 16-byte synchronizer contains:

| Offset | Value | Store |
| --- | --- | --- |
| 0 | Registered consumer pointer | `0x30210` |
| 8 | `Thread::self()` result during registration | `0x30218` |

`TouchSynchronizer::OnTouch`, `0x3950c`, compares the saved thread
identifier with a fresh `Thread::self()` result at `0x39550`–`0x39558`.
Equality selects `OnTouchSync`, `0x395e0`; inequality selects
`OnTouchAsync`, `0x39670`. Both preserve all 40 bytes of the
`MotionEventEntity` payload.

The synchronous branch copies the payload to its stack and invokes
consumer slot 16 at `0x39620`–`0x39628`. The asynchronous branch
allocates a 64-byte message containing:

| Offset | Value |
| --- | --- |
| 0 | Motion-event pointer |
| 8–47 | Copied 40-byte timing entity |
| 48 | Consumer pointer |
| 56 | Event-deletion flag |

Those stores occur at `0x396bc`–`0x396d0`. It creates a `Handler`
with `TouchSynchronizer::HandlerCallback`, `0x39778`, and calls
`Handler::SendMessage` at `0x39704`. GOT `0x41d58` binds the callback.
The callback reconstructs the same entity and invokes consumer slot 16
at `0x397cc`–`0x397f0`.

This establishes the branch and payload behavior. It does not by itself
establish which looper handles the queued message; that requires the
separate `Handler` implementation.

## Completion delivers a generated event or a null event

`PredictorBase::OnPredictionComplete`, Predictor `0x2fb28`, calls
`GetPredictedPenEvent` at `0x2fb74`. If an event exists, it reapplies
the saved transform at `0x2fbb4`. Both the non-null and null event
paths can reach `TouchSynchronizer::OnTouch` at `0x2fbe4` when a
synchronizer is registered.

The completion call copies the supplied entity unchanged and sets the
event-deletion flag to true at `0x2fbd8`. After consumer delivery:

- `OnTouchSync` destroys and deletes a non-null event when the flag is
  true at `0x3962c`–`0x39640`.
- `HandlerCallback` does the same at `0x397f4`–`0x39810`, then releases
  its handler/message resources.

Composer consumer slot 16 is `0x4da4bc`. It copies the full entity,
loads its presenter pointer and calls `TouchPresenter::OnPredictTouch`
at `0x4da4f0`. It does not manufacture new timing values during this
handoff.

## An empty proxy has its own direct-delivery path

Proxy slot 32, Composer `0x4dad78`, forwards an input event to concrete
predictor slot 32 when one exists. With no concrete predictor and a
registered consumer, `0x4dadd0`–`0x4dade8` instead constructs an
all-zero 40-byte entity and invokes consumer slot 16 with the original
event argument.

That fallback must not be confused with a completed prediction carrying
measured frame timing. Its zero fields come from explicit stores in the
proxy. This function does not delete that input event.

## Validation and remaining work

The APK digest and both native byte streams were verified. Exported
functions, RTTI, vtable targets, dynamic-loader relocations and all
documented instruction addresses were checked. The 40-byte payload
copies and the two event-deletion paths were followed separately.

The timing-field producer and neural task contents need their own trace.
Runtime predictor selection, handler/looper delivery and device behavior
remain unmeasured. No SDK code changed, and no new SDOCX fixture or
device execution was used.
