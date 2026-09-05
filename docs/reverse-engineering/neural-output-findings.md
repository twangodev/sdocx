# Neural outputs become candidate pen points

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
This follows the [interpreter setup](neural-inference-setup-findings.md)
and [feature preparation](neural-feature-findings.md) through output
coordinate conversion, channel copying and timestamp construction.

The native caller consumes two float coordinates per output horizon.
This identifies what the caller reads, without establishing the complete
serialized output-tensor shape or executing a model.

## Each output contributes an XY offset

`PredictionTask::Run`, `0x2c530`, names output index `i` through
`getOutputNameById` at `0x2cd44`. When task offset 32 contains a
signature runner, `0x2cd64` gets that output tensor and `0x2cd68`
loads its data pointer. Otherwise `0x2cd70`–`0x2cd78` uses the
interpreter's positional output through helper `0x2da98`.

That positional helper looks up the graph tensor index, rejects a negative
index, checks tensor-table bounds and presence, and requires tensor type
code 1 before returning the data pointer at `0x2dadc`.

At `0x2cdc4`, the caller loads eight bytes from the output data as two
binary32 values. It multiplies by task X/Y DPI and divides by the float
constant 25.4 at `0x2cdcc`–`0x2cdd8`:

```text
offset_x = f32(f32(output_x * dpi_x) / f32(25.4))
offset_y = f32(f32(output_y * dpi_y) / f32(25.4))
```

The result is stored at offsets 32/36 of a temporary 88-byte record.
The conversion is consistent with model outputs in millimetres and
candidate offsets in the configured DPI coordinate system. It does not
apply an extra elapsed-time multiplier or cumulatively add earlier
outputs. Each output is anchored independently to the last real record.

The float operation order matters. With output `(25.4f, 25.4f)` and
X/Y DPI `(254, 508)`, the reconstructed offsets are:

| Axis | Offset | Binary32 bits |
| --- | --- | --- |
| X | 254.00001525878906 | `0x437e0001` |
| Y | 508.0000305175781 | `0x43fe0001` |

Cancelling the two apparent 25.4 factors would lose that rounding.
For comparison, output `(1, 1)` with the same DPI yields `(10, 20)`.

## The helper reverses rotation and adds the real-point anchor

`Run` calls `NNPredictorHelper::GetPenEvent`, `0x26354`, at
`0x2ce3c`, supplying the temporary offset record, the last real record
and an output candidate. The helper first checks its validity byte at
offset 8. If unset, it returns false without constructing the candidate.

For a valid helper, it reads the stored double rotation angle, converts
it to float at `0x26394`, and calls `RotateOverPoint`, `0x26408`,
using a zero-coordinate origin at `0x26398`–`0x263bc`.

Unlike the input rotation, the output helper applies the inverse matrix.
For finite values, its geometric form is:

```text
rotated_offset_x = cos(angle) * offset_x + sin(angle) * offset_y
rotated_offset_y = -sin(angle) * offset_x + cos(angle) * offset_y
candidate_x = last_real_x + rotated_offset_x
candidate_y = last_real_y + rotated_offset_y
```

The sine/cosine call is at `0x2643c`. The lane construction and float
multiply/FMA operations at `0x26488`–`0x264ac` establish the opposite
sine signs from input `RotateOverIndex`. The real-point addition is
at `0x263c0`–`0x263cc`.

This geometric formula does not specify bit-exact trigonometry. The
native path rounds intermediate float operations, uses fused operations,
and scales through DPI before reversing rotation.

## Pen channels come from the last real record

After setting candidate XY, `GetPenEvent` copies 16 bytes from the last
real record's offset 48 to the candidate at `0x263d0`–`0x263d4`:

| Offset | Copied channel |
| --- | --- |
| 48 | Tilt |
| 52 | Pressure |
| 56 | Orientation |
| 60 | Second orientation field |

The neural output read does not supply these channels. In particular,
the `p` input feature is a pressure difference, but this output path
does not integrate predicted pressure differences. Every horizon copies
the same final real-record channel values.

## Horizons update millisecond and nanosecond fields independently

The task loads `PredictTime[i]` from model offset 56 at `0x2cde4`.
After successful coordinate conversion, `0x2ce44`–`0x2ce80`
constructs the candidate times:

```text
candidate_ms = last_real_ms + trunc_i64(f32(horizon_us / 1000))
candidate_ns = last_real_ns + trunc_i64(f32(horizon_us * 1000))
```

The float results are widened to double before signed 64-bit truncation;
integer addition to the real timestamps occurs afterward. The two fields
are not derived from one another. Candidate offset 8 is separately copied
from last-real offset 8 at `0x2ce84`–`0x2ce88`.

The bundled horizon conversions are:

| Model | Horizon, microseconds | Added milliseconds | Added nanoseconds |
| --- | --- | --- | --- |
| M16 | 16,000 | 16 | 16,000,000 |
| M20 | 5,600 | 5 | 5,600,000 |
| M20 | 11,100 | 11 | 11,100,000 |
| M20 | 19,400 | 19 | 19,400,000 |
| M22 | 6,300 | 6 | 6,300,000 |
| M22 | 14,600 | 14 | 14,600,000 |
| M22 | 22,900 | 22 | 22,900,000 |

For example, the 5,600-microsecond horizon advances the millisecond field
by 5 and the nanosecond field by 5.6 million. Preserving only the rounded
millisecond increment would discard timing information.

## Appending a candidate adds a separate aligned timestamp

Subject to the task's selection and rejection branches, `0x2cf98` calls
`PredictorBase::AddPredictedPoint`, `0x311a0`, with the candidate.
Unless base-object byte 60 is set, it locks the real-point critical
section and appends a copy to the prediction vector at base offsets
64/72/80. The copy is 88 bytes at `0x31250`–`0x31260` or the
allocation counterpart at `0x312f8`–`0x31300`.

After appending, `0x31354` loads the copied record's nanosecond field
at offset 24 and calls predictor slot 232 at `0x31360`. The neural
vtable address point `0x40998` binds that slot to
`GetAlignedToVSyncTimeNano`, `0x256a0`.

The returned aligned value is stored in the record's separate offset 80
at `0x31368`. It does not replace the candidate's millisecond or
nanosecond fields. The alignment calculation is described in the
[predictor timing trace](predictor-timing-findings.md).

## Validation and remaining work

The APK digest and native byte stream were verified. Output-data loads,
DPI arithmetic, inverse-rotation lane construction, channel copies,
timestamp conversions, prediction-vector copies and vtable dispatch were
checked against ARM64 instructions.

A disposable float reconstruction checked positive, zero and negative
coordinate offsets, the one-ULP scaling example and all seven bundled
horizon conversions. Five ideal geometric rotations were inverted as a
separate check, without claiming native trigonometric equivalence.

Remaining work includes output-index selection, task rejection and the
conversion of selected records into callback events. These findings do
not establish which candidates a device delivers or serializes. No SDK
code or fixture changed.
