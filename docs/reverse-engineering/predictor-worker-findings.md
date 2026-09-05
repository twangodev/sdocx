# External prediction worker scheduling

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and `libSPenComposer.so` from the
[identified APK](README.md#sources-and-validation).

This trace covers task dispatch, the worker state machine, input capture,
and normal-return ownership. The scheduling examples reconstruct possible
instruction ordering; they do not establish how often those orderings occur
on a device. These are transient input-prediction semantics, not additional
fields in a saved SDOCX stroke.

## Unbuffered dispatch selects inline execution

`NNPredictor(bool)`, `0x24e2c`, initially clears its worker pointer at
offset 1784. A true constructor argument allocates a 120-byte
`PredictorThread` at `0x24ea8`–`0x24eb8`. That worker owns a TFLite
holder, but the predictor also allocates a separate local holder and stores
it at offset 1904 through `0x24f1c`. A false argument creates only the
local holder. Both present holders receive the selected model.

Composer copies its presenter's Boolean constructor argument into proxy
byte 33 at `0x4d7068`/`0x4d7078`. Proxy creation later passes that byte
as the factory's third integer argument at `0x4dab44`–`0x4dab5c`.
`CreatePredictor` normalizes that argument and passes it to the neural
constructor at `0x35a38`/`0x35a3c`. The
[device-policy trace](predictor-device-policy-findings.md) recovers the
model-prefix and cached-SDK checks producing the original argument;
the active device values remain unmeasured.

After the earlier [admission checks](neural-admission-findings.md),
`NNPredictor::DoPredict` selects its execution route at
`0x25c08`–`0x25c20`:

| Worker exists | Unbuffered dispatch | Route and holder |
| --- | --- | --- |
| No | Either | Inline, predictor-local holder |
| Yes | True | Inline, predictor-local holder |
| Yes | False | Worker, worker-owned holder |

The inline branch allocates a task, calls `SetTFParams` at `0x25cb0`,
calls `Run` at `0x25cc0`, then invokes the deleting destructor through
`0x25cf4`. The worker branch allocates the same task layout and calls
`PredictorThread::Execute(task)` at `0x25e50`.

## The worker has one task pointer and four states

The constructor at `0x35dc0` establishes this layout:

| Offset | Member | Initial value |
| --- | --- | --- |
| 0 | Pending or running task pointer | Null |
| 8 | `SPen::Thread` pointer | Newly allocated thread |
| 16 | Condition variable | Initialized storage |
| 64 | Mutex | Initialized storage |
| 104 | State integer | 0 |
| 108 | Busy byte | 0 |
| 112 | Owned 208-byte TFLite holder | Newly allocated holder |

Thread construction at `0x35e3c` uses name `PredictorThread`, the
worker object as callback data, and priority argument -9. The callback
GOT entry `0x41d28` resolves to `ThreadFunc`, `0x35e98`, which forwards
to the worker loop. The constructor starts the thread at `0x35e48`
and calls `WaitInit` at `0x35e50`.

`SetState`, `0x35f9c`, locks the mutex at `0x35fbc`. Its comparisons
at `0x35fcc`–`0x36034` permit only these changes:

| Current state | Permitted different states |
| --- | --- |
| 0, starting | 1 |
| 1, idle | 2, 3 |
| 2, pending or running | 1, 3 |
| 3, stopping | None |

The descriptive names are inferred from use; exported enum names have
not been recovered. An unchanged state logs and returns without consuming
the supplied task. Other disallowed transitions also leave it unconsumed.

Entering state 2 stores the supplied task at `0x360b8`, then calls its
`SetTFParams` slot with holder offset 112 at `0x360c8`. If the task
member is already non-null, that branch logs and skips the store and bind.
Entering state 1 with a task already present performs the work:

1. Set busy to 1 at `0x36054`.
2. Invoke task slot 16, `Run`, at `0x36060`.
3. Invoke task slot 8, the deleting destructor, at `0x36074`.
4. Clear the task and busy members at `0x3607c`/`0x36080`.

Only afterward does the shared tail write state 1 at `0x360d0`, notify
waiters at `0x360d4`, and unlock at `0x360e8`. Consequently `Run`
executes while this mutex is held and while the stored state is still 2.
The busy byte distinguishes running work from pending work in that state.
This layout and control flow provide no FIFO of pending tasks.

## WaitInit and Wait have different predicates

`WaitInit`, `0x35e9c`, locks the mutex and waits on the condition
variable while state remains 0. The condition call at `0x35ee8` loops
through `0x35ef0`/`0x35ef4`. It is a startup barrier.

`Wait(bool)`, `0x36134`, also acquires the mutex. Its initial branch is
based on `(state != 1) XOR argument` at `0x36178`–`0x36184`. When
that expression's low bit is zero, it enters a second test that waits
only while state equals 1 at `0x361bc`–`0x361d8`.

For ordinary Boolean arguments and state unchanged while the lock is held:

| State after acquiring the mutex | `Wait(false)` | `Wait(true)` |
| --- | --- | --- |
| 0 | Returns | Returns |
| 1 | Waits until state differs from 1 | Returns |
| 2 | Returns | Returns |
| 3 | Returns | Returns |

`Wait(true)` can block acquiring the mutex while a task is running. It
does not wait for a pending state-2 task to run, and releases the mutex
before returning. It therefore supplies neither a queue-drain guarantee
nor a lock covering its caller's subsequent operations.

The worker loop at `0x3628c` initially requests state 1, then calls
`Wait(false)` at `0x362b4`. A state-2 result causes `SetState(1)` at
`0x362d0`, which runs and deletes the task as above. It waits again
at `0x362dc`. A wake observed in any state other than 2 exits the loop.

## Execute reports the busy precheck rather than confirmed acceptance

`Execute(task)`, `0x361e0`, reads busy before acquiring the mutex.
When busy is set, it deletes the supplied task at `0x36244` and returns
false. Otherwise it calls `SetState(2, task)` at `0x36208` and then
sets its return value to true at `0x36224`, without checking whether
the state change consumed the task.

The neural caller returns on true at `0x25e54`. On false, it takes
the direct completion path through `0x25e78`–`0x25e8c`, relying on
`Execute` to have disposed of the rejected task.

A reconstructed pending-work ordering exposes the distinction:

| Step | State | Busy | Stored task | Result |
| --- | --- | --- | --- | --- |
| Ready | 1 | 0 | Null | — |
| Submit A | 2 | 0 | A | True |
| Submit B before the worker starts A | 2 | 0 | A | True; same-state request ignored |
| Worker requests state 1 | 2 during `Run` | 1 | A | A runs and is deleted |
| Normal completion | 1 | 0 | Null | B has neither run nor been deleted by this path |

This is a statically reachable ownership gap in the isolated call sequence.
It is not evidence of a measured application leak: active dispatch mode,
input pacing, and outer caller coordination still determine whether the
sequence occurs. Even a single submitting thread can issue the two calls
before the worker is scheduled; multiple producers are not required by
this reconstruction.

Conversely, a caller that samples busy as false just before a run starts
may block on `SetState`'s mutex and submit after that run finishes.
`Execute` is therefore not an unconditional immediate rejection whenever
execution overlaps its call. `IsBusy`, `0x36270`, reads only busy;
`StartJob`, `0x36280`, directly requests state 2 without that precheck
or rejection cleanup.

## Task metadata is captured before retained points

GOT `0x41cf0` resolves to the task vtable at `0x40c00`, with primary
address point `0x40c10`. Slots 8, 16 and 24 identify the deleting
destructor at `0x2be64`, `Run` at `0x2c530`, and `SetTFParams` at
`0x2be88`.

The task constructor and both inline construction paths copy the 40-byte
[callback entity](predictor-timing-findings.md) into offsets 48–87 and
the later aligned clock into offset 88. They retain the predictor pointer
at offset 8. They do not copy a retained-point vector.

`SetTFParams` binds the current tool's holder entry. It copies the model
pointer to task offset 16 at `0x2befc`, two inference pointers to offsets
24 and 32 at `0x2bf14`, and a reference into the holder entry to offset
40 at `0x2bf0c`. It also obtains DPI and calculates the normalization
factor, stored at `0x2bf78`. The worker performs this binding during
submission, while its state mutex is held.

Only later, in `Run`, does `CopyRealPointsToVector` copy retained points
at `0x2c6a0`. After that helper returns, a separate predictor critical
section copies the 88-byte last-unfiltered record at `0x2c6b0`–`0x2c6c8`.
The two copies are separate critical-section intervals. `Run` then asks
the predictor for its current minimum count at `0x2c6e4` and compares
that value with the copied vector length at `0x2c704`.

A delayed task can thus combine earlier callback metadata and model
bindings with later retained points and a later minimum-count query.
There is no complete immutable input snapshot in the submitted object.
Actual changes between these operations require caller activity that this
static trace does not measure.

## Stop and model changes do not inherently drain pending work

The worker destructor at `0x35f2c` requests state 3 at `0x35f44`,
joins the thread at `0x35f4c`, then destroys the thread, holder, mutex,
and condition variable. Entering state 3 writes and notifies without
running or deleting the task member. The destructor itself does not
visit that member.

If a task already holds the mutex, the stop request waits for its normal
completion. If stop changes pending state 2 to state 3 before the worker
executes it, the worker can exit with the task still stored. The traced
destructor path supplies no deletion for that pending task. This is a
second ownership boundary to test against caller lifecycle constraints,
not a demonstrated device failure. `CleanSession`, `0x36100`, is a
single return instruction and provides no additional cleanup.

`PredictorThread::SetModel`, `0x36104`, calls `Wait(true)` before
updating the holder. `NNPredictor::SetPredictionLength` similarly calls
`Wait(true)` at `0x253e0`, then updates worker and local holders. It
stores the new prediction ID earlier, at `0x253bc`. These calls cannot
be described as draining pending work. The
[model lifecycle trace](neural-lifecycle-findings.md) follows holder
replacement and the lifetime of already-bound inference pointers.

## Validation and follow-up

Both native byte streams were matched to the APK. The documented
instruction addresses, task vtable slots, and imported thread, mutex,
condition-variable, and worker functions were checked against ELF bytes
and relocations.

A disposable state model checked all 16 ordinary state transitions,
all eight Boolean wait cases, four execution-route combinations, busy
rejection, successive submissions before execution, and stop before or
after task completion. These checks validate the reconstruction, not
native execution or the absence of additional application synchronization.

The model lifecycle trace establishes that replacement destroys resources
referenced by earlier task bindings. Which outer callers serialize model
changes and teardown remains unresolved. Saved SDOCX/PDF pairs alone cannot
resolve these scheduling questions; they need runtime input and lifecycle traces. No SDK code or
corpus fixture changed.
