# Unbuffered drawing cadence

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenEngine.so` from the
[identified APK](README.md#sources-and-validation).

Composer's `UBDDrawChrono` controls separate real-bitmap and fallback
presentation paths. It uses time/VSync equations equivalent to the
[neural pacing backends](predictor-chrono-findings.md), but owns different
state and resets after drawing calls. It must not be modeled as the neural
predictor's task chronometer.

## The presenter owns a separate chronometer and receiver

The presenter allocates 160 bytes at `0x4d701c`–`0x4d7020`, passes short
argument 5 to constructor `0x4db3a8` at `0x4d702c`, and stores the object
at presenter offset 520 through `0x4d7030`.

GOT `0x5a2bf0` resolves to vtable `0x5812b0`, primary address point
`0x5812c0`. RTTI `0x59bdc8` names `SPen::UBDDrawChrono`. Its layout is:

| Offset | Role | Initial state |
| --- | --- | --- |
| 0 | Primary vtable pointer | `0x5812c0` |
| 8 | Secondary VSync receiver | Vtable address point `0x581318` |
| 16 | 40-byte time-backend pointer | Constructed with short 5 |
| 24 | 32-byte VSync-backend pointer | Constructed separately |
| 32 | Active backend pointer | Time backend |
| 40 | Integer backend mode | 0 |
| 48 | Latest received VSync timestamp | 0 |
| 64 | Registration callable storage | Empty until installation |
| 96 | Registration callable pointer | 0 |
| 112 | Removal callable storage | Empty until installation |
| 144 | Removal callable pointer | 0 |

Backend stores are at `0x4db3e8`/`0x4db410`; initial timestamp, callable
pointers and mode are cleared at `0x4db404`–`0x4db414`.

Primary slot 64 is `0x4db64c`, which stores a supplied timestamp at offset
48. Secondary slot 0, `0x4db654`, stores it at receiver-relative offset
40, the same object member. Its vtable offset-to-top is -8. Neither setter
changes or resamples the timestamp.

## Callback installation and removal are recovered; registration remains open

The presenter constructs two callables and passes their pair to
`UBDDrawChrono::SetVSyncEventCallback`, `0x4db5f8`, at `0x4d71c8`.
The method's signature string is at `0x1f248e`. It copies pair offset 0
into object offset 64 at `0x4db62c`–`0x4db634`, and pair offset 48 into
object offset 112 at `0x4db638`–`0x4db648`.

| Pair offset | Callable address point | Invocation slot | Provider operation |
| --- | --- | --- | --- |
| 0 | `0x581058` | `0x4da75c` | Register receiver through provider slot 16 |
| 48 | `0x581000` | `0x4da55c` | Remove receiver through provider slot 24 |

Both callables obtain `IVSyncProvider::GetInstance` through PLT `0x55e860`.
The [provider trace](vsync-delivery-findings.md) establishes the receiver
vector, Java subscription and unchanged frame-timestamp dispatch behind
those provider slots.

The assignment helpers `0x4db790`/`0x4db824` and swap helper `0x4da5b0`
copy, move and destroy callable state. Their clone/destruction slot calls
do not invoke the registration operation at callable slot 48. The traced
constructor and setter therefore install callbacks without registering
this receiver themselves.

Destruction is explicit: `0x4db4a4` loads the removal callable, forms the
receiver address `this + 8` at `0x4db4b0`, and invokes callable slot 48 at
`0x4db4c8` when non-null. The presenter reaches this destructor through
the owned object's deleting-destructor slot at `0x4d7680`.

The traced construction, installation, pacing and teardown functions do
not identify the initial registration invocation. This remains an unresolved
caller edge, not evidence that frame delivery occurs or never occurs.
The neural predictor's action-based registration sequence must not be
assigned to this distinct receiver.

## A drawing check selects its backend before sampling time

The check at `0x4db65c` receives a Boolean request in `w1` and a float
refresh rate in `s0`. It reads registration pointer 96 at `0x4db66c`,
converts presence to 0/1 and ANDs it with the request at `0x4db678`–
`0x4db680`. The result selects the backend through `0x4db684`.

`SetCurrentUBDDrawChrono`, `0x4db5a4`, maps mode 1 to member 24 and
other modes to member 16. It stores the active pointer at offset 32
through `0x4db5f0`. An unchanged mode returns; a changed mode does not
clear either backend. The observed presenter call sites both supply
request value 1.

The check then:

1. Reads `GetNano` at `0x4db688`.
2. If latest VSync member 48 is nonzero, aligns that reading to a period
   `trunc_to_i64(1_000_000_000.0 / f64(rate_f32))` at `0x4db698`–`0x4db6c0`.
   Signed-division correction floors to a frame boundary. A zero origin
   leaves the clock reading unchanged.
3. Forwards the aligned origin through slot 32 at `0x4db6d0` and the
   supplied rate through slot 40 at `0x4db6e4`.
4. Calls active-backend `IsExpired` through slot 16 at `0x4db6f4`.
5. Calls `SaveResetTime` through slot 24 at `0x4db70c` only when due,
   then returns that due result at `0x4db710`.

Mode selection requires an installed callable, not a nonzero received
timestamp. Pending registration uncertainty therefore does not prevent
the local code from choosing its VSync backend.

## Backend arithmetic matches neural pacing, with independent storage

GOT entries `0x5a2be0`/`0x5a2be8` resolve to vtables `0x581210`/`0x581260`.
RTTI names `UBDDrawChronoTime` and `UBDDrawChronoVsync`.

The time constructor, `0x4db1b8`, stores the sign-extended short duration
multiplied by 1,000,000 and samples `steady_clock::now` at `0x4db1f0`.
PLT `0x55e1f0`, relocation `0x5ab4f0`, identifies that clock import.
For constructor argument 5, the operations are:

| Operation | Implementation | Effect |
| --- | --- | --- |
| Check due | `0x4db204` | Fresh clock minus offset 16 is strictly greater than 5,000,000 ns |
| Save reset | `0x4db234` | Fresh clock into offset 24 |
| Commit reset | `0x4db260` | Copy offset 24 to offset 16 |

The VSync constructor at `0x4db270` zeroes current and saved origins at
offsets 8/16 and initializes rate/coefficient at 24/28 to 60/0 using
constant `0x1f9720`. For finite coefficients and ordinary positive rates,
its inlined check at `0x4db294` evaluates:

```text
frame_ns = trunc_to_i64(1_000_000_000.0 / f64(rate_f32))
period_us = trunc_to_i64(f32(f32(1_000_000) / rate_f32))
phase_budget_us = trunc_to_i64(f32(coefficient_f32 * f32(period_us)))
phase_age_us = trunc_to_i64(f64(phase_now_ns - current_origin_ns) * 0.001)
phase_due = phase_age_us < 0 or phase_age_us > phase_budget_us
elapsed_ns = elapsed_now_ns - saved_origin_ns
due = elapsed_ns >= 2 * frame_ns or (elapsed_ns >= frame_ns and phase_due)
```

Constants are at `0x1f9150` (float 1,000,000), `0x1f94b0` (double
1,000,000,000) and `0x1f96e8` (double 0.001). Period/budget conversions
occur at `0x4db2c0`–`0x4db2dc`, phase comparisons at `0x4db2fc`–
`0x4db304`, and the outer interval tests at `0x4db34c`–`0x4db364`.
The two clock reads remain separate, including any time spent logging
between them.

Save-reset `0x4db380` immediately copies current origin 8 to saved origin
16. Commit-reset `0x4db39c` does nothing. Thus VSync state advances on
the due check, while time state waits for the later commit call. The
[position coefficient](predictor-position-findings.md) also targets only
the active backend; its time setter is a no-op.

## Two presenter paths apply the drawing gate

Engine configuration primary vtable `0x17bb50`, slot 176, resolves to
getter `0xfaed8`, which returns unbuffered-dispatch byte 123. The following
descriptions apply after reaching their respective presenter branches;
they are not complete drawing-eligibility rules for every input.

On the branch logged as `Present Real Pen Bitmap #1` by string `0x1e5e79`:

- Payload byte 17 must be nonzero at `0x4d8728`–`0x4d872c`.
- Action values 1 or 3 set the finishing byte at presenter offset 344
  through `0x4d8758`–`0x4d8764`.
- Unbuffered dispatch and a clear finishing byte call the chronometer at
  `0x4d87c0`. A false result branches past the drawing call at `0x4d87c4`.
- Buffered dispatch, finishing actions, or a due result reach the member-80
  drawing delegate's slot 24 at `0x4d8810`.
- After that call returns, slot 48 commits the timer reset at `0x4d8820`,
  without testing a drawing result.

The separate tip fallback branch checks action 2 at `0x4d8864` and
presenter `IsTip` byte 8 at `0x4d886c`. With unbuffered dispatch, it calls
the chronometer at `0x4d8894` and skips the fallback on a false result.
Otherwise it invokes `TouchPresenter::OnPredictTouch`, `0x4d94e0`, with
a null prediction event at `0x4d88c0`–`0x4d88c4`. The call receives a
locally constructed timing entity; this is not completion from an inferred
prediction. After return, it commits the drawing timer through slot 48 at
`0x4d88d4`.

These commit calls are unconditional after their drawing/fallback calls
return, including paths that bypassed the check. They do not establish
that pixels were drawn. In contrast, neural
[prediction completion](predictor-chrono-findings.md#completion-commits-the-time-reset-only-when-an-event-exists)
calls its separate reset only when `GetPredictedPenEvent` returns non-null.

## Validation and remaining work

Both native byte streams were matched to the APK. Object/backend/receiver
vtables and RTTI, callable installation/removal, configuration getter,
clock imports, float/double constants, alignment, phase/interval tests and
the two post-call reset sites were checked against ARM64 instructions.

The already reconstructed time/VSync boundary cases apply after verifying
the matching arithmetic here: strict 5 ms equality, negative sub-microsecond
age truncation, phase equality, and one/two-frame boundaries at rates 60,
90 and 120. Separate state checks distinguish saving a due time from
committing it, and a null fallback call from the predictor's null-event
completion condition. None of these checks executes the native library.

Initial registration of the draw receiver, complete fallback eligibility
and measured drawing cadence remain unresolved. The recovered equations
and callback presence alone do not prove active VSync delivery. No SDK code
or corpus fixture changed.
