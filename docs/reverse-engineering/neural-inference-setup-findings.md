# Neural interpreter setup and input-time validation

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
This extends the [model configuration](neural-model-findings.md) and
[feature preparation](neural-feature-findings.md) traces with the native
caller's tensor names, requested input shapes and installed feature validator.

Requested shapes and local setup branches are confirmed. The models'
serialized tensor declarations, successful initialization on a device,
inference results and output quality remain unvalidated.

## The caller requests a three-dimensional input

`TFLiteHolder::AddModel`, `0x3822c`, reads the model's feature-string
length at `0x38284`–`0x382a0` and `InputSize` at `0x38454`. It
constructs a three-integer vector at `0x38458`–`0x38474`:

```text
[1, model.InputSize, featureString.length]
```

Consequently the bundled models request:

| Model | Input shape passed to the resize API | Float count from the feature loop |
| --- | --- | --- |
| M16 | `[1, 12, 6]` | 72 |
| M20 | `[1, 19, 6]` | 114 |
| M22 | `[1, 29, 6]` | 174 |

These dimensions describe the native resize request, independently of
whether the encoded model accepts it. The existing feature loop fills
the last dimension in `xysotp` order for each interval.

## Signature and positional interpreter paths

The holder constructs a `BuiltinOpResolver` at `0x37fb4`.
`AddModel` obtains a flat-buffer model from the decoded pointer and
expected byte count at `0x383c0`–`0x383dc`, then calls the TFLite
interpreter builder at `0x38424`–`0x38440`.

It first requests signature `serving_default`, string `0x1238e`,
through `Interpreter::GetSignatureRunner` at `0x38484`.
When that runner is available, it resolves:

| Tensor role | Requested name | Evidence |
| --- | --- | --- |
| Input | `input` | String `0x124a6`; call at `0x384bc` |
| Output index `i` | `output_` followed by decimal `i` | Prefix `0x152b2`; `getOutputNameById`, `0x38120` |

`getOutputNameById` converts the supplied integer with `to_string` at
`0x38144`, then inserts the prefix at `0x38158`. The caller starts
with index 0 and visits `OutputSize` names through
`0x384fc`–`0x385e4`. M16 therefore requests `output_0`; M20 and M22
also request `output_1` and `output_2`.

The signature setup requires a nonnull input tensor and a nonnull output
tensor for every requested index. `0x3862c`–`0x38640` checks the input
and the collected output count. It then calls
`SignatureRunner::ResizeInputTensor` at `0x38654` and its subgraph's
`AllocateTensors` at `0x3865c`.

An absent signature, missing tensor, or nonzero signature allocation
status reaches the positional interpreter path at `0x38688`. That path:

1. Calls `Interpreter::SetNumThreads(1)` at `0x38690`.
2. Resizes input index 0 to the same dimensions at `0x386a0`.
3. Calls `Interpreter::AllocateTensors` at `0x386a8`.
4. Saves the model/interpreter on successful allocation.

The signature path also retains the owning interpreter. Its successful
branch saves the runner at map-node offset 64 through `0x38914` and
the interpreter through `0x38930`–`0x3893c`.

The map entry consumed by `PredictionTask::SetTFParams` has:

| Map-node offset | Role | Task destination |
| --- | --- | --- |
| 40 | `NNModelInfo` pointer | 16 |
| 48 | Owned flat-buffer model | Kept by holder |
| 56 | Owned interpreter | 24 |
| 64 | Signature-runner pointer, when used | 32 |
| 72 | Character-keyed input-validator map | 40, pointer to map |

`0x2befc`–`0x2bf14` establishes the task copies. `Run` checks whether
both runtime handles are null at `0x2c570`–`0x2c578`; if so, it
completes without inference. Otherwise it uses the signature runner when
present and the positional interpreter when absent. Their invocation
calls are at `0x2c9bc` and `0x2c9c8` respectively.

This distinguishes a selected model-info record from a usable runtime
handle; the former can exist before interpreter setup succeeds.

## Only the time feature receives this validator

After runtime setup, `AddModel` scans the feature string at
`0x38734`–`0x387ec`. It creates an `InputTimeValidator` only for
character `s`, tested at `0x38774`–`0x3877c`.

The 16-byte validator stores its threshold at offset 8. The immediate
assembled at `0x38760`/`0x3876c` and stored at `0x387c4` is:

```text
bits: 0x3f4ccccd
value: 0.800000011920929
```

GOT `0x41d38` resolves to validator vtable `0x40d78`. Its address
point is `0x40d88`; slot 16 resolves to `Validate`, `0x36594`.
The validator object is stored in the character-map node at offset 40
through `0x387cc`, matching the lookup and call in the task's feature
loop at `0x2c834`–`0x2c860`.

For finite inputs, `Validate` returns true exactly when:

```text
normalized_time_interval <= f32(0.8)
```

The comparison at `0x365d8` is threshold versus supplied value, and
`0x365dc` uses condition `pl`. Equality passes. There is no lower-bound
check in this method. The feature helper's negative-time and repeated-zero
checks remain separate and occur earlier.

The loop installs no corresponding validator for `x`, `y`, `o`, `t`
or `p`. This is the behavior of the traced `AddModel` setup, not a claim
that those channels can never be rejected elsewhere.

## The threshold yields concrete millisecond boundaries

Applying the confirmed scalar conversion
`f32(f32(delta_ms * 1000) / time_normalizer_us)` with the bundled
normalizers gives:

| Model | Largest passing integer interval | Normalized value | First failing integer interval | Normalized value |
| --- | --- | --- | --- | --- |
| M16 | 30 ms | 0.7826087474822998 | 31 ms | 0.8086956739425659 |
| M20 | 48 ms | 0.7908467054367065 | 49 ms | 0.8073226809501648 |
| M22 | 55 ms | 0.791604220867157 | 56 ms | 0.8059970140457153 |

These are per-interval boundaries under the recovered model defaults.
They are not minimum input sampling rates or measured device latency.
One interval exceeding its limit makes the task take completion at
`0x2c970`–`0x2c9a8` before either inference invocation.

Adjacent binary32 boundary checks also confirm that `0x3f4ccccc` and
`0x3f4ccccd` pass, while `0x3f4cccce` fails. Those checks isolate the
inclusive threshold from millisecond-to-float conversion.

## Validation and remaining work

The APK digest and native byte stream were verified. Requested tensor
dimensions, name strings, TFLite call bindings, runtime-handle stores,
validator construction and vtable, float constants and branch conditions
were checked against the ELF and disassembly. Disposable numerical checks
covered all six integer-interval examples and the three adjacent float
values. Markdown links and whitespace checks passed.

The [output conversion trace](neural-output-findings.md) follows output
tensors into candidate coordinates, pen channels and timestamp fields.
Horizon selection and rejection after inference remain separate work.
The [lifecycle trace](neural-lifecycle-findings.md) recovers resource
replacement, failure state, and the ownership of copied task handles.
Model graph contents and actual inference remain unvalidated. No SDK code
or corpus fixture changed.
