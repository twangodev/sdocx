# External predictor dispatch and forced completion

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and `libSPenBase.so` from the
[identified APK](README.md#sources-and-validation).

`PredictorBase::Predict`, `0x2e7e4`, makes separate decisions about calling
`DoPredict`, calling `OnPredictionComplete` directly, and returning true
or false. Its return value does not establish that inference ran or that
a non-null prediction reached the consumer.

This trace connects the [speed gate](predictor-speed-findings.md),
[task pacing](predictor-chrono-findings.md) and
[callback delivery](predictor-callback-findings.md). It describes native
live-input control flow, not stored SDOCX records.

## Prediction length zero exits before input admission

After its input-transform and optional diagnostic work, the base predictor
reads integer member 8 at `0x2ead4`. If zero, it branches at `0x2ead8`
to the return-false path at `0x2ec18`–`0x2ec3c`. That path does not call
`AddRealPenEvent`, the chronometer, `DoPredict` or `OnPredictionComplete`.

The member identity follows from `SetPredictionLength`, `0x2fdb4`,
which writes offset 8 at `0x2fdd0`, and `GetPredictionLength`, `0x30250`,
which reads it. A nonzero value passing this check is not proof that its
[model holder](neural-model-findings.md) or interpreter is available.

## Input status and measured speed select the first branch

For a nonzero prediction length, `AddRealPenEvent` is called at `0x2eae4`.
Its packed return value is assembled at `0x2f318`: low word status,
high word newly added record count. The caller saves the entire return
at `0x2eae8` and extracts the high word at `0x2eaec`.

The low status bit is named `needPredict` in the diagnostic string at
`0x143ac`. The later branch checks that bit at `0x2eb74`. It is not the
neural model's exact-window comparison; that happens inside the
[prediction task](neural-feature-findings.md#the-task-requires-an-exact-sample-count).

Before dispatch, the base predictor saves its unfiltered endpoint at
`0x2eaf4`, reads its callback clock at `0x2eaf8` and the incoming event's
nanosecond time at `0x2eb04`, then filters input at `0x2eb20`, updates
[acceleration](predictor-acceleration-findings.md) at `0x2eb38` and checks
speed at `0x2eb40`.

Either `needPredict == false` or low speed selects `0x2ec40`, bypassing
`DoPredict` for this invocation. Passing both conditions reaches the
separate pacing check. These first conditions do not bypass pacing's
state updates: both branches call the chronometer once.

## The force-completion threshold uses incoming event history

At `0x2eb44`–`0x2eb70`, the predictor computes:

```text
interval_ms = f32(f32(1000) / refresh_rate_f32)
scaled_interval = f32(interval_ms / f32(2.4315))
history_threshold = trunc_to_i32(f64(scaled_interval) * 0.8)
```

The divisor is the float at `0x15610`, approximately
2.43149995803833. The final multiplier is double 0.8 at `0x15730`.
Division occurs in float before conversion to double at `0x2eb68`;
the final conversion truncates toward zero.

The count being compared comes from the incoming motion event. At
`0x2eca4`/`0x2ed4c`, the caller passes `x19`, saved from the original
event argument at `0x2e820`, to `MotionEvent::GetHistorySize`. Its PLT
is `0x3be30`, identified by relocation `0x424e8`.

Base `MotionEvent::GetHistorySize`, `0xc07a8`, obtains the history pointer
vector's count from private offsets 80/88 and divides it by pointer count
at private offset 4 through `0xc07ac`–`0xc07bc`. The result counts
historical samples, excluding the separate current sample. It does not
count the predictor's retained real-point deque or newly accepted samples.

The force condition is strictly `history_size > history_threshold`:
`0x2ecac`–`0x2ecb4` and `0x2ed54`–`0x2ed58`. For ordinary positive
rates, reconstruction gives:

| Refresh rate | Threshold | First historical-sample count forcing completion |
| --- | --- | --- |
| 60 | 5 | 6 |
| 90 | 3 | 4 |
| 120 | 2 | 3 |

These are unrelated to the model windows of 13, 20 or 30 retained records.
A large event history forces only the completion branch; it does not
override speed or timing to start a task.

## The complete base branch table

For this table, `due` is the result of predictor virtual slot 248,
`large_history` is the strict comparison above, and `unbuffered` is the
virtual getter bound to base member 1768. The completion column means a
direct base call to `OnPredictionComplete`; delegated prediction can call
completion separately.

| Conditions after the nonzero-length check | Call `DoPredict` | Direct completion condition | Return value |
| --- | --- | --- | --- |
| `!needPredict || low_speed` | No | `due || large_history || !unbuffered` | True |
| `needPredict && !low_speed && due` | Yes | None here; delegated to predictor | True |
| `needPredict && !low_speed && !due` | No | `large_history || !unbuffered` | False |

The first row calls slot 248 at `0x2ec9c`, then overrides its local
completion flag when history is large at `0x2ecb4`. The unbuffered test
at `0x2ecec`–`0x2ecf4` skips completion only when unbuffered is true and
both other conditions are false. Every exit from this row reaches the
unconditional true assignment at `0x2ed10`.

For the other rows, the timing call is at `0x2ebe0`. If due, `0x2ec10`
calls virtual slot 256, `DoPredict`, then proceeds to the same true
assignment. If not due, `0x2ed24` reads the unbuffered flag. Buffered
dispatch reaches completion at `0x2eda4` directly; unbuffered dispatch
reaches it only when history exceeds the threshold. Both outcomes then
set the return value to false at `0x2eda8`.

The ordinary branches restore the input transform at `0x2edb4` and return
the selected Boolean through `0x2edc8`. The enabled path's Boolean can be
summarized as `!needPredict || low_speed || due`, independently of whether
direct completion occurs.

Examples established by these branches include:

- Low speed, unbuffered dispatch, not due and small history: true return,
  no `DoPredict`, no direct completion.
- Input status and speed pass, buffered dispatch, not due: false return,
  no `DoPredict`, direct completion.
- Input status and speed pass, due: true return and `DoPredict`; later
  task gates still decide whether inference or output production succeeds.

For linear prediction, slot 248 always returns true. Consequently the
third row is unreachable through that implementation's timing predicate.
The shared base table does not imply that linear prediction uses the
neural time/VSync pacing rules.

## Diagnostic labels do not redefine the branch conditions

The low-status/speed branch logs the `needPredict` and speed values using
string `0x143ac`. Its large-history override uses the `Force draw` string
at `0x15252`.

The not-due branch uses strings `0x1318a` and `0x13743`, which call the
task expired and optionally say `Force draw`. In this branch, however,
the actual preceding check at `0x2ebf0` took the false result from
`CheckExpiredAndReset`: the next pacing interval has not elapsed.
It did not run the separate too-late neural expiry predicate.

Likewise, `Force draw` identifies a request for completion. Whether a
visible stroke is drawn depends on event construction and downstream
consumer behavior; no inference call is added by that override.

## Completion references retain their branch-dependent origin

The first row builds entity `[0, incoming_event_ns, base_clock_ns, 0, 0]`
at `0x2ed00`–`0x2ed08` and calls completion at `0x2ed0c`.

The other rows use the last retained real record's nanosecond field,
loaded at `0x2ebc8`. Their initial entity is
`[0, retained_record_ns, base_clock_ns, 0, 0]`, passed to `DoPredict`
at `0x2ec10` or direct completion at `0x2eda4`. The
[timing producer trace](predictor-timing-findings.md) follows the neural
VSync/period additions that occur only after entering `DoPredict`.

Calling completion does not establish a newly inferred event. It calls
`GetPredictedPenEvent` against current prediction state and can deliver a
null event when the prediction vector is empty. The
[event selection trace](neural-selection-findings.md) and
[consumer handoff](predictor-callback-findings.md) govern that result.
Only a non-null constructed event triggers the time backend's saved-reset
commit; the [pacing trace](predictor-chrono-findings.md) records that
additional distinction.

## Validation and remaining work

Both native byte streams were matched to the APK. Input argument retention,
packed return handling, branch instructions, float/double constants,
history getter identity, timing-reference stores and Boolean returns were
checked against ARM64 instructions and relocations. Disposable reconstruction
checked all 64 combinations of enabled/status/speed/due/unbuffered/history
conditions, plus equality and first-forced-count boundaries at three rates.

These are static control-flow checks, not native execution or measured
callback rates. The [position trace](predictor-position-findings.md) recovers
coefficient caller ordering. Full input-status producer semantics and
concurrent worker behavior remain separate work. No SDK code or corpus
fixture changed.
