# External predictor task pacing

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and its bundled `libc++_shared.so` from the
[identified APK](README.md#sources-and-validation).

`NNPredictor::CheckExpiredAndReset`, `0x259f8`, decides whether the
interval before another prediction has elapsed. A true result allows the
base predictor to enter `DoPredict`. This precedes the separate
[neural expiry checks](neural-admission-findings.md), where true means
that work is too late and should be rejected. These predicates have
different roles despite their similar names.

## Neural pacing selects one of two persistent backends

The base constructor creates a 40-byte `TaskChrono` with short argument
5 at `0x2ded4`–`0x2dedc`, storing its pointer at base offset 536
through `0x2dee4`. Its constructor, `0x391e8`, allocates both backends:

| Wrapper offset | Value | Constructor store |
| --- | --- | --- |
| 8 | 40-byte `TaskChronoTime` pointer | `0x39224` |
| 16 | 32-byte `TaskChronoVsync` pointer | `0x39240` |
| 24 | Active backend, initially the time backend | `0x39240` |
| 32 | Integer mode, initially zero | `0x39244` |

GOT entries `0x41d40`, `0x41d48` and `0x41d50` resolve the time,
VSync and wrapper vtables at `0x40df0`, `0x40e40` and `0x40e90`.
Their primary address points are each 16 bytes later. RTTI confirms the
three class identities.

At `0x25a04`–`0x25a34`, neural pacing selects mode 1 only when both
the registration callable at neural offset 1840 is non-null and virtual
`GetUnbufferedDispatch` returns true. Otherwise it selects mode 0.
The callable's identity and installation are established by the
[VSync delivery trace](vsync-delivery-findings.md).

`TaskChrono::SetCurrentTaskChrono`, `0x392a4`, maps mode 1 to the VSync
backend and every other mode to the time backend. It changes only the
active pointer and mode at `0x392c4`/`0x392f0`; switching does not reset
or reconstruct either backend. An unchanged mode returns immediately.

The mode test does not require a nonzero saved VSync timestamp or proof
that a provider callback has already arrived. Installed registration plus
unbuffered dispatch is sufficient to select the VSync backend.

## A neural check updates inputs, checks elapsed time and conditionally saves

The remainder of `CheckExpiredAndReset` performs these operations:

| Operation | Predictor call site |
| --- | --- |
| Read `GetNano()` | `0x25a38` |
| Align that reading through neural slot 232 | `0x25a4c` |
| Set the active backend's current VSync origin through wrapper slot 32 | `0x25a60` |
| Obtain the predictor refresh rate through slot 208 | `0x25a70` |
| Set that rate through wrapper slot 40 | `0x25a80` |
| Call the active backend's `IsExpired`, wrapper slot 16 | `0x25a90` |
| If true, call its `SaveResetTime`, wrapper slot 24 | `0x25aa8` |

The function returns the original expiry result at `0x25aac`. The origin
is the [aligned native clock](predictor-timing-findings.md#the-neural-tasks-later-time-remains-separate),
not a direct copy of the most recent Java callback. With no stored VSync,
alignment returns the sampled clock unchanged.

The wrapper's forwarding functions load the currently active pointer at
offset 24 on each call. They do not retain a task-specific backend or
reset timestamp in the callback entity.

## The time backend uses a strict five-millisecond interval

`TaskChronoTime`, `0x38f68`, sign-extends its short constructor argument
and multiplies it by 1,000,000 at `0x38f78`–`0x38f8c`. It stores that
duration at offset 8 and a fresh `steady_clock::now()` result at offset
16 through `0x38fa4`. The saved candidate reset at offset 24 starts at
zero. For the base constructor's argument, the duration is 5,000,000 ns.

Predictor PLT `0x3c490`, relocation `0x42818`, names
`std::__ndk1::chrono::steady_clock::now`. The bundled C++ library exports
that function at `0xccab0`. It calls `clock_gettime` with clock ID 1 at
`0xccac4`–`0xccac8`, then returns `seconds * 1_000_000_000 + nanoseconds`
at `0xccad0`–`0xccadc`. Its PLT target is identified by relocation
`0x142270`, rather than the disassembler's nearest-symbol label.

`TaskChronoTime::IsExpired`, `0x38fb4`, obtains another clock reading
and evaluates at `0x38fc8`–`0x38fd4`:

```text
due = now_ns - reset_ns > duration_ns
```

Equality at exactly 5 ms is false. `SaveResetTime`, `0x38fe4`, samples
the clock again and writes offset 24 at `0x38ff8`. It leaves the active
reset at offset 16 unchanged. `ResetWithSavedTime`, `0x39010`, later
copies offset 24 into offset 16. VSync origin, refresh-rate and pen-location
setters are no-ops in this backend.

There is also a separate exported `TaskChronoTime::CheckExpiredAndReset`,
`0x39020`. It checks expiry and then replaces offset 16 with a fresh clock
reading regardless of the result. That function is not the sequence used
by the neural virtual slot: neural pacing calls `IsExpired` and conditionally
`SaveResetTime` through the wrapper.

## VSync pacing combines a phase threshold with one- and two-frame intervals

The VSync constructor, `0x39080`, establishes:

| Backend offset | Value | Initial value |
| --- | --- | --- |
| 8 | Current aligned origin | 0 |
| 16 | Origin saved by the last due check | 0 |
| 24 | Float refresh rate | 60 |
| 28 | Float pen-location coefficient | 0 |

The two float defaults come from `0x15728`, loaded and stored at
`0x3908c`/`0x39098`. This constructor does not use the short duration
argument. The setters at `0x391cc`, `0x391d4` and `0x391e0` store the
origin, rate and coefficient respectively, without a local clamp.

For ordinary positive finite rates, the inner `IsExpired(long long, float)`,
`0x39114`, reconstructs:

```text
period_us = trunc_to_i64(f32(f32(1_000_000) / refresh_rate_f32))
phase_budget_us = trunc_to_i64(f32(f32(period_us) * coefficient_f32))
phase_age_us = trunc_to_i64(f64(phase_now_ns - current_origin_ns) * 0.001)
phase_due = phase_age_us < 0 or phase_age_us > phase_budget_us
```

The float division, integer truncation, conversion back to float and
coefficient multiplication occur at `0x39130`–`0x39140`. The separate
clock reading and double-based age conversion occur at `0x39144`–
`0x3915c`. Comparisons at `0x39160`–`0x39168` are strict at the upper
bound. Negative ages from -999 through -1 ns truncate to zero; -1000 ns
becomes -1 microsecond and makes this inner predicate true.

The outer `IsExpired`, `0x390a4`, calculates a separate period using
double precision and obtains another clock reading after the inner call:

```text
frame_ns = trunc_to_i64(1_000_000_000.0 / f64(refresh_rate_f32))
elapsed_ns = elapsed_now_ns - saved_origin_ns
due = elapsed_ns >= 2 * frame_ns or (elapsed_ns >= frame_ns and phase_due)
```

Period conversion is at `0x390c8`–`0x390d0`; the later clock read and
interval comparisons are at `0x390dc`–`0x390f8`. At exactly one frame,
the phase predicate still decides. At exactly two frames, the outer check
returns true regardless of phase. A negative inner age alone cannot bypass
the outer interval requirement.

`SaveResetTime`, `0x391c0`, immediately copies current origin offset 8 to
saved origin offset 16. It stores the aligned origin, not either fresh
clock reading. VSync `ResetWithSavedTime`, `0x391dc`, is a no-op. Thus a
due check advances this backend even if later model work produces no event.

## Completion commits the time reset only when an event exists

`PredictorBase::OnPredictionComplete`, `0x2fb28`, obtains the generated
event at `0x2fb74`. The null test at `0x2fb7c` skips the reset block when
no event exists. Otherwise the code enters the predictor critical section,
loads the wrapper from offset 536 at `0x2fb8c`, and calls wrapper slot 48
at `0x2fb98`: `ResetWithSavedTime`.

This happens before checking for a registered synchronizer at `0x2fba4`.
The condition is event existence, not successful consumer delivery. Null
completion can still deliver a callback through the
[ordinary completion path](predictor-callback-findings.md).

For the time backend, a due check followed by an empty result leaves the
old active reset intact. Another input may therefore be due immediately.
A nonempty result commits the most recently saved check-time reading,
not a new completion-time reading. The saved value is shared backend state;
this trace does not establish task-specific ownership under concurrent work.
For the VSync backend, completion does not change the origin saved earlier.

## Linear prediction and coefficient forwarding differ

Linear virtual slot 248 is `LinearPredictor::CheckExpiredAndReset`,
`0x24d50`, which returns true unconditionally. It does not run either
backend's expiry predicate. The neural pacing formulas must not be assigned
to the linear implementation merely because both inherit the same wrapper.

Base `SetPenLocationInScreenCoef`, `0x301c8`, forwards its float through
wrapper slot 56. The wrapper at `0x39348` forwards only to the backend
active at that moment. Calling it while the time backend is active has
no effect on the VSync backend's stored coefficient. Switching mode later
does not replay the earlier setter. The
[position-coefficient trace](predictor-position-findings.md) recovers
Composer's calculation and setter-before-dispatch ordering. Active device
coefficient values remain unmeasured.

## Boundary reconstruction and validation

Disposable arithmetic gives these independently calculated periods and
phase budgets:

| Refresh rate | Frame interval, ns | Coefficient 0, us | Coefficient 0.5, us | Coefficient 1, us |
| --- | --- | --- | --- | --- |
| 60 | 16,666,666 | 0 | 8,333 | 16,666 |
| 90 | 11,111,111 | 0 | 5,555 | 11,111 |
| 120 | 8,333,333 | 0 | 4,166 | 8,333 |

At rate 120 and coefficient 0.5, an age of 4,166,999 ns truncates to
4,166 us and does not pass the phase check. At 4,167,000 ns it passes.
The outer elapsed-frame condition must also hold. A freshly constructed
VSync backend with saved origin zero is due whenever its later clock
reading is at least two frame intervals, even when phase is still zero.

Both native byte streams were matched to the APK. Constructors, constant
bytes, class identities, vtable forwarding, the imported clock implementation
and documented instructions were checked. Disposable reconstruction covered
strict time equality, pending reset replacement, null/non-null completion,
all four mode combinations, microsecond truncation, and the one/two-frame
boundaries at three rates.

These checks reconstruct static control flow and arithmetic; they do not
execute native prediction or measure device scheduling. The
[base dispatch trace](predictor-dispatch-findings.md) recovers callback
decisions around this gate, and the
[position trace](predictor-position-findings.md) identifies the coefficient
caller chain. The [worker trace](predictor-worker-findings.md) separately
recovers task routing, pending ownership, wait predicates and input capture.
No SDK code or corpus fixture changed.
