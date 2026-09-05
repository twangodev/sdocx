# Bundled neural predictor models

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
The library contains three model configurations, selected by prediction-length
IDs 1 through 3. Their exported identifiers are `M16`, `M20` and `M22`.

This extends the [predictor factory trace](predictor-callback-findings.md).
It identifies configuration and caller behavior without executing the models,
recovering their weights, or establishing which configuration a device selects.

## Prediction length selects a model holder

`GetModelInfoHolder`, `0x3a910`, accepts IDs 1, 2 and 3. The unsigned
comparison at `0x3a910`–`0x3a91c` rejects every other 32-bit value,
including zero. Invalid IDs log and return null at `0x3a94c`.
Accepted IDs resolve through `0x3a958`–`0x3a968` to:

```text
holder = 0x111bb0 + (id - 1) * 24
```

The static initializer beginning at `0x39a90` constructs:

| ID | Holder address | Configuration | Constructor call |
| --- | --- | --- | --- |
| 1 | `0x111bb0` | M16 | `0x39e40` |
| 2 | `0x111bc8` | M20 | `0x3a170` |
| 3 | `0x111be0` | M22 | `0x3a490` |

Each holder owns a vector of two 160-byte `NNModelInfo` records. Both
records receive the same model globals, with input-type fields 0 and 1
respectively. Constructor `0x3a970` copies the first 132 bytes and the
string at offset 136; its record stride is 160 bytes.

Composer's proxy also has a mapping to neural prediction length 4.
That mapping does not establish a fourth model: this library's holder
selector rejects 4. Whether application configuration reaches that
combination remains unresolved.

## Model configuration values

| Field | M16 | M20 | M22 |
| --- | --- | --- | --- |
| `InputSize` | 12 | 19 | 29 |
| `OutputSize` | 1 | 3 | 3 |
| `PredictTime`, microseconds | 16,000 | 5,600; 11,100; 19,400 | 6,300; 14,600; 22,900 |
| `samplingRateInHz` | 360 | 360 | 480 |
| `featureString` | `xysotp` | `xysotp` | `xysotp` |
| Encrypted-data byte count | 221,733 | 272,324 | 304,695 |
| Expected decoded byte count | 283,108 | 383,996 | 465,008 |

The exported input/output/time globals begin at `0x7cc84`, `0xbf4d0`
and `0x109b90`, respectively. M22 has an intervening exported integer
between `InputSize` and `OutputSize`; these values were read by symbol
address, not by assuming the globals have identical contiguous layouts.
The rate globals are at `0x7ccc0`, `0xbf510` and `0x109bd8`.
The feature-string pointer globals immediately follow those rates.

The time unit follows the consuming instructions. `PredictionTask::Run`
loads a configured horizon at `0x2cde4`, divides it by 1,000 for the
millisecond field and multiplies it by 1,000 for the nanosecond field at
`0x2ce44`–`0x2ce80`. Integer conversion occurs after float arithmetic.

`OutputSize` counts configured output horizons. It does not by itself
prove that every horizon will be delivered: the task also selects output
indices and applies rejection checks.

## Input type controls access and lazy decoding

`PredictorBase::GetInputType`, `0x32094`, constructs a two-entry map
from the integer pairs at `0x15670`. The map yields:

| Motion-event tool-type value | Predictor input-type value |
| --- | --- |
| 1 | 0 |
| 2 | 1 |
| Any other value | 2 |

`NNModelInfoHolder::GetModel`, `0x3ab6c`, returns null unless input type
equals 1. For type 1, it selects the second record at
`holder_vector_begin + 160`. Thus the first initialized record does not
establish a supported finger model through this accessor.

If the selected record's data pointer is already nonzero, it returns
the record. Otherwise `0x3ab94`–`0x3aba4` passes its encoded data and
codec parameters to `PredictorModelCodec::Decode`, `0x3afa8`.
The decoded byte count is compared with the expected size at
`0x3abb4`. A mismatch logs, but this accessor still stores the returned
pointer at `0x3ac08` and returns the record. A nonnull record alone
therefore does not prove successful decoding or interpreter creation.

`TFLiteHolder::SetModel`, `0x381d4`, tries input types 0 and 1 and
calls `AddModel` only for a nonnull record.

The availability check at `0x25484` additionally rejects tool type 2
when `MotionEvent::GetSource()` differs from `0x5002`. It rejects an
unknown mapped input type and otherwise checks the holder accessor's
result. These are local native gates, not proof of the application's
active input source or model configuration.

## Changing the model updates prediction parameters

`NNPredictor::SetPredictionLength`, `0x25378`, returns immediately
when the requested value already matches. Otherwise it stores the new
ID, obtains its holder, waits for an existing worker, and updates both
worker and local TFLite holders through `0x253c8`–`0x253f8`.

It obtains the current tool's model and applies:

| Operation | Evidence |
| --- | --- |
| Set minimum sample count to `InputSize + 1` | `0x25414`–`0x25428` |
| Reset and configure Kalman filters | `0x25434` calls `SetKalmanFilter` |
| Test the feature string for `c` | `0x25438`–`0x25444`; result stored at member 1797 |
| Clear the multi-output byte | `0x25458`, member 1796 |
| Save the last configured horizon | `0x25448`–`0x25468`, member 1792 |

The resulting minimum counts are 13, 20 and 30. All three bundled
feature strings omit `c`. Clearing the multi-output byte is a separate
operation from reading `OutputSize`, which is 3 for M20 and M22.

## All three configurations initially enable only XY filtering

`NNPredictor::SetKalmanFilter`, `0x24ff0`, first calls
`PredictorBase::ResetKalmanFilter` at `0x25004`. Model offset 64 is
the master enable byte; offsets 72, 84, 108 and 96 gate XY, pressure,
tilt and orientation respectively.

The bundled master and XY bytes are 1. Pressure, tilt and orientation
bytes reside in zero-initialized storage and are 0. The enabled XY
branch passes type mask 1 and model offsets 76/80 to
`SetKalmanFilterEnabled` at `0x25018`–`0x25024`.

All three configurations supply identical noise parameters. Hex values
below are the exact IEEE-754 binary32 bit patterns; decimals are compact
representations, not substitutes for those bits.

| Channel | Process noise | Observation noise | Initially enabled |
| --- | --- | --- | --- |
| XY | `0x3727c5ac` ≈ 0.00001 | `0x387ba882` ≈ 0.00006 | Yes |
| Pressure | `0x360637bd` ≈ 0.000002 | `0x391d4952` ≈ 0.00015 | No |
| Orientation | `0x350637bd` ≈ 0.0000005 | `0x38d1b717` ≈ 0.0001 | No |
| Tilt | `0x350637bd` ≈ 0.0000005 | `0x38d1b717` ≈ 0.0001 | No |

This recovers model-driven filter configuration. The external predictor's
filter implementation remains distinct from the separately traced
[InkPen2 Kalman implementation](inkpen2-kalman-findings.md).

## Validation and remaining work

The APK digest and extracted predictor byte stream were verified.
Exported global addresses, relocated pointers, zero-initialized bytes,
holder construction, selector boundaries, input mapping, decoder call,
configuration stores and filter calls were checked against the ELF and
ARM64 instructions.

The next steps are the feature-buffer contract, model tensor metadata,
output conversion and candidate rejection. No SDK behavior or saved
stroke decoding changed. These findings concern live pen prediction;
they do not establish that predicted points are serialized into SDOCX.
